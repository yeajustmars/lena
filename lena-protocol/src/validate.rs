// lena-protocol/src/validate.rs

use crate::schema::{
    ATTR_CALL_CALLEE, ATTR_CALL_CALLER, ATTR_ENTITY_TYPE, ATTR_LOC_FILE, ATTR_SYMBOL_EXTERNAL,
    ATTR_SYMBOL_FQSN, ATTR_SYMBOL_MODULE, ATTR_SYMBOL_NAME, TYPE_CALL_SITE,
};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ValidationError {
    #[error("Missing mandatory attribute '{attr}' on entity {entity}")]
    MissingAttribute { entity: u64, attr: &'static str },
    #[error("File extension detected in module scope '{module}' on entity {entity}")]
    InvalidModuleScope { entity: u64, module: String },
    #[error("Delimiter detected in atomic symbol name '{name}' on entity {entity}")]
    InvalidSymbolName { entity: u64, name: String },
}

/// A lightweight representation of a fact for validation boundaries.
#[derive(Debug, Clone)]
pub struct ValidationFact<'a> {
    pub e: u64,
    pub ident: &'a str,
    pub value_str: &'a str,
}

#[derive(Debug, Default)]
pub struct FactValidator {
    pub is_permissive: bool,
}

impl FactValidator {
    #[must_use]
    pub fn new(is_permissive: bool) -> Self {
        Self { is_permissive }
    }

