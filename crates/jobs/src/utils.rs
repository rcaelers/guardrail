#[derive(Debug, Clone, PartialEq)]
pub struct JsonHelpers;

impl JsonHelpers {
    pub fn get_string(obj: &serde_json::Value, key: &str) -> Option<String> {
        obj.get(key)?.as_str().map(|s| s.to_string())
    }

    pub fn get_u32(obj: &serde_json::Value, key: &str) -> Option<u32> {
        obj.get(key)?.as_u64().and_then(|n| u32::try_from(n).ok())
    }

    pub fn get_array<'a>(obj: &'a serde_json::Value, key: &str) -> Vec<&'a serde_json::Value> {
        obj.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_json_loading() {
        // Test loading the crash.json file using
        let content =
            std::fs::read_to_string("../../dev/crash.json").expect("Failed to load crash.json");
        let crash_data_value: serde_json::Value =
            serde_json::from_str(&content).expect("Failed to parse crash.json");

        // Verify basic structure using CrashDataHelper - inline get_crash_info
        let pid = JsonHelpers::get_u32(&crash_data_value, "pid");
        let status = JsonHelpers::get_string(&crash_data_value, "status");
        let thread_count = JsonHelpers::get_array(&crash_data_value, "threads").len();
        let (_pid, _status, thread_count) = (pid, status, thread_count);
        assert!(thread_count > 0, "Should have at least one thread");

        // Inline get_thread
        let threads = JsonHelpers::get_array(&crash_data_value, "threads");
        let thread_data = threads
            .first()
            .copied()
            .expect("Should be able to get first thread");

        // Check frames using CrashDataHelper - inline get_frames
        let frames = JsonHelpers::get_array(thread_data, "frames");
        assert!(!frames.is_empty(), "First thread should have frames");
    }

    #[test]
    fn test_crash_data_conversion() {
        // Test that conversion from serde_json::Value to thread data works
        let content =
            std::fs::read_to_string("../../dev/crash.json").expect("Failed to load crash.json");
        let crash_data_value: serde_json::Value =
            serde_json::from_str(&content).expect("Failed to parse crash.json");

        // Inline get_thread
        let threads = JsonHelpers::get_array(&crash_data_value, "threads");
        let thread_data = threads.first().copied().expect("Should get thread data");

        // Check frames using JsonHelpers - inline get_frames
        let frames = JsonHelpers::get_array(thread_data, "frames");
        assert!(!frames.is_empty(), "Converted thread data should have frames");

        // Verify that frame data is properly accessible
        if let Some(first_frame) = frames.first() {
            assert!(
                first_frame.get("function").is_some() || first_frame.get("module").is_some(),
                "Frame should have function or module"
            );
        }
    }
}
