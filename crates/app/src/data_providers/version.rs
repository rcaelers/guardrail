use crate::classes::ClassesPreset;
use crate::data::QueryParams;
#[cfg(feature = "ssr")]
use crate::data::{
    add, count_with, delete_by_id, get_all_names_with, get_all_with, get_by_id, update, ColumnInfo,
};
#[cfg(feature = "ssr")]
use crate::entity;
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::ops::Range;
use uuid::Uuid;

#[cfg(feature = "ssr")]
use sea_orm::*;

use super::{ExtraRowTrait, ExtraTableDataProvider};

#[derive(TableRow, Debug, Clone)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct VersionRow {
    pub id: Uuid,
    pub product: String,
    pub name: String,
    pub hash: String,
    pub tag: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
    #[table(skip)]
    pub product_id: Option<Uuid>,
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
impl ColumnInfo for entity::version::Column {
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

impl ExtraRowTrait for VersionRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, Clone)]
pub struct VersionTableDataProvider {
    sort: VecDeque<(usize, ColumnSort)>,
    name: RwSignal<String>,
    update: RwSignal<u64>,
    product_id: Option<Uuid>,
}

impl VersionTableDataProvider {
    pub fn new(product_id: Option<Uuid>) -> Self {
        Self {
            sort: VecDeque::new(),
            name: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
            product_id,
        }
    }
}

impl ExtraTableDataProvider<VersionRow> for VersionTableDataProvider {
    fn get_filter_signal(&self) -> RwSignal<String> {
        self.name
    }

    fn update(&self) {
        self.update.set(self.update.get() + 1);
    }
}

impl TableDataProvider<VersionRow> for VersionTableDataProvider {
    async fn get_rows(
        &self,
        range: Range<usize>,
    ) -> Result<(Vec<VersionRow>, Range<usize>), String> {
        let versions = version_list(
            self.product_id,
            QueryParams {
                name: self.name.get_untracked().trim().to_string(),
                sorting: self.sort.clone(),
                range: range.clone(),
            },
        )
        .await
        .map_err(|e| format!("{e:?}"))?
        .into_iter()
        .map(|version| VersionRow {
            id: version.id,
            product_id: Some(version.product_id),
            product: version.product.clone(),
            hash: version.hash.clone(),
            tag: version.tag.clone(),
            created_at: version.created_at,
            updated_at: version.updated_at,
            name: version.name.clone(),
        })
        .collect::<Vec<VersionRow>>();

        let len = versions.len();
        Ok((versions, range.start..range.start + len))
    }

    async fn row_count(&self) -> Option<usize> {
        version_count(self.product_id).await.ok()
    }

    fn set_sorting(&mut self, sorting: &VecDeque<(usize, ColumnSort)>) {
        self.sort = sorting.clone();
    }

    fn track(&self) {
        self.name.track();
        self.update.track();
    }
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
