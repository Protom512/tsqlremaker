//! Identifier types for SQL names.

/// A SQL identifier (table name, column name, alias, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier {
    value: String,
    quoted: bool,
}

impl Identifier {
    /// Creates a new unquoted identifier.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self {
            value,
            quoted: false,
        }
    }

    /// Creates a new quoted identifier.
    #[must_use]
    pub fn new_quoted(value: String) -> Self {
        Self {
            value,
            quoted: true,
        }
    }

    /// Returns the identifier value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns `true` if this identifier was quoted in the source.
    #[must_use]
    pub fn quoted(&self) -> bool {
        self.quoted
    }
}

/// A qualified name (e.g., `schema.table`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedName {
    schema: Option<String>,
    name: String,
}

impl QualifiedName {
    /// Creates a new qualified name.
    #[must_use]
    pub fn new(schema: Option<String>, name: String) -> Self {
        Self { schema, name }
    }

    /// Returns the schema name, if any.
    #[must_use]
    pub fn schema(&self) -> Option<&str> {
        self.schema.as_deref()
    }

    /// Returns the object name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A table alias with optional column aliases.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TableAlias {
    name: String,
    columns: Vec<String>,
}

impl TableAlias {
    /// Creates a new table alias.
    #[must_use]
    pub fn new(name: String, columns: Vec<String>) -> Self {
        Self { name, columns }
    }

    /// Returns the alias name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the column aliases.
    #[must_use]
    pub fn columns(&self) -> &[String] {
        &self.columns
    }
}
