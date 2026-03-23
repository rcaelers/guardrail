use std::sync::Arc;

use common::settings::ValidationScript;
use tracing::error;

use crate::error::ApiError;

#[derive(Debug, Clone)]
pub enum CompiledValidationScript {
    Global(String),
    ProductSpecific {
        pattern: String,
        regex: Arc<fancy_regex::Regex>,
        script: String,
    },
}

impl CompiledValidationScript {
    pub fn compile(script: &ValidationScript) -> Result<Self, ApiError> {
        match script {
            ValidationScript::Global(script_file) => {
                Ok(CompiledValidationScript::Global(script_file.clone()))
            }
            ValidationScript::ProductSpecific { product, script } => {
                let regex = fancy_regex::Regex::new(product).map_err(|e| {
                    error!(
                        product_pattern = %product,
                        error = %e,
                        "Invalid regex pattern in product validation script configuration"
                    );
                    ApiError::Failure(format!(
                        "Invalid regex pattern '{product}' in validation script configuration: {e}"
                    ))
                })?;
                Ok(CompiledValidationScript::ProductSpecific {
                    pattern: product.clone(),
                    regex: Arc::new(regex),
                    script: script.clone(),
                })
            }
        }
    }

    pub fn compile_all(scripts: &[ValidationScript]) -> Result<Vec<Self>, ApiError> {
        scripts.iter().map(Self::compile).collect()
    }
}
