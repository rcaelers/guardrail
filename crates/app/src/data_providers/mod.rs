pub mod crash;
pub mod user;

use leptos::prelude::*;

pub trait ExtraTableDataProvider<T> {
    fn refresh_table(&self);
    fn get_filter_signal(&self) -> RwSignal<String>;
}

#[macro_export]
macro_rules! table_data_provider_impl {
    ($provider:ty) => {
        impl TableDataProvider<<Self as DataTableTrait>::RowType> for $provider {
            async fn get_rows(
                &self,
                range: Range<usize>,
            ) -> Result<(Vec<<Self as DataTableTrait>::RowType>, Range<usize>), String> {
                let data = <Self as DataTableTrait>::list(
                    self.parents.clone(),
                    QueryParams {
                        filter: Some(self.filter.get_untracked().trim().to_string()),
                        sorting: self.sort.clone(),
                        range: Some(range.clone()),
                    },
                )
                .await
                .map_err(|e| format!("{e:?}"))?
                .into_iter()
                .map(|data| data.into())
                .collect::<Vec<<Self as DataTableTrait>::RowType>>();

                let len = data.len();
                Ok((data, range.start..range.start + len))
            }

            async fn row_count(&self) -> Option<usize> {
                <Self as DataTableTrait>::count(self.parents.clone())
                    .await
                    .ok()
                    .map(|c| c as usize)
            }

            fn set_sorting(&mut self, sorting: &VecDeque<(usize, ColumnSort)>) {
                self.sort = sorting
                    .iter()
                    .map(|(col, sort)| {
                        let columns = <Self as DataTableTrait>::get_columns();
                        let field = if *col < columns.len() {
                            columns[*col].clone()
                        } else {
                            "id".to_string()
                        };
                        let order = match sort {
                            ColumnSort::Ascending => common::SortOrder::Ascending,
                            ColumnSort::Descending => common::SortOrder::Descending,
                            _ => common::SortOrder::Ascending,
                        };
                        (field, order)
                    })
                    .collect();
            }

            fn track(&self) {
                self.filter.track();
                self.update.track();
            }
        }

        impl ExtraTableDataProvider<<Self as DataTableTrait>::RowType> for $provider {
            fn get_filter_signal(&self) -> RwSignal<String> {
                self.filter
            }

            fn refresh_table(&self) {
                self.update.set(self.update.get() + 1);
            }
        }
    };
}

#[cfg(feature = "ssr")]
mod ssr {
    use crate::authenticated_user;
    use leptos::prelude::*;
    use repos::Repo;

    pub async fn get_authenticated_transaction()
    -> Result<sqlx::Transaction<'static, sqlx::Postgres>, ServerFnError> {
        let repo = use_context::<Repo>()
            .ok_or(ServerFnError::new("No database connection".to_string()))?;

        let user = authenticated_user()
            .await?
            .ok_or(ServerFnError::new("No authenticated user".to_string()))?;
        let tx = repo.begin(&user.username).await?;
        Ok(tx)
    }
}

#[cfg(feature = "ssr")]
pub use ssr::*;
