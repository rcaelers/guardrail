use async_trait::async_trait;
use sea_orm::DatabaseConnection;

use crate::{
    entity::{prelude::Version, version},
    model::{
        base::Repo,
        version::{VersionCreateDto, VersionUpdateDto},
    },
};

use super::{
    base::{Resource, ResourceFilter},
    error::ApiError,
};

impl Resource for Version {
    type Entity = version::Entity;
    type ActiveModel = version::ActiveModel;
    type Data = version::Model;
    type CreateData = VersionCreateDto;
    type UpdateData = VersionUpdateDto;
    type Filter = Version;
}

#[async_trait]
impl ResourceFilter for Version {
    async fn req(
        db: &DatabaseConnection,
        json: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
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
            let mut json = json.clone();
            json["product_id"] = serde_json::Value::String(product_id.to_string());
            return Ok(json);
        }
        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use crate::api::base::tests::*;
    use crate::entity::version;
    use serial_test::serial;

    #[derive(serde::Deserialize, Debug)]
    pub struct ApiResponseWithPayload {
        pub result: String,
        pub payload: version::Model,
    }

    #[derive(serde::Deserialize, Debug)]
    pub struct ApiResponseWithVecPayload {
        pub result: String,
        pub payload: Vec<version::Model>,
    }

    #[serial]
    #[tokio::test]
    async fn test_add_version() {
        let server = run_server().await;

        let response = server
            .post("/api/product")
            .content_type("application/json")
            .json(&serde_json::json!({
                "name":"Workrave" ,
            }))
            .await;
        response.assert_status_ok();
        let product1 = response.json::<ApiResponseWithId>();

        let response = server
            .post("/api/product")
            .content_type("application/json")
            .json(&serde_json::json!({
                "name":"Scroom" ,
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

        let response = server
        .post("/api/version")
        .content_type("application/json")
        .json(&serde_json::json!({
            "name":"1.12", "hash":"1234567890", "tag": "v1.12", "product_id": format!("{}", product2.id)
        }))
        .await;
        response.assert_status_ok();
        let version3 = response.json::<ApiResponseWithId>();
        assert_eq!(version3.result, "ok");

        let response = server
            .get("/api/version")
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let versions = response.json::<ApiResponseWithVecPayload>();
        assert_eq!(versions.result, "ok");
        assert_eq!(versions.payload.len(), 3);
        assert_eq!(versions.payload[0].name, "1.11");
        assert_eq!(versions.payload[1].name, "1.12");
        assert_eq!(versions.payload[2].name, "1.12");
        assert_eq!(versions.payload[0].id.to_string(), version1.id);
        assert_eq!(versions.payload[1].id.to_string(), version2.id);
        assert_eq!(versions.payload[2].id.to_string(), version3.id);
        assert_eq!(versions.payload[0].product_id.to_string(), product1.id);
        assert_eq!(versions.payload[1].product_id.to_string(), product1.id);
        assert_eq!(versions.payload[2].product_id.to_string(), product2.id);
        assert_eq!(versions.payload[0].tag, "v1.11");
        assert_eq!(versions.payload[1].tag, "v1.12");
        assert_eq!(versions.payload[2].tag, "v1.12");
        assert_eq!(versions.payload[0].hash, "1234567890");
        assert_eq!(versions.payload[1].hash, "1234567890");
        assert_eq!(versions.payload[2].hash, "1234567890");

        let response = server
            .get(format!("/api/version/{}", version1.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let mut version = response.json::<ApiResponseWithPayload>();
        assert_eq!(version.result, "ok");
        assert_eq!(version.payload.name, "1.11");
        assert_eq!(version.payload.id.to_string(), version1.id);
        assert_eq!(version.payload.product_id.to_string(), product1.id);
        assert_eq!(version.payload.tag, "v1.11");
        assert_eq!(version.payload.hash, "1234567890");

        version.payload.name = "1.11.2".to_string();
        version.payload.tag = "v1.11.2".to_string();
        version.payload.hash = "23894723894".to_string();

        let response = server
            .put(format!("/api/version/{}", version1.id).as_str())
            .content_type("application/json")
            .json(&serde_json::json!(version.payload))
            .await;
        response.assert_status_ok();
        let version = response.json::<ApiResponse>();
        assert_eq!(version.result, "ok");

        let response = server
            .get(format!("/api/version/{}", version1.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let version = response.json::<ApiResponseWithPayload>();
        assert_eq!(version.result, "ok");
        assert_eq!(version.payload.name, "1.11.2");
        assert_eq!(version.payload.id.to_string(), version1.id);
        assert_eq!(version.payload.product_id.to_string(), product1.id);
        assert_eq!(version.payload.tag, "v1.11.2");
        assert_eq!(version.payload.hash, "23894723894");

        let response = server
            .delete(format!("/api/version/{}", version2.id).as_str())
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let version = response.json::<ApiResponse>();
        assert_eq!(version.result, "ok");

        let response = server
            .get("/api/version")
            .content_type("application/json")
            .await;
        response.assert_status_ok();
        let versions = response.json::<ApiResponseWithVecPayload>();
        assert_eq!(versions.result, "ok");
        assert_eq!(versions.payload.len(), 2);
        assert_eq!(versions.payload[0].id.to_string(), version1.id);
        assert_eq!(versions.payload[1].id.to_string(), version3.id);
    }

    #[serial]
    #[tokio::test]
    async fn test_product_not_found() {
        let server = run_server().await;

        let response = server
            .post("/api/version")
            .content_type("application/json")
            .json(&serde_json::json!({
                "name":"1.11", "hash":"1234567890", "tag": "v1.11", "product": "Workrave"
            }))
            .await;
        response.assert_status_not_found();
        let version1 = response.json::<ApiResponseFailed>();
        assert_eq!(version1.result, "failed");
    }

    #[serial]
    #[tokio::test]
    async fn test_not_unique() {
        let server = run_server().await;

        let response = server
            .post("/api/product")
            .content_type("application/json")
            .json(&serde_json::json!({
                "name":"Workrave" ,
            }))
            .await;
        response.assert_status_ok();
        let product1 = response.json::<ApiResponseWithId>();

        let response = server
            .post("/api/version")
            .content_type("application/json")
            .json(&serde_json::json!({
                "name":"1.11", "hash":"1234567890", "tag": "v1.11", "product": "Workrave"
            }))
            .await;
        response.assert_status_ok();
        let version = response.json::<ApiResponse>();
        assert_eq!(version.result, "ok");

        let response = server
            .post("/api/version")
            .content_type("application/json")
            .json(&serde_json::json!({
                "name":"1.11", "hash":"1234567890", "tag": "v1.11", "product": "Workrave"
            }))
            .await;

        response.assert_status_bad_request();
        let version = response.json::<ApiResponseFailed>();
        assert_eq!(version.result, "failed");
    }

    #[serial]
    #[tokio::test]
    async fn test_incomplete_json() {
        let server = run_server().await;

        let response = server
            .post("/api/product")
            .content_type("application/json")
            .json(&serde_json::json!({
                "name":"Workrave" ,
            }))
            .await;
        response.assert_status_ok();
        let product1 = response.json::<ApiResponseWithId>();

        let response = server
            .post("/api/version")
            .content_type("application/json")
            .json(&serde_json::json!({
                "hash":"1234567890", "tag": "v1.11", "product": "Workrave"
            }))
            .await;

        response.assert_status_bad_request();
        let version = response.json::<ApiResponseFailed>();
        assert_eq!(version.result, "failed");
    }
}
