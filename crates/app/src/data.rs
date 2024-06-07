#[cfg(feature = "ssr")]
use crate::entity;
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
#[cfg(feature = "ssr")]
use sea_orm::*;
#[cfg(feature = "ssr")]
use sea_orm::{DatabaseConnection, EntityTrait, PaginatorTrait};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    ops::Range,
};
use uuid::Uuid;

#[cfg(feature = "ssr")]
trait ColumnType: ColumnTrait {
    fn name_column() -> Self;
    fn from_index(index: usize) -> Option<Self>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[cfg(feature = "ssr")]
impl ColumnType for entity::product::Column {
    fn name_column() -> Self {
        entity::product::Column::Name
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(entity::product::Column::Id),
            1 => Some(entity::product::Column::Name),
            2 => Some(entity::product::Column::CreatedAt),
            3 => Some(entity::product::Column::UpdatedAt),
            _ => None,
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
impl From<Product> for entity::product::ActiveModel {
    fn from(product: Product) -> Self {
        Self {
            id: Set(product.id),
            name: Set(product.name),
            created_at: sea_orm::NotSet,
            updated_at: sea_orm::NotSet,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub name: String,
    pub hash: String,
    pub tag: String,
    pub product_id: Uuid,
}

#[cfg(feature = "ssr")]
impl From<entity::version::Model> for Version {
    fn from(model: entity::version::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            hash: model.hash,
            tag: model.tag,
            product_id: model.product_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryParams {
    #[serde(default)]
    pub sorting: VecDeque<(usize, ColumnSort)>,
    pub range: Range<usize>,
    pub name: String,
}

#[cfg(feature = "ssr")]
async fn get_by_id<Type, E>(id: uuid::Uuid) -> Result<Type, ServerFnError<String>>
where
    E: EntityTrait,
    <E::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType: From<uuid::Uuid>,
    Type: From<E::Model>,
{
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    let items = <E as EntityTrait>::find_by_id(id)
        .one(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?
        .ok_or(ServerFnError::WrappedServerError("not found".to_string()))?;

    Ok(items.into())
}

#[cfg(feature = "ssr")]
async fn get_all<Type, E>(query_params: QueryParams) -> Result<Vec<Type>, ServerFnError<String>>
where
    E: EntityTrait,
    Type: From<E::Model>,
    E::Column: ColumnType,
{
    let QueryParams {
        sorting,
        range,
        name,
    } = query_params;

    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    let mut query = <E as EntityTrait>::find().filter(E::Column::name_column().contains(name));

    for (col, col_sort) in sorting {
        query = match col_sort {
            ColumnSort::Ascending => match E::Column::from_index(col) {
                Some(column) => query.order_by_asc(column),
                None => query,
            },
            ColumnSort::Descending => match E::Column::from_index(col) {
                Some(column) => query.order_by_desc(column),
                None => query,
            },
            ColumnSort::None => query,
        };
    }

    let items = query
        .limit(Some(range.len() as u64))
        .offset(range.start as u64)
        .all(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?
        .into_iter()
        .map(|item| item.into())
        .collect();
    Ok(items)
}

#[cfg(feature = "ssr")]
async fn get_all_names<E>() -> Result<HashSet<String>, ServerFnError<String>>
where
    E: EntityTrait,
    E::Column: ColumnType,
{
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
    enum QueryAs {
        ProductName,
    }

    let items: HashSet<String> = <E as EntityTrait>::find()
        .select_only()
        .column_as(entity::product::Column::Name, QueryAs::ProductName)
        .into_values::<_, QueryAs>()
        .all(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?
        .into_iter()
        .collect();

    Ok(items)
}

#[cfg(feature = "ssr")]
async fn add<Type, E>(item: Type) -> Result<(), ServerFnError<String>>
where
    E: EntityTrait,
    E::Column: ColumnType,
    E::ActiveModel: ActiveModelTrait<Entity = E> + ActiveModelBehavior + Send,
    Type: Into<E::ActiveModel>,
    <E as EntityTrait>::Model: IntoActiveModel<<E as EntityTrait>::ActiveModel>,
{
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    let am: E::ActiveModel = item.into();
    am.insert(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?;
    Ok(())
}

#[cfg(feature = "ssr")]
async fn update<Type, E>(item: Type) -> Result<(), ServerFnError<String>>
where
    E: EntityTrait,
    E::Column: ColumnType,
    E::ActiveModel: ActiveModelTrait<Entity = E> + ActiveModelBehavior + Send,
    Type: Into<E::ActiveModel>,
    <E as EntityTrait>::Model: IntoActiveModel<<E as EntityTrait>::ActiveModel>,
{
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    let am: E::ActiveModel = item.into();
    am.update(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?;
    Ok(())
}

#[cfg(feature = "ssr")]
async fn delete_by_id<E>(id: uuid::Uuid) -> Result<(), ServerFnError<String>>
where
    E: EntityTrait,
    <<E as sea_orm::EntityTrait>::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType:
        From<uuid::Uuid>,
{
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    <E as EntityTrait>::delete_by_id(id)
        .exec(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?;
    Ok(())
}

#[cfg(feature = "ssr")]
async fn count<'db, E>() -> Result<usize, ServerFnError<String>>
where
    E: EntityTrait,
    E::Model: Sync,
{
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    let select = <E as EntityTrait>::find();
    let count = PaginatorTrait::count(select, &db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?;

    Ok(count as usize)
}

#[server]
pub async fn product_get(id: Uuid) -> Result<Product, ServerFnError<String>> {
    get_by_id::<Product, entity::product::Entity>(id).await
}

#[server]
pub async fn product_list(query: QueryParams) -> Result<Vec<Product>, ServerFnError<String>> {
    get_all::<Product, entity::product::Entity>(query).await
}
#[server]
pub async fn product_list_names() -> Result<HashSet<String>, ServerFnError<String>> {
    get_all_names::<entity::product::Entity>().await
}

#[server]
pub async fn product_add(product: Product) -> Result<(), ServerFnError<String>> {
    add::<Product, entity::product::Entity>(product).await
}

#[server]
pub async fn product_update(product: Product) -> Result<(), ServerFnError<String>> {
    update::<Product, entity::product::Entity>(product).await
}

#[server]
pub async fn product_remove(product: Product) -> Result<(), ServerFnError<String>> {
    delete_by_id::<entity::product::Entity>(product.id).await
}

#[server]
pub async fn product_count() -> Result<usize, ServerFnError<String>> {
    count::<entity::product::Entity>().await
}
