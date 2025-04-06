use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Annotation {
    pub id: uuid::Uuid,
    pub key: String,
    pub kind: String,
    pub value: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub crash_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct NewAnnotation {
    pub key: String,
    pub kind: String,
    pub value: String,
    pub crash_id: uuid::Uuid,
    pub product_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AnnotationKind {
    System,
    User,
}

impl AnnotationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnnotationKind::System => "system",
            AnnotationKind::User => "user",
        }
    }
}

impl TryFrom<&str> for AnnotationKind {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "system" => Ok(AnnotationKind::System),
            "user" => Ok(AnnotationKind::User),
            _ => Err(format!("Invalid annotation kind: {}", s)),
        }
    }
}

#[cfg(feature = "ssr")]
pub mod ssr {
    use super::{Annotation, NewAnnotation};
    use crate::{QueryParams, Repo, error::RepoError};
    use sqlx::{Postgres, QueryBuilder};
    use tracing::error;

    pub struct AnnotationRepo {}

    impl AnnotationRepo {
        pub async fn get_by_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<Option<Annotation>, RepoError> {
            sqlx::query_as!(
                Annotation,
                r#"
                SELECT *
                FROM guardrail.annotations
                WHERE guardrail.annotations.id = $1
            "#,
                id
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                error!("Failed to retrieve annotation {id}: {err}");
                RepoError::DatabaseError("Failed to retrieve annotation".to_string())
            })
        }

        pub async fn get_all(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            params: QueryParams,
        ) -> Result<Vec<Annotation>, RepoError> {
            let mut builder = QueryBuilder::new("SELECT * FROM guardrail.annotations");
            Repo::build_query(
                &mut builder,
                &params,
                &["id", "key", "kind", "value"],
                &["key", "kind", "value"],
            )?;

            let query = builder.build_query_as();

            query.fetch_all(executor).await.map_err(|err| {
                error!("Failed to retrieve all annotations: {err}");
                RepoError::DatabaseError("Failed to retrieve annotations".to_string())
            })
        }

        pub async fn create(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            annotation: NewAnnotation,
        ) -> Result<uuid::Uuid, RepoError> {
            if !["system", "user"].contains(&annotation.kind.as_str()) {
                return Err(RepoError::InvalidColumn(format!(
                    "Invalid annotation kind: {}",
                    annotation.kind
                )));
            }

            sqlx::query_scalar!(
                r#"
                INSERT INTO guardrail.annotations
                  (
                    key,
                    kind,
                    value,
                    crash_id,
                    product_id
                  )
                VALUES ($1, $2, $3, $4, $5)
                RETURNING
                  id
            "#,
                annotation.key,
                annotation.kind,
                annotation.value,
                annotation.crash_id,
                annotation.product_id
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                error!("Failed to create annotation: {err}");
                RepoError::DatabaseError(format!("Failed to create annotation: {}", err))
            })
        }

        pub async fn update(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            annotation: Annotation,
        ) -> Result<Option<uuid::Uuid>, RepoError> {
            if !["system", "user"].contains(&annotation.kind.as_str()) {
                return Err(RepoError::InvalidColumn(format!(
                    "Invalid annotation kind: {}",
                    annotation.kind
                )));
            }

            sqlx::query_scalar!(
                r#"
                UPDATE guardrail.annotations
                SET key = $1, kind = $2, value = $3
                WHERE id = $4
                RETURNING id
            "#,
                annotation.key,
                annotation.kind,
                annotation.value,
                annotation.id,
            )
            .fetch_optional(executor)
            .await
            .map_err(|err| {
                error!("Failed to update annotation {}: {err}", annotation.id);
                RepoError::DatabaseError("Failed to update annotation".to_string())
            })
        }

        pub async fn remove(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            id: uuid::Uuid,
        ) -> Result<(), RepoError> {
            sqlx::query!(
                r#"
                DELETE FROM guardrail.annotations
                WHERE id = $1
            "#,
                id
            )
            .execute(executor)
            .await
            .map_err(|err| {
                error!("Failed to remove annotation {}: {}", id, err);
                RepoError::DatabaseError("Failed to remove annotation".to_string())
            })?;
            Ok(())
        }

        pub async fn count(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
        ) -> Result<i64, RepoError> {
            sqlx::query_scalar!(
                r#"
                SELECT COUNT(*)
                FROM guardrail.annotations
            "#
            )
            .fetch_one(executor)
            .await
            .map_err(|err| {
                error!("Failed to count annotations: {err}");
                RepoError::DatabaseError("Failed to count annotations".to_string())
            })
            .map(|count| count.unwrap_or(0))
        }

        pub async fn get_by_crash_id(
            executor: impl sqlx::Executor<'_, Database = Postgres>,
            crash_id: uuid::Uuid,
            params: QueryParams,
        ) -> Result<Vec<Annotation>, RepoError> {
            let mut builder =
                QueryBuilder::new("SELECT * FROM guardrail.annotations WHERE crash_id = ");
            builder.push_bind(crash_id);

            if !params.sorting.is_empty() || params.range.is_some() {
                Repo::build_query(
                    &mut builder,
                    &params,
                    &["id", "key", "kind", "value", "created_at"],
                    &[],
                )?;
            }

            let query = builder.build_query_as();

            query.fetch_all(executor).await.map_err(|err| {
                error!("Failed to retrieve annotations for crash {crash_id}: {err}");
                RepoError::DatabaseError("Failed to retrieve annotations by crash ID".to_string())
            })
        }


    }
}
