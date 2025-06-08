// Test product specific validation script
// This script validates basic test product requirements

let product = crash_info.get("authorized_product");
if product != "TestProduct" {
    throw(`Expected product 'TestProduct', got '${product}'`);
}

// Ensure version format is correct for test product
let version = crash_info.get_annotation("version");
if version == () {
    throw("Missing version annotation");
}

// Test product should have a specific version pattern
if !version.contains(".") {
    throw("TestProduct version must contain dots (e.g., '1.0.0')");
}

print("Test product validation passed");
