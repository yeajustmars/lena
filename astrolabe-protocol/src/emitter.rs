//! ND-EDN Emitter for Astrolabe Adapters.
//! Ensures adapters emit compliant EDN without depending on the full mesodb-core runtime.

use std::fmt::{self, Write};

#[derive(Debug, Clone, PartialEq)]
pub enum EdnValue<'a> {
    String(&'a str),
    Int(i64),
    Float(f64),
    Bool(bool),
    Ref(u64),
    Keyword(&'a str),
}

impl<'a> fmt::Display for EdnValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(s) => {
                f.write_char('"')?;
                for c in s.chars() {
                    match c {
                        '"' => f.write_str("\\\"")?,
                        '\\' => f.write_str("\\\\")?,
                        '\n' => f.write_str("\\n")?,
                        '\r' => f.write_str("\\r")?,
                        '\t' => f.write_str("\\t")?,
                        _ => f.write_char(c)?,
                    }
                }
                f.write_char('"')
            }
            Self::Keyword(k) => {
                // Ensure keywords have exactly one leading colon and are valid strings.
                if k.starts_with(':') {
                    f.write_str(k)
                } else {
                    f.write_char(':')?;
                    f.write_str(k)
                }
            }
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(fl) => write!(f, "{fl}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Ref(r) => write!(f, "{r}"),
        }
    }
}

/// Accumulates facts for a single logical transaction and formats them as a valid EDN vector.
#[derive(Debug, Default)]
pub struct TransactionBuilder {
    buffer: String,
    fact_count: usize,
}

impl TransactionBuilder {
    #[must_use]
    pub fn new() -> Self {
        let mut builder = Self {
            buffer: String::with_capacity(1024),
            fact_count: 0,
        };
        builder.buffer.push('[');
        builder
    }

    /// Adds an assertion fact (`:db/add`) to the transaction.
    ///
    /// # Errors
    /// Returns `fmt::Error` if string formatting fails.
    pub fn add(
        &mut self,
        entity: u64,
        attribute: &str,
        value: EdnValue,
    ) -> Result<&mut Self, fmt::Error> {
        self.push_fact(":db/add", entity, attribute, value)?;
        Ok(self)
    }

    /// Adds a retraction fact (`:db/retract`) to the transaction.
    ///
    /// # Errors
    /// Returns `fmt::Error` if string formatting fails.
    pub fn retract(
        &mut self,
        entity: u64,
        attribute: &str,
        value: EdnValue,
    ) -> Result<&mut Self, fmt::Error> {
        self.push_fact(":db/retract", entity, attribute, value)?;
        Ok(self)
    }

    fn push_fact(
        &mut self,
        op: &str,
        entity: u64,
        attribute: &str,
        value: EdnValue,
    ) -> Result<(), fmt::Error> {
        if self.fact_count > 0 {
            self.buffer.push(' ');
        }
        write!(self.buffer, "[{op} {entity} {attribute} {value}]")?;
        self.fact_count += 1;
        Ok(())
    }

    /// Returns the finalized ND-EDN transaction string.
    #[must_use]
    pub fn finish(mut self) -> String {
        self.buffer.push(']');
        self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edn_value_formatting() {
        assert_eq!(EdnValue::Int(42).to_string(), "42");
        assert_eq!(EdnValue::Bool(true).to_string(), "true");
        assert_eq!(
            EdnValue::Keyword(":type/function").to_string(),
            ":type/function"
        );
        assert_eq!(
            EdnValue::Keyword("type/function").to_string(),
            ":type/function"
        );
        assert_eq!(EdnValue::Ref(999).to_string(), "999");

        let s = EdnValue::String("Line 1\nLine 2 with \"quotes\"");
        assert_eq!(s.to_string(), r#""Line 1\nLine 2 with \"quotes\"""#);
    }

    #[test]
    fn test_transaction_builder() {
        let mut tx = TransactionBuilder::new();
        tx.add(1001, ":entity/type", EdnValue::Keyword(":type/function"))
            .unwrap();
        tx.add(1001, ":symbol/name", EdnValue::String("process_order"))
            .unwrap();

        let result = tx.finish();
        assert_eq!(
            result,
            r#"[[:db/add 1001 :entity/type :type/function] [:db/add 1001 :symbol/name "process_order"]]"#
        );
    }
}
