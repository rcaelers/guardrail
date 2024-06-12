use crate::classes::ClassesPreset;
use crate::data::{version_count, version_list, QueryParams};
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
use std::collections::VecDeque;
use std::ops::Range;
use uuid::Uuid;

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
