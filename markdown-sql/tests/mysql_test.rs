//! MySQL 集成测试
//!
//! 需要本地运行 MySQL 容器：
//! docker run -d --name mysql-test -e MYSQL_ROOT_PASSWORD=root123 -e MYSQL_DATABASE=testdb -p 3306:3306 mysql:8.0
//!
//! **注意**：本测试严格遵守 CLAUDE.md 规范：
//! - 所有 SQL 都在 Markdown 文件中
//! - 所有数据库操作通过 #[repository] 宏 + trait 定义
//! - 禁止直接调用底层函数

#![allow(async_fn_in_trait)]
#![allow(private_interfaces)]

use markdown_sql::{repository, DbType, MySqlDbPool, SqlManager, TypedParams};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::{FromRow, MySql, Pool};

// ============================================================================
// 实体和参数定义
// ============================================================================

/// 用户实体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
struct User {
    id: i64,
    name: String,
    age: i32,
    status: i32,
}

/// 用户插入参数
#[derive(Debug, Serialize, TypedParams)]
struct UserInsert {
    name: String,
    age: i32,
    status: i32,
}

// ============================================================================
// Repository 定义（遵守规范：通过 trait + 宏定义）
// ============================================================================

/// MySQL 测试 Repository
///
/// 使用 db_type = "mysql" 指定数据库类型
#[repository(sql_file = "tests/sql/MySqlTestRepository.md", db_type = "mysql")]
pub trait MySqlTestRepository {
    /// 删除表
    async fn drop_table(&self) -> Result<u64, markdown_sql::MarkdownSqlError>;

    /// 创建表
    async fn create_table(&self) -> Result<u64, markdown_sql::MarkdownSqlError>;

    /// 清空表
    async fn truncate_table(&self) -> Result<u64, markdown_sql::MarkdownSqlError>;

    /// 查询所有用户
    async fn find_all(&self) -> Result<Vec<User>, markdown_sql::MarkdownSqlError>;

    /// 插入用户
    async fn insert(&self, user: &UserInsert) -> Result<u64, markdown_sql::MarkdownSqlError>;
}

// ============================================================================
// 全局 SqlManager（Lazy 初始化）
// ============================================================================

static SQL_MANAGER: Lazy<SqlManager> = Lazy::new(|| {
    let mut manager = SqlManager::builder()
        .db_type(DbType::Mysql)
        .debug(true)
        .build()
        .expect("创建 SqlManager 失败");

    manager
        .load_file("tests/sql/MySqlTestRepository.md")
        .expect("加载 SQL 文件失败");

    manager
});

/// 获取 Repository 实例
fn get_repo() -> MySqlTestRepositoryImpl {
    MySqlTestRepositoryImpl::new(&SQL_MANAGER)
}

// ============================================================================
// 数据库连接封装
// ============================================================================

/// MySQL 数据库封装
struct TestDb {
    pool: Pool<MySql>,
}

impl MySqlDbPool for TestDb {
    fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }
}

/// 创建测试数据库连接
async fn setup_database() -> Option<TestDb> {
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect("mysql://root:root123@localhost:3306/testdb")
        .await
        .ok()?;

    let db = TestDb { pool };
    let repo = get_repo();

    // 使用 Repository 方法初始化表（遵守规范！）
    let _ = repo.drop_table(&db).await;
    repo.create_table(&db).await.ok()?;

    Some(db)
}

/// 清理表
async fn cleanup(db: &TestDb) {
    let repo = get_repo();
    let _ = repo.truncate_table(db).await;
}

// ============================================================================
// 测试用例
// ============================================================================

#[tokio::test]
async fn test_mysql_insert_and_query() {
    let db = match setup_database().await {
        Some(x) => x,
        None => {
            println!("⚠️ MySQL 未运行，跳过测试");
            return;
        }
    };

    let repo = get_repo();

    // 插入用户（通过 Repository 方法）
    let user = UserInsert {
        name: "MySQL测试用户".to_string(),
        age: 25,
        status: 1,
    };

    let affected = repo.insert(&db, &user).await.expect("插入失败");
    assert_eq!(affected, 1, "应该插入 1 条数据");

    // 查询所有（通过 Repository 方法）
    let users = repo.find_all(&db).await.expect("查询失败");
    assert_eq!(users.len(), 1, "应该查询到 1 条数据");
    assert_eq!(users[0].name, "MySQL测试用户");

    cleanup(&db).await;
    println!("✅ MySQL 插入和查询测试通过");
}

#[tokio::test]
async fn test_mysql_batch_insert() {
    let db = match setup_database().await {
        Some(x) => x,
        None => {
            println!("⚠️ MySQL 未运行，跳过测试");
            return;
        }
    };

    let repo = get_repo();
    cleanup(&db).await;

    // 批量插入（通过多次调用 Repository 方法）
    for i in 0..5 {
        let user = UserInsert {
            name: format!("MySQL批量用户{}", i),
            age: 20 + i,
            status: 1,
        };
        repo.insert(&db, &user).await.expect("插入失败");
    }

    // 验证
    let users = repo.find_all(&db).await.expect("查询失败");
    assert_eq!(users.len(), 5, "应该有 5 条数据");

    cleanup(&db).await;
    println!("✅ MySQL 批量操作测试通过");
}
