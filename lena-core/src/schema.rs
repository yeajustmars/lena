// lena-core/src/schema.rs

//! Schema bootstrapping for Lena.
//! Maps the UIR vocabulary defined in `lena-protocol` to `mesodb_core::schema::SchemaMap`.

use lena_protocol::schema::{
    ATTR_CALL_CALLEE, ATTR_CALL_CALLER, ATTR_CALL_STATUS, ATTR_CODE_HASH, ATTR_CODE_RAW,
    ATTR_CODE_SIGNATURE, ATTR_DOC_TEXT, ATTR_ENTITY_LANGUAGE, ATTR_ENTITY_TYPE, ATTR_LOC_END_COL,
    ATTR_LOC_END_LINE, ATTR_LOC_FILE, ATTR_LOC_START_COL, ATTR_LOC_START_LINE, ATTR_PARENT_ENTITY,
    ATTR_SYMBOL_CONTAINER, ATTR_SYMBOL_EXTERNAL, ATTR_SYMBOL_FQSN, ATTR_SYMBOL_KIND,
    ATTR_SYMBOL_MODULE, ATTR_SYMBOL_NAME, ATTR_SYMBOL_SUBMODULE, ATTR_SYMBOL_VISIBILITY,
};
use mesodb_core::schema::{SchemaMap, ValueType};

/// Bootstraps the complete Universal Intermediate Representation (UIR) schema.
#[must_use]
pub fn bootstrap() -> SchemaMap {
    let mut schema = SchemaMap::new();

    // Identity & Classification
    schema.add_attribute(ATTR_ENTITY_TYPE, ValueType::String, false);
    schema.add_attribute(ATTR_ENTITY_LANGUAGE, ValueType::String, false);
    schema.add_attribute(ATTR_SYMBOL_FQSN, ValueType::String, true); // Unique identifier
    schema.add_attribute(ATTR_SYMBOL_MODULE, ValueType::String, false);
    schema.add_attribute(ATTR_SYMBOL_SUBMODULE, ValueType::String, false);
    schema.add_attribute(ATTR_SYMBOL_CONTAINER, ValueType::String, false);
    schema.add_attribute(ATTR_SYMBOL_NAME, ValueType::String, false);
    schema.add_attribute(ATTR_SYMBOL_KIND, ValueType::String, false);
    schema.add_attribute(ATTR_SYMBOL_VISIBILITY, ValueType::String, false);
    schema.add_attribute(ATTR_SYMBOL_EXTERNAL, ValueType::Boolean, false);

    // Code Payloads & Docs
    schema.add_attribute(ATTR_CODE_RAW, ValueType::String, false);
    schema.add_attribute(ATTR_CODE_SIGNATURE, ValueType::String, false);
    schema.add_attribute(ATTR_CODE_HASH, ValueType::String, false);
    schema.add_attribute(ATTR_DOC_TEXT, ValueType::String, false);

    // Location Tracking
    schema.add_attribute(ATTR_LOC_FILE, ValueType::String, false);
    schema.add_attribute(ATTR_LOC_START_LINE, ValueType::Int64, false);
    schema.add_attribute(ATTR_LOC_START_COL, ValueType::Int64, false);
    schema.add_attribute(ATTR_LOC_END_LINE, ValueType::Int64, false);
    schema.add_attribute(ATTR_LOC_END_COL, ValueType::Int64, false);

    // Graph Edges
    schema.add_attribute(ATTR_PARENT_ENTITY, ValueType::Ref, false);
    schema.add_attribute(ATTR_CALL_CALLER, ValueType::Ref, false);
    schema.add_attribute(ATTR_CALL_CALLEE, ValueType::Ref, false);
    schema.add_attribute(ATTR_CALL_STATUS, ValueType::String, false);

    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_bootstrap_contains_all_attributes() {
        let schema = bootstrap();

        let fqsn = schema.get_by_ident(ATTR_SYMBOL_FQSN).expect("Missing FQSN");
        assert_eq!(fqsn.value_type, ValueType::String);
        assert!(fqsn.is_unique, "FQSN must be a unique attribute");

        let callee = schema
            .get_by_ident(ATTR_CALL_CALLEE)
            .expect("Missing Callee");
        assert_eq!(callee.value_type, ValueType::Ref);
        assert!(!callee.is_unique);
    }

    #[test]
    fn test_bootstrap_identity_attributes() {
        let schema = bootstrap();

        let module = schema
            .get_by_ident(ATTR_SYMBOL_MODULE)
            .expect("Missing Module");
        assert_eq!(module.value_type, ValueType::String);
        assert!(!module.is_unique);

        let external = schema
            .get_by_ident(ATTR_SYMBOL_EXTERNAL)
            .expect("Missing External flag");
        assert_eq!(external.value_type, ValueType::Boolean);
    }

    #[test]
    fn test_bootstrap_submodule_and_container_attributes() {
        let schema = bootstrap();

        let submodule = schema
            .get_by_ident(ATTR_SYMBOL_SUBMODULE)
            .expect("Missing Submodule attribute");
        assert_eq!(submodule.value_type, ValueType::String);
        assert!(!submodule.is_unique);

        let container = schema
            .get_by_ident(ATTR_SYMBOL_CONTAINER)
            .expect("Missing Container attribute");
        assert_eq!(container.value_type, ValueType::String);
        assert!(!container.is_unique);
    }

    #[test]
    fn test_bootstrap_external_attribute_properties() {
        let schema = bootstrap();

        let external = schema
            .get_by_ident(ATTR_SYMBOL_EXTERNAL)
            .expect("Missing External attribute");
        assert_eq!(external.value_type, ValueType::Boolean);
        assert!(!external.is_unique);
    }
}
