#![cfg(feature = "ssr")]

use repos::SortOrder;
use repos::error::RepoError;
use repos::*;
use sqlx::{Postgres, QueryBuilder};
use std::collections::VecDeque;

#[test]
fn test_build_query_with_filter() {
    let mut builder = QueryBuilder::new("SELECT * FROM table");

    let params = QueryParams {
        filter: Some("test".to_string()),
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name", "description"];

    let result = Repo::build_query(&mut builder, &params, allowed_columns, filter_columns);
    assert!(result.is_ok());
}

#[test]
fn test_build_query_with_invalid_filter_column() {
    let mut builder = QueryBuilder::new("SELECT * FROM table");

    let params = QueryParams {
        filter: Some("test".to_string()),
        ..Default::default()
    };

    let allowed_columns = &["name"];
    let filter_columns = &["invalid_column"];

    let result = Repo::build_query(&mut builder, &params, allowed_columns, filter_columns);
    assert!(result.is_err());

    if let Err(RepoError::InvalidColumn(col)) = result {
        assert_eq!(col, "invalid_column");
    } else {
        panic!("Expected InvalidColumn error");
    }
}

#[test]
fn test_build_query_with_empty_filter_columns() {
    let mut builder = QueryBuilder::new("SELECT * FROM table");

    let params = QueryParams {
        filter: Some("test".to_string()),
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &[];

    let result = Repo::build_query(&mut builder, &params, allowed_columns, filter_columns);
    assert!(result.is_err());

    if let Err(RepoError::InvalidColumn(msg)) = result {
        assert_eq!(msg, "No filter columns specified");
    } else {
        panic!("Expected InvalidColumn error");
    }
}

#[test]
fn test_build_query_with_sorting() {
    let mut builder = QueryBuilder::new("SELECT * FROM table");

    let mut params = QueryParams::default();
    let mut sorting = VecDeque::new();
    sorting.push_back(("name".to_string(), SortOrder::Ascending));
    params.sorting = sorting;

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query(&mut builder, &params, allowed_columns, filter_columns);
    assert!(result.is_ok());
}

#[test]
fn test_build_query_with_invalid_sort_column() {
    let mut builder = QueryBuilder::new("SELECT * FROM table");

    let mut params = QueryParams::default();
    let mut sorting = VecDeque::new();
    sorting.push_back(("invalid_column".to_string(), SortOrder::Ascending));
    params.sorting = sorting;

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query(&mut builder, &params, allowed_columns, filter_columns);
    assert!(result.is_err());

    if let Err(RepoError::InvalidColumn(col)) = result {
        assert_eq!(col, "invalid_column");
    } else {
        panic!("Expected InvalidColumn error");
    }
}

#[test]
fn test_build_query_with_range() {
    let mut builder = QueryBuilder::new("SELECT * FROM table");

    let params = QueryParams {
        range: Some(0..10),
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query(&mut builder, &params, allowed_columns, filter_columns);
    assert!(result.is_ok());
}

#[test]
fn test_build_query_with_both_filter_and_sorting() {
    let mut builder = QueryBuilder::new("SELECT * FROM table");

    let mut sorting = VecDeque::new();
    sorting.push_back(("name".to_string(), SortOrder::Ascending));

    let params = QueryParams {
        filter: Some("test".to_string()),
        sorting,
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query(&mut builder, &params, allowed_columns, filter_columns);
    assert!(result.is_ok());
}

#[test]
fn test_build_query_with_filter_sorting_and_range() {
    let mut builder = QueryBuilder::new("SELECT * FROM table");

    let mut sorting = VecDeque::new();
    sorting.push_back(("name".to_string(), SortOrder::Ascending));

    let params = QueryParams {
        filter: Some("test".to_string()),
        sorting,
        range: Some(0..10),
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query(&mut builder, &params, allowed_columns, filter_columns);
    assert!(result.is_ok());
}

#[test]
fn test_build_query_with_multiple_sort_columns() {
    let mut builder = QueryBuilder::new("SELECT * FROM table");

    let mut params = QueryParams::default();
    let mut sorting = VecDeque::new();
    sorting.push_back(("name".to_string(), SortOrder::Ascending));
    sorting.push_back(("description".to_string(), SortOrder::Descending));
    params.sorting = sorting;

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name"];

    let result = Repo::build_query(&mut builder, &params, allowed_columns, filter_columns);
    assert!(result.is_ok());
}

#[test]
fn test_build_query_with_multiple_filter_columns() {
    let mut builder = QueryBuilder::new("SELECT * FROM table");

    let params = QueryParams {
        filter: Some("test".to_string()),
        ..Default::default()
    };

    let allowed_columns = &["name", "description"];
    let filter_columns = &["name", "description"];

    let result = Repo::build_query(&mut builder, &params, allowed_columns, filter_columns);
    assert!(result.is_ok());
}
