use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use uuid::Uuid;

use crate::{
    error::{RepoError, handle_surreal_error},
    record_key,
};
use data::api_token::{ApiToken, NewApiToken};

pub struct ApiTokenRepo {}

impl ApiTokenRepo {
    pub async fn get_by_id(
        db: &Surreal<Any>,
        id: impl ToString,
    ) -> Result<Option<ApiToken>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id, IF product_id != NONE THEN meta::id(product_id) ELSE NONE END as product_id, IF user_id != NONE THEN meta::id(user_id) ELSE NONE END as user_id FROM ONLY type::record('api_tokens', $id)")
            .bind(("id", record_key(id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    pub async fn get_by_token_id(
        db: &Surreal<Any>,
        token_id: Uuid,
    ) -> Result<Option<ApiToken>, RepoError> {
        let mut result = db
            .query(
                "SELECT *, meta::id(id) as id, IF product_id != NONE THEN meta::id(product_id) ELSE NONE END as product_id, IF user_id != NONE THEN meta::id(user_id) ELSE NONE END as user_id FROM api_tokens WHERE token_id = $token_id LIMIT 1",
            )
            .bind(("token_id", token_id))
            .await
            .map_err(handle_surreal_error)?;
        let tokens: Vec<ApiToken> = crate::take_many(&mut result, 0)?;
        Ok(tokens.into_iter().next())
    }

    pub async fn update_last_used(
        db: &Surreal<Any>,
        token_id: impl ToString,
    ) -> Result<(), RepoError> {
        db.query("UPDATE type::record('api_tokens', $id) SET last_used_at = time::now()")
            .bind(("id", record_key(token_id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn get_by_product_id(
        db: &Surreal<Any>,
        product_id: impl ToString,
    ) -> Result<Vec<ApiToken>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id, IF product_id != NONE THEN meta::id(product_id) ELSE NONE END as product_id, IF user_id != NONE THEN meta::id(user_id) ELSE NONE END as user_id FROM api_tokens WHERE product_id = type::record('products', $product_id) ORDER BY created_at DESC")
            .bind(("product_id", record_key(product_id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn get_by_user_id(
        db: &Surreal<Any>,
        user_id: impl ToString,
    ) -> Result<Vec<ApiToken>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id, IF product_id != NONE THEN meta::id(product_id) ELSE NONE END as product_id, IF user_id != NONE THEN meta::id(user_id) ELSE NONE END as user_id FROM api_tokens WHERE user_id = type::record('users', $user_id) ORDER BY created_at DESC")
            .bind(("user_id", record_key(user_id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }

    pub async fn create(db: &Surreal<Any>, new_token: NewApiToken) -> Result<String, RepoError> {
        let id = Uuid::new_v4().to_string();
        let _: Option<serde_json::Value> = db
            .query(
                "CREATE type::record('api_tokens', $id) CONTENT {
                description: $description,
                token_id: $token_id,
                token_hash: $token_hash,
                product_id: IF $product_id != NONE THEN type::record('products', $product_id) ELSE NONE END,
                user_id: IF $user_id != NONE THEN type::record('users', $user_id) ELSE NONE END,
                entitlements: $entitlements,
                expires_at: $expires_at,
                is_active: $is_active,
                created_at: time::now(),
                updated_at: time::now(),
            }",
            )
            .bind(("id", id.clone()))
            .bind(("description", new_token.description.clone()))
            .bind(("token_id", new_token.token_id))
            .bind(("token_hash", new_token.token_hash.clone()))
            .bind(("product_id", new_token.product_id.as_ref().map(record_key)))
            .bind(("user_id", new_token.user_id.as_ref().map(record_key)))
            .bind(("entitlements", new_token.entitlements.clone()))
            .bind(("expires_at", new_token.expires_at))
            .bind(("is_active", new_token.is_active))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(id)
    }

    pub async fn update(db: &Surreal<Any>, token: ApiToken) -> Result<(), RepoError> {
        db.query(
            "UPDATE type::record('api_tokens', $id) SET
                description = $description,
                entitlements = $entitlements,
                expires_at = $expires_at,
                is_active = $is_active,
                updated_at = time::now()",
        )
        .bind(("id", token.id.clone()))
        .bind(("description", token.description.clone()))
        .bind(("entitlements", token.entitlements.clone()))
        .bind(("expires_at", token.expires_at))
        .bind(("is_active", token.is_active))
        .await
        .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn revoke(db: &Surreal<Any>, token_id: impl ToString) -> Result<(), RepoError> {
        db.query("UPDATE type::record('api_tokens', $id) SET is_active = false, updated_at = time::now()")
            .bind(("id", record_key(token_id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn delete(db: &Surreal<Any>, token_id: impl ToString) -> Result<(), RepoError> {
        db.query("DELETE type::record('api_tokens', $id)")
            .bind(("id", record_key(token_id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn get_all(db: &Surreal<Any>) -> Result<Vec<ApiToken>, RepoError> {
        let mut result = db
            .query("SELECT *, meta::id(id) as id, IF product_id != NONE THEN meta::id(product_id) ELSE NONE END as product_id, IF user_id != NONE THEN meta::id(user_id) ELSE NONE END as user_id FROM api_tokens")
            .await
            .map_err(handle_surreal_error)?;
        crate::take_many(&mut result, 0)
    }
}
