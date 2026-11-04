// lena-protocol/src/schema.rs

//! Standard Universal Intermediate Representation (UIR) Vocabularies

// --- Identity & Classification ---
pub const ATTR_ENTITY_TYPE: &str = ":entity/type";
pub const ATTR_ENTITY_LANGUAGE: &str = ":entity/language";
pub const ATTR_SYMBOL_FQSN: &str = ":symbol/fqsn";
pub const ATTR_SYMBOL_NAME: &str = ":symbol/name";
pub const ATTR_SYMBOL_MODULE: &str = ":symbol/module";
pub const ATTR_SYMBOL_SUBMODULE: &str = ":symbol/submodule";
pub const ATTR_SYMBOL_CONTAINER: &str = ":symbol/container";
pub const ATTR_SYMBOL_KIND: &str = ":symbol/kind";
pub const ATTR_SYMBOL_VISIBILITY: &str = ":symbol/visibility";
pub const ATTR_SYMBOL_EXTERNAL: &str = ":symbol/external";

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

// --- Standard Entity Types ---
pub const TYPE_MODULE: &str = ":type/module";
pub const TYPE_FUNCTION: &str = ":type/function";
pub const TYPE_CALL_SITE: &str = ":type/call-site";
pub const TYPE_STRUCT: &str = ":type/struct";
pub const TYPE_ENUM: &str = ":type/enum";
pub const TYPE_TRAIT: &str = ":type/trait";
pub const TYPE_INTERFACE: &str = ":type/interface";

// --- Standard Symbol Kinds ---
pub const KIND_FN: &str = ":kind/fn";
pub const KIND_METHOD: &str = ":kind/method";
pub const KIND_MACRO: &str = ":kind/macro";
pub const KIND_STRUCT: &str = ":kind/struct";
pub const KIND_ENUM: &str = ":kind/enum";
pub const KIND_TRAIT: &str = ":kind/trait";
pub const KIND_INTERFACE: &str = ":kind/interface";
pub const KIND_TYPE: &str = ":kind/type";
pub const KIND_CONST: &str = ":kind/const";
pub const KIND_VAR: &str = ":kind/var";
pub const KIND_MODULE: &str = ":kind/module";

// --- Supported Languages ---
// Strictly defined to prevent adapter drift
pub const LANG_BASH: &str = ":lang/bash";
pub const LANG_C: &str = ":lang/c";
pub const LANG_CPP: &str = ":lang/c++";
pub const LANG_CSHARP: &str = ":lang/c#";
pub const LANG_CLOJURE: &str = ":lang/clojure";
pub const LANG_GO: &str = ":lang/go";
pub const LANG_JAVA: &str = ":lang/java";
pub const LANG_JAVASCRIPT: &str = ":lang/javascript";
pub const LANG_PHP: &str = ":lang/php";
pub const LANG_PYTHON: &str = ":lang/python";
pub const LANG_RUST: &str = ":lang/rust";
pub const LANG_TYPESCRIPT: &str = ":lang/typescript";

// --- Standard Status & Visibility ---
pub const VIS_PUBLIC: &str = ":vis/public";
pub const VIS_PRIVATE: &str = ":vis/private";
pub const CALL_STATUS_RESOLVED: &str = ":call.status/resolved";
pub const CALL_STATUS_AMBIGUOUS: &str = ":call.status/ambiguous";
