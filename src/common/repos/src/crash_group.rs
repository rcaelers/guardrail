use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::{
    error::{RepoError, handle_surreal_error},
    record_key,
};
use data::crash_group::{CrashGroup, NewCrashGroup};

pub struct CrashGroupRepo {}

impl CrashGroupRepo {
    pub async fn get_by_id(
        db: &Surreal<Any>,
        id: impl ToString,
    ) -> Result<Option<CrashGroup>, RepoError> {
        let mut result = db
            .query(
                "SELECT *, meta::id(id) as id, meta::id(product_id) as product_id \
                 FROM ONLY type::record('crash_groups', $id)",
            )
            .bind(("id", record_key(id.to_string())))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    /// The group a fingerprint belongs to: its own, or the one it was merged
    /// into. A group's own fingerprint wins over an alias, so a group that was
    /// merged away and later re-created still takes precedence for new crashes.
    pub async fn find_by_fingerprint(
        db: &Surreal<Any>,
        product_id: &str,
        fingerprint: &str,
    ) -> Result<Option<CrashGroup>, RepoError> {
        let mut result = db
            .query(
                "SELECT *, meta::id(id) as id, meta::id(product_id) as product_id, \
                        fingerprint = $fingerprint AS exact \
                 FROM crash_groups \
                 WHERE product_id = type::record('products', $product_id) \
                   AND (fingerprint = $fingerprint \
                        OR $fingerprint IN (merged_fingerprints ?? [])) \
                 ORDER BY exact DESC \
                 LIMIT 1",
            )
            .bind(("product_id", record_key(product_id)))
            .bind(("fingerprint", fingerprint.to_string()))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one(&mut result, 0)
    }

    /// Create a new crash group. Returns `Some(id)` if created, `None` if a group with the
    /// same (product_id, fingerprint) already exists (silent duplicate — no DB error raised).
    pub async fn create(
        db: &Surreal<Any>,
        group: NewCrashGroup,
    ) -> Result<Option<String>, RepoError> {
        let id = uuid::Uuid::new_v4().to_string();
        let rows: Vec<serde_json::Value> = db
            .query(
                "INSERT IGNORE INTO crash_groups {
                    id: type::record('crash_groups', $id),
                    product_id: type::record('products', $product_id),
                    fingerprint: $fingerprint,
                    signal: $signal,
                    count: 1,
                    first_seen: time::now(),
                    last_seen: time::now(),
                    status: 'new',
                    created_at: time::now(),
                    updated_at: time::now()
                }",
            )
            .bind(("id", id.clone()))
            .bind(("product_id", record_key(&group.product_id)))
            .bind(("fingerprint", group.fingerprint))
            .bind(("signal", group.signal))
            .await
            .map_err(handle_surreal_error)?
            .take(0)
            .map_err(handle_surreal_error)?;
        Ok(if rows.is_empty() { None } else { Some(id) })
    }

    /// Increment the crash count and push `last_seen` forward.
    /// Called for every crash that joins an existing group.
    /// Reopens a group whose fix did not hold. Only ever moves away from a
    /// closed state, so a group somebody has since re-triaged is left alone.
    pub async fn mark_regressed(db: &Surreal<Any>, id: &str) -> Result<(), RepoError> {
        db.query(
            "UPDATE type::record('crash_groups', $id) SET \
                status = 'regressed', \
                updated_at = time::now() \
             WHERE status IN ['resolved', 'wontfix']",
        )
        .bind(("id", record_key(id)))
        .await
        .map_err(handle_surreal_error)?;
        Ok(())
    }

    pub async fn touch(db: &Surreal<Any>, id: &str) -> Result<(), RepoError> {
        db.query(
            "UPDATE type::record('crash_groups', $id) SET \
                count += 1, \
                last_seen = time::now(), \
                updated_at = time::now()",
        )
        .bind(("id", record_key(id)))
        .await
        .map_err(handle_surreal_error)?;
        Ok(())
    }
}

/// Why a merge was refused: the two groups disagree on a decision a person has
/// to make. The text is what `data::crash_group::merge_blocker` returned.
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("cannot merge: {0}")]
    Conflict(String),
    #[error("crash group not found")]
    NotFound,
    #[error(transparent)]
    Repo(#[from] RepoError),
}

/// What the merge needs of each group before it starts.
#[derive(Debug, serde::Deserialize)]
struct MergeRow {
    status: Option<String>,
    fixed_in_version: Option<String>,
    fingerprint: String,
    count: Option<i64>,
    first_seen: Option<String>,
    last_seen: Option<String>,
    #[serde(default)]
    merged_fingerprints: Vec<String>,
    assignee: Option<String>,
}

impl MergeRow {
    fn side(&self) -> data::crash_group::MergeSide {
        data::crash_group::MergeSide {
            status: self.status.clone().unwrap_or_else(|| "new".into()),
            fixed_in_version: self.fixed_in_version.clone().filter(|v| !v.is_empty()),
            assignee: self.assignee.clone().filter(|v| !v.is_empty()),
        }
    }
}

impl CrashGroupRepo {
    async fn merge_row(db: &Surreal<Any>, id: &str) -> Result<MergeRow, MergeError> {
        let mut result = db
            .query(
                "SELECT status, fixed_in_version, fingerprint, count, first_seen, last_seen, \
                        merged_fingerprints ?? [] AS merged_fingerprints, \
                        IF assignee != NONE THEN meta::id(assignee) ELSE NONE END AS assignee \
                 FROM ONLY type::record('crash_groups', $id)",
            )
            .bind(("id", record_key(id)))
            .await
            .map_err(handle_surreal_error)?;
        crate::take_one::<MergeRow>(&mut result, 0)?.ok_or(MergeError::NotFound)
    }

