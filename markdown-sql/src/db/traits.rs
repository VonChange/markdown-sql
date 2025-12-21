//! 数据库抽象层
//!
//! 定义统一的数据库连接 trait，使 Repository 代码与具体数据库解耦。
//! 支持 SQLite、MySQL、PostgreSQL 三种数据库。
//!
//! ## 使用方式
//!
//! ```ignore
//! use markdown_sql::SqliteDbPool;
//!
//! // 实现 SqliteDbPool trait
//! impl SqliteDbPool for AppDb {
//!     fn pool(&self) -> &Pool<Sqlite> {
//!         &self.sqlite
//!     }
//! }
//!
//! // 使用时直接传 &db
//! repo.find_by_id(&db, &params).await
//! ```

use std::sync::Arc;

// ============================================================================
// SQLite 支持
// ============================================================================

#[cfg(feature = "sqlite")]
/// SQLite 数据库连接池 trait
pub trait SqliteDbPool {
    /// 获取 SQLite 连接池
    fn pool(&self) -> &sqlx::Pool<sqlx::Sqlite>;
}

#[cfg(feature = "sqlite")]
impl SqliteDbPool for sqlx::Pool<sqlx::Sqlite> {
    fn pool(&self) -> &sqlx::Pool<sqlx::Sqlite> {
        self
    }
}

#[cfg(feature = "sqlite")]
impl SqliteDbPool for &sqlx::Pool<sqlx::Sqlite> {
    fn pool(&self) -> &sqlx::Pool<sqlx::Sqlite> {
        self
    }
}

#[cfg(feature = "sqlite")]
impl<T: SqliteDbPool> SqliteDbPool for Arc<T> {
    fn pool(&self) -> &sqlx::Pool<sqlx::Sqlite> {
        (**self).pool()
    }
}

// ============================================================================
// MySQL 支持
// ============================================================================

#[cfg(feature = "mysql")]
/// MySQL 数据库连接池 trait
pub trait MySqlDbPool {
    /// 获取 MySQL 连接池
    fn pool(&self) -> &sqlx::Pool<sqlx::MySql>;
}

#[cfg(feature = "mysql")]
impl MySqlDbPool for sqlx::Pool<sqlx::MySql> {
    fn pool(&self) -> &sqlx::Pool<sqlx::MySql> {
        self
    }
}

#[cfg(feature = "mysql")]
impl MySqlDbPool for &sqlx::Pool<sqlx::MySql> {
    fn pool(&self) -> &sqlx::Pool<sqlx::MySql> {
        self
    }
}

#[cfg(feature = "mysql")]
impl<T: MySqlDbPool> MySqlDbPool for Arc<T> {
    fn pool(&self) -> &sqlx::Pool<sqlx::MySql> {
        (**self).pool()
    }
}

// ============================================================================
// PostgreSQL 支持
// ============================================================================

#[cfg(feature = "postgres")]
/// PostgreSQL 数据库连接池 trait
pub trait PgDbPool {
    /// 获取 PostgreSQL 连接池
    fn pool(&self) -> &sqlx::Pool<sqlx::Postgres>;
}

#[cfg(feature = "postgres")]
impl PgDbPool for sqlx::Pool<sqlx::Postgres> {
    fn pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        self
    }
}

#[cfg(feature = "postgres")]
impl PgDbPool for &sqlx::Pool<sqlx::Postgres> {
    fn pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        self
    }
}

#[cfg(feature = "postgres")]
impl<T: PgDbPool> PgDbPool for Arc<T> {
    fn pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        (**self).pool()
    }
}

// ============================================================================
// 向后兼容：DbPool 作为 SqliteDbPool 的别名
// ============================================================================

#[cfg(feature = "sqlite")]
/// 向后兼容的 DbPool trait（等同于 SqliteDbPool）
///
/// 保持与旧代码的兼容性
pub trait DbPool: SqliteDbPool {}

#[cfg(feature = "sqlite")]
impl<T: SqliteDbPool> DbPool for T {}

#[cfg(test)]
mod tests {
    #[cfg(feature = "sqlite")]
    use super::SqliteDbPool;

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_sqlite_trait() {
        fn _accepts_sqlite<D: SqliteDbPool>(_db: &D) {}
    }

    #[cfg(feature = "mysql")]
    use super::MySqlDbPool;

    #[cfg(feature = "mysql")]
    #[test]
    fn test_mysql_trait() {
        fn _accepts_mysql<D: MySqlDbPool>(_db: &D) {}
    }

    #[cfg(feature = "postgres")]
    use super::PgDbPool;

    #[cfg(feature = "postgres")]
    #[test]
    fn test_pg_trait() {
        fn _accepts_pg<D: PgDbPool>(_db: &D) {}
    }

    #[cfg(feature = "sqlite")]
    use super::DbPool;

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_db_pool_compat() {
        fn _accepts_db_pool<D: DbPool>(_db: &D) {}
    }
}
