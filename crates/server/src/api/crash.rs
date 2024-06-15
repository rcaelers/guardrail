use super::{
    base::{Resource, ResourceFilter},
    error::ApiError,
};
use crate::{
    entity::{crash, prelude::Crash},
    model::{
        base::Repo,
        crash::{CrashCreateDto, CrashUpdateDto},
        version::VersionRepo,
    },
};
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use uuid::Uuid;

impl Resource for Crash {
    type Entity = crash::Entity;
    type ActiveModel = crash::ActiveModel;
    type Data = crash::Model;
    type CreateData = CrashCreateDto;
    type UpdateData = CrashUpdateDto;
    type Filter = crash::Model;
}

#[async_trait]
impl ResourceFilter for crash::Model {
    async fn req(
        db: &DatabaseConnection,
        json: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        let mut json = json.clone();
        let product = json["product"].as_str();
        if let Some(product) = product {
            let product_id = Repo::get_by_column::<crate::entity::product::Entity, _, _>(
                db,
                crate::entity::product::Column::Name,
                product.to_owned(),
            )
            .await?
            .map(|product| product.id)
            .ok_or_else(|| ApiError::ForeignKeyError("product".to_owned(), product.to_owned()))?;
            json["product_id"] = serde_json::Value::String(product_id.to_string());
        }
        let version = json["version"].as_str();
        if let Some(version) = version {
            let product_id = json["product_id"]
                .as_str()
                .ok_or_else(|| ApiError::APIFailure("no product_id".to_owned()))?;
            let product_id =
                Uuid::from_str(product_id).map_err(|e| ApiError::APIFailure(e.to_string()))?;

            let version_id =
                VersionRepo::get_by_product_and_name(db, product_id, version.to_owned())
                    .await?
                    .map(|version| version.id)
                    .ok_or_else(|| {
                        ApiError::ForeignKeyError("version".to_owned(), version.to_owned())
                    })?;

            json["version_id"] = serde_json::Value::String(version_id.to_string());
        }
        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use crate::{api::base::tests::*, entity::crash};
    use axum_test::TestServer;
    use serial_test::serial;

    #[derive(serde::Deserialize, Debug)]
    struct ApiResponseWithPayload {
        pub result: String,
        pub payload: crash::Model,
    }

    #[derive(serde::Deserialize, Debug)]
    struct ApiResponseWithVecPayload {
        pub result: String,
        pub payload: Vec<crash::Model>,
    }

    struct Context {
        pub server: TestServer,
        pub product1: ApiResponseWithId,
        pub product2: ApiResponseWithId,
        pub version1: ApiResponseWithId,
        pub version2: ApiResponseWithId,
    }

    impl Context {
        pub async fn new() -> Context {
            let server = run_server().await;

            let response = server
                .post("/api/product")
                .content_type("application/json")
                .json(&serde_json::json!({
                   "name":"Workrave"
                }))
                .await;
            response.assert_status_ok();
            let product1 = response.json::<ApiResponseWithId>();

            let response = server
                .post("/api/product")
                .content_type("application/json")
                .json(&serde_json::json!({
                  "name":"Scroom"
                }))
                .await;
            response.assert_status_ok();
            let product2 = response.json::<ApiResponseWithId>();

            let response = server
                .post("/api/version")
                .content_type("application/json")
                .json(&serde_json::json!({
                    "name":"1.11", "hash":"1234567890", "tag": "v1.11", "product": "Workrave"
                }))
                .await;
            response.assert_status_ok();
            let version1 = response.json::<ApiResponseWithId>();
            assert_eq!(version1.result, "ok");

            let response = server
              .post("/api/version")
              .content_type("application/json")
              .json(&serde_json::json!({
                  "name":"1.12", "hash":"1234567890", "tag": "v1.12", "product_id": format!("{}", product1.id)
              }))
              .await;
            response.assert_status_ok();
            let version2 = response.json::<ApiResponseWithId>();
            assert_eq!(version2.result, "ok");

            Context {
                server,
                product1,
                product2,
                version1,
                version2,
            }
        }
    }

    #[serial]
    #[tokio::test]
    async fn test_add_crash() {
        let context = Context::new().await;

        let response = context
            .server
            .post("/api/crash")
            .content_type("application/json")
            .json(&serde_json::json!({
               "report":"Report1", "version": "1.11", "product": "Workrave", "summary": "Summary1"
            }))
            .await;
        response.assert_status_ok();
        let crash1 = response.json::<ApiResponseWithId>();
        assert_eq!(crash1.result, "ok");

        let response = context
            .server
            .post("/api/crash")
            .content_type("application/json")
            .json(&serde_json::json!({
              "report":"Report2", "version": "1.12", "product_id": format!("{}", context.product1.id), "summary": "Summary1"
            }))
            .await;
        response.assert_status_ok();
        let crash2 = response.json::<ApiResponseWithId>();
        assert_eq!(crash2.result, "ok");

        let response = context
            .server
            .get("/api/crash")
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let crashes = response.json::<ApiResponseWithVecPayload>();
        assert_eq!(crashes.result, "ok");
        assert_eq!(crashes.payload.len(), 2);
        assert_eq!(crashes.payload[0].id.to_string(), crash1.id);
        assert_eq!(crashes.payload[1].id.to_string(), crash2.id);
        assert_eq!(crashes.payload[0].report, "Report1");
        assert_eq!(crashes.payload[1].report, "Report2");

        let response = context
            .server
            .get(format!("/api/crash/{}", crash1.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let mut crash = response.json::<ApiResponseWithPayload>();
        assert_eq!(crash.result, "ok");
        assert_eq!(crash.payload.id.to_string(), crash1.id);
        assert_eq!(crash.payload.report, "Report1");

        crash.payload.report = serde_json::json!("Report1a");

        let response = context
            .server
            .put(format!("/api/crash/{}", crash1.id).as_str())
            .content_type("application/json")
            .json(&serde_json::json!(crash.payload))
            .await;
        response.assert_status_ok();
        let crash = response.json::<ApiResponse>();
        assert_eq!(crash.result, "ok");

        let response = context
            .server
            .get(format!("/api/crash/{}", crash1.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let crash = response.json::<ApiResponseWithPayload>();
        assert_eq!(crash.result, "ok");
        assert_eq!(crash.payload.id.to_string(), crash1.id);
        assert_eq!(crash.payload.report, "Report1a");

        let response = context
            .server
            .delete(format!("/api/crash/{}", crash2.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let crash = response.json::<ApiResponse>();
        assert_eq!(crash.result, "ok");

        let response = context
            .server
            .get("/api/crash")
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let crashes = response.json::<ApiResponseWithVecPayload>();
        assert_eq!(crashes.result, "ok");
        assert_eq!(crashes.payload.len(), 1);
        assert_eq!(crashes.payload[0].id.to_string(), crash1.id);
    }

    #[serial]
    #[tokio::test]
    async fn test_incomplete_json() {
        let context = Context::new().await;

        let response = context
            .server
            .post("/api/crash")
            .content_type("application/json")
            .json(&serde_json::json!({}))
            .await;

        response.assert_status_bad_request();
        let crash = response.json::<ApiResponseFailed>();
        assert_eq!(crash.result, "failed");
    }
}
