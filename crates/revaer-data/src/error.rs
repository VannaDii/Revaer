//! Error types for the data access layer.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

/// Result alias for data layer operations.
pub type Result<T> = std::result::Result<T, DataError>;

/// Errors raised by the data access layer.
#[derive(Debug)]
pub enum DataError {
    /// Migration execution failed.
    MigrationFailed {
        /// Underlying migration error.
        source: sqlx::migrate::MigrateError,
    },
    /// A database operation failed.
    QueryFailed {
        /// Operation identifier.
        operation: &'static str,
        /// Underlying SQL error.
        source: sqlx::Error,
    },
    /// A path could not be represented as UTF-8.
    PathNotUtf8 {
        /// Field name that contained the invalid path.
        field: &'static str,
        /// Path value.
        path: PathBuf,
    },
}

impl Display for DataError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MigrationFailed { .. } => formatter.write_str("migration failed"),
            Self::QueryFailed { .. } => formatter.write_str("database operation failed"),
            Self::PathNotUtf8 { .. } => formatter.write_str("path contained invalid utf-8"),
        }
    }
}

impl Error for DataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MigrationFailed { source } => Some(source),
            Self::QueryFailed { source, .. } => Some(source),
            Self::PathNotUtf8 { .. } => None,
        }
    }
}

impl From<sqlx::Error> for DataError {
    fn from(source: sqlx::Error) -> Self {
        Self::QueryFailed {
            operation: "sqlx operation",
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_error_display_and_source() {
        let migration = DataError::MigrationFailed {
            source: sqlx::migrate::MigrateError::VersionMissing(1),
        };
        assert_eq!(migration.to_string(), "migration failed");
        assert!(migration.source().is_some());

        let query = DataError::QueryFailed {
            operation: "fetch",
            source: sqlx::Error::RowNotFound,
        };
        assert_eq!(query.to_string(), "database operation failed");
        assert!(query.source().is_some());

        let path = DataError::PathNotUtf8 {
            field: "root",
            path: PathBuf::from(".server_root/revaer"),
        };
        assert_eq!(path.to_string(), "path contained invalid utf-8");
        assert!(path.source().is_none());

        let from = DataError::from(sqlx::Error::RowNotFound);
        assert_eq!(from.to_string(), "database operation failed");
        assert!(from.source().is_some());
    }
}
