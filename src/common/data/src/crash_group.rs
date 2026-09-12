use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CrashGroup {
    pub id: String,
    pub product_id: String,
    pub fingerprint: String,
    pub signal: String,
    pub count: i64,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub status: String,
    pub fixed_in_version: Option<String>,
    #[serde(default)]
    pub merged_fingerprints: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NewCrashGroup {
    pub product_id: String,
    pub fingerprint: String,
    /// Human-readable display label — typically mirrors the fingerprint.
    pub signal: String,
}

/// What a merge needs to know about one side.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeSide {
    pub status: String,
    pub fixed_in_version: Option<String>,
    pub assignee: Option<String>,
}

/// Why two groups cannot be merged, or `None` when they can. Symmetric: the
/// answer does not depend on which side is the survivor.
///
/// A merge combines evidence about one bug, so it is refused where the two
/// records disagree about a human decision rather than about facts: fixed in
/// two different releases, one fixed while the other is not to be fixed, or
/// two different people assigned.
pub fn merge_blocker(a: &MergeSide, b: &MergeSide) -> Option<String> {
    if let (Some(x), Some(y)) = (&a.fixed_in_version, &b.fixed_in_version)
        && x != y
    {
        let (lo, hi) = if x <= y { (x, y) } else { (y, x) };
        return Some(format!("fixed in different versions ({lo} and {hi})"));
    }
    let statuses = (a.status.as_str(), b.status.as_str());
    if matches!(statuses, ("resolved", "wontfix") | ("wontfix", "resolved")) {
        return Some("one is resolved, the other won't be fixed".into());
    }
    if let (Some(x), Some(y)) = (&a.assignee, &b.assignee)
        && x != y
    {
        return Some("assigned to different people".into());
    }
    None
}

/// The status the merged group takes: the one that says more. A regression is
/// evidence and beats everything; resolved and wontfix are decisions and beat
/// triage; triage beats new; obsolete says nothing about the bug at all -- its
/// fingerprint can no longer occur -- so it yields even to new. Resolved
/// against wontfix never gets here, since `merge_blocker` refuses that pair.
pub fn merged_status<'a>(a: &'a str, b: &'a str) -> &'a str {
    fn rank(status: &str) -> i8 {
        match status {
            "regressed" => 3,
            "resolved" | "wontfix" => 2,
            "triaged" => 1,
            "obsolete" => -1,
            _ => 0,
        }
    }
    if rank(b) > rank(a) { b } else { a }
}
