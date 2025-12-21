//! 数据库抽象层
//!
//! 定义统一的数据库连接 trait，使 Repository 代码与具体数据库解耦。
//!
//! ## 使用方式
//!
//! ```ignore
//! use markdown_sql::DbPool;
//!
//! // 实现 DbPool trait
//! impl DbPool for AppDb {
//!     fn pool(&self) -> &Pool<Sqlite> {
//!         &self.sqlite
//!     }
//! }
//!
//! // 使用时直接传 &db，不需要 &db.sqlite
//! repo.find_by_id(&db, &params).await
//! ```

use std::sync::Arc;

use sqlx::{Pool, Sqlite};

/// 数据库连接池 trait
///
/// 使用方需要实现此 trait，提供统一的数据库访问。
/// 这样 Repository 代码就不需要关心具体的数据库连接来源。
///
/// ## 示例
///
/// ```ignore
/// pub struct AppDb {
///     pub sqlite: Pool<Sqlite>,
///     // 其他字段...
/// }
///
/// impl DbPool for AppDb {
///     fn pool(&self) -> &Pool<Sqlite> {
///         &self.sqlite
///     }
/// }
/// ```
///
/// ## 换数据库
///
/// 如果以后要换成 MySQL，只需要：
/// 1. 修改 `Pool<Sqlite>` 为 `Pool<MySql>`
/// 2. 修改 `AppDb` 的实现
/// 3. Repository 调用方代码不需要改动
pub trait DbPool {
    /// 获取 SQLite 连接池
    ///
    /// 返回对连接池的引用，用于执行 SQL 查询
    fn pool(&self) -> &Pool<Sqlite>;
}

/// 为 Pool<Sqlite> 实现 DbPool
///
/// 这样可以直接传递 `&Pool<Sqlite>`
impl DbPool for Pool<Sqlite> {
    fn pool(&self) -> &Pool<Sqlite> {
        self
    }
}

/// 为 &Pool<Sqlite> 实现 DbPool
impl DbPool for &Pool<Sqlite> {
    fn pool(&self) -> &Pool<Sqlite> {
        self
    }
}

/// 为 Arc<T> 实现 DbPool（当 T 实现 DbPool 时）
///
/// 这样可以直接传递 `&Arc<AppDb>`
impl<T: DbPool> DbPool for Arc<T> {
    fn pool(&self) -> &Pool<Sqlite> {
        (**self).pool()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 由于创建 Pool 需要实际连接，这里只测试 trait 定义是否正确
    #[test]
    fn test_trait_definition() {
        fn _accepts_db_pool<D: DbPool>(_db: &D) {}
        // 编译通过即表示 trait 定义正确
    }
}
