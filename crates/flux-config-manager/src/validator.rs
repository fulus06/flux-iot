use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// 校验错误
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Required field missing: {0}")]
    RequiredFieldMissing(String),
    
    #[error("Value out of range: {field} = {value}, expected {min}..{max}")]
    OutOfRange {
        field: String,
        value: String,
        min: String,
        max: String,
    },
    
    #[error("Invalid format: {field} = {value}, reason: {reason}")]
    InvalidFormat {
        field: String,
        value: String,
        reason: String,
    },
    
    #[error("Conflict detected: {0}")]
    Conflict(String),
    
    #[error("Custom validation failed: {0}")]
    Custom(String),
}

/// 校验规则
pub trait ValidationRule<T>: Send + Sync {
    fn validate(&self, config: &T) -> Result<(), ValidationError>;
    fn name(&self) -> &str;
}

/// 配置校验器
pub struct ConfigValidator<T> {
    rules: Vec<Box<dyn ValidationRule<T>>>,
}

impl<T> ConfigValidator<T> {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// 添加校验规则
    pub fn add_rule(&mut self, rule: Box<dyn ValidationRule<T>>) {
        self.rules.push(rule);
    }

    /// 执行所有校验规则
    pub fn validate(&self, config: &T) -> Result<()> {
        let mut errors = Vec::new();

        for rule in &self.rules {
            if let Err(e) = rule.validate(config) {
                errors.push(format!("[{}] {}", rule.name(), e));
            }
        }

        if !errors.is_empty() {
            return Err(anyhow::anyhow!(
                "Validation failed:\n{}",
                errors.join("\n")
            ));
        }

        Ok(())
    }

    /// 获取规则数量
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl<T> Default for ConfigValidator<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// 范围校验规则（用于数值类型）
pub struct RangeRule<T, F>
where
    F: Fn(&T) -> i64 + Send + Sync,
{
    name: String,
    field_name: String,
    extractor: F,
    min: i64,
    max: i64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F> RangeRule<T, F>
where
    F: Fn(&T) -> i64 + Send + Sync,
{
    pub fn new(name: String, field_name: String, extractor: F, min: i64, max: i64) -> Self {
        Self {
            name,
            field_name,
            extractor,
            min,
            max,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, F> ValidationRule<T> for RangeRule<T, F>
where
    T: Send + Sync,
    F: Fn(&T) -> i64 + Send + Sync,
{
    fn validate(&self, config: &T) -> Result<(), ValidationError> {
        let value = (self.extractor)(config);
        
        if value < self.min || value > self.max {
            return Err(ValidationError::OutOfRange {
                field: self.field_name.clone(),
                value: value.to_string(),
                min: self.min.to_string(),
                max: self.max.to_string(),
            });
        }
        
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 自定义校验规则
pub struct CustomRule<T, F>
where
    F: Fn(&T) -> Result<(), ValidationError> + Send + Sync,
{
    name: String,
    validator: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F> CustomRule<T, F>
where
    F: Fn(&T) -> Result<(), ValidationError> + Send + Sync,
{
    pub fn new(name: String, validator: F) -> Self {
        Self {
            name,
            validator,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, F> ValidationRule<T> for CustomRule<T, F>
where
    T: Send + Sync,
    F: Fn(&T) -> Result<(), ValidationError> + Send + Sync,
{
    fn validate(&self, config: &T) -> Result<(), ValidationError> {
        (self.validator)(config)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestConfig {
        port: u16,
        max_connections: i32,
        name: String,
    }

    #[test]
    fn test_range_rule() {
        let rule = RangeRule::new(
            "port_range".to_string(),
            "port".to_string(),
            |c: &TestConfig| c.port as i64,
            1024,
            65535,
        );

        let valid_config = TestConfig {
            port: 8080,
            max_connections: 100,
            name: "test".to_string(),
        };

        assert!(rule.validate(&valid_config).is_ok());

        let invalid_config = TestConfig {
            port: 80,
            max_connections: 100,
            name: "test".to_string(),
        };

        assert!(rule.validate(&invalid_config).is_err());
    }

    #[test]
    fn test_custom_rule() {
        let rule = CustomRule::new(
            "name_not_empty".to_string(),
            |c: &TestConfig| {
                if c.name.is_empty() {
                    Err(ValidationError::Custom("Name cannot be empty".to_string()))
                } else {
                    Ok(())
                }
            },
        );

        let valid_config = TestConfig {
            port: 8080,
            max_connections: 100,
            name: "test".to_string(),
        };

        assert!(rule.validate(&valid_config).is_ok());

        let invalid_config = TestConfig {
            port: 8080,
            max_connections: 100,
            name: "".to_string(),
        };

        assert!(rule.validate(&invalid_config).is_err());
    }

    #[test]
    fn test_validator() {
        let mut validator = ConfigValidator::new();

        validator.add_rule(Box::new(RangeRule::new(
            "port_range".to_string(),
            "port".to_string(),
            |c: &TestConfig| c.port as i64,
            1024,
            65535,
        )));

        validator.add_rule(Box::new(CustomRule::new(
            "name_not_empty".to_string(),
            |c: &TestConfig| {
                if c.name.is_empty() {
                    Err(ValidationError::Custom("Name cannot be empty".to_string()))
                } else {
                    Ok(())
                }
            },
        )));

        let valid_config = TestConfig {
            port: 8080,
            max_connections: 100,
            name: "test".to_string(),
        };

        assert!(validator.validate(&valid_config).is_ok());

        let invalid_config = TestConfig {
            port: 80,
            max_connections: 100,
            name: "".to_string(),
        };

        assert!(validator.validate(&invalid_config).is_err());
    }
}
