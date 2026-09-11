use tracing::debug;

/// True when a crash from `crash_version` proves a fix released in
/// `fixed_in_version` did not hold.
///
/// Deliberately conservative: an unparseable version on either side means no
/// answer, so a group is left closed. A wrong reopen is noise on every later
/// crash, while a missed one costs only the automatic signal.
pub fn is_regression(crash_version: Option<&str>, fixed_in_version: Option<&str>) -> bool {
    let (Some(crash_version), Some(fixed)) = (crash_version, fixed_in_version) else {
        return false;
    };
    let (Ok(crash_version), Ok(fixed)) = (
        semver::Version::parse(crash_version.trim()),
        semver::Version::parse(fixed.trim()),
    ) else {
        debug!(crash_version, fixed, "Version not comparable; not treating as a regression");
        return false;
    };
    crash_version >= fixed
}

#[cfg(test)]
mod tests {
    use super::is_regression;

    #[test]
    fn regression_needs_both_versions_to_parse_and_compare() {
        assert!(is_regression(Some("1.11.2"), Some("1.11.2")));
        assert!(is_regression(Some("1.11.3"), Some("1.11.2")));
        assert!(is_regression(Some("2.0.0"), Some("1.11.2")));

        assert!(!is_regression(Some("1.11.1"), Some("1.11.2")));
        assert!(!is_regression(Some("1.10.52"), Some("1.11.2")));

        // A pre-release sorts before its release.
        assert!(!is_regression(Some("1.11.2-rc.4"), Some("1.11.2")));
        assert!(is_regression(Some("1.11.2"), Some("1.11.2-rc.4")));

        assert!(!is_regression(Some("1.11.2"), None));
        assert!(!is_regression(None, Some("1.11.2")));
        assert!(!is_regression(Some("not a version"), Some("1.11.2")));
    }
}
