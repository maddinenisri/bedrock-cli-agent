use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::env;
use bedrock_core::{BedrockError, Result};

// Regex for finding environment variable patterns with optional default values
// Supports both ${VAR} and ${VAR:-default}
static ENV_VAR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)(?::-([^}]*))?\}").expect("Invalid regex pattern")
});

/// Recursively substitute environment variables in a JSON value
pub fn substitute_env_vars(value: &mut Value) -> Result<()> {
    match value {
        Value::String(s) => {
            *s = substitute_in_string(s)?;
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                substitute_env_vars(v)?;
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                substitute_env_vars(v)?;
            }
        }
        _ => {} // Numbers, booleans, and null don't need substitution
    }
    Ok(())
}

/// Substitute environment variables in a single string
/// Supports ${VAR} and ${VAR:-default} patterns
fn substitute_in_string(input: &str) -> Result<String> {
    let mut result = input.to_string();
    let mut missing_vars = Vec::new();
    
    // Find all environment variable references
    for cap in ENV_VAR_REGEX.captures_iter(input) {
        let full_match = &cap[0];
        let var_name = &cap[1];
        let default_value = cap.get(2).map(|m| m.as_str());
        
        match env::var(var_name) {
            Ok(value) => {
                result = result.replace(full_match, &value);
            }
            Err(_) => {
                if let Some(default) = default_value {
                    result = result.replace(full_match, default);
                } else {
                    // Special handling for common variables
                    if var_name == "HOME" {
                        if let Ok(home) = env::var("HOME") {
                            result = result.replace(full_match, &home);
                        } else if let Ok(home) = env::var("USERPROFILE") {
                            // Windows fallback
                            result = result.replace(full_match, &home);
                        } else {
                            missing_vars.push(var_name.to_string());
                        }
                    } else {
                        missing_vars.push(var_name.to_string());
                    }
                }
            }
        }
    }
    
    // Report missing variables (only those without defaults)
    if !missing_vars.is_empty() {
        return Err(BedrockError::ConfigError(format!(
            "Missing required environment variables: {}. Please set these variables before loading the configuration.",
            missing_vars.join(", ")
        )));
    }
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_substitute_env_vars() {
        env::set_var("TEST_VAR", "test_value");
        
        let mut value = json!({
            "path": "${TEST_VAR}/some/path",
            "default": "${NON_EXISTENT:-default_value}",
            "nested": {
                "value": "${TEST_VAR}"
            }
        });
        
        substitute_env_vars(&mut value).unwrap();
        
        assert_eq!(value["path"], "test_value/some/path");
        assert_eq!(value["default"], "default_value");
        assert_eq!(value["nested"]["value"], "test_value");
        
        env::remove_var("TEST_VAR");
    }
}