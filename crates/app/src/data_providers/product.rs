use ::chrono::NaiveDateTime;
use cfg_if::cfg_if;
use leptos::prelude::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

cfg_if! { if #[cfg(feature="ssr")] {
    use sea_orm::*;
    use sea_query::Expr;
    use std::collections::HashMap;
    use crate::authenticated_user;
    use entities::entity;
    use crate::auth::AuthenticatedUser;
    use crate::data::{
        add, count, delete_by_id, get_all, get_all_names, get_by_id, update, EntityInfo,
    };
}}

use super::ExtraRowTrait;
use crate::classes::ClassesPreset;
use crate::data::QueryParams;

#[derive(TableRow, Clone, Debug)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct ProductRow {
    pub id: Uuid,
    pub name: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
}

#[cfg(not(feature = "ssr"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Default, Serialize, Deserialize, FromQueryResult)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[cfg(feature = "ssr")]
impl EntityInfo for entity::product::Entity {
    type View = Product;

    fn filter_column() -> Self::Column {
        entity::product::Column::Name
    }

    fn index_to_column(index: usize) -> Option<Self::Column> {
        match index {
            0 => Some(entity::product::Column::Id),
            1 => Some(entity::product::Column::Name),
            2 => Some(entity::product::Column::CreatedAt),
            3 => Some(entity::product::Column::UpdatedAt),
            _ => None,
        }
    }

    fn get_product_query(
        _user: &AuthenticatedUser,
        data: &Self::View,
    ) -> Option<Select<entity::product::Entity>> {
        let query = entity::product::Entity::find_by_id(data.id);
        Some(query)
    }
}

impl From<Product> for ProductRow {
    fn from(product: Product) -> Self {
        Self {
            id: product.id,
            name: product.name,
            created_at: product.created_at,
            updated_at: product.updated_at,
        }
    }
}
#[cfg(feature = "ssr")]
impl From<entity::product::Model> for Product {
    fn from(model: entity::product::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[cfg(feature = "ssr")]
impl crate::data::MyIntoActiveModel<entities::entity::product::ActiveModel> for Product {
    fn into_active_model(self) -> entities::entity::product::ActiveModel {
        entities::entity::product::ActiveModel {
            id: Set(self.id),
            name: Set(self.name),
            created_at: sea_orm::NotSet,
            updated_at: sea_orm::NotSet,
        }
    }
}

impl ExtraRowTrait for ProductRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}

#[server]
pub async fn product_get(id: Uuid) -> Result<Product, ServerFnError> {
    get_by_id::<entity::product::Entity>(id).await
}

#[server]
pub async fn product_list(query: QueryParams) -> Result<Vec<Product>, ServerFnError> {
    get_all::<entity::product::Entity>(query, HashMap::new()).await
}

#[server]
pub async fn product_list_names() -> Result<HashSet<String>, ServerFnError> {
    get_all_names::<entity::product::Entity>(HashMap::new()).await
}

#[server]
pub async fn product_add(product: Product) -> Result<(), ServerFnError> {
    add::<entity::product::Entity>(product).await
}

#[server]
pub async fn product_update(product: Product) -> Result<(), ServerFnError> {
    update::<entity::product::Entity>(product).await
}

#[server]
pub async fn product_remove(id: Uuid) -> Result<(), ServerFnError> {
    delete_by_id::<entity::product::Entity>(id).await
}

#[server]
pub async fn product_count() -> Result<usize, ServerFnError> {
    count::<entity::product::Entity>(HashMap::new()).await
}

#[server]
pub async fn product_get_by_name(name: String) -> Result<Product, ServerFnError> {
    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    let user = authenticated_user()
        .await?
        .ok_or(ServerFnError::new("No authenticated user".to_string()))?;

    let mut query = entity::product::Entity::find();
    query = entity::product::Entity::extend_query_for_view(query);
    query = entity::product::Entity::extend_query_for_access(query, user, vec![]);
    query =
        query.filter(Expr::col((entity::product::Entity, entity::product::Column::Name)).eq(name));

    let items = query
        .into_model::<Product>()
        .one(&db)
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?
        .ok_or(ServerFnError::new("not found".to_string()))?;

    Ok(items)
}
