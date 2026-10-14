//! Schema bootstrapping for Lena.
//! Maps the UIR vocabulary defined in `lena-protocol` to `mesodb_core::schema::SchemaMap`.

use lena_protocol::schema::{
    ATTR_CALL_CALLEE, ATTR_CALL_CALLER, ATTR_CALL_STATUS, ATTR_CODE_HASH, ATTR_CODE_RAW,
    ATTR_CODE_SIGNATURE, ATTR_DOC_TEXT, ATTR_ENTITY_LANGUAGE, ATTR_ENTITY_TYPE, ATTR_LOC_END_COL,
    ATTR_LOC_END_LINE, ATTR_LOC_FILE, ATTR_LOC_START_COL, ATTR_LOC_START_LINE, ATTR_PARENT_ENTITY,
    ATTR_SYMBOL_FQSN, ATTR_SYMBOL_KIND, ATTR_SYMBOL_NAME, ATTR_SYMBOL_NAMESPACE,
    ATTR_SYMBOL_VISIBILITY,
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
    schema.add_attribute(ATTR_SYMBOL_NAMESPACE, ValueType::String, false);
    schema.add_attribute(ATTR_SYMBOL_NAME, ValueType::String, false);
    schema.add_attribute(ATTR_SYMBOL_KIND, ValueType::String, false);
    schema.add_attribute(ATTR_SYMBOL_VISIBILITY, ValueType::String, false);

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

        // Check a few critical attributes to ensure types and uniqueness are mapped properly
        let fqsn = schema.get_by_ident(ATTR_SYMBOL_FQSN).expect("Missing FQSN");
        assert_eq!(fqsn.value_type, ValueType::String);
        assert!(fqsn.is_unique, "FQSN must be a unique attribute");

        let callee = schema
            .get_by_ident(ATTR_CALL_CALLEE)
            .expect("Missing Callee");
        assert_eq!(callee.value_type, ValueType::Ref);
        assert!(!callee.is_unique);

        let start_line = schema
            .get_by_ident(ATTR_LOC_START_LINE)
            .expect("Missing Start Line");
        assert_eq!(start_line.value_type, ValueType::Int64);
    }

    #[test]
    fn test_bootstrap_identity_attributes() {
        let schema = bootstrap();

        // FQSN must be a String and MUST be unique
        let fqsn = schema.get_by_ident(ATTR_SYMBOL_FQSN).expect("Missing FQSN");
        assert_eq!(fqsn.value_type, ValueType::String);
        assert!(fqsn.is_unique, "FQSN must be unique");

        // Entity type is a string but not unique
        let ent_type = schema
            .get_by_ident(ATTR_ENTITY_TYPE)
            .expect("Missing entity type");
        assert_eq!(ent_type.value_type, ValueType::String);
        assert!(!ent_type.is_unique);
    }

    #[test]
    fn test_bootstrap_code_payload_attributes() {
        let schema = bootstrap();

        let raw = schema
            .get_by_ident(ATTR_CODE_RAW)
            .expect("Missing raw code");
        assert_eq!(raw.value_type, ValueType::String);
        assert!(!raw.is_unique);

        let hash = schema
            .get_by_ident(ATTR_CODE_HASH)
            .expect("Missing code hash");
        assert_eq!(hash.value_type, ValueType::String);
        assert!(!hash.is_unique);
    }

    #[test]
    fn test_bootstrap_location_attributes() {
        let schema = bootstrap();

        let start_line = schema
            .get_by_ident(ATTR_LOC_START_LINE)
            .expect("Missing start line");
        assert_eq!(start_line.value_type, ValueType::Int64);
        assert!(!start_line.is_unique);

        let end_col = schema
            .get_by_ident(ATTR_LOC_END_COL)
            .expect("Missing end col");
        assert_eq!(end_col.value_type, ValueType::Int64);
        assert!(!end_col.is_unique);
    }

    #[test]
    fn test_bootstrap_graph_edge_attributes() {
        let schema = bootstrap();

        let parent = schema
            .get_by_ident(ATTR_PARENT_ENTITY)
            .expect("Missing parent entity");
        assert_eq!(parent.value_type, ValueType::Ref);
        assert!(!parent.is_unique);

        let caller = schema
            .get_by_ident(ATTR_CALL_CALLER)
            .expect("Missing call caller");
        assert_eq!(caller.value_type, ValueType::Ref);
        assert!(!caller.is_unique);
    }

    #[test]
    fn test_bootstrap_unknown_attribute() {
        let schema = bootstrap();

        // Querying an attribute that was not bootstrapped should safely return None
        let missing = schema.get_by_ident(":missing/attribute");
        assert!(missing.is_none());
    }
}
