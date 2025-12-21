//! markdown-sql 使用示例
//!
//! 展示如何使用 markdown-sql 框架：
//! - 加载 Markdown SQL 文件
//! - 执行 CRUD 操作
//! - 事务处理
//! - 批量操作

use markdown_sql::{
    batch_execute, begin_transaction, execute, execute_tx, query_list, query_optional,
    query_scalar, DbPool, DbType, SqlManager,
};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{FromRow, Pool, Sqlite, SqlitePool};
use tracing::info;

/// 用户实体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct User {
    id: i64,
    name: String,
    age: i32,
    email: Option<String>,
    status: i32,
    created_at: Option<String>,
}

/// 用户查询参数
#[derive(Debug, Serialize)]
struct UserQuery {
    name: Option<String>,
    status: Option<i32>,
    min_age: Option<i32>,
}

/// 用户插入参数
#[derive(Debug, Serialize)]
struct UserInsert {
    name: String,
    age: i32,
    email: Option<String>,
    status: i32,
}

/// ID 参数
#[derive(Debug, Serialize)]
struct IdParams {
    id: i64,
}

/// IDs 参数
#[derive(Debug, Serialize)]
struct IdsParams {
    ids: Vec<i64>,
}

/// 统计参数
#[derive(Debug, Serialize)]
struct CountParams {
    status: Option<i32>,
}

/// 应用数据库（实现 DbPool trait）
struct AppDb {
    pool: Pool<Sqlite>,
}

impl DbPool for AppDb {
    fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("info,markdown_sql=debug")
        .init();

    info!("=== markdown-sql 示例程序 ===");

    // 创建数据库连接
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await?;

    // 创建表
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            age INTEGER NOT NULL,
            email TEXT,
            status INTEGER NOT NULL DEFAULT 1,
            created_at TEXT,
            updated_at TEXT
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // 创建 SQL 管理器
    let mut manager = SqlManager::builder()
        .db_type(DbType::Sqlite)
        .debug(true)
        .build()?;

    // 加载 SQL 文件
    manager.load_file("sql/UserRepository.md")?;
    info!("SQL 文件加载完成");

    // 封装数据库
    let db = AppDb { pool };

    // ==================== 1. 插入数据 ====================
    info!("\n=== 1. 插入数据 ===");

    let user1 = UserInsert {
        name: "张三".to_string(),
        age: 25,
        email: Some("zhangsan@example.com".to_string()),
        status: 1,
    };

    let affected = execute(&manager, &db, "insert", &user1).await?;
    info!("插入用户: {:?}, 影响行数: {}", user1, affected);

    let user2 = UserInsert {
        name: "李四".to_string(),
        age: 30,
        email: Some("lisi@example.com".to_string()),
        status: 1,
    };
    execute(&manager, &db, "insert", &user2).await?;

    let user3 = UserInsert {
        name: "王五".to_string(),
        age: 35,
        email: None,
        status: 0,
    };
    execute(&manager, &db, "insert", &user3).await?;

    // ==================== 2. 查询数据 ====================
    info!("\n=== 2. 查询数据 ===");

    // 查询所有用户
    let users: Vec<User> = query_list(&manager, &db, "findAll", &serde_json::json!({})).await?;
    info!("所有用户: {} 条", users.len());
    for user in &users {
        info!("  - {:?}", user);
    }

    // 根据 ID 查询
    let user: Option<User> = query_optional(&manager, &db, "findById", &IdParams { id: 1 }).await?;
    info!("ID=1 的用户: {:?}", user);

    // 条件查询
    let query = UserQuery {
        name: None,
        status: Some(1),
        min_age: Some(28),
    };
    let users: Vec<User> = query_list(&manager, &db, "findByCondition", &query).await?;
    info!("状态=1 且 年龄>=28 的用户: {} 条", users.len());

    // 统计
    let count: i64 = query_scalar(&manager, &db, "count", &CountParams { status: Some(1) }).await?;
    info!("状态=1 的用户数量: {}", count);

    // IN 查询
    let users: Vec<User> = query_list(
        &manager,
        &db,
        "findByIds",
        &IdsParams { ids: vec![1, 2] },
    )
    .await?;
    info!("ID IN (1, 2) 的用户: {} 条", users.len());

    // ==================== 3. 事务操作 ====================
    info!("\n=== 3. 事务操作 ===");

    // 开启事务
    let mut tx = begin_transaction(&db).await?;

    // 在事务中插入
    let user4 = UserInsert {
        name: "赵六".to_string(),
        age: 28,
        email: Some("zhaoliu@example.com".to_string()),
        status: 1,
    };
    execute_tx(&manager, &mut tx, "insert", &user4).await?;
    info!("事务中插入: {:?}", user4);

    // 提交事务
    tx.commit().await?;
    info!("事务已提交");

    // 验证
    let count: i64 = query_scalar(&manager, &db, "count", &CountParams { status: None }).await?;
    info!("提交后总用户数: {}", count);

    // ==================== 4. 批量操作 ====================
    info!("\n=== 4. 批量操作 ===");

    let batch_users: Vec<UserInsert> = (0..5)
        .map(|i| UserInsert {
            name: format!("批量用户{}", i),
            age: 20 + i,
            email: Some(format!("batch{}@example.com", i)),
            status: 1,
        })
        .collect();

    let affected = batch_execute(&manager, &db, "insert", &batch_users).await?;
    info!("批量插入 {} 条，影响行数: {}", batch_users.len(), affected);

    // 最终统计
    let count: i64 = query_scalar(&manager, &db, "count", &CountParams { status: None }).await?;
    info!("\n最终用户总数: {}", count);

    info!("\n=== 示例程序完成 ===");
    Ok(())
}
