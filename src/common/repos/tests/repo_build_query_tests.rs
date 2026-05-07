use std::collections::VecDeque;

use common::{QueryParams, SortOrder};
use repos::error::RepoError;
use repos::*;

#[test]
fn test_build_query_suffix_with_filter() {
    let params = QueryParams {
        filter: Some("test".to_string()),
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name", "description"];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_ok());
    let suffix = result.unwrap();
    assert!(suffix.contains("WHERE"));
    assert!(suffix.contains("string::lowercase(name) CONTAINS string::lowercase($filter)"));
    assert!(suffix.contains("string::lowercase(description) CONTAINS string::lowercase($filter)"));
}

#[test]
fn test_build_query_suffix_with_invalid_filter_column() {
    let params = QueryParams {
        filter: Some("test".to_string()),
        ..Default::default()
    };

    let allowed_columns = &["name"];
    let filter_columns = &["invalid_column"];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_err());

    if let Err(RepoError::InvalidColumn(col)) = result {
        assert_eq!(col, "invalid_column");
    } else {
        panic!("Expected InvalidColumn error");
    }
}

#[test]
fn test_build_query_suffix_with_empty_filter_columns() {
    let params = QueryParams {
        filter: Some("test".to_string()),
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &[];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_err());

    if let Err(RepoError::InvalidColumn(msg)) = result {
        assert_eq!(msg, "No filter columns specified");
    } else {
        panic!("Expected InvalidColumn error");
    }
}

#[test]
fn test_build_query_suffix_with_sorting() {
    let mut params = QueryParams::default();
    let mut sorting = VecDeque::new();
    sorting.push_back(("name".to_string(), SortOrder::Ascending));
    params.sorting = sorting;

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_ok());
    let suffix = result.unwrap();
    assert!(suffix.contains("ORDER BY name ASC"));
}

#[test]
fn test_build_query_suffix_with_invalid_sort_column() {
    let mut params = QueryParams::default();
    let mut sorting = VecDeque::new();
    sorting.push_back(("invalid_column".to_string(), SortOrder::Ascending));
    params.sorting = sorting;

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_err());

    if let Err(RepoError::InvalidColumn(col)) = result {
        assert_eq!(col, "invalid_column");
    } else {
        panic!("Expected InvalidColumn error");
    }
}

#[test]
fn test_build_query_suffix_with_range() {
    let params = QueryParams {
        range: Some(0..10),
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_ok());
    let suffix = result.unwrap();
    assert!(suffix.contains("LIMIT 10 START 0"));
}

#[test]
fn test_build_query_suffix_with_both_filter_and_sorting() {
    let mut sorting = VecDeque::new();
    sorting.push_back(("name".to_string(), SortOrder::Ascending));

    let params = QueryParams {
        filter: Some("test".to_string()),
        sorting,
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_ok());
    let suffix = result.unwrap();
    assert!(suffix.contains("WHERE"));
    assert!(suffix.contains("ORDER BY"));
}

#[test]
fn test_build_query_suffix_with_filter_sorting_and_range() {
    let mut sorting = VecDeque::new();
    sorting.push_back(("name".to_string(), SortOrder::Ascending));

    let params = QueryParams {
        filter: Some("test".to_string()),
        sorting,
        range: Some(0..10),
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_ok());
    let suffix = result.unwrap();
    assert!(suffix.contains("WHERE"));
    assert!(suffix.contains("ORDER BY"));
    assert!(suffix.contains("LIMIT"));
}

#[test]
fn test_build_query_suffix_with_multiple_sort_columns() {
    let mut params = QueryParams::default();
    let mut sorting = VecDeque::new();
    sorting.push_back(("name".to_string(), SortOrder::Ascending));
    sorting.push_back(("description".to_string(), SortOrder::Descending));
    params.sorting = sorting;

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_ok());
    let suffix = result.unwrap();
    assert!(suffix.contains("name ASC"));
    assert!(suffix.contains("description DESC"));
}

#[test]
fn test_build_query_suffix_with_multiple_filter_columns() {
    let params = QueryParams {
        filter: Some("test".to_string()),
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name", "description"];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_ok());
    let suffix = result.unwrap();
    assert!(suffix.contains(" OR "));
}

#[test]
fn test_build_query_suffix_no_params() {
    let params = QueryParams::default();

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query_suffix(&params, allowed_columns, filter_columns);
    assert!(result.is_ok());
    let suffix = result.unwrap();
    assert!(suffix.is_empty());
}