    /// Validates a batch of facts. In strict mode, returns the first error encountered.
    ///
    /// # Errors
    /// Returns `ValidationError` if the batch violates the protocol invariant in strict mode.
    pub fn validate_batch(&self, facts: &[ValidationFact<'_>]) -> Result<(), Vec<ValidationError>> {
        // Group attributes by entity ID
        let mut entity_map: HashMap<u64, HashMap<&str, &str>> = HashMap::new();

        for fact in facts {
            entity_map
                .entry(fact.e)
                .or_default()
                .insert(fact.ident, fact.value_str);
        }

        let mut errors = Vec::new();

        for (e, attrs) in &entity_map {
            let ent_type = attrs.get(ATTR_ENTITY_TYPE).copied().unwrap_or("");
            let is_external = attrs.get(ATTR_SYMBOL_EXTERNAL).is_some();

            if ent_type == TYPE_CALL_SITE {
                // Call sites must have both caller and callee
                if !attrs.contains_key(ATTR_CALL_CALLER) {
                    errors.push(ValidationError::MissingAttribute {
                        entity: *e,
                        attr: ATTR_CALL_CALLER,
                    });
                }
                if !attrs.contains_key(ATTR_CALL_CALLEE) {
                    errors.push(ValidationError::MissingAttribute {
                        entity: *e,
                        attr: ATTR_CALL_CALLEE,
                    });
                }
                continue;
            }

            // All non-call-site symbols must have an FQSN and a name
            if !attrs.contains_key(ATTR_SYMBOL_FQSN) {
                errors.push(ValidationError::MissingAttribute {
                    entity: *e,
                    attr: ATTR_SYMBOL_FQSN,
                });
            }
            if let Some(name) = attrs.get(ATTR_SYMBOL_NAME) {
                if name.contains('.') || name.contains("::") {
                    errors.push(ValidationError::InvalidSymbolName {
                        entity: *e,
                        name: (*name).to_string(),
                    });
                }
            } else {
                errors.push(ValidationError::MissingAttribute {
                    entity: *e,
                    attr: ATTR_SYMBOL_NAME,
                });
            }

            // Internal symbols must have module and file location
            if !is_external {
                if !attrs.contains_key(ATTR_LOC_FILE) {
                    errors.push(ValidationError::MissingAttribute {
                        entity: *e,
                        attr: ATTR_LOC_FILE,
                    });
                }
                if let Some(module) = attrs.get(ATTR_SYMBOL_MODULE) {
                    if module.ends_with(".rs")
                        || module.ends_with(".go")
                        || module.ends_with(".clj")
                    {
                        errors.push(ValidationError::InvalidModuleScope {
                            entity: *e,
                            module: (*module).to_string(),
                        });
                    }
                } else {
                    errors.push(ValidationError::MissingAttribute {
                        entity: *e,
                        attr: ATTR_SYMBOL_MODULE,
                    });
                }
            }
        }

        if !errors.is_empty() && !self.is_permissive {
            Err(errors)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_function_entity() {
        let validator = FactValidator::new(false);
        let facts = vec![
            ValidationFact {
                e: 1,
                ident: ATTR_ENTITY_TYPE,
                value_str: ":type/function",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_FQSN,
                value_str: "rust:core::run",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_NAME,
                value_str: "run",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_MODULE,
                value_str: "core",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_LOC_FILE,
                value_str: "src/main.rs",
            },
        ];

        assert!(validator.validate_batch(&facts).is_ok());
    }

    #[test]
    fn test_invalid_symbol_name_delimiter() {
        let validator = FactValidator::new(false);
        let facts = vec![
            ValidationFact {
                e: 1,
                ident: ATTR_ENTITY_TYPE,
                value_str: ":type/function",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_FQSN,
                value_str: "go:net/http.Server.Close",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_NAME,
                value_str: "Server.Close", // Invalid: contains delimiter
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_MODULE,
                value_str: "net/http",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_LOC_FILE,
                value_str: "http.go",
            },
        ];

        let result = validator.validate_batch(&facts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(
            errors[0],
            ValidationError::InvalidSymbolName { .. }
        ));
    }

    #[test]
    fn test_invalid_module_extension() {
        let validator = FactValidator::new(false);
        let facts = vec![
            ValidationFact {
                e: 1,
                ident: ATTR_ENTITY_TYPE,
                value_str: ":type/function",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_FQSN,
                value_str: "rust:main",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_NAME,
                value_str: "main",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_MODULE,
                value_str: "main.rs", // Invalid: contains file extension
            },
            ValidationFact {
                e: 1,
                ident: ATTR_LOC_FILE,
                value_str: "src/main.rs",
            },
        ];

        let result = validator.validate_batch(&facts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(
            errors[0],
            ValidationError::InvalidModuleScope { .. }
        ));
    }

    #[test]
    fn test_valid_external_stub() {
        let validator = FactValidator::new(false);
        let facts = vec![
            ValidationFact {
                e: 99,
                ident: ATTR_SYMBOL_FQSN,
                value_str: "go:fmt/Println",
            },
            ValidationFact {
                e: 99,
                ident: ATTR_SYMBOL_NAME,
                value_str: "Println",
            },
            ValidationFact {
                e: 99,
                ident: ATTR_SYMBOL_EXTERNAL,
                value_str: "true",
            },
            // Missing LOC_FILE and MODULE, but valid because it is external
        ];

        assert!(validator.validate_batch(&facts).is_ok());
    }

    #[test]
    fn test_permissive_mode() {
        let validator = FactValidator::new(true); // Permissive
        let facts = vec![
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_NAME,
                value_str: "Server.Close", // Invalid delimiter
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_MODULE,
                value_str: "http.go", // Invalid extension
            },
        ];

        // Should return Ok(()) despite protocol violations
        assert!(validator.validate_batch(&facts).is_ok());
    }

    #[test]
    fn test_invalid_call_site_missing_callee() {
        let validator = FactValidator::new(false);
        let facts = vec![
            ValidationFact {
                e: 10,
                ident: ATTR_ENTITY_TYPE,
                value_str: TYPE_CALL_SITE,
            },
            ValidationFact {
                e: 10,
                ident: ATTR_CALL_CALLER,
                value_str: "1",
            },
            // Missing ATTR_CALL_CALLEE
        ];

        let result = validator.validate_batch(&facts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(
            errors[0],
            ValidationError::MissingAttribute {
                entity: 10,
                attr: ATTR_CALL_CALLEE,
            }
        );
    }

    #[test]
    fn test_missing_mandatory_file_for_internal_symbol() {
        let validator = FactValidator::new(false);
        let facts = vec![
            ValidationFact {
                e: 1,
                ident: ATTR_ENTITY_TYPE,
                value_str: ":type/function",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_FQSN,
                value_str: "rust:core::run",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_NAME,
                value_str: "run",
            },
            ValidationFact {
                e: 1,
                ident: ATTR_SYMBOL_MODULE,
                value_str: "core",
            },
            // Missing ATTR_LOC_FILE
        ];

        let result = validator.validate_batch(&facts);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(
            errors[0],
            ValidationError::MissingAttribute {
                entity: 1,
                attr: ATTR_LOC_FILE,
            }
        );
    }
}
