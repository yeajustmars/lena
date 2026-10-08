//! Standard Universal Intermediate Representation (UIR) Vocabularies

// --- Identity & Classification ---
pub const ATTR_ENTITY_TYPE: &str = ":entity/type";
pub const ATTR_ENTITY_LANGUAGE: &str = ":entity/language";
pub const ATTR_SYMBOL_FQSN: &str = ":symbol/fqsn";
pub const ATTR_SYMBOL_NAMESPACE: &str = ":symbol/namespace";
pub const ATTR_SYMBOL_NAME: &str = ":symbol/name";
pub const ATTR_SYMBOL_KIND: &str = ":symbol/kind";
pub const ATTR_SYMBOL_VISIBILITY: &str = ":symbol/visibility";

// --- Code Payloads & Docs ---
pub const ATTR_CODE_RAW: &str = ":code/raw";
pub const ATTR_CODE_SIGNATURE: &str = ":code/signature";
pub const ATTR_CODE_HASH: &str = ":code/hash";
pub const ATTR_DOC_TEXT: &str = ":doc/text";

// --- Location Tracking ---
pub const ATTR_LOC_FILE: &str = ":location/file";
pub const ATTR_LOC_START_LINE: &str = ":location/start-line";
pub const ATTR_LOC_START_COL: &str = ":location/start-col";
pub const ATTR_LOC_END_LINE: &str = ":location/end-line";
pub const ATTR_LOC_END_COL: &str = ":location/end-col";

// --- Graph Edges ---
pub const ATTR_PARENT_ENTITY: &str = ":parent/entity";
pub const ATTR_CALL_CALLER: &str = ":call/caller";
pub const ATTR_CALL_CALLEE: &str = ":call/callee";
pub const ATTR_CALL_STATUS: &str = ":call/status";

// --- Standard Keyword Values ---
pub const TYPE_MODULE: &str = ":type/module";
pub const TYPE_FUNCTION: &str = ":type/function";
pub const TYPE_CALL_SITE: &str = ":type/call-site";
pub const TYPE_STRUCT: &str = ":type/struct";

pub const LANG_CLOJURE: &str = ":lang/clojure";
pub const LANG_RUST: &str = ":lang/rust";

pub const KIND_FN: &str = ":kind/fn";
pub const KIND_MACRO: &str = ":kind/macro";
pub const KIND_TYPE: &str = ":kind/type";

pub const VIS_PUBLIC: &str = ":vis/public";
pub const VIS_PRIVATE: &str = ":vis/private";

pub const CALL_STATUS_RESOLVED: &str = ":call.status/resolved";
pub const CALL_STATUS_AMBIGUOUS: &str = ":call.status/ambiguous";
