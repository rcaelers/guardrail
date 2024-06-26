pub mod crash;
pub mod product;
pub mod symbols;
pub mod user;
pub mod version;

use leptos::*;
use uuid::Uuid;

pub trait ExtraTableDataProvider<T> {
    fn refresh_table(&self);
    fn get_filter_signal(&self) -> RwSignal<String>;
}

pub trait ExtraRowTrait {
    fn get_id(&self) -> Uuid;
    fn get_name(&self) -> String;
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
                        filter: self.filter.get_untracked().trim().to_string(),
                        sorting: self.sort.clone(),
                        range: range.clone(),
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
            }

            fn set_sorting(&mut self, sorting: &VecDeque<(usize, ColumnSort)>) {
                self.sort = sorting.clone();
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
