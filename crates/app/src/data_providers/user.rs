use crate::classes::ClassesPreset;
use crate::data::{user_count, user_list, QueryParams};
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
use std::collections::VecDeque;
use std::ops::Range;
use uuid::Uuid;

#[derive(TableRow, Debug, Clone)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct UserTableDataProvider {
    sort: VecDeque<(usize, ColumnSort)>,
    name: RwSignal<String>,
    update: RwSignal<u64>,
}

impl UserTableDataProvider {
    pub fn new() -> Self {
        Self {
            sort: VecDeque::new(),
            name: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
        }
    }

    pub fn get_name_signal(&self) -> RwSignal<String> {
        self.name
    }

    pub fn update(&self) {
        self.update.set(self.update.get() + 1);
    }
}

impl Default for UserTableDataProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TableDataProvider<UserRow> for UserTableDataProvider {
    async fn get_rows(&self, range: Range<usize>) -> Result<(Vec<UserRow>, Range<usize>), String> {
        let users = user_list(QueryParams {
            name: self.name.get_untracked().trim().to_string(),
            sorting: self.sort.clone(),
            range: range.clone(),
        })
        .await
        .map_err(|e| format!("{e:?}"))?
        .into_iter()
        .map(|user| UserRow {
            id: user.id,
            username: user.username.clone(),
            created_at: user.created_at,
            updated_at: user.updated_at,
        })
        .collect::<Vec<UserRow>>();

        let len = users.len();
        Ok((users, range.start..range.start + len))
    }

    async fn row_count(&self) -> Option<usize> {
        user_count().await.ok()
    }

    fn set_sorting(&mut self, sorting: &VecDeque<(usize, ColumnSort)>) {
        self.sort = sorting.clone();
    }

    fn track(&self) {
        self.name.track();
        self.update.track();
    }
}
