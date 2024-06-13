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

#[cfg(feature = "ssr")]
pub trait ColumnInfo: ColumnTrait {
    fn name_column() -> Self;
    fn from_index(index: usize) -> Option<Self>
    where
        Self: Sized;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryParams {
    #[serde(default)]
    pub sorting: VecDeque<(usize, ColumnSort)>,
    pub range: Range<usize>,
    pub name: String,
}

#[cfg(feature = "ssr")]
pub async fn get_by_id<Type, E>(id: uuid::Uuid) -> Result<Type, ServerFnError<String>>
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
pub async fn get_all<Type, E>(query_params: QueryParams) -> Result<Vec<Type>, ServerFnError<String>>
where
    E: EntityTrait,
    Type: From<E::Model>,
    E::Column: ColumnInfo,
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
pub async fn get_all_with<Type, E>(
    query_params: QueryParams,
    related_column: E::Column,
    related_id: Option<uuid::Uuid>,
) -> Result<Vec<Type>, ServerFnError<String>>
where
    E: EntityTrait,
    Type: From<E::Model>,
    E::Column: ColumnInfo,
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
pub async fn get_all_names<E>() -> Result<HashSet<String>, ServerFnError<String>>
where
    E: EntityTrait,
    E::Column: ColumnInfo,
{
    use std::collections::HashSet;

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
pub async fn get_all_names_with<E>(
    related_column: E::Column,
    related_id: Option<uuid::Uuid>,
) -> Result<HashSet<String>, ServerFnError<String>>
where
    E: EntityTrait,
    E::Column: ColumnInfo,
{
    use std::collections::HashSet;

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
pub async fn add<Type, E>(item: Type) -> Result<(), ServerFnError<String>>
where
    E: EntityTrait,
    E::Column: ColumnInfo,
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
pub async fn update<Type, E>(item: Type) -> Result<(), ServerFnError<String>>
where
    E: EntityTrait,
    E::Column: ColumnInfo,
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
pub async fn delete_by_id<E>(id: uuid::Uuid) -> Result<(), ServerFnError<String>>
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
pub async fn count<'db, E>() -> Result<usize, ServerFnError<String>>
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
pub async fn count_with<'db, E>(
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
