use axum::extract::multipart::Field;
use axum::extract::{Multipart, State};
use axum::{Extension, Json};
use object_store::path::Path;
use object_store::{ObjectStore, PutPayload};
use rhai::Engine;
use serde::Serialize;
use sqlx::Postgres;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

use super::annotations::{AnnotationEntry, TrackedAnnotations};
use super::error::ApiError;
use crate::state::AppState;
use crate::utils::{get_product_by_id, stream_to_s3};
use data::api_token::ApiToken;

pub struct MinidumpApi;

#[derive(Debug, Serialize)]
pub struct MinidumpResponse {
    pub result: String,
    pub crash_id: Option<uuid::Uuid>,
}

#[derive(Default, Debug, Serialize)]
struct Minidump {
    filename: String,
    content_type: String,
    size: u64,
    storage_path: String,
    storage_id: uuid::Uuid,
}

#[derive(Default, Debug, Serialize)]
struct Attachment {
    name: String,
    filename: String,
    content_type: String,
    size: u64,
    storage_path: String,
    storage_id: uuid::Uuid,
}

#[derive(Default, Debug, Serialize)]
struct CrashInfo {
    crash_id: uuid::Uuid,
    submission_timestamp: String,
    authorized_product: Option<String>,
    product_id: Option<uuid::Uuid>,
    minidump: Option<Minidump>,
    attachments: Vec<Attachment>,
    annotations: HashMap<String, AnnotationEntry>,
}

impl MinidumpApi {
    fn validate_annotation_content_type(content_type: &str) -> Result<(), ApiError> {
        let is_valid = content_type == "text/plain"
            || content_type == "text/markdown"
            || content_type.is_empty();

        if !is_valid {
            error!(content_type, "Invalid annotation content type");
            return Err(ApiError::Failure(format!(
                "invalid annotation content type: {content_type}"
            )));
        }
        Ok(())
    }

    fn validate_minidump_content_type(content_type: &str) -> Result<(), ApiError> {
        let is_valid = content_type == "application/octet-stream" || content_type.is_empty();

        if !is_valid {
            error!(content_type, "Invalid minidump content type");
            return Err(ApiError::Failure(format!(
                "invalid minidump content type: {content_type}"
            )));
        }
        Ok(())
    }

    fn validate_key(key: &str) -> Result<(), ApiError> {
        if !key.chars().all(|c| c.is_ascii() && !c.is_ascii_control()) {
            error!(key, "Annotation key contains non-printable ASCII characters");
            return Err(ApiError::Failure(
                "annotation key must contain only printable ASCII characters".to_string(),
            ));
        }
        Ok(())
    }

    #[instrument(skip(field, crash_info, state), fields(crash_id = %crash_info.crash_id))]
    async fn handle_minidump_upload(
        field: Field<'_>,
        crash_info: &mut CrashInfo,
        state: AppState,
    ) -> Result<(), ApiError> {
        debug!("Processing minidump");
        let content_type = field.content_type().unwrap_or_default().to_owned();

        Self::validate_minidump_content_type(&content_type)?;

        let storage_id = uuid::Uuid::new_v4();
        let storage_path = format!("minidumps/{storage_id}");

        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "unnamed_minidump".to_string());