    /// Merge `merged` into `primary`, which survives. Both must exist and be
    /// visible on `db`; the caller has already checked they belong to the same
    /// product and that the actor may merge.
    ///
    /// The result does not depend on which side survives: status is the one
    /// that says more, fixed version and assignee are whichever exists, notes
    /// are kept from both, the seen range widens, and the merged fingerprints
    /// are recorded so later crashes follow the merge. A resolved group that
    /// ends up holding crashes from its fixed release becomes regressed. Pairs
    /// that disagree on a decision are refused with `MergeError::Conflict`.
    pub async fn merge(
        db: &Surreal<Any>,
        product_id: &str,
        primary: &str,
        merged: &str,
        author: &str,
    ) -> Result<(), MergeError> {
        // Fetch both up front: SurrealDB loses $token context in UPDATE
        // subqueries, so RLS would filter a subquery on the other group to NONE.
        let primary_row = Self::merge_row(db, primary).await?;
        let merged_row = Self::merge_row(db, merged).await?;
        if let Some(why) = data::crash_group::merge_blocker(&primary_row.side(), &merged_row.side())
        {
            return Err(MergeError::Conflict(why));
        }

        let primary_side = primary_row.side();
        let merged_side = merged_row.side();
        let status =
            data::crash_group::merged_status(&primary_side.status, &merged_side.status).to_string();
        let fixed_in_version = primary_side
            .fixed_in_version
            .or(merged_side.fixed_in_version);
        let assignee = primary_side.assignee.or(merged_side.assignee);
        let mut aliases = merged_row.merged_fingerprints.clone();
        aliases.push(merged_row.fingerprint.clone());
        let merged_count = merged_row.count.unwrap_or(0);

        db.query(
            "UPDATE crashes SET group_id = type::record('crash_groups', $pid) \
             WHERE group_id = type::record('crash_groups', $mid)",
        )
        .bind(("pid", record_key(primary)))
        .bind(("mid", record_key(merged)))
        .await
        .map_err(handle_surreal_error)?;
        // The merged group's notes are part of the bug's history; keep them.
        db.query(
            "UPDATE annotations SET group_id = type::record('crash_groups', $pid) \
             WHERE group_id = type::record('crash_groups', $mid)",
        )
        .bind(("pid", record_key(primary)))
        .bind(("mid", record_key(merged)))
        .await
        .map_err(handle_surreal_error)?;

        // A resolved group now holding crashes from the fixed release is a
        // regression, exactly as it would be had those crashes arrived later.
        let status = if status == "resolved" && fixed_in_version.is_some() {
            let mut result = db
                .query(
                    "SELECT VALUE report.version FROM crashes \
                     WHERE group_id = type::record('crash_groups', $pid)",
                )
                .bind(("pid", record_key(primary)))
                .await
                .map_err(handle_surreal_error)?;
            let versions: Vec<Option<String>> = crate::take_many(&mut result, 0)?;
            let regressed = versions
                .iter()
                .any(|v| common::version::is_regression(v.as_deref(), fixed_in_version.as_deref()));
            if regressed {
                "regressed".to_string()
            } else {
                status
            }
        } else {
            status
        };

        // The surviving group now spans both, so its seen range widens to match.
        db.query(
            "UPDATE type::record('crash_groups', $pid) SET \
               count = count + $c, \
               status = $status, \
               fixed_in_version = IF $fixed = NULL THEN NONE ELSE $fixed END, \
               assignee = IF $assignee = NULL THEN NONE ELSE type::record('users', $assignee) END, \
               merged_fingerprints = array::union(merged_fingerprints ?? [], $aliases), \
               first_seen = IF $fs != NULL AND type::datetime($fs) < first_seen \
                            THEN type::datetime($fs) ELSE first_seen END, \
               last_seen = IF $ls != NULL AND type::datetime($ls) > last_seen \
                           THEN type::datetime($ls) ELSE last_seen END, \
               updated_at = time::now()",
        )
        .bind(("pid", record_key(primary)))
        .bind(("c", merged_count))
        .bind(("status", status))
        .bind(("fixed", fixed_in_version))
        .bind(("assignee", assignee))
        .bind(("aliases", aliases))
        .bind(("fs", merged_row.first_seen))
        .bind(("ls", merged_row.last_seen))
        .await
        .map_err(handle_surreal_error)?;

        let note = format!(
            "Merged crash group {} ({} {}) into this one.\n{}",
            &merged[..merged.len().min(8)],
            merged_count,
            if merged_count == 1 {
                "crash"
            } else {
                "crashes"
            },
            merged_row.fingerprint
        );
        db.query(
            "CREATE annotations CONTENT { \
                source: 'user', \
                value: $body, \
                author: $author, \
                group_id: type::record('crash_groups', $pid), \
                product_id: type::record('products', $product), \
                created_at: time::now(), \
                updated_at: time::now() \
             }",
        )
        .bind(("body", note))
        .bind(("author", author.to_string()))
        .bind(("pid", record_key(primary)))
        .bind(("product", record_key(product_id)))
        .await
        .map_err(handle_surreal_error)?;

        db.query("DELETE type::record('crash_groups', $mid)")
            .bind(("mid", record_key(merged)))
            .await
            .map_err(handle_surreal_error)?;
        Ok(())
    }
}
