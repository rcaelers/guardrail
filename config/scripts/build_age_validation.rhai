fn check_build_age(annotations) {
    let build_date_str = annotations["build_date"];
    if build_date_str == () {
        print("No build_date annotation found, allowing upload");
        return validation_success();
    }

    let build_timestamp = parse_iso8601(build_date_str);
    if build_timestamp == () {
        print("Invalid build_date format: " + build_date_str + ", allowing upload");
        return validation_success();
    }

    let channel = annotations["channel"];
    let max_age_days;

    if channel == "release" || channel == () {
        max_age_days = 2 * 365;
        print("Using 2-year limit for " + (if channel == () { "unset" } else { channel } ) + " channel");
    } else {
        max_age_days = 6 * 30;
        print("Using 6-month limit for " + channel + " channel");
    }

    let now = timestamp();
    let max_age_ago = now - (max_age_days * 24 * 60 * 60);

    debug("Checking build timestamp " + build_timestamp + " against threshold " + max_age_ago);

    if build_timestamp < max_age_ago {
        let error_msg = if channel == "release" || channel == () {
            "Release build is older than 2 years"
        } else {
            "Build is older than 6 months"
        };
        print("Build validation failed: " + error_msg);
        return validation_error(error_msg);
    }

    print("Build age validation passed");
    return validation_success();
}

let annotations = crash_info["annotations"];
check_build_age(annotations);
