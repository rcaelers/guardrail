use cfg_if::cfg_if;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::{collections::VecDeque, ops::Range};

cfg_if! { if #[cfg(feature="ssr")] {
    use crate::authenticated_user;
    use crate::auth::AuthenticatedUser;
    use std::str::FromStr;
    use sea_orm::*;
    use sea_orm::{DatabaseConnection, EntityTrait, PaginatorTrait};
    use sea_query::Expr;
    use leptos::*;
    use std::collections::{HashMap, HashSet};
    use uuid::Uuid;
    use crate::entity;
}}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryParams {
    #[serde(default)]
    pub sorting: VecDeque<(usize, ColumnSort)>,
    pub range: Range<usize>,
    pub filter: String,
}

#[cfg(feature = "ssr")]
pub trait EntityInfo
where
    Self: EntityTrait,
    <Self::Column as FromStr>::Err: std::fmt::Debug,
{
    type View: FromQueryResult + Debug;

    fn filter_column() -> Self::Column;
    fn index_to_column(index: usize) -> Option<Self::Column>;
    fn extend_query_for_view(query: Select<Self>) -> Select<Self> {
        query
    }

    fn get_product_query(
        _user: &AuthenticatedUser,
        _data: &Self::View,
    ) -> Option<Select<entity::product::Entity>> {
        None
    }

    fn extend_query_for_access(
        query: Select<Self>,
        user: AuthenticatedUser,
        _roles: Vec<String>,
    ) -> Select<Self> {
        if user.is_admin {
            return query;
        }
        query
            .join(
                JoinType::InnerJoin,
                entity::product::Entity::belongs_to(entity::role::Entity)
                    .from(entity::product::Column::Id)
                    .to(entity::role::Column::ProductId)
                    .into(),
            )
            .join(
                JoinType::InnerJoin,
                entity::role::Entity::belongs_to(entity::user::Entity)
                    .from(entity::role::Column::UserId)
                    .to(entity::user::Column::Id)
                    .into(),
            )
            .filter(Expr::col((entity::user::Entity, entity::user::Column::Id)).eq(user.id))
    }

    fn id_to_column(_id_name: String) -> Option<Self::Column> {
        None
    }
}

#[cfg(feature = "ssr")]
pub async fn check_access_by_id<E>(
    id: uuid::Uuid,
    roles: Vec<String>,
) -> Result<bool, ServerFnError>
where
    E: EntityTrait + EntityInfo,
    <E::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType: From<uuid::Uuid>,
    <E::Column as FromStr>::Err: std::fmt::Debug,
{
    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    let user = authenticated_user()
        .await?
        .ok_or(ServerFnError::new("No authenticated user".to_string()))?;
    if user.is_admin {
        return Ok(true);
    }

    let mut query = <E as EntityTrait>::find_by_id(id);
    query = <E as EntityInfo>::extend_query_for_view(query);
    query = <E as EntityInfo>::extend_query_for_access(query, user, roles);

    query
        .one(&db)
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?
        .ok_or(ServerFnError::new("no access".to_string()))?;

    Ok(true)
}

#[cfg(feature = "ssr")]
pub async fn check_access_by_data<E>(
    data: &E::View,
    roles: Vec<String>,
) -> Result<bool, ServerFnError>
where
    E: EntityTrait + EntityInfo,
    <E::Column as FromStr>::Err: std::fmt::Debug,
{
    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    let user = authenticated_user()
        .await?
        .ok_or(ServerFnError::new("No authenticated user".to_string()))?;
    if user.is_admin {
        return Ok(true);
    }

    let query = <E as EntityInfo>::get_product_query(&user, data);

    if let Some(query) = query {
        let query = entity::product::Entity::extend_query_for_access(query, user, roles);

        query
            .one(&db)
            .await
            .map_err(|e| ServerFnError::new(format!("{e:?}")))?
            .ok_or(ServerFnError::new("no access".to_string()))?;
    } else {
        return Ok(user.is_admin);
    }

    Ok(true)
}

#[cfg(feature = "ssr")]
pub async fn get_by_id<E>(id: uuid::Uuid) -> Result<E::View, ServerFnError>
where
    E: EntityTrait + EntityInfo,
    <E::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType: From<uuid::Uuid>,
    E::View: From<E::Model>,
    <E::Column as FromStr>::Err: std::fmt::Debug,
{
    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    let user = authenticated_user()
        .await?
        .ok_or(ServerFnError::new("No authenticated user".to_string()))?;

    let mut query = <E as EntityTrait>::find_by_id(id);
    query = <E as EntityInfo>::extend_query_for_view(query);
    query = <E as EntityInfo>::extend_query_for_access(query, user, vec![]);

    let items = query
        .into_model::<<E as EntityInfo>::View>()
        .one(&db)
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?
        .ok_or(ServerFnError::new("not found".to_string()))?;

    Ok(items)
}

