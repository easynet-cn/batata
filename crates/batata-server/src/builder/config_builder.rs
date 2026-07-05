//! Configuration builder
//!
//! This module provides a builder for loading and validating configuration.

use std::sync::Arc;

use crate::initializer::traits::ValidationError;
use crate::model::common::Configuration;

/// Configuration builder
///
/// Builds and validates the application configuration.
pub struct ConfigBuilder;

impl ConfigBuilder {
    /// Create a new configuration from environment and config files
    pub fn new() -> Result<Arc<Configuration>, Box<dyn std::error::Error + Send + Sync>> {
        let config = Configuration::new()?;
        Ok(Arc::new(config))
    }
    
    /// Validate the configuration
    pub fn validate(
        config: &Configuration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let errors = ConfigBuilder::validate_config(config);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Box::new(ValidationErrors::new(errors)))
        }
    }
    
    /// Validate configuration and return errors
    fn validate_config(config: &Configuration) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        
        // Check JWT secret when auth is enabled
        if config.auth_enabled() && config.token_secret_key().is_empty() {
            errors.push(ValidationError::new(
                "Authentication is enabled but no JWT secret key is configured",
            ).with_field("batata.core.auth.plugin.default.token.secret.key"));
        }
        
        // Check for default secret key
        const DEFAULT_SECRET_KEYS: &[&str] = &[
            "NzViOWFlNjYtMWM3MC00ZDYwLTg4OWUtMjYxYTdhMzA1Y2Jm",
            "VGhpc0lzTXlDdXN0b21TZWNyZXRLZXkwMTIzNDU2Nzg=",
        ];
        
        if config.auth_enabled() {
            let secret = config.token_secret_key();
            if !secret.is_empty() && DEFAULT_SECRET_KEYS.contains(&secret.as_str()) {
                tracing::warn!(
                    "Using the default JWT secret key. This is insecure for production!"
                );
            }
        }
        
        errors
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self
    }
}

/// Validation errors container
#[derive(Debug, Clone)]
pub struct ValidationErrors {
    errors: Vec<ValidationError>,
}

impl ValidationErrors {
    pub fn new(errors: Vec<ValidationError>) -> Self {
        Self { errors }
    }
    
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
    
    pub fn len(&self) -> usize {
        self.errors.len()
    }
    
    pub fn iter(&self) -> impl Iterator<Item = &ValidationError> {
        self.errors.iter()
    }
}

impl std::fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Validation errors ({}):\n", self.errors.len())?;
        for error in &self.errors {
            writeln!(f, "  - {}", error)?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

#[cfg(test)]
mod tests {
    use super::*;
    
    // We can't easily test ConfigBuilder without a full environment setup,
    // but we can test the ValidationErrors type
    
    #[test]
    fn test_validation_errors_empty() {
        let errors = ValidationErrors::new(Vec::new());
        assert!(errors.is_empty());
        assert_eq!(errors.len(), 0);
    }
    
    #[test]
    fn test_validation_errors_with_errors() {
        let errors = vec![
            ValidationError::new("Error 1"),
            ValidationError::new("Error 2").with_field("field_name"),
        ];
        let validation_errors = ValidationErrors::new(errors);
        
        assert!(!validation_errors.is_empty());
        assert_eq!(validation_errors.len(), 2);
    }
    
    #[test]
    fn test_validation_error_display() {
        let error = ValidationError::new("Test error message");
        assert_eq!(format!("{}", error), "Test error message");
        
        let error_with_field = ValidationError::new("Test error").with_field("my_field");
        assert_eq!(format!("{}", error_with_field), "Test error [field: my_field]");
    }
    
    #[test]
    fn test_validation_errors_display() {
        let errors = vec![
            ValidationError::new("Error 1"),
            ValidationError::new("Error 2").with_field("field"),
        ];
        let validation_errors = ValidationErrors::new(errors);
        
        let display = format!("{}", validation_errors);
        assert!(display.contains("Validation errors (2)"));
        assert!(display.contains("Error 1"));
        assert!(display.contains("Error 2"));
    }
}
