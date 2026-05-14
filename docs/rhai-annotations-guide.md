
# Rhai Annotation System - Complete Guide

## Overview

Guardrail provides a powerful and intuitive system for working with crash annotations in Rhai validation scripts. Scripts can read, modify, add, and remove annotations with automatic source tracking.

## Quick Start

```rhai
// Access annotations directly from crash data
let annotations = crash_info["annotations"];

// Read existing annotations
let product = annotations["product"];

// Modify existing annotations (source becomes 'script')
annotations["product"] = product + "-verified";

// Add new annotations (source is 'script')
annotations["processed_by"] = "validation_script";
annotations.add("timestamp", timestamp().to_string());

// Remove unwanted annotations
annotations.remove("debug_temp");

// Return success - changes are automatically applied
validation_success()
```

## API Reference

### Annotation Access

**Single access point:** `crash_info["annotations"]` provides full read-write access to all annotations.

### Built-in Rhai Operations

```rhai
let annotations = crash_info["annotations"];

// Core operations
annotations["key"] = "value";        // Direct assignment
let value = annotations["key"];      // Direct access
let count = annotations.len();       // Get size
let empty = annotations.is_empty();  // Check if empty
let keys = annotations.keys();       // Get all keys
let values = annotations.values();   // Get all values
```

### Custom Convenience Methods

| Method            | Description                                | Example                                  |
| ----------------- | ------------------------------------------ | ---------------------------------------- |
| `add(key, value)` | Add annotation (more descriptive than `=`) | `annotations.add("status", "processed")` |
| `contains(key)`   | Check if key exists                        | `if annotations.contains("product")`     |
| `get(key)`        | Safe access with `()` fallback             | `let val = annotations.get("version")`   |
| `remove(key)`     | Remove and return value                    | `let old = annotations.remove("temp")`   |

### Validation Functions

```rhai
validation_success()           // Return success
validation_error("message")    // Return error with message
```

## Smart Source Tracking

The system automatically manages the 'source' field for annotations:

- **Unchanged annotations**: Keep their original source (`submission`, `user`, etc.)
- **Modified annotations**: Source automatically becomes `script`
- **New annotations**: Source is `script`
- **Removed annotations**: Completely removed from the crash

## Examples

### Basic Usage

```rhai
let annotations = crash_info["annotations"];

// Add processing metadata
annotations["processed_by"] = "validation_script";
annotations.add("processing_time", timestamp().to_string());

// Conditional logic
if annotations.contains("product") {
    let product = annotations["product"];
    annotations["product_category"] = if product == "myapp" { "internal" } else { "external" };
}

validation_success()
```

### Advanced Pattern Matching

```rhai
let annotations = crash_info["annotations"];

// Version-based categorization
let version = annotations.get("version");
if version != () {
    if version.starts_with("1.") {
        annotations["version_family"] = "legacy";
        annotations["support_tier"] = "limited";
    } else if version.starts_with("2.") {
        annotations["version_family"] = "current";
        annotations["support_tier"] = "full";
    }
}

// Build age analysis
let build_time = crash_info["build_time"];
if build_time != () {
    let age_days = (timestamp() - build_time) / 86400;
    annotations["build_age_days"] = age_days.to_string();
    annotations["build_age_category"] = if age_days < 30 { "recent" } else { "old" };
}

validation_success()
```

### Data Cleanup

```rhai
let annotations = crash_info["annotations"];

// Remove debug/temporary annotations
let debug_keys = ["debug_temp", "internal_id", "test_marker"];
for key in debug_keys {
    if annotations.contains(key) {
        annotations.remove(key);
    }
}

// Normalize product names
let product = annotations.get("product");
if product != () {
    let normalized = product.to_lower().replace(" ", "_");
    annotations["product"] = normalized;
    annotations["product_normalized"] = "true";
}

validation_success()
```

## Example Scripts

The following example scripts demonstrate different aspects of the annotation system:

### Available Scripts in `config/scripts/`

- **`product_validation.rhai`** - Production validation script for product authorization
- **`build_age_validation.rhai`** - Production validation script for build age limits
- **`example_validation.rhai`** - Complete example showing annotation analysis and enhancement
- **`comprehensive_example.rhai`** - Comprehensive demo of all annotation features

### Running Examples

To test the annotation system with example scripts:

```bash
# Use any validation script with crash data
# The scripts will automatically enhance annotations based on crash content
```

## Migration Guide

### From Legacy Return-Based Approach

**Old approach (no longer supported):**

```rhai
let script_annotations = create_annotations();
script_annotations.add("key", "value");
return validation_success_with_annotations(script_annotations);
```

**New approach:**

```rhai
let annotations = crash_info["annotations"];
annotations["key"] = "value";
return validation_success();
```

### Key Changes

1. **No separate annotation creation**: Use `crash_info["annotations"]` directly
2. **No return-based annotations**: Modify annotations in place
3. **Simplified validation**: Just call `validation_success()`
4. **Automatic source tracking**: No manual source management needed

## Best Practices

1. **Use descriptive keys**: `annotations["processing_status"]` vs `annotations["status"]`
2. **Leverage built-ins**: Use `annotations.len()` instead of custom `size()` methods
3. **Safe access**: Use `annotations.get("key")` when the key might not exist
4. **Conditional logic**: Check existence with `annotations.contains("key")`
5. **Clean up**: Remove unnecessary debug/temporary annotations

## Implementation Details

- **Location**: Primary logic in `/crates/api/src/minidump.rs`
- **Initialization**: Annotations from `crash_info_map` are made available as read-write
- **Post-processing**: System compares final vs original state to assign sources
- **Performance**: Leverages Rhai's optimized built-in map operations
- **Safety**: Unchanged annotations preserve their original provenance

The annotation system provides a **natural, powerful, and safe** way to enhance crash analysis through scripting!
