use sea_orm_migration::prelude::*;

/// Performs the `long_text` operation.
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

/// Performs the `long_text_null` operation.
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

/// Performs the `tiny_int` operation.
pub fn tiny_int<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.tiny_integer().not_null();
        }
        _ => {
            def.small_integer().not_null();
        }
    }
    def.take()
}

/// Performs the `unsigned_tiny_int` operation.
pub fn unsigned_tiny_int<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.tiny_integer().not_null();
        }
        _ => {
            def.small_integer().not_null();
        }
    }
    def.take()
}

/// Performs the `tiny_int_null` operation.
pub fn tiny_int_null<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.tiny_integer().null();
        }
        _ => {
            def.small_integer().null();
        }
    }
    def.take()
}

/// Performs the `signed_int` operation.
///
/// A true 4-byte `INTEGER` on both backends for `i32` semantic fields
/// (enum-ish columns such as `role_type`, `permission_type`, `line_num`).
/// ID columns must use [`unsigned_int`] (now `BIGINT`) instead — the entity
/// layer widened all primary/foreign keys to `i64`, and PostgreSQL rejects
/// decoding `INT8` into `i32`.
pub fn signed_int<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.integer().not_null();
        }
        _ => {
            def.integer().not_null();
        }
    }
    def.take()
}

/// Nullable counterpart of [`signed_int`].
pub fn signed_int_null<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.integer().null();
        }
        _ => {
            def.integer().null();
        }
    }
    def.take()
}

/// Performs the `unsigned_int` operation.
///
/// Despite the MySQL-inherited name, this maps to `BIGINT` on both backends:
/// all Apollo primary keys and integer foreign keys are `i64` in the entity
/// layer, and PostgreSQL has no `UNSIGNED` integer types at all. Emitting a
/// 4-byte `INTEGER` here would overflow once ids exceed `i32::MAX`.
pub fn unsigned_int<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.big_integer().not_null();
        }
        _ => {
            def.big_integer().not_null();
        }
    }
    def.take()
}

/// Performs the `unsigned_int_null` operation.
///
/// Nullable counterpart of [`unsigned_int`]; also `BIGINT`.
pub fn unsigned_int_null<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.big_integer().null();
        }
        _ => {
            def.big_integer().null();
        }
    }
    def.take()
}

/// Performs the `unsigned_big_int` operation.
pub fn unsigned_big_int<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.big_integer().not_null();
        }
        _ => {
            def.big_integer().not_null();
        }
    }
    def.take()
}

/// Performs the `unsigned_big_int_null` operation.
pub fn unsigned_big_int_null<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.big_integer().null();
        }
        _ => {
            def.big_integer().null();
        }
    }
    def.take()
}

/// Performs the `bit` operation.
pub fn bit<T: IntoIden>(col: T, _length: Option<u32>) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.boolean().not_null().default(false);
    def.take()
}

/// Performs the `string_len` operation.
pub fn string_len<T: IntoIden>(col: T, len: u32) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.string_len(len).not_null();
    def.take()
}

/// Performs the `string_len_null` operation.
pub fn string_len_null<T: IntoIden>(col: T, len: u32) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.string_len(len).null();
    def.take()
}

/// Performs the `string_len_default` operation.
pub fn string_len_default<T: IntoIden>(col: T, len: u32, default: &str) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.string_len(len).not_null().default(default);
    def.take()
}

/// Performs the `char_len` operation.
pub fn char_len<T: IntoIden>(col: T, len: u32, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.custom(Alias::new(format!("CHAR({})", len))).not_null();
        }
        _ => {
            def.char_len(len).not_null();
        }
    }
    def.take()
}

/// Performs the `char_len_null` operation.
pub fn char_len_null<T: IntoIden>(col: T, len: u32, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.custom(Alias::new(format!("CHAR({})", len))).null();
        }
        _ => {
            def.char_len(len).null();
        }
    }
    def.take()
}

/// Performs the `char_len_default` operation.
pub fn char_len_default<T: IntoIden>(col: T, len: u32, default: &str, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.custom(Alias::new(format!("CHAR({})", len))).not_null().default(default);
        }
        _ => {
            def.char_len(len).not_null().default(default);
        }
    }
    def.take()
}

/// Performs the `date_time` operation.
pub fn date_time<T: IntoIden>(col: T) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.date_time().not_null().default(Expr::current_timestamp());
    def.take()
}

/// Performs the `date_time_on_update` operation.
pub fn date_time_on_update<T: IntoIden>(col: T) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.date_time().null().default(Expr::current_timestamp());
    def.take()
}

/// Performs the `datetime_null` operation.
pub fn datetime_null<T: IntoIden>(col: T) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.date_time().null();
    def.take()
}

/// Performs the `datetime_default` operation.
pub fn datetime_default<T: IntoIden>(col: T, default: &str) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.date_time().not_null().default(default);
    def.take()
}