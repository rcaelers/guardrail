use crate::classes::ClassesPreset;
#[cfg(feature = "ssr")]
use crate::entity;
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
#[cfg(feature = "ssr")]
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::ops::Range;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(TableRow, Debug, Clone)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct ProductRow {
    pub id: Uuid,
    #[table(renderer = "NameCellRenderer")]
    pub name: String,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
    #[table(renderer = "ActionsCellRenderer")]
    pub actions: (Uuid, String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductQuery {
    #[serde(default)]
    sorting: VecDeque<(usize, ColumnSort)>,
    range: Range<usize>,
    name: String,
}

#[derive(Clone, Debug)]
struct ProductContext {
    pub on_delete: fn(Uuid, String),
    pub on_related: fn(Uuid),
}

#[server]
pub async fn list_products(query: ProductQuery) -> Result<Vec<Product>, ServerFnError<String>> {
    let ProductQuery {
        sorting,
        range,
        name,
    } = query;

    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    let mut products_query =
        entity::product::Entity::find().filter(entity::product::Column::Name.contains(name));

    for (col, col_sort) in sorting {
        products_query = match col_sort {
            ColumnSort::Ascending => match col {
                0 => products_query.order_by_asc(entity::product::Column::Id),
                1 => products_query.order_by_asc(entity::product::Column::Name),
                2 => products_query.order_by_asc(entity::product::Column::CreatedAt),
                3 => products_query.order_by_asc(entity::product::Column::UpdatedAt),
                _ => products_query,
            },
            ColumnSort::Descending => match col {
                0 => products_query.order_by_desc(entity::product::Column::Id),
                1 => products_query.order_by_desc(entity::product::Column::Name),
                2 => products_query.order_by_desc(entity::product::Column::CreatedAt),
                3 => products_query.order_by_desc(entity::product::Column::UpdatedAt),
                _ => products_query,
            },
            ColumnSort::None => products_query,
        };
    }

    let products = products_query
        .limit(Some(range.len() as u64))
        .offset(range.start as u64)
        .all(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?
        .into_iter()
        .map(|product| Product {
            id: product.id,
            created_at: product.created_at,
            updated_at: product.updated_at,
            name: product.name,
        })
        .collect();
    Ok(products)
}

#[server]
pub async fn product_count() -> Result<usize, ServerFnError<String>> {
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;
    let count = entity::product::Entity::find()
        .count(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?;
    Ok(count as usize)
}

#[component]
fn NameCellRenderer<F>(
    class: String,
    #[prop(into)] value: MaybeSignal<String>,
    on_change: F,
    #[allow(unused)] index: usize,
) -> impl IntoView
where
    F: Fn(String) + 'static,
{
    view! {
        <td class=class>
            <input
                type="text"
                value=value
                on:change=move |evt| {
                    on_change(event_target_value(&evt));
                }
            />

        </td>
    }
}

#[component]
#[allow(unused_variables)]
fn ActionsCellRenderer<F>(
    class: String,
    #[prop(into)] value: MaybeSignal<(Uuid, String)>,
    on_change: F,
    index: usize,
) -> impl IntoView
where
    F: Fn((Uuid, String)) + 'static + Clone,
{
    let context = use_context::<ProductContext>().unwrap();
    let v = value.get_untracked();
    let id = v.0;
    let name = v.1;

    view! {
        <td class=class>
            <button
                class="px-4 py-2 bg-gray-300 text-gray-700 rounded"
                on:click=move |_| (context.on_related)(id)
            >
                "Edit"
            </button>
            <button
                class="px-4 py-2 bg-gray-300 text-gray-700 rounded"
                on:click=move |_| (context.on_delete)(id, name.clone())
            >
                "Delete"
            </button>
            <button
                class="px-4 py-2 bg-gray-300 text-gray-700 rounded"
                on:click=move |_| (context.on_related)(id)
            >
                "Show Versions"
            </button>
        </td>
    }
}

pub struct ProductTableDataProvider {
    sort: VecDeque<(usize, ColumnSort)>,
    pub name: RwSignal<String>,
}

impl ProductTableDataProvider {
    pub fn new() -> Self {
        let context = ProductContext {
            on_delete: |id, _| {
                info!("delete {:?}", id);
            },
            on_related: |id| {
                info!("related {:?}", id);
            },
        };
        provide_context(context.clone());

        Self {
            sort: VecDeque::new(),
            name: RwSignal::new("".to_string()),
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
        let products = list_products(ProductQuery {
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
            actions: (product.id, product.name),
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
    }
}
