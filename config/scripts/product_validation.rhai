fn check_product_match(crash_info, annotations) {
    let product_annotation = annotations["product"];
    if product_annotation == () {
        print("No product annotation found, skipping product validation");
        return validation_success();
    }

    let product = crash_info["product"];
    if product == () {
        print("No authorized product found, rejecting upload");
        return validation_error("access denied for product " + product_annotation);
    }

    if product_annotation != product {
        print("Product mismatch: annotation='" + product_annotation + "', authorized='" + product + "'");
        return validation_error("access denied for product " + product);
    }

    print("Product validation passed: " + product_annotation);
    return validation_success();
}

let annotations = crash_info["annotations"];
check_product_match(crash_info, annotations)
