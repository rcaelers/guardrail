use rhai::{CustomType, TypeBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tracing::{debug, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationEntry {
    pub value: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackedAnnotations {
    data: Arc<Mutex<TrackedAnnotationsData>>,
}

#[derive(Debug, Serialize)]
struct TrackedAnnotationsData {
    original: HashMap<String, AnnotationEntry>,
    modified: HashMap<String, AnnotationEntry>,
}

impl TrackedAnnotations {
    pub fn new() -> Self {
        debug!("Creating new TrackedAnnotations instance");
        Self {
            data: Arc::new(Mutex::new(TrackedAnnotationsData {
                original: HashMap::new(),
                modified: HashMap::new(),
            })),
        }
    }

    pub fn from_map(annotations: HashMap<String, AnnotationEntry>) -> Self {
        Self {
            data: Arc::new(Mutex::new(TrackedAnnotationsData {
                original: annotations,
                modified: HashMap::new(),
            })),
        }
    }

    pub fn get(&self, key: &str) -> Option<AnnotationEntry> {
        match self.data.lock() {
            Ok(data) => data.modified.get(key)
                .or_else(|| data.original.get(key))
                .cloned(),
            Err(_) => {
                debug!("Failed to lock TrackedAnnotations for get operation");
                None
            }
        }
    }

    #[instrument()]
    pub fn set(&self, key: String, value: String) {
        debug!("Setting script annotation: {} = {}", key, value);
        let entry = AnnotationEntry {
            value,
            source: "script".to_string(),
        };
        match self.data.lock() {
            Ok(mut data) => {
                data.modified.insert(key, entry);
                debug!("({} modified entries)", data.modified.len());
            }
            Err(_) => {
                debug!("Failed to lock TrackedAnnotations for set operation");
            }
        }
    }

    pub fn remove(&self, key: &str) -> Option<AnnotationEntry> {
        match self.data.lock() {
            Ok(mut data) => data.modified.remove(key),
            Err(_) => {
                debug!("Failed to lock TrackedAnnotations for remove operation");
                None
            }
        }
    }

    pub fn keys(&self) -> Vec<String> {
        match self.data.lock() {
            Ok(data) => {
                let mut all_keys: HashSet<String> = HashSet::new();
                for key in data.original.keys() {
                    all_keys.insert(key.clone());
                }
                for key in data.modified.keys() {
                    all_keys.insert(key.clone());
                }
                all_keys.into_iter().collect()
            }
            Err(_) => {
                debug!("Failed to lock TrackedAnnotations for keys operation");
                Vec::new()
            }
        }
    }

    #[instrument()]
    pub fn finalize(self) -> HashMap<String, AnnotationEntry> {
        debug!("Finalizing annotations");
        match self.data.lock() {
            Ok(data) => {
                let mut result = data.original.clone();
                for (key, value) in &data.modified {
                    result.insert(key.clone(), value.clone());
                }
                result
            }
            Err(_) => {
                debug!("Failed to lock TrackedAnnotations for finalize operation");
                HashMap::new()
            }
        }
    }

    #[instrument()]
    pub fn was_modified(&self) -> bool {
        match self.data.lock() {
            Ok(data) => {
                debug!("Checking if annotations were modified ({} modified entries)", data.modified.len());
                !data.modified.is_empty()
            }
            Err(_) => {
                debug!("Failed to lock TrackedAnnotations for was_modified operation");
                false
            }
        }
    }
}

impl Default for TrackedAnnotations {
    fn default() -> Self {
        debug!("Creating default TrackedAnnotations instance");
        Self::new()
    }
}

impl CustomType for TrackedAnnotations {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("TrackedAnnotations")
            .with_fn("get", |annotations: &mut Self, key: String| {
                annotations
                    .get(&key)
                    .map(|entry| entry.value)
                    .unwrap_or_default()
            })
            .with_fn("set", |annotations: &mut Self, key: String, value: String| {
                debug!("Setting annotation set: {} = {}", key, value);
                annotations.set(key, value);
            })
            .with_fn("remove", |annotations: &mut Self, key: String| {
                annotations
                    .remove(&key)
                    .map(|entry| entry.value)
                    .unwrap_or_default()
            })
            .with_fn("keys", |annotations: &mut Self| annotations.keys())
            .with_fn("has", |annotations: &mut Self, key: String| annotations.get(&key).is_some())
            .with_indexer_get(|annotations: &mut Self, key: String| {
                debug!("Getting annotation: {}", key);
                annotations
                    .get(&key)
                    .map(|entry| entry.value)
                    .unwrap_or_default()
            })
            .with_indexer_set(|annotations: &mut Self, key: String, value: String| {
                debug!("Setting annotation: {} = {}", key, value);
                annotations.set(key, value);
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::{Engine, Scope};

    #[test]
    fn test_tracked_annotations_bracket_notation() {
        // Create a Rhai engine with our custom type
        let mut engine = Engine::new();
        engine.build_type::<TrackedAnnotations>();

        // Create a tracked annotations instance
        let tracked = TrackedAnnotations::new();
        tracked.set("initial_key".to_string(), "initial_value".to_string());

        // Create a scope with the annotations
        let mut scope = Scope::new();
        scope.push("annotations", tracked);

        // Test script using bracket notation
        let script = r#"
            // Test reading with bracket notation
            let initial = annotations["initial_key"];

            // Test writing with bracket notation
            annotations["new_key"] = "new_value";
            let new_val = annotations["new_key"];

            // Return both values
            #{
                initial: initial,
                new_val: new_val
            }
        "#;

        let result = engine
            .eval_with_scope::<rhai::Map>(&mut scope, script)
            .unwrap();

        // Verify the script worked correctly
        assert_eq!(
            result
                .get("initial")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "initial_value"
        );
        assert_eq!(
            result
                .get("new_val")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "new_value"
        );

        // Get the modified annotations back from scope
        let modified_annotations = scope
            .get_value::<TrackedAnnotations>("annotations")
            .unwrap();

        // Verify tracking worked
        assert!(modified_annotations.was_modified());
        let finalized = modified_annotations.finalize();
        assert_eq!(finalized.get("initial_key").unwrap().value, "initial_value");
        assert_eq!(finalized.get("initial_key").unwrap().source, "script");
        assert_eq!(finalized.get("new_key").unwrap().value, "new_value");
        assert_eq!(finalized.get("new_key").unwrap().source, "script");
    }

    #[test]
    fn test_tracked_annotations_get_all_annotations() {
        // Create original annotations
        let mut original_annotations = HashMap::new();
        original_annotations.insert(
            "existing_key".to_string(),
            AnnotationEntry {
                value: "existing_value".to_string(),
                source: "original".to_string(),
            },
        );
        original_annotations.insert(
            "another_key".to_string(),
            AnnotationEntry {
                value: "another_value".to_string(),
                source: "original".to_string(),
            },
        );

        // Create TrackedAnnotations from original map
        let tracked = TrackedAnnotations::from_map(original_annotations);

        // Add a new annotation
        tracked.set("new_key".to_string(), "new_value".to_string());

        // Test finalize method
        let finalized = tracked.finalize();
        assert_eq!(finalized.len(), 3);
        assert_eq!(finalized.get("existing_key").unwrap().value, "existing_value");
        assert_eq!(finalized.get("new_key").unwrap().value, "new_value");
    }

    #[test]
    fn test_tracked_annotations_simple() {
        // Test basic functionality
        let original = HashMap::from([
            (
                "key1".to_string(),
                AnnotationEntry {
                    value: "value1".to_string(),
                    source: "test".to_string(),
                },
            ),
            (
                "key2".to_string(),
                AnnotationEntry {
                    value: "value2".to_string(),
                    source: "test".to_string(),
                },
            ),
        ]);

        let tracked = TrackedAnnotations::from_map(original);

        // Test getting existing values
        assert_eq!(tracked.get("key1").unwrap().value, "value1");
        assert_eq!(tracked.get("key2").unwrap().value, "value2");
        assert!(tracked.get("nonexistent").is_none());

        // Test setting new values
        tracked.set("key3".to_string(), "value3".to_string());
        assert_eq!(tracked.get("key3").unwrap().value, "value3");
        assert_eq!(tracked.get("key3").unwrap().source, "script");

        // Test was_modified
        assert!(tracked.was_modified());

        // Test keys
        let keys = tracked.keys();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }
}
