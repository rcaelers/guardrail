use axum::Json;
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Path as AxumPath, State};
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use rhai::Engine;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

use crate::annotations::{AnnotationEntry, TrackedAnnotations};
use crate::error::ApiError;
use crate::state::AppState;
use crate::utils::stream_to_s3;

pub struct MinidumpApi;

#[derive(Debug, Serialize)]
pub struct MinidumpResponse {
    pub result: String,
    pub crash_id: Option<String>,
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
    crash_id: String,
    submission_timestamp: String,
    product: Option<String>,
    product_id: Option<String>,
    product_metadata: Option<serde_json::Value>,
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
            "user-text" => Self::handle_attachment_upload(field, crash_info, state).await,
            _ => {
                if file_name.is_empty() {
                    Self::handle_annotation_upload(field, crash_info).await
                } else {
                    Self::handle_attachment_upload(field, crash_info, state).await
                }
            }
        }
    }

    #[instrument(skip(crash_info), fields(crash_id = %crash_info.crash_id))]
    fn validate_minidump_presence(crash_info: &CrashInfo) -> Result<(), ApiError> {
        debug!(crash_id = %crash_info.crash_id, "Validating minidump presence");
        if crash_info.minidump.is_none() {
            error!("No minidump found in submission");
            return Err(ApiError::Failure("no minidump found in submission".to_string()));
        }
        Ok(())
    }

    #[instrument(skip(crash_info, mandatory_annotations), fields(crash_id = %crash_info.crash_id))]
    fn validate_mandatory_annotations(
        crash_info: &CrashInfo,
        mandatory_annotations: &[String],
    ) -> Result<(), ApiError> {
        debug!(crash_id = %crash_info.crash_id, "Validating mandatory annotations");

        for required_field in mandatory_annotations {
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

        Ok(())
    }

    #[instrument(skip(crash_info, scripts), fields(crash_id = %crash_info.crash_id))]
    fn run_validation_scripts(
        crash_info: &mut CrashInfo,
        scripts: &[common::product_info::CachedValidationScript],
    ) -> Result<(), ApiError> {
        debug!(crash_id = %crash_info.crash_id, count = scripts.len(), "Running validation scripts");

        for script in scripts {
            debug!(crash_id = %crash_info.crash_id, script_name = %script.name, "Running validation script");
            Self::validate_with_rhai_script_content(&script.content, crash_info)?;
        }

        Ok(())
    }

    #[instrument(skip(script_content, crash_info), fields(crash_id = %crash_info.crash_id))]
    fn validate_with_rhai_script_content(
        script_content: &str,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError> {
        debug!(crash_id = %crash_info.crash_id, "Running Rhai validation script from content");

        let engine = Self::create_rhai_engine(crash_info.crash_id.as_str());
        let crash_info_map = Self::convert_crash_info_to_rhai_map(crash_info);
        let mut scope = rhai::Scope::new();
        scope.push("crash_info", crash_info_map);

        match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, script_content) {
            Ok(result) => Self::handle_validation_result(result, &mut scope, crash_info),
            Err(e) => {
                error!(crash_id = %crash_info.crash_id, error = %e, "Rhai validation script execution failed");
                Err(ApiError::ValidationError(
                    crash_info.product.clone().unwrap_or_default(),
                    "Validation script failed".to_string(),
                ))
            }
        }
    }

    #[instrument(skip(storage, crash_info), fields(crash_id = %crash_id))]
    async fn upload_crash(
        storage: Arc<dyn ObjectStore>,
        crash_id: &str,
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

    #[instrument(skip(state, crash_info, multipart), fields(crash_id))]
    async fn handle_upload(
        state: AppState,
        token: &str,
        mut multipart: Multipart,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError> {
        let product = state
            .product_cache
            .get_product_by_token(token)
            .await?
            .ok_or_else(|| {
                error!(token = %token, "Product not found for product token");
                ApiError::ProductNotFound(token.to_string())
            })?;

        if !product.accepting_crashes {
            return Err(ApiError::ProductNotAcceptingCrashes(product.name.clone()));
        }

        crash_info.product = Some(product.name.clone());
        crash_info.product_id = Some(product.id.clone());
        crash_info.product_metadata = Some(product.metadata.clone());
        crash_info.annotations.insert(
            "product".to_string(),
            AnnotationEntry {
                value: product.name.clone(),
                source: "submission".to_string(),
            },
        );

        info!(product = %product.name, "Processing crash for product");

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            error!(error = ?e, "Failed to get next multipart field");
            ApiError::Failure("failed to read multipart field from upload".to_string())
        })? {
            Self::process_field(field, crash_info, state.clone()).await?;
        }

        Self::validate_minidump_presence(crash_info)?;
        Self::validate_mandatory_annotations(crash_info, &product.mandatory_annotations)?;
        Self::run_validation_scripts(crash_info, &product.validation_scripts)?;

        let mut crash_info_json = serde_json::to_value(&crash_info).map_err(|e| {
            error!(error = ?e, "Failed to serialize crash info");
            ApiError::Failure("failed to serialize crash info".to_string())
        })?;

        if let Some(ref ps) = product.processor_settings {
            crash_info_json["processor_settings"] =
                serde_json::to_value(ps).unwrap_or(serde_json::Value::Null);
        }

        Self::upload_crash(
            state.storage.clone(),
            crash_info.crash_id.as_str(),
            crash_info_json.clone(),
        )
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

    #[instrument(skip(state, multipart), fields(crash_id))]
    pub async fn upload(
        State(state): State<AppState>,
        AxumPath(token): AxumPath<String>,
        multipart: Multipart,
    ) -> Result<Json<MinidumpResponse>, ApiError> {
        let crash_id = uuid::Uuid::new_v4().to_string();
        tracing::Span::current().record("crash_id", crash_id.clone());
        info!(crash_id = %crash_id, "Received minidump upload request");

        let mut crash_info = CrashInfo {
            crash_id: crash_id.clone(),
            submission_timestamp: chrono::Utc::now().to_rfc3339(),
            product: None,
            minidump: None,
            attachments: Vec::new(),
            annotations: HashMap::new(),
            ..Default::default()
        };

        let r = Self::handle_upload(state.clone(), &token, multipart, &mut crash_info).await;
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

    #[instrument(skip(scope, crash_info), fields(crash_id = %crash_info.crash_id))]
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

    #[instrument(skip(map, crash_info), fields(crash_id = %crash_info.crash_id))]
    fn handle_validation_failure(map: &rhai::Map, crash_info: &CrashInfo) -> Result<(), ApiError> {
        let error_message = map
            .get("error")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| "Validation failed".to_string());

        error!(crash_id = %crash_info.crash_id, error = %error_message, "Validation script returned failure");
        Err(ApiError::ValidationError(
            crash_info.product.clone().unwrap_or_default(),
            error_message,
        ))
    }

    #[instrument(skip(map, crash_info), fields(crash_id = %crash_info.crash_id))]
    fn extract_validation_result(
        map: &rhai::Map,
        crash_info: &CrashInfo,
    ) -> Result<bool, ApiError> {
        map.get("valid")
            .and_then(|v| v.as_bool().ok())
            .ok_or_else(|| {
                error!(crash_id = %crash_info.crash_id, "Validation result missing 'valid' field");
                ApiError::ValidationError(
                    crash_info.product.clone().unwrap_or_default(),
                    "Invalid validation result".to_string(),
                )
            })
    }

    #[instrument(skip(result, scope, crash_info), fields(crash_id = %crash_info.crash_id))]
    fn handle_validation_result(
        result: rhai::Dynamic,
        scope: &mut rhai::Scope,
        crash_info: &mut CrashInfo,
    ) -> Result<(), ApiError> {
        debug!(crash_id = %crash_info.crash_id, "Rhai validation script executed successfully");

        let map = result.try_cast::<rhai::Map>().ok_or_else(|| {
            error!(crash_id = %crash_info.crash_id, "Validation script must return a validation result");
            ApiError::ValidationError(
                crash_info.product.clone().unwrap_or_default(),
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

    fn json_to_rhai_dynamic(value: &serde_json::Value) -> rhai::Dynamic {
        match value {
            serde_json::Value::Null => rhai::Dynamic::UNIT,
            serde_json::Value::Bool(b) => (*b).into(),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    i.into()
                } else if let Some(f) = n.as_f64() {
                    f.into()
                } else {
                    rhai::Dynamic::UNIT
                }
            }
            serde_json::Value::String(s) => s.clone().into(),
            serde_json::Value::Array(arr) => {
                let rhai_arr: Vec<rhai::Dynamic> =
                    arr.iter().map(Self::json_to_rhai_dynamic).collect();
                rhai_arr.into()
            }
            serde_json::Value::Object(obj) => {
                let mut map = rhai::Map::new();
                for (k, v) in obj {
                    map.insert(k.as_str().into(), Self::json_to_rhai_dynamic(v));
                }
                map.into()
            }
        }
    }

    #[instrument(skip(crash_info), fields(crash_id = %crash_info.crash_id))]
    fn convert_crash_info_to_rhai_map(crash_info: &CrashInfo) -> rhai::Map {
        let mut map = rhai::Map::new();

        map.insert("crash_id".into(), crash_info.crash_id.clone().into());
        map.insert("submission_timestamp".into(), crash_info.submission_timestamp.clone().into());

        if let Some(ref product) = crash_info.product {
            map.insert("product".into(), product.clone().into());
        }

        if let Some(ref product_id) = crash_info.product_id {
            map.insert("product_id".into(), product_id.clone().into());
        }

        if let Some(ref metadata) = crash_info.product_metadata {
            map.insert("product_metadata".into(), Self::json_to_rhai_dynamic(metadata));
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

    #[instrument(fields(crash_id = %crash_id))]
    fn create_rhai_engine(crash_id: &str) -> Engine {
        let mut engine = Engine::new();
        engine.build_type::<TrackedAnnotations>();

        let crash_id_for_print = crash_id.to_string();

        engine.on_print(move |message| {
            info!(crash_id = %crash_id_for_print, rhai_log = true, "{}", message);
        });

        let crash_id_for_debug = crash_id.to_string();
        engine.on_debug(move |message, source, pos| {
            if let Some(source) = source {
                debug!(crash_id = %crash_id_for_debug, rhai_log = true, script = %source, line = pos.line().unwrap_or(0), "{}", message);
            } else {
                debug!(crash_id = %crash_id_for_debug, rhai_log = true, "{}", message);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations::TrackedAnnotations;
    use crate::product_cache::ProductCache;
    use crate::worker::TestWorker;
    use axum::body::Body;
    use axum::extract::FromRequest;
    use axum::http::Request;
    use axum::http::header::CONTENT_TYPE;
    use common::product_info::ProductInfo;
    use object_store::memory::InMemory;
    use rhai::{Engine, Scope};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn crash_info() -> CrashInfo {
        CrashInfo {
            crash_id: "crash-1".to_string(),
            submission_timestamp: "2023-01-01T00:00:00Z".to_string(),
            product: Some("TestProduct".to_string()),
            product_id: Some("product-1".to_string()),
            product_metadata: Some(json!({
                "string": "value",
                "bool": true,
                "int": 7,
                "float": 1.5,
                "array": [1, "two"],
                "object": {"nested": "yes"},
                "null": null
            })),
            minidump: Some(Minidump {
                filename: "crash.dmp".to_string(),
                content_type: "application/octet-stream".to_string(),
                size: 123,
                storage_path: "minidumps/one".to_string(),
                storage_id: uuid::Uuid::nil(),
            }),
            attachments: vec![Attachment {
                name: "log".to_string(),
                filename: "log.txt".to_string(),
                content_type: "text/plain".to_string(),
                size: 12,
                storage_path: "attachments/one".to_string(),
                storage_id: uuid::Uuid::nil(),
            }],
            annotations: HashMap::from([(
                "product".to_string(),
                AnnotationEntry {
                    value: "TestProduct".to_string(),
                    source: "submission".to_string(),
                },
            )]),
        }
    }

    #[test]
    fn validators_accept_and_reject_expected_values() {
        assert!(MinidumpApi::validate_annotation_content_type("text/plain").is_ok());
        assert!(MinidumpApi::validate_annotation_content_type("text/markdown").is_ok());
        assert!(MinidumpApi::validate_annotation_content_type("").is_ok());
        assert!(matches!(
            MinidumpApi::validate_annotation_content_type("application/json"),
            Err(ApiError::Failure(message)) if message.contains("invalid annotation")
        ));

        assert!(MinidumpApi::validate_minidump_content_type("application/octet-stream").is_ok());
        assert!(MinidumpApi::validate_minidump_content_type("").is_ok());
        assert!(matches!(
            MinidumpApi::validate_minidump_content_type("text/plain"),
            Err(ApiError::Failure(message)) if message.contains("invalid minidump")
        ));

        assert!(MinidumpApi::validate_key("product").is_ok());
        assert!(matches!(
            MinidumpApi::validate_key("bad\nkey"),
            Err(ApiError::Failure(message)) if message.contains("printable ASCII")
        ));
        assert!(matches!(
            MinidumpApi::validate_key("cafe\u{00e9}"),
            Err(ApiError::Failure(message)) if message.contains("printable ASCII")
        ));
    }

    #[test]
    fn validates_minidump_and_mandatory_annotations() {
        let mut info = crash_info();
        assert!(MinidumpApi::validate_minidump_presence(&info).is_ok());
        assert!(
            MinidumpApi::validate_mandatory_annotations(&info, &["product".to_string()]).is_ok()
        );

        info.minidump = None;
        assert!(matches!(
            MinidumpApi::validate_minidump_presence(&info),
            Err(ApiError::Failure(message)) if message.contains("no minidump")
        ));

        info.minidump = crash_info().minidump;
        assert!(matches!(
            MinidumpApi::validate_mandatory_annotations(&info, &["version".to_string()]),
            Err(ApiError::Failure(message)) if message.contains("required annotation 'version' is missing")
        ));

        info.annotations.insert(
            "version".to_string(),
            AnnotationEntry {
                value: "   ".to_string(),
                source: "submission".to_string(),
            },
        );
        assert!(matches!(
            MinidumpApi::validate_mandatory_annotations(&info, &["version".to_string()]),
            Err(ApiError::Failure(message)) if message.contains("cannot be empty")
        ));
    }

    #[test]
    fn converts_crash_info_to_rhai_map_with_nested_metadata() {
        let map = MinidumpApi::convert_crash_info_to_rhai_map(&crash_info());

        assert_eq!(map["crash_id"].clone().into_string().unwrap(), "crash-1");
        assert_eq!(map["product"].clone().into_string().unwrap(), "TestProduct");
        assert!(map.contains_key("product_metadata"));
        assert!(map.contains_key("minidump"));
        assert_eq!(map["attachments"].clone().into_array().unwrap().len(), 1);
    }

    #[test]
    fn rhai_engine_helpers_return_expected_values() {
        let engine = MinidumpApi::create_rhai_engine("crash-1");
        let valid = engine
            .eval::<rhai::Map>("validation_success()")
            .expect("validation_success should return a map");
        assert!(valid["valid"].as_bool().unwrap());

        let invalid = engine
            .eval::<rhai::Map>("validation_error(\"bad\")")
            .expect("validation_error should return a map");
        assert!(!invalid["valid"].as_bool().unwrap());
        assert_eq!(invalid["error"].clone().into_string().unwrap(), "bad");

        let timestamp = engine
            .eval::<rhai::Dynamic>("parse_iso8601(\"2023-01-01T00:00:00Z\")")
            .unwrap();
        assert_eq!(timestamp.as_int().unwrap(), 1_672_531_200);
        assert!(
            engine
                .eval::<rhai::Dynamic>("parse_iso8601(\"not-a-date\")")
                .unwrap()
                .is_unit()
        );
    }

    #[test]
    fn validation_result_paths_are_checked() {
        let mut info = crash_info();
        let mut scope = Scope::new();

        assert!(matches!(
            MinidumpApi::handle_validation_result("not a map".into(), &mut scope, &mut info),
            Err(ApiError::ValidationError(_, message)) if message == "Validation script failed"
        ));

        let missing_valid = rhai::Map::new();
        assert!(matches!(
            MinidumpApi::extract_validation_result(&missing_valid, &info),
            Err(ApiError::ValidationError(_, message)) if message == "Invalid validation result"
        ));

        let mut failure = rhai::Map::new();
        failure.insert("valid".into(), false.into());
        failure.insert("error".into(), "blocked".into());
        assert!(matches!(
            MinidumpApi::handle_validation_result(failure.into(), &mut scope, &mut info),
            Err(ApiError::ValidationError(product, message))
                if product == "TestProduct" && message == "blocked"
        ));

        let mut failure_without_error = rhai::Map::new();
        failure_without_error.insert("valid".into(), false.into());
        assert!(matches!(
            MinidumpApi::handle_validation_result(
                failure_without_error.into(),
                &mut scope,
                &mut info
            ),
            Err(ApiError::ValidationError(_, message)) if message == "Validation failed"
        ));

        let mut success = rhai::Map::new();
        success.insert("valid".into(), true.into());
        assert!(
            MinidumpApi::handle_validation_result(success.into(), &mut scope, &mut info).is_ok()
        );
    }

    #[test]
    fn validation_success_applies_modified_annotations_from_scope() {
        let mut info = crash_info();
        let mut map = MinidumpApi::convert_crash_info_to_rhai_map(&info);
        let annotations = map
            .get("annotations")
            .and_then(|value| value.clone().try_cast::<TrackedAnnotations>())
            .unwrap();
        annotations.set("script_key".to_string(), "script_value".to_string());
        map.insert("annotations".into(), rhai::Dynamic::from(annotations));

        let mut scope = Scope::new();
        scope.push("crash_info", map);

        MinidumpApi::handle_validation_success(&mut scope, &mut info).unwrap();

        assert_eq!(info.annotations.get("script_key").unwrap().value, "script_value");
    }

    #[test]
    fn run_validation_scripts_executes_all_scripts_in_order() {
        let script_content = r#"
            crash_info.annotations["validated"] = "yes";
            validation_success()
        "#;
        let script = common::product_info::CachedValidationScript {
            id: "s1".to_string(),
            name: "test.rhai".to_string(),
            content: script_content.to_string(),
        };
        let mut info = crash_info();

        MinidumpApi::run_validation_scripts(&mut info, &[script]).unwrap();
        assert_eq!(info.annotations["validated"].value, "yes");
    }

    #[test]
    fn run_validation_scripts_empty_slice_is_noop() {
        let mut info = crash_info();
        MinidumpApi::run_validation_scripts(&mut info, &[]).unwrap();
    }

    #[test]
    fn validate_with_rhai_script_content_reports_invalid_results_and_runtime_errors() {
        let mut info = crash_info();
        assert!(matches!(
            MinidumpApi::validate_with_rhai_script_content("42", &mut info),
            Err(ApiError::ValidationError(_, message)) if message == "Validation script failed"
        ));

        assert!(matches!(
            MinidumpApi::validate_with_rhai_script_content(
                "let x = 1 / 0; validation_success()",
                &mut info
            ),
            Err(ApiError::ValidationError(_, message)) if message == "Validation script failed"
        ));
    }

    #[tokio::test]
    async fn upload_crash_writes_json_to_storage() {
        let store = Arc::new(InMemory::new());
        MinidumpApi::upload_crash(store.clone(), "crash-1", json!({"ok": true}))
            .await
            .unwrap();

        let bytes = store
            .get(&object_store::path::Path::from("crashes/crash-1.json"))
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), br#"{"ok":true}"#);
    }

    #[tokio::test]
    async fn process_crash_reports_worker_failures() {
        let worker = Arc::new(TestWorker::new());
        worker.failure();
        let settings = crate::settings::Settings::test_default();
        let token = "worker_failure_test_token_000001";
        let product = ProductInfo {
            id: "product-1".to_string(),
            name: "TestProduct".to_string(),
            accepting_crashes: true,
            metadata: json!({}),
            mandatory_annotations: vec![],
            validation_scripts: vec![],
            processor_settings: None,
        };
        let state = AppState {
            product_cache: ProductCache::from_token_map(HashMap::from([(
                token.to_string(),
                product,
            )])),
            settings: Arc::new(settings),
            storage: Arc::new(InMemory::new()),
            worker,
        };
        let mut info = crash_info();
        info.minidump = None;
        info.annotations.clear();
        let boundary = "----guardrail-ingestion-boundary";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"upload_file_minidump\"; filename=\"test.dmp\"\r\nContent-Type: application/octet-stream\r\n\r\nMINIDUMP DATA\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"product\"\r\nContent-Type: text/plain\r\n\r\nTestProduct\r\n\
             --{boundary}--\r\n"
        );
        let request = Request::builder()
            .header(CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
            .body(Body::from(body))
            .unwrap();
        let multipart = Multipart::from_request(request, &state).await.unwrap();

        assert!(matches!(
            MinidumpApi::handle_upload(state, token, multipart, &mut info).await,
            Err(ApiError::Failure(message)) if message == "failed to queue minidump job"
        ));
    }

    #[test]
    fn test_crash_info_annotations_is_tracked_annotations_in_rhai() {
        // Create a test CrashInfo with some annotations
        let mut crash_info = CrashInfo {
            crash_id: uuid::Uuid::new_v4().to_string(),
            submission_timestamp: "2023-01-01T00:00:00Z".to_string(),
            product: Some("TestProduct".to_string()),
            product_id: Some(uuid::Uuid::new_v4().to_string()),
            minidump: None,
            attachments: vec![],
            annotations: HashMap::new(),
            ..Default::default()
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
            fn check(crash_info, annotations) {
                annotations["check_product1"] = "pending";
            }

            // Test that we can read existing annotations with bracket notation
            let existing = crash_info.annotations["existing_key"];

            // Test that we can write to annotations with bracket notation
            crash_info.annotations["script_key"] = "script_value";

            let annotations = crash_info["annotations"];
            annotations["check_product2"] = "pending";
            check(crash_info, annotations);

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
