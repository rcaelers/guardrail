use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInfo {
    pub id: String,
    pub name: String,
    pub accepting_crashes: bool,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

pub fn product_cache_key(product_identifier: &str) -> String {
    format!("product:by-name:{}", product_identifier.trim().to_ascii_lowercase())
}

pub fn product_cache_keys(product_name: &str, product_slug: Option<&str>) -> Vec<String> {
    let mut keys = Vec::new();

    for identifier in [Some(product_name), product_slug].into_iter().flatten() {
        let key = product_cache_key(identifier);
        if !keys.contains(&key) {
            keys.push(key);
        }
    }

    keys
}

#[cfg(test)]
mod tests {
    use super::{product_cache_key, product_cache_keys};

    #[test]
    fn product_cache_key_normalizes_case_and_whitespace() {
        assert_eq!(product_cache_key(" Workrave "), "product:by-name:workrave");
        assert_eq!(product_cache_key("workrave"), "product:by-name:workrave");
    }

    #[test]
    fn product_cache_keys_includes_name_and_slug_without_duplicates() {
        assert_eq!(
            product_cache_keys("Workrave Demo", Some("workrave-demo")),
            vec![
                "product:by-name:workrave demo".to_string(),
                "product:by-name:workrave-demo".to_string()
            ]
        );

        assert_eq!(
            product_cache_keys("Workrave", Some("workrave")),
            vec!["product:by-name:workrave".to_string()]
        );
    }
}
