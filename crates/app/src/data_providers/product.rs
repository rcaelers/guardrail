use crate::classes::ClassesPreset;
use crate::data::{product_count, product_list, QueryParams};
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
use std::collections::VecDeque;
use std::ops::Range;
use uuid::Uuid;

use super::{ExtraRowTrait, ExtraTableDataProvider};

#[derive(TableRow, Debug, Clone)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct ProductRow {
    pub id: Uuid,
    pub name: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
}

impl ExtraRowTrait for ProductRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}
#[derive(Debug, Clone)]
pub struct ProductTableDataProvider {
    sort: VecDeque<(usize, ColumnSort)>,
    name: RwSignal<String>,
    update: RwSignal<u64>,
}

impl ExtraTableDataProvider<ProductRow> for ProductTableDataProvider {
    fn get_filter_signal(&self) -> RwSignal<String> {
        self.name
    }

    fn update(&self) {
        self.update.set(self.update.get() + 1);
    }
}

impl ProductTableDataProvider {
    pub fn new() -> Self {
        Self {
            sort: VecDeque::new(),
            name: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
        }
    }
}

impl Default for ProductTableDataProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TableDataProvider<ProductRow> for ProductTableDataProvider {
    async fn get_rows(
        &self,
        range: Range<usize>,
    ) -> Result<(Vec<ProductRow>, Range<usize>), String> {
        let products = product_list(QueryParams {
            name: self.name.get_untracked().trim().to_string(),
            sorting: self.sort.clone(),
            range: range.clone(),
        })
        .await
        .map_err(|e| format!("{e:?}"))?
        .into_iter()
        .map(|product| ProductRow {
            id: product.id,
            created_at: product.created_at,
            updated_at: product.updated_at,
            name: product.name.clone(),
        })
        .collect::<Vec<ProductRow>>();

        let len = products.len();
        Ok((products, range.start..range.start + len))
    }

    async fn row_count(&self) -> Option<usize> {
        product_count().await.ok()
    }

    fn set_sorting(&mut self, sorting: &VecDeque<(usize, ColumnSort)>) {
        self.sort = sorting.clone();
    }

    fn track(&self) {
        self.name.track();
        self.update.track();
    }
}
