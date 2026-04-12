//! Cross-database column type helpers.
//!
//! MySQL uses LONGTEXT (4GB) for large text fields while PostgreSQL's TEXT
//! is already unlimited. This module provides helpers that emit the correct
//! DDL for each backend.

use sea_orm_migration::prelude::*;

/// Create a NOT NULL large-text column.
/// MySQL: LONGTEXT, PostgreSQL: TEXT.
pub fn long_text<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.custom(Alias::new("LONGTEXT")).not_null();
        }
        _ => {
            def.text().not_null();
        }
    }
    def.take()
}

/// Create a nullable large-text column.
/// MySQL: LONGTEXT, PostgreSQL: TEXT.
pub fn long_text_null<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.custom(Alias::new("LONGTEXT")).null();
        }
        _ => {
            def.text().null();
        }
    }
    def.take()
}
