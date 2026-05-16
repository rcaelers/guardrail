use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    error::{RepoError, handle_surreal_error},
    record_key,
};
use data::product_settings::{EmailSettings, MinidumpSettings, ProcessorSettings, ProductSettings};

pub struct ProductSettingsRepo {}

impl ProductSettingsRepo {
    /// Fetch settings for a product. Returns `None` if no record exists yet.
    pub async fn get(
        db: &Surreal<Any>,
        product_id: &str,
    ) -> Result<Option<ProductSettings>, RepoError> {
        let mut result = db
            .query(
                "SELECT *, meta::id(id) AS id, meta::id(product_id) AS product_id \
                 FROM ONLY type::record('product_settings', $id)",
            )
            .bind(("id", record_key(product_id)))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    /// Fetch settings for a product, creating a default record if none exists.
    /// The UPSERT is atomic, so concurrent calls safely converge on one record.
    pub async fn get_or_create(
        db: &Surreal<Any>,
        product_id: &str,
    ) -> Result<ProductSettings, RepoError> {
        let key = record_key(product_id);
        let mut result = db
            .query(
                "UPSERT type::record('product_settings', $id) SET \
                 product_id = type::record('products', $id), \
                 created_at = created_at OR time::now(), \
                 updated_at = updated_at OR time::now() \
                 RETURN *, meta::id(id) AS id, meta::id(product_id) AS product_id",
            )
            .bind(("id", key))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one::<ProductSettings>(&mut result, 0)?
            .ok_or_else(|| RepoError::DatabaseError("upsert returned no row".into()))
    }

    /// Upsert the processor section of a product's settings.
    pub async fn upsert_processor(
        db: &Surreal<Any>,
        product_id: &str,
        processor: ProcessorSettings,
    ) -> Result<ProductSettings, RepoError> {
        let key = record_key(product_id);
        let mut result = db
            .query(
                "UPSERT type::record('product_settings', $id) SET \
                 product_id = type::record('products', $id), \
                 processor.skip_patterns = $skip_patterns, \
                 processor.end_patterns = $end_patterns, \
                 processor.delimiter = $delimiter, \
                 processor.maximum_frame_count = $maximum_frame_count, \
                 created_at = created_at OR time::now(), \
                 updated_at = time::now() \
                 RETURN *, meta::id(id) AS id, meta::id(product_id) AS product_id",
            )
            .bind(("id", key))
            .bind(("skip_patterns", processor.skip_patterns))
            .bind(("end_patterns", processor.end_patterns))
            .bind(("delimiter", processor.delimiter))
            .bind(("maximum_frame_count", processor.maximum_frame_count))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one::<ProductSettings>(&mut result, 0)?
            .ok_or_else(|| RepoError::DatabaseError("upsert returned no row".into()))
    }

    /// Upsert the minidump section of a product's settings.
    pub async fn upsert_minidump(
        db: &Surreal<Any>,
        product_id: &str,
        minidump: MinidumpSettings,
    ) -> Result<ProductSettings, RepoError> {
        let key = record_key(product_id);
        let mut result = db
            .query(
                "UPSERT type::record('product_settings', $id) SET \
                 product_id = type::record('products', $id), \
                 minidump.mandatory_annotations = $mandatory_annotations, \
                 created_at = created_at OR time::now(), \
                 updated_at = time::now() \
                 RETURN *, meta::id(id) AS id, meta::id(product_id) AS product_id",
            )
            .bind(("id", key))
            .bind(("mandatory_annotations", minidump.mandatory_annotations))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one::<ProductSettings>(&mut result, 0)?
            .ok_or_else(|| RepoError::DatabaseError("upsert returned no row".into()))
    }

    /// Upsert the email section of a product's settings.
    /// Creates the record if it doesn't exist; updates only the email fields if it does.
    pub async fn upsert_email(
        db: &Surreal<Any>,
        product_id: &str,
        email: EmailSettings,
    ) -> Result<ProductSettings, RepoError> {
        let key = record_key(product_id);
        let mut result = db
            .query(
                "UPSERT type::record('product_settings', $id) SET \
                 product_id = type::record('products', $id), \
                 email.invite_subject = $subject, \
                 email.invite_html_template = $html, \
                 email.invite_text_template = $text, \
                 created_at = created_at OR time::now(), \
                 updated_at = time::now() \
                 RETURN *, meta::id(id) AS id, meta::id(product_id) AS product_id",
            )
            .bind(("id", key))
            .bind(("subject", email.invite_subject))
            .bind(("html", email.invite_html_template))
            .bind(("text", email.invite_text_template))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one::<ProductSettings>(&mut result, 0)?
            .ok_or_else(|| RepoError::DatabaseError("upsert returned no row".into()))
    }
}
