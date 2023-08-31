use async_trait::async_trait;
use sea_orm::DatabaseConnection;

use super::{base::BaseApi, error::ApiError};

use crate::model::{base::BaseRepoWithSecondaryKey, product::ProductRepo, version::VersionRepo};

pub struct VersionApi;

#[async_trait]
impl BaseApi<VersionRepo> for VersionApi {
    async fn req(
        db: &DatabaseConnection,
        json: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        let product = &json["product"];
        if !product.is_null() {
            let product_id = ProductRepo::get_by_secondary_id(
                db,
                product.as_str().ok_or(ApiError::Failure)?.to_owned(),
            )
            .await
            .map(|product| product.id)?;
            let mut json = json.clone();
            json["product_id"] = serde_json::Value::String(product_id.to_string());
            return Ok(json);
        }
        Ok(json)
    }
}
