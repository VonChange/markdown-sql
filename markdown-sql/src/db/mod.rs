//! 数据库模块
//!
//! 包含数据库抽象层和各数据库的具体实现。
//!
//! ## 模块结构
//!
//! - `traits` - 数据库连接池 trait 定义
//! - `sqlite` - SQLite 实现
//! - `mysql` - MySQL 实现
//! - `postgres` - PostgreSQL 实现

pub mod traits;

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "mysql")]
pub mod mysql;

#[cfg(feature = "postgres")]
pub mod postgres;

// 重新导出 traits
#[cfg(feature = "sqlite")]
pub use traits::{DbPool, SqliteDbPool};

#[cfg(feature = "mysql")]
pub use traits::MySqlDbPool;

#[cfg(feature = "postgres")]
pub use traits::PgDbPool;

// 重新导出 SQLite 函数（默认）
#[cfg(feature = "sqlite")]
pub use sqlite::{
    batch_execute, batch_execute_tx, begin_transaction, execute, execute_tx, query_list,
    query_list_tx, query_one, query_one_tx, query_optional, query_optional_tx, query_scalar,
    query_scalar_tx, with_transaction, EmptyParams, Repository,
};
