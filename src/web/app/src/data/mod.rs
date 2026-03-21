pub mod crash;
pub mod product;
// pub mod symbols;
pub mod user;

#[cfg(feature = "ssr")]
mod ssr {
    use leptos::prelude::*;
    use repos::Repo;

    use crate::authenticated_user;

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

#[macro_export]
macro_rules! data_server_functions {
    ($datatype:ty, $table:expr,) => {
        paste! {
            #[server]
            pub async fn [< $table _get >](id: Uuid) -> Result<$datatype, ServerFnError> {
                let mut tx = super::get_authenticated_transaction().await?;

                let row = paste::item!([<$datatype Repo>]::get_by_id(&mut *tx, id))
                    .await?
                    .ok_or(ServerFnError::new(format!("{} not found", $table)))?;
                tx.commit().await?;
                Ok(row)
            }

            #[server]
            pub async fn [< $table _list >](query: QueryParams) -> Result<Vec<$datatype>, ServerFnError> {
                let mut tx = super::get_authenticated_transaction().await?;
                let rows = [< $datatype Repo >]::get_all(&mut *tx, query).await?;
                tx.commit().await?;
                Ok(rows)
            }

            #[server]
            pub async fn [< $table _add>](data: [< New $datatype>]) -> Result<(), ServerFnError> {
                let mut tx = super::get_authenticated_transaction().await?;
                [< $datatype Repo >]::create(
                    &mut *tx,
                    data
                )
                .await?;
                tx.commit().await?;
                Ok(())
            }

            #[server]
            pub async fn [< $table _update>](product: $datatype) -> Result<(), ServerFnError> {
                let mut tx = super::get_authenticated_transaction().await?;
                [< $datatype Repo >]::update(&mut *tx, product).await?;
                tx.commit().await?;
                Ok(())
            }

            #[server]
            pub async fn [< $table _remove>](id: Uuid) -> Result<(), ServerFnError> {
                let mut tx = super::get_authenticated_transaction().await?;
                [< $datatype Repo >]::remove(&mut *tx, id).await?;
                tx.commit().await?;
                Ok(())
            }

            #[server]
            pub async fn [< $table _count>]() -> Result<i64, ServerFnError> {
                let mut tx = super::get_authenticated_transaction().await?;
                let count = [< $datatype Repo >]::count(&mut *tx).await?;
                tx.commit().await?;
                Ok(count)
            }
        }
    };
}

#[macro_export]
macro_rules! data_server_name_functions {
    ($datatype:ty, $table:expr,) => {
        paste! {
            #[server]
            pub async fn [< $table _list_names >]() -> Result<HashSet<String>, ServerFnError> {
                let mut tx = super::get_authenticated_transaction().await?;
                let rows = [< $datatype Repo >]::get_all_names(&mut *tx).await?;
                tx.commit().await?;
                Ok(rows)
            }

            #[server]
            pub async fn [< $table _get_by_name >](name: String) -> Result<$datatype, ServerFnError> {
                let mut tx = super::get_authenticated_transaction().await?;
                let row = [< $datatype Repo >]::get_by_name(&mut *tx, name.as_str())
                    .await?
                    .ok_or(ServerFnError::new(format!("{} not found", $table)))?;
                tx.commit().await?;
                Ok(row)
            }
        }
    };
}
