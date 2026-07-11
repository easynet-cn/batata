use sea_orm_migration::prelude::*;

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

pub fn unsigned_int<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.integer().unsigned().not_null();
        }
        _ => {
            def.integer().not_null();
        }
    }
    def.take()
}

pub fn unsigned_int_null<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.integer().unsigned().null();
        }
        _ => {
            def.integer().null();
        }
    }
    def.take()
}

pub fn unsigned_big_int<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.big_integer().unsigned().not_null();
        }
        _ => {
            def.big_integer().not_null();
        }
    }
    def.take()
}

pub fn unsigned_big_int_null<T: IntoIden>(col: T, backend: sea_orm::DatabaseBackend) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match backend {
        sea_orm::DatabaseBackend::MySql => {
            def.big_integer().unsigned().null();
        }
        _ => {
            def.big_integer().null();
        }
    }
    def.take()
}

pub fn bit<T: IntoIden>(col: T, _length: Option<u32>) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.boolean().not_null().default(false);
    def.take()
}

pub fn string_len<T: IntoIden>(col: T, len: u32) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.string_len(len).not_null();
    def.take()
}

pub fn string_len_null<T: IntoIden>(col: T, len: u32) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.string_len(len).null();
    def.take()
}

pub fn string_len_default<T: IntoIden>(col: T, len: u32, default: &str) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.string_len(len).not_null().default(default);
    def.take()
}

pub fn date_time<T: IntoIden>(col: T) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.date_time().not_null().default(Expr::current_timestamp());
    def.take()
}

pub fn date_time_on_update<T: IntoIden>(col: T) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.date_time().null().default(Expr::current_timestamp());
    def.take()
}

pub fn datetime_null<T: IntoIden>(col: T) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.date_time().null();
    def.take()
}

pub fn datetime_default<T: IntoIden>(col: T, default: &str) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    def.date_time().not_null().default(default);
    def.take()
}