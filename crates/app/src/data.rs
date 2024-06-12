#[cfg(feature = "ssr")]
use crate::entity;
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
#[cfg(feature = "ssr")]
use sea_orm::*;
#[cfg(feature = "ssr")]
use sea_orm::{DatabaseConnection, EntityTrait, FromQueryResult, PaginatorTrait};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use std::collections::HashMap;
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[cfg(feature = "ssr")]
#[derive(FromQueryResult, Debug, Default, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: Uuid,
    pub product: String,
    pub name: String,
    pub hash: String,
    pub tag: String,
    pub product_id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[cfg(not(feature = "ssr"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Version {
    pub id: Uuid,
    pub product: String,
    pub name: String,
    pub hash: String,
    pub tag: String,
    pub product_id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[cfg(feature = "ssr")]
impl ColumnType for entity::version::Column {
    fn name_column() -> Self {
        entity::version::Column::Name
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(entity::version::Column::Id),
            1 => Some(entity::version::Column::Name),
            2 => Some(entity::version::Column::Hash),
            3 => Some(entity::version::Column::Tag),
            4 => Some(entity::version::Column::ProductId),
            5 => Some(entity::version::Column::CreatedAt),
            6 => Some(entity::version::Column::UpdatedAt),

            _ => None,
        }
    }
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
            product: "".to_string(),
        }
    }
}

#[cfg(feature = "ssr")]
impl From<Version> for entity::version::ActiveModel {
    fn from(version: Version) -> Self {
        Self {
            id: Set(version.id),
            name: Set(version.name),
            hash: Set(version.hash),
            tag: Set(version.tag),
            product_id: Set(version.product_id),
            created_at: sea_orm::NotSet,
            updated_at: sea_orm::NotSet,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_login_at: Option<NaiveDateTime>,
    pub roles: Vec<String>,
}

#[cfg(feature = "ssr")]
impl ColumnType for entity::user::Column {
    fn name_column() -> Self {
        entity::user::Column::Username
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(entity::user::Column::Id),
            1 => Some(entity::user::Column::Username),
            2 => Some(entity::user::Column::CreatedAt),
            3 => Some(entity::user::Column::UpdatedAt),
            _ => None,
        }
    }
}

#[cfg(feature = "ssr")]
impl From<entity::user::Model> for User {
    fn from(model: entity::user::Model) -> Self {
        Self {
            id: model.id,
            username: model.username,
            created_at: model.created_at,
            updated_at: model.updated_at,
            last_login_at: model.last_authenticated,
            roles: vec![],
        }
    }
}

#[cfg(feature = "ssr")]
impl From<User> for entity::user::ActiveModel {
    fn from(user: User) -> Self {
        Self {
            id: Set(user.id),
            username: Set(user.username),
            created_at: sea_orm::NotSet,
            updated_at: sea_orm::NotSet,
            last_authenticated: sea_orm::NotSet,
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
async fn get_all_with<Type, E>(
    query_params: QueryParams,
    related_column: E::Column,
    related_id: Option<uuid::Uuid>,
) -> Result<Vec<Type>, ServerFnError<String>>
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

    if let Some(related_id) = related_id {
        query = query.filter(Condition::all().add(related_column.eq(related_id)))
    }

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
        Name,
    }

    let items: HashSet<String> = <E as EntityTrait>::find()
        .select_only()
        .column_as(E::Column::name_column(), QueryAs::Name)
        .into_values::<_, QueryAs>()
        .all(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?
        .into_iter()
        .collect();

    Ok(items)
}

#[cfg(feature = "ssr")]
async fn get_all_names_with<E>(
    related_column: E::Column,
    related_id: Option<uuid::Uuid>,
) -> Result<HashSet<String>, ServerFnError<String>>
where
    E: EntityTrait,
    E::Column: ColumnType,
{
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
    enum QueryAs {
        Name,
    }

    let mut query = <E as EntityTrait>::find();

    if let Some(related_id) = related_id {
        query = query.filter(Condition::all().add(related_column.eq(related_id)))
    }

    let items: HashSet<String> = query
        .select_only()
        .column_as(E::Column::name_column(), QueryAs::Name)
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

#[cfg(feature = "ssr")]
async fn count_with<'db, E>(
    related_column: E::Column,
    related_id: Option<uuid::Uuid>,
) -> Result<usize, ServerFnError<String>>
where
    E: EntityTrait,
    E::Model: Sync,
{
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    let mut query = <E as EntityTrait>::find();

    if let Some(related_id) = related_id {
        query = query.filter(Condition::all().add(related_column.eq(related_id)))
    }

    let count = PaginatorTrait::count(query, &db)
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
pub async fn product_remove(id: Uuid) -> Result<(), ServerFnError<String>> {
    delete_by_id::<entity::product::Entity>(id).await
}

#[server]
pub async fn product_count() -> Result<usize, ServerFnError<String>> {
    count::<entity::product::Entity>().await
}

#[server]
pub async fn version_get(id: Uuid) -> Result<Version, ServerFnError<String>> {
    get_by_id::<Version, entity::version::Entity>(id).await
}

#[server]
pub async fn version_list2(
    product_id: Option<Uuid>,
    query: QueryParams,
) -> Result<Vec<Version>, ServerFnError<String>> {
    let versions = get_all_with::<Version, entity::version::Entity>(
        query,
        entity::version::Column::ProductId,
        product_id,
    )
    .await?
    .into_iter()
    .map(|mut version| {
        version.product = "x".to_string();
        version
    })
    .collect::<Vec<Version>>();
    Ok(versions)
}

#[server]
pub async fn version_list(
    product_id: Option<Uuid>,
    query_params: QueryParams,
) -> Result<Vec<Version>, ServerFnError<String>> {
    let QueryParams {
        sorting,
        range,
        name,
    } = query_params;

    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    let mut query = <entity::version::Entity as EntityTrait>::find()
        .join(JoinType::LeftJoin, entity::version::Relation::Product.def())
        .column_as(entity::product::Column::Name, "product")
        .filter(<entity::version::Entity as EntityTrait>::Column::name_column().contains(name));

    if let Some(product_id) = product_id {
        query =
            query.filter(Condition::all().add(entity::version::Column::ProductId.eq(product_id)))
    }

    for (col, col_sort) in sorting {
        query = match col_sort {
            ColumnSort::Ascending => {
                match <entity::version::Entity as EntityTrait>::Column::from_index(col) {
                    Some(column) => query.order_by_asc(column),
                    None => query,
                }
            }
            ColumnSort::Descending => {
                match <entity::version::Entity as EntityTrait>::Column::from_index(col) {
                    Some(column) => query.order_by_desc(column),
                    None => query,
                }
            }
            ColumnSort::None => query,
        };
    }

    let items = query
        .limit(Some(range.len() as u64))
        .offset(range.start as u64)
        .into_model::<Version>()
        .all(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?
        .into_iter()
        //.map(|item| item.into())
        .collect();

    Ok(items)
}

#[server]
pub async fn version_list_names(
    product_id: Option<Uuid>,
) -> Result<HashSet<String>, ServerFnError<String>> {
    get_all_names_with::<entity::version::Entity>(entity::version::Column::ProductId, product_id)
        .await
}

#[server]
pub async fn version_add(version: Version) -> Result<(), ServerFnError<String>> {
    add::<Version, entity::version::Entity>(version).await
}

#[server]
pub async fn version_update(version: Version) -> Result<(), ServerFnError<String>> {
    update::<Version, entity::version::Entity>(version).await
}

#[server]
pub async fn version_remove(id: Uuid) -> Result<(), ServerFnError<String>> {
    delete_by_id::<entity::version::Entity>(id).await
}

#[server]
pub async fn version_count(product_id: Option<Uuid>) -> Result<usize, ServerFnError<String>> {
    count_with::<entity::version::Entity>(entity::version::Column::ProductId, product_id).await
}

#[server]
pub async fn user_get(id: Uuid) -> Result<User, ServerFnError<String>> {
    get_by_id::<User, entity::user::Entity>(id).await
}

#[server]
pub async fn user_list(query: QueryParams) -> Result<Vec<User>, ServerFnError<String>> {
    get_all::<User, entity::user::Entity>(query).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserWithRoles {
    user: User,
    roles: Vec<String>,
}

#[server]
async fn list_users_with_roles() -> Result<Vec<UserWithRoles>, ServerFnError<String>> {
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    let rows = entity::user::Entity::find()
        .left_join(entity::role::Entity)
        .select_also(entity::role::Entity)
        .all(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?;

    let mut user_map: HashMap<Uuid, UserWithRoles> = HashMap::new();

    for (user, role) in rows {
        let entry = user_map.entry(user.id).or_insert(UserWithRoles {
            user: user.into(),
            roles: Vec::new(),
        });

        if let Some(role) = role {
            entry.roles.push(role.name);
        }
    }

    Ok(user_map.into_values().collect())
}

#[server]
pub async fn user_list_names() -> Result<HashSet<String>, ServerFnError<String>> {
    get_all_names::<entity::user::Entity>().await
}

#[server]
pub async fn user_add(user: User) -> Result<(), ServerFnError<String>> {
    add::<User, entity::user::Entity>(user).await
}

#[server]
pub async fn user_update(user: User) -> Result<(), ServerFnError<String>> {
    update::<User, entity::user::Entity>(user).await
}

#[server]
pub async fn user_remove(id: Uuid) -> Result<(), ServerFnError<String>> {
    delete_by_id::<entity::user::Entity>(id).await
}

#[server]
pub async fn user_count() -> Result<usize, ServerFnError<String>> {
    count::<entity::user::Entity>().await
}
