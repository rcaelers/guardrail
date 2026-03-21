// Example validation script demonstrating modern annotation API
// Shows direct annotation access, modification, and validation patterns

fn analyze_crash(crash_info) {
    print("=== Crash Analysis Example ===");

    let annotations = crash_info["annotations"];

    // Add processing metadata
    annotations["script_processed"] = "true";
    annotations["processing_timestamp"] = timestamp().to_string();

    // Product analysis
    let product = annotations["product"];
    if product != () {
        print("Found product: " + product);

        // Enhance product information
        if product == "myapp" {
            annotations["product_category"] = "desktop";
            annotations["priority"] = "high";
        } else {
            annotations["product_category"] = "unknown";
            annotations["priority"] = "normal";
        }

        annotations.add("product_verified", "true");
    } else {
        annotations.add("product", "unknown");
        annotations.add("product_verified", "false");
        print("No product found, marked as unknown");
    }

    // Version analysis
    let version = annotations["version"];
    if version != () {
        if version.starts_with("1.") {
            annotations.add("version_family", "legacy");
        } else {
            annotations.add("version_family", "current");
        }
        print("Version categorized: " + version);
    }

    // Calculate risk score based on available data
    let risk_score = 0;
    if annotations["product_category"] == "unknown" { risk_score = risk_score + 2; }
    if annotations["version_family"] == "legacy" { risk_score = risk_score + 1; }

    annotations.add("risk_score", risk_score.to_string());
    annotations.add("analysis_complete", "true");

    print("Analysis complete. Risk score: " + risk_score);
    return validation_success();
}