#[cfg(feature = "ssr")]
pub async fn get_all<E>(
    query_params: QueryParams,
    parents: HashMap<String, Uuid>,
) -> Result<Vec<E::View>, ServerFnError>
where
    E: EntityTrait + EntityInfo,
    <E::Column as FromStr>::Err: std::fmt::Debug,
{
    let QueryParams {
        sorting,
        range,
        filter,
    } = query_params;

    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    let user = authenticated_user()
        .await?
        .ok_or(ServerFnError::new("No authenticated user".to_string()))?;

    let mut query = <E as EntityTrait>::find();

    query = <E as EntityInfo>::extend_query_for_view(query);
    query = <E as EntityInfo>::extend_query_for_access(query, user, vec![]);

    if !filter.is_empty() {
        query = query.filter(E::filter_column().contains(filter));
    }

    for (parent, parent_id) in parents {
        match <E as EntityInfo>::id_to_column(parent) {
            Some(column) => {
                query = query.filter(Condition::all().add(column.eq(parent_id)));
            }
            None => {
                return Err(ServerFnError::ServerError(
                    "Invalid parent column".to_string(),
                ))
            }
        };
    }

    for (col, col_sort) in sorting {
        query = match col_sort {
            ColumnSort::Ascending => match E::index_to_column(col) {
                Some(column) => query.order_by_asc(column),
                None => query,
            },
            ColumnSort::Descending => match E::index_to_column(col) {
                Some(column) => query.order_by_desc(column),
                None => query,
            },
            ColumnSort::None => query,
        };
    }

    let items = query
        .limit(Some(range.len() as u64))
        .offset(range.start as u64)
        .into_model::<<E as EntityInfo>::View>()
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?
        .into_iter()
        .collect();

    Ok(items)
}

#[cfg(feature = "ssr")]
pub async fn get_all_names<E>(
    parents: HashMap<String, Uuid>,
) -> Result<HashSet<String>, ServerFnError>
where
    E: EntityTrait + EntityInfo,
    <E::Column as FromStr>::Err: std::fmt::Debug,
{
    use std::collections::HashSet;

    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    let user = authenticated_user()
        .await?
        .ok_or(ServerFnError::new("No authenticated user".to_string()))?;

    #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
    enum QueryAs {
        Name,
    }

    let mut query = <E as EntityTrait>::find();
    query = <E as EntityInfo>::extend_query_for_view(query);
    query = <E as EntityInfo>::extend_query_for_access(query, user, vec![]);

    for (parent, parent_id) in parents {
        match <E as EntityInfo>::id_to_column(parent) {
            Some(column) => {
                query = query.filter(Condition::all().add(column.eq(parent_id)));
            }
            None => {
                return Err(ServerFnError::ServerError(
                    "Invalid parent column".to_string(),
                ))
            }
        };
    }

    let items: HashSet<String> = query
        .select_only()
        .column_as(E::filter_column(), QueryAs::Name)
        .into_values::<_, QueryAs>()
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?
        .into_iter()
        .collect();

    Ok(items)
}

#[cfg(feature = "ssr")]
pub async fn add<E>(item: E::View) -> Result<(), ServerFnError>
where
    E: EntityTrait + EntityInfo,
    E::ActiveModel: ActiveModelTrait<Entity = E> + ActiveModelBehavior + Send,
    E::View: Into<E::ActiveModel>,
    <E as EntityTrait>::Model: IntoActiveModel<<E as EntityTrait>::ActiveModel>,
    <E::Column as FromStr>::Err: std::fmt::Debug,
{
    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    check_access_by_data::<E>(&item, vec!["admin".to_string()])
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?;

    let am: E::ActiveModel = item.into();
    am.insert(&db)
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?;
    Ok(())
}

#[cfg(feature = "ssr")]
pub async fn update<E>(item: E::View) -> Result<(), ServerFnError>
where
    E: EntityTrait + EntityInfo,
    E::ActiveModel: ActiveModelTrait<Entity = E> + ActiveModelBehavior + Send,
    E::View: Into<E::ActiveModel>,
    <E as EntityTrait>::Model: IntoActiveModel<<E as EntityTrait>::ActiveModel>,
    <E::Column as FromStr>::Err: std::fmt::Debug,
{
    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    check_access_by_data::<E>(&item, vec!["admin".to_string()])
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?;

    let am: E::ActiveModel = item.into();
    am.update(&db)
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?;
    Ok(())
}

#[cfg(feature = "ssr")]
pub async fn delete_by_id<E>(id: uuid::Uuid) -> Result<(), ServerFnError>
where
    E: EntityTrait + EntityInfo,
    <<E as sea_orm::EntityTrait>::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType:
        From<uuid::Uuid>,
    <E::Column as FromStr>::Err: std::fmt::Debug,
{
    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    check_access_by_id::<E>(id, vec!["admin".to_string()])
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?;

    <E as EntityTrait>::delete_by_id(id)
        .exec(&db)
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?;
    Ok(())
}

#[cfg(feature = "ssr")]
pub async fn count<'db, E>(parents: HashMap<String, Uuid>) -> Result<usize, ServerFnError>
where
    E: EntityTrait + EntityInfo,
    E::Model: Sync,
    <E::Column as FromStr>::Err: std::fmt::Debug,
{
    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    let user = authenticated_user().await?;
    if user.is_none() {
        return Ok(0);
    }
    let user = user.unwrap();

    let mut query = <E as EntityTrait>::find();
    query = <E as EntityInfo>::extend_query_for_access(query, user, vec![]);

    for (parent, parent_id) in parents {
        match <E as EntityInfo>::id_to_column(parent) {
            Some(column) => {
                query = query.filter(Condition::all().add(column.eq(parent_id)));
            }
            None => return Err(ServerFnError::new("Invalid parent column".to_string())),
        };
    }

    let count = PaginatorTrait::count(query, &db)
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?;

    Ok(count as usize)
}