        debug!(
            filename = %filename,
            content_type = %content_type,
            "Uploading minidump"
        );
        let size = stream_to_s3(state.storage.clone(), &storage_path, field)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to stream minidump to S3");
                ApiError::Failure("failed to store minidump".to_string())
            })?;

        info!(filename = %filename,
              size = %size,
              storage_path = %storage_path,
              storage_id = %storage_id,
              "Adding minidump");

        crash_info.minidump = Some(Minidump {
            filename,
            content_type,
            size,
            storage_path,
            storage_id,
        });

        Ok(())
    }

    #[instrument(skip(field, crash_info, state), fields(crash_id = %crash_info.crash_id))]
    async fn handle_attachment_upload(
        field: Field<'_>,
        crash_info: &mut CrashInfo,
        state: AppState,
    ) -> Result<(), ApiError> {
        info!("Processing attachment");
        let content_type = field.content_type().unwrap_or_default().to_owned();

        let storage_id = uuid::Uuid::new_v4();
        let storage_path = format!("attachments/{storage_id}");

        let filename = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "unnamed_attachment".to_string());

        let name = field
            .name()
            .ok_or_else(|| {
                error!("Name field is missing for attachment");
                ApiError::Failure("name field for attachment is missing".to_string())
            })?
            .to_string();

        info!(name = %name, filename = %filename, content_type = %content_type, "Uploading attachment");

        let size = stream_to_s3(state.storage.clone(), &storage_path, field)
            .await
            .map_err(|e| {
                error!(error = ?e, attachment_name = name, "Failed to stream attachment to S3");
                ApiError::Failure("failed to store attachment".to_string())
            })?;

        info!(name = %name, filename = %filename, content_type = %content_type, "Adding attachment");

        crash_info.attachments.push(Attachment {
            name,
            filename,
            content_type,
            size,
            storage_path,
            storage_id,
        });
        Ok(())
    }

    #[instrument(skip(field, crash_info), fields(crash_id = %crash_info.crash_id))]
    async fn handle_annotation_upload(
        field: Field<'_>,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError> {
        debug!("Processing annotation");
        let key = field
            .name()
            .ok_or_else(|| {
                error!("Name field is missing for annotation");
                ApiError::Failure("name field is missing for annotation".to_string())
            })?
            .to_string();
        Self::validate_key(&key)?;

        let content_type = field.content_type().unwrap_or_default().to_owned();
        Self::validate_annotation_content_type(&content_type)?;

        let value = field.text().await.map_err(|e| {
            error!(error = ?e, key, "Failed to read field text for annotation");
            ApiError::Failure(format!("failed to read field text for annotation '{key}'"))
        })?;

        info!(key = %key, content_type = %content_type, "Adding annotation");
        crash_info.annotations.insert(
            key,
            AnnotationEntry {
                value,
                source: "submission".to_string(),
            },
        );
        Ok(())
    }

    #[instrument(skip(field, crash_info, state), fields(crash_id = %crash_info.crash_id))]
    async fn process_field(
        field: Field<'_>,
        crash_info: &mut CrashInfo,
        state: AppState,
    ) -> Result<(), ApiError> {
        let content_type = field.content_type().unwrap_or_default().to_owned();
        let field_name = field.name().unwrap_or_default();
        let file_name = field.file_name().unwrap_or_default();

        debug!(field_name = %field_name, file_name = %file_name, content_type = %content_type, "Processing multipart field");
        match field_name {
            "upload_file_minidump" => Self::handle_minidump_upload(field, crash_info, state).await,
            _ => {
                if file_name.is_empty() {
                    Self::handle_annotation_upload(field, crash_info).await
                } else {
                    Self::handle_attachment_upload(field, crash_info, state).await
                }
            }
        }
    }

    #[instrument(skip(_tx, _api_token, state, crash_info), fields(crash_id = %crash_info.crash_id))]
    async fn validate_crash<E>(
        _tx: &mut E,
        _api_token: &ApiToken,
        state: &AppState,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError>
    where
        for<'a> &'a mut E: sqlx::Executor<'a, Database = Postgres>,
    {
        debug!(crash_id = %crash_info.crash_id, "Validating crash info");
        if crash_info.minidump.is_none() {
            error!("No minidump found in submission");
            return Err(ApiError::Failure("no minidump found in submission".to_string()));
        }

        let mandatory: Vec<String> = state
            .settings
            .minidumps
            .mandatory_annotations
            .clone()
            .unwrap_or_default();

        for required_field in mandatory.iter() {
            let value = crash_info.annotations.get(required_field).cloned();

            if value.is_none() {
                error!(required_field, "Required annotation is missing");
                return Err(ApiError::Failure(format!(
                    "required annotation '{required_field}' is missing"
                )));
            }

            if let Some(value) = value
                && value.value.trim().is_empty()
            {
                error!(required_field, "Required annotation is empty");
                return Err(ApiError::Failure(format!(
                    "required annotation '{required_field}' cannot be empty"
                )));
            }
        }

        if let Some(validation_scripts) = &state.settings.minidumps.validation_scripts {
            for validation_script in validation_scripts {
                match validation_script {
                    common::settings::ValidationScript::Global(script_file) => {
                        let script_file = format!("{}/{}", &state.settings.config_dir, script_file);
                        debug!(script_file = %script_file, "Running global validation script");
                        Self::validate_with_rhai_script(&script_file, crash_info)?;
                    }
                    common::settings::ValidationScript::ProductSpecific { product, script } => {
                        if let Some(authorized_product) = &crash_info.authorized_product {
                            match fancy_regex::Regex::new(product) {
                                Ok(product_regex) => {
                                    match product_regex.is_match(authorized_product) {
                                        Ok(true) => {
                                            let script_file = format!(
                                                "{}/{}",
                                                &state.settings.config_dir, script
                                            );
                                            debug!(
                                                script_file = %script_file,
                                                product_pattern = %product,
                                                authorized_product = %authorized_product,
                                                "Running product-specific validation script"
                                            );
                                            Self::validate_with_rhai_script(
                                                &script_file,
                                                crash_info,
                                            )?;
                                        }
                                        Ok(false) => {
                                            debug!(
                                                product_pattern = %product,
                                                authorized_product = %authorized_product,
                                                "Product pattern does not match authorized product, skipping script"
                                            );
                                        }
                                        Err(e) => {
                                            error!(
                                                error = %e,
                                                product_pattern = %product,
                                                "Failed to execute regex match for product pattern"
                                            );
                                            return Err(ApiError::Failure(format!(
                                                "Invalid regex execution for product pattern '{product}': {e}"
                                            )));
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!(
                                        product_pattern = %product,
                                        error = %e,
                                        "Invalid regex pattern in product validation script configuration"
                                    );
                                    return Err(ApiError::Failure(format!(
                                        "Invalid regex pattern '{product}' in validation script configuration: {e}"
                                    )));
                                }
                            }
                        } else {
                            debug!(
                                product_pattern = %product,
                                "No authorized product found, skipping product-specific validation script"
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(storage, crash_info), fields(crash_id = %crash_id))]
    async fn upload_crash(
        storage: Arc<dyn ObjectStore>,
        crash_id: uuid::Uuid,
        crash_info: serde_json::Value,
    ) -> Result<(), ApiError> {
        debug!(crash_id = %crash_id, "Uploading crash info to S3");
        let path = Path::from(format!("crashes/{crash_id}.json"));
        let crash_info_json = crash_info.to_string();
        let payload = PutPayload::from(crash_info_json.into_bytes());
        storage.put(&path, payload).await.map_err(|e| {
            error!(error = ?e, "Failed to upload crash info to S3");
            ApiError::Failure("failed to upload crash info to S3".to_string())
        })?;
        Ok(())
    }

    #[instrument(skip(state, api_token, crash_info, multipart), fields(crash_id))]
    pub async fn handle_upload(
        state: AppState,
        api_token: ApiToken,
        mut multipart: Multipart,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError> {
        let mut tx = state.repo.begin_admin().await?;

        let product_id = api_token.product_id.ok_or_else(|| {
            error!("API token does not have a product ID");
            ApiError::ProductAccessDenied(
                "API token is not associated with any product".to_string(),
            )
        })?;

        let authorized_product = get_product_by_id(&mut *tx, product_id).await?;
        crash_info.authorized_product = Some(authorized_product.name.clone());

        info!(product = %authorized_product.name, "Processing crash for product");

        if !authorized_product.accepting_crashes {
            return Err(ApiError::ProductNotAcceptingCrashes(authorized_product.name));
        }

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!(error = ?e, "Failed to get next multipart field");
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            Self::process_field(field, crash_info, state.clone()).await?;
        }

        Self::validate_crash(&mut *tx, &api_token, &state, crash_info).await?;

        state.repo.end(tx).await?;

        let crash_info_json = serde_json::to_value(&crash_info).map_err(|e| {
            error!(error = ?e, "Failed to serialize crash info");
            ApiError::Failure("failed to serialize crash info".to_string())
        })?;

        Self::upload_crash(state.storage.clone(), crash_info.crash_id, crash_info_json.clone())
            .await?;

        state
            .worker
            .queue_minidump(crash_info_json)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to queue minidump job");
                ApiError::Failure("failed to queue minidump job".to_string())
            })?;

        Ok(())
    }

    #[instrument(skip(state, api_token, multipart), fields(crash_id))]
    pub async fn upload(
        State(state): State<AppState>,
        Extension(api_token): Extension<ApiToken>,
        multipart: Multipart,
    ) -> Result<Json<MinidumpResponse>, ApiError> {
        let crash_id = uuid::Uuid::new_v4();
        tracing::Span::current().record("crash_id", format!("{crash_id}"));
        info!(crash_id = %crash_id, "Received minidump upload request");

        let mut crash_info = CrashInfo {
            crash_id,
            submission_timestamp: chrono::Utc::now().to_rfc3339(),
            authorized_product: None,
            minidump: None,
            attachments: Vec::new(),
            annotations: HashMap::new(),
            ..Default::default()
        };

        let r = Self::handle_upload(state.clone(), api_token, multipart, &mut crash_info).await;
        if let Err(e) = r {
            error!(error = ?e, "Failed to handle minidump upload");

            if let Some(minidump) = &crash_info.minidump {
                info!(storage_path = %minidump.storage_path, "Deleting minidump from storage");
                let _ = state
                    .storage
                    .delete(&Path::from(minidump.storage_path.as_str()))
                    .await;
            }

            for attachment in &crash_info.attachments {
                info!(storage_path = %attachment.storage_path, "Deleting attachment from storage");
                let _ = state
                    .storage
                    .delete(&Path::from(attachment.storage_path.as_str()))
                    .await;
            }

            info!(crash_id = %crash_info.crash_id, "Deleting crash info from storage");
            let _ = state
                .storage
                .delete(&Path::from(format!("crashes/{}.json", crash_info.crash_id)))
                .await;
            return Err(e);
        }

        Ok(Json(MinidumpResponse {
            result: "ok".to_string(),
            crash_id: Some(crash_id),
        }))
    }

    fn handle_validation_success(
        scope: &mut rhai::Scope,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError> {
        debug!(crash_id = %crash_info.crash_id, "Validation script returned success");

        if let Some(crash_info_map) = scope.get_value::<rhai::Map>("crash_info")
            && let Some(annotations_dynamic) = crash_info_map.get("annotations")
            && let Some(modified_annotations) =
                annotations_dynamic.clone().try_cast::<TrackedAnnotations>()
            && modified_annotations.was_modified()
        {
            debug!(crash_id = %crash_info.crash_id, "Script modified annotations");
            crash_info.annotations = modified_annotations.finalize();
        }
        Ok(())
    }

    fn handle_validation_failure(map: &rhai::Map, crash_info: &CrashInfo) -> Result<(), ApiError> {
        let error_message = map
            .get("error")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| "Validation failed".to_string());

        error!(crash_id = %crash_info.crash_id, error = %error_message, "Validation script returned failure");
        Err(ApiError::ValidationError(
            crash_info.authorized_product.clone().unwrap_or_default(),
            error_message,
        ))
    }

    fn extract_validation_result(
        map: &rhai::Map,
        crash_info: &CrashInfo,
    ) -> Result<bool, ApiError> {
        map.get("valid")
            .and_then(|v| v.as_bool().ok())
            .ok_or_else(|| {
                error!(crash_id = %crash_info.crash_id, "Validation result missing 'valid' field");
                ApiError::ValidationError(
                    crash_info.authorized_product.clone().unwrap_or_default(),
                    "Invalid validation result".to_string(),
                )
            })
    }

    fn handle_validation_result(
        result: rhai::Dynamic,
        scope: &mut rhai::Scope,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError> {
        debug!(crash_id = %crash_info.crash_id, "Rhai validation script executed successfully");

        let map = result.try_cast::<rhai::Map>().ok_or_else(|| {
            error!(crash_id = %crash_info.crash_id, "Validation script must return a validation result");
            ApiError::ValidationError(
                crash_info.authorized_product.clone().unwrap_or_default(),
                "Validation script failed".to_string(),
            )
        })?;

        let valid = Self::extract_validation_result(&map, crash_info)?;

        if valid {
            Self::handle_validation_success(scope, crash_info)
        } else {
            Self::handle_validation_failure(&map, crash_info)
        }
    }

    fn convert_crash_info_to_rhai_map(crash_info: &CrashInfo) -> rhai::Map {
        let mut map = rhai::Map::new();

        map.insert("crash_id".into(), crash_info.crash_id.to_string().into());
        map.insert("submission_timestamp".into(), crash_info.submission_timestamp.clone().into());

        if let Some(ref product) = crash_info.authorized_product {
            map.insert("authorized_product".into(), product.clone().into());
        }

        if let Some(ref product_id) = crash_info.product_id {
            map.insert("product_id".into(), product_id.to_string().into());
        }

        let tracked_annotations = TrackedAnnotations::from_map(crash_info.annotations.clone());
        map.insert("annotations".into(), rhai::Dynamic::from(tracked_annotations));

        if let Some(ref minidump) = crash_info.minidump {
            let mut minidump_map = rhai::Map::new();
            minidump_map.insert("filename".into(), minidump.filename.clone().into());
            minidump_map.insert("content_type".into(), minidump.content_type.clone().into());
            minidump_map.insert("size".into(), (minidump.size as i64).into());
            map.insert("minidump".into(), minidump_map.into());
        }

        let mut attachments: Vec<rhai::Dynamic> = Vec::new();
        for attachment in &crash_info.attachments {
            let mut attachment_map = rhai::Map::new();
            attachment_map.insert("name".into(), attachment.name.clone().into());
            attachment_map.insert("filename".into(), attachment.filename.clone().into());
            attachment_map.insert("content_type".into(), attachment.content_type.clone().into());
            attachment_map.insert("size".into(), (attachment.size as i64).into());
            attachments.push(attachment_map.into());
        }
        map.insert("attachments".into(), attachments.into());

        map
    }

    fn create_rhai_engine(crash_id: uuid::Uuid) -> Engine {
        let mut engine = Engine::new();
        engine.build_type::<TrackedAnnotations>();

        engine.on_print(move |message| {
            info!(crash_id = %crash_id, rhai_log = true, "{}", message);
        });

        engine.on_debug(move |message, source, pos| {
            if let Some(source) = source {
                debug!(crash_id = %crash_id, rhai_log = true, script = %source, line = pos.line().unwrap_or(0), "{}", message);
            } else {
                debug!(crash_id = %crash_id, rhai_log = true, "{}", message);
            }
        });

        engine.register_fn("timestamp", || chrono::Utc::now().timestamp());
        engine.register_fn("parse_iso8601", |s: &str| -> rhai::Dynamic {
            match chrono::DateTime::parse_from_rfc3339(s) {
                Ok(dt) => dt.timestamp().into(),
                Err(_) => rhai::Dynamic::UNIT,
            }
        });

        engine.register_fn("validation_error", |message: &str| -> rhai::Map {
            let mut result = rhai::Map::new();
            result.insert("valid".into(), false.into());
            result.insert("error".into(), message.to_string().into());
            result
        });
        engine.register_fn("validation_success", || -> rhai::Map {
            let mut result = rhai::Map::new();
            result.insert("valid".into(), true.into());
            result
        });

        engine
    }

    fn validate_with_rhai_script(
        script_path: &str,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError> {
        debug!(crash_id = %crash_info.crash_id, script_path = %script_path, "Running Rhai validation script");

        let script = std::fs::read_to_string(script_path).map_err(|e| {
            error!(script_path = %script_path, error = ?e, "Failed to load validation script");
            ApiError::Failure(format!("Failed to load validation script '{script_path}': {e}"))
        })?;

        let engine = Self::create_rhai_engine(crash_info.crash_id);
        let crash_info_map = Self::convert_crash_info_to_rhai_map(crash_info);
        let mut scope = rhai::Scope::new();
        scope.push("crash_info", crash_info_map);

        match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, &script) {
            Ok(result) => Self::handle_validation_result(result, &mut scope, crash_info),
            Err(e) => {
                error!(crash_id = %crash_info.crash_id, error = %e, "Rhai validation script execution failed");
                Err(ApiError::ValidationError(
                    crash_info.authorized_product.clone().unwrap_or_default(),
                    "Validation script failed".to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations::TrackedAnnotations;
    use rhai::{Engine, Scope};

    #[test]
    fn test_crash_info_annotations_is_tracked_annotations_in_rhai() {
        // Create a test CrashInfo with some annotations
        let mut crash_info = CrashInfo {
            crash_id: uuid::Uuid::new_v4(),
            submission_timestamp: "2023-01-01T00:00:00Z".to_string(),
            authorized_product: Some("TestProduct".to_string()),
            product_id: Some(uuid::Uuid::new_v4()),
            minidump: None,
            attachments: vec![],
            annotations: HashMap::new(),
        };

        crash_info.annotations.insert(
            "existing_key".to_string(),
            AnnotationEntry {
                value: "existing_value".to_string(),
                source: "test".to_string(),
            },
        );

        // Create a Rhai engine and convert crash_info to map
        let mut engine = Engine::new();
        engine.build_type::<TrackedAnnotations>();

        let mut scope = Scope::new();
        let crash_info_map = MinidumpApi::convert_crash_info_to_rhai_map(&crash_info);
        scope.push("crash_info", crash_info_map);

        // Test script that uses crash_info.annotations as TrackedAnnotations
        let script = r#"
            // Test that we can read existing annotations with bracket notation
            let existing = crash_info.annotations["existing_key"];

            // Test that we can write to annotations with bracket notation
            crash_info.annotations["script_key"] = "script_value";

            // Test that we can read the value we just wrote
            let script_val = crash_info.annotations["script_key"];

            // Return the values to verify they work
            #{
                existing: existing,
                script_val: script_val
            }
        "#;

        let result = engine
            .eval_with_scope::<rhai::Map>(&mut scope, script)
            .unwrap();

        // Verify the script could read and write annotations
        assert_eq!(
            result
                .get("existing")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "existing_value"
        );
        assert_eq!(
            result
                .get("script_val")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "script_value"
        );

        // Verify that the modifications are tracked in the scope
        let updated_crash_info = scope.get_value::<rhai::Map>("crash_info").unwrap();
        let annotations = updated_crash_info
            .get("annotations")
            .unwrap()
            .clone()
            .try_cast::<TrackedAnnotations>()
            .unwrap();
        assert!(annotations.was_modified());

        let finalized = annotations.finalize();
        assert_eq!(finalized.get("existing_key").unwrap().value, "existing_value");
        assert_eq!(finalized.get("script_key").unwrap().value, "script_value");
        assert_eq!(finalized.get("script_key").unwrap().source, "script");
    }
}
