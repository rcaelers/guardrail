use repos::*;
use std::collections::VecDeque;

#[test]
fn test_sort_order_to_sql() {
    assert_eq!(SortOrder::Ascending.to_sql(), "ASC");
    assert_eq!(SortOrder::Descending.to_sql(), "DESC");
}

#[test]
fn test_query_params_default() {
    let params: QueryParams = QueryParams::default();
    assert!(params.sorting.is_empty());
    assert!(params.range.is_none());
    assert!(params.filter.is_none());
}

#[test]
fn test_query_params_with_sorting() {
    let mut params = QueryParams::default();
    let mut sorting = VecDeque::new();
    sorting.push_back(("name".to_string(), SortOrder::Ascending));
    params.sorting = sorting;

    assert_eq!(params.sorting.len(), 1);
    let (col, order) = &params.sorting[0];
    assert_eq!(col, "name");
    if let SortOrder::Ascending = order {
        // Test passed
    } else {
        panic!("Expected Ascending order");
    }
}

#[test]
fn test_query_params_with_range() {
    let mut params = QueryParams::default();
    params.range = Some(5..15);

    assert!(params.range.is_some());
    let range = params.range.as_ref().unwrap();
    assert_eq!(range.start, 5);
    assert_eq!(range.end, 15);
}

#[test]
fn test_query_params_with_filter() {
    let mut params = QueryParams::default();
    params.filter = Some("test filter".to_string());

    assert!(params.filter.is_some());
    assert_eq!(params.filter.as_ref().unwrap(), "test filter");
}
