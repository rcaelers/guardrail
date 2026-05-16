use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CachedProcessorSettings {
    pub skip_patterns: Option<Vec<String>>,
    pub end_patterns: Option<Vec<String>>,
    pub delimiter: Option<String>,
    pub maximum_frame_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedValidationScript {
    pub id: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInfo {
    pub id: String,
    pub name: String,
    pub accepting_crashes: bool,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub mandatory_annotations: Vec<String>,
    #[serde(default)]
    pub validation_scripts: Vec<CachedValidationScript>,
    #[serde(default)]
    pub processor_settings: Option<CachedProcessorSettings>,
}

pub fn product_cache_key(product_identifier: &str) -> String {
    format!("guardrail::product:by-name:{}", product_identifier.trim().to_ascii_lowercase())
}

pub fn product_token_cache_key(token: &str) -> String {
    format!("guardrail::product:by-token:{token}")
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
        assert_eq!(product_cache_key(" Workrave "), "guardrail::product:by-name:workrave");
        assert_eq!(product_cache_key("workrave"), "guardrail::product:by-name:workrave");
    }

    #[test]
    fn product_cache_keys_includes_name_and_slug_without_duplicates() {
        assert_eq!(
            product_cache_keys("Workrave Demo", Some("workrave-demo")),
            vec![
                "guardrail::product:by-name:workrave demo".to_string(),
                "guardrail::product:by-name:workrave-demo".to_string()
            ]
        );

        assert_eq!(
            product_cache_keys("Workrave", Some("workrave")),
            vec!["guardrail::product:by-name:workrave".to_string()]
        );
    }
}
