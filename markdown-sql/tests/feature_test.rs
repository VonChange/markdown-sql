//! 功能测试
//!
//! 测试 markdown-sql 框架的完整功能：
//! - CRUD 操作
//! - 动态 SQL
//! - 事务处理
//! - 批量操作
//! - DbPool trait
//!
//! **注意**：本测试严格遵守 CLAUDE.md 规范：
//! - 所有 SQL 都在 Markdown 文件中
//! - 所有数据库操作通过 #[repository] 宏 + trait 定义
//! - 禁止直接调用底层函数

#![allow(async_fn_in_trait)]
#![allow(private_interfaces)]

use markdown_sql::{repository, transactional, DbType, MarkdownSqlError, SqliteDbPool, SqlManager};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Sqlite, SqlitePool};

// ============================================================================
// 实体定义
// ============================================================================

/// 用户实体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
struct User {
    id: i64,
    name: String,
    age: i32,
    email: Option<String>,
    status: i32,
    created_at: Option<String>,
}

// ============================================================================
// 参数定义
// ============================================================================

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

/// 用户更新参数
#[derive(Debug, Serialize)]
struct UserUpdate {
    id: i64,
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

// ============================================================================
// Repository 定义（遵守规范：通过 trait + 宏定义）
// ============================================================================

/// 功能测试 Repository
#[repository(sql_file = "tests/sql/FeatureTestRepository.md", db_type = "sqlite")]
pub trait FeatureTestRepository {
    // DDL
    async fn create_table(&self) -> Result<u64, markdown_sql::MarkdownSqlError>;
    async fn drop_table(&self) -> Result<u64, markdown_sql::MarkdownSqlError>;
    async fn truncate_table(&self) -> Result<u64, markdown_sql::MarkdownSqlError>;

    // 查询
    async fn find_all(&self) -> Result<Vec<User>, markdown_sql::MarkdownSqlError>;
    async fn find_by_id(&self, params: &IdParams) -> Result<Option<User>, markdown_sql::MarkdownSqlError>;
    async fn find_by_condition(&self, params: &UserQuery) -> Result<Vec<User>, markdown_sql::MarkdownSqlError>;
    async fn find_by_ids(&self, params: &IdsParams) -> Result<Vec<User>, markdown_sql::MarkdownSqlError>;

    // 写入
    async fn insert(&self, params: &UserInsert) -> Result<u64, markdown_sql::MarkdownSqlError>;
    async fn update(&self, params: &UserUpdate) -> Result<u64, markdown_sql::MarkdownSqlError>;
    async fn delete_by_id(&self, params: &IdParams) -> Result<u64, markdown_sql::MarkdownSqlError>;

    // 统计
    async fn count(&self, params: &CountParams) -> Result<i64, markdown_sql::MarkdownSqlError>;
}

// ============================================================================
// 全局 SqlManager
// ============================================================================

static SQL_MANAGER: Lazy<SqlManager> = Lazy::new(|| {
    let mut manager = SqlManager::builder()
        .db_type(DbType::Sqlite)
        .debug(true)
        .build()
        .expect("创建 SqlManager 失败");

    manager
        .load_file("tests/sql/FeatureTestRepository.md")
        .expect("加载 SQL 文件失败");

    manager
});

fn get_repo() -> FeatureTestRepositoryImpl {
    FeatureTestRepositoryImpl::new(&SQL_MANAGER)
}

// ============================================================================
// 数据库连接
// ============================================================================

struct AppDb {
    pool: Pool<Sqlite>,
}

impl SqliteDbPool for AppDb {
    fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}

async fn setup_database() -> AppDb {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("创建 SQLite 连接失败");

    let db = AppDb { pool };
    let repo = get_repo();

    // 使用 Repository 方法创建表
    repo.create_table(&db).await.expect("创建表失败");

    db
}

// ============================================================================
// 测试用例
// ============================================================================

#[tokio::test]
async fn test_insert_and_query() {
    let db = setup_database().await;
    let repo = get_repo();

    // 插入用户
    let user = UserInsert {
        name: "张三".to_string(),
        age: 25,
        email: Some("zhangsan@example.com".to_string()),
        status: 1,
    };
    let affected = repo.insert(&db, &user).await.expect("插入失败");
    assert_eq!(affected, 1);

    // 查询所有
    let users = repo.find_all(&db).await.expect("查询失败");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "张三");

    println!("✅ 插入和查询测试通过");
}

#[tokio::test]
async fn test_find_by_id() {
    let db = setup_database().await;
    let repo = get_repo();

    // 插入用户
    repo.insert(&db, &UserInsert {
        name: "李四".to_string(),
        age: 30,
        email: None,
        status: 1,
    }).await.expect("插入失败");

    // 根据 ID 查询
    let user = repo.find_by_id(&db, &IdParams { id: 1 })
        .await
        .expect("查询失败")
        .expect("用户不存在");
    assert_eq!(user.name, "李四");

    // 查询不存在的用户
    let not_found = repo.find_by_id(&db, &IdParams { id: 999 }).await.expect("查询失败");
    assert!(not_found.is_none());

    println!("✅ 根据 ID 查询测试通过");
}

#[tokio::test]
async fn test_dynamic_sql() {
    let db = setup_database().await;
    let repo = get_repo();

    // 插入多个用户
    for i in 0..5 {
        repo.insert(&db, &UserInsert {
            name: format!("用户{}", i),
            age: 20 + i,
            email: None,
            status: if i % 2 == 0 { 1 } else { 0 },
        }).await.expect("插入失败");
    }

    // 按状态查询
    let active_users = repo.find_by_condition(&db, &UserQuery {
        name: None,
        status: Some(1),
        min_age: None,
    }).await.expect("查询失败");
    assert_eq!(active_users.len(), 3, "应该有 3 个激活用户");

    // 按年龄查询
    let older_users = repo.find_by_condition(&db, &UserQuery {
        name: None,
        status: None,
        min_age: Some(23),
    }).await.expect("查询失败");
    assert_eq!(older_users.len(), 2, "应该有 2 个 >= 23 岁的用户");

    println!("✅ 动态 SQL 测试通过");
}

#[tokio::test]
async fn test_in_query() {
    let db = setup_database().await;
    let repo = get_repo();

    // 插入多个用户
    for i in 1..=5 {
        repo.insert(&db, &UserInsert {
            name: format!("用户{}", i),
            age: 20 + i,
            email: None,
            status: 1,
        }).await.expect("插入失败");
    }

    // IN 查询
    let users = repo.find_by_ids(&db, &IdsParams { ids: vec![1, 3, 5] })
        .await
        .expect("查询失败");
    assert_eq!(users.len(), 3);
    assert_eq!(users[0].name, "用户1");
    assert_eq!(users[1].name, "用户3");
    assert_eq!(users[2].name, "用户5");

    println!("✅ IN 查询测试通过");
}

#[tokio::test]
async fn test_update() {
    let db = setup_database().await;
    let repo = get_repo();

    // 插入用户
    repo.insert(&db, &UserInsert {
        name: "原名".to_string(),
        age: 25,
        email: None,
        status: 1,
    }).await.expect("插入失败");

    // 更新用户
    repo.update(&db, &UserUpdate {
        id: 1,
        name: "新名".to_string(),
        age: 26,
        email: Some("new@example.com".to_string()),
        status: 1,
    }).await.expect("更新失败");

    // 验证
    let user = repo.find_by_id(&db, &IdParams { id: 1 })
        .await
        .expect("查询失败")
        .expect("用户不存在");
    assert_eq!(user.name, "新名");
    assert_eq!(user.age, 26);

    println!("✅ 更新测试通过");
}

#[tokio::test]
async fn test_delete() {
    let db = setup_database().await;
    let repo = get_repo();

    // 插入用户
    repo.insert(&db, &UserInsert {
        name: "待删除".to_string(),
        age: 25,
        email: None,
        status: 1,
    }).await.expect("插入失败");

    // 删除
    repo.delete_by_id(&db, &IdParams { id: 1 }).await.expect("删除失败");

    // 验证
    let user = repo.find_by_id(&db, &IdParams { id: 1 }).await.expect("查询失败");
    assert!(user.is_none());

    println!("✅ 删除测试通过");
}

#[tokio::test]
async fn test_count() {
    let db = setup_database().await;
    let repo = get_repo();

    // 插入多个用户
    for i in 0..5 {
        repo.insert(&db, &UserInsert {
            name: format!("用户{}", i),
            age: 20 + i,
            email: None,
            status: if i % 2 == 0 { 1 } else { 0 },
        }).await.expect("插入失败");
    }

    // 统计所有
    let total = repo.count(&db, &CountParams { status: None }).await.expect("统计失败");
    assert_eq!(total, 5);

    // 统计激活用户
    let active = repo.count(&db, &CountParams { status: Some(1) }).await.expect("统计失败");
    assert_eq!(active, 3);

    println!("✅ 统计测试通过");
}

#[tokio::test]
async fn test_batch_insert() {
    let db = setup_database().await;
    let repo = get_repo();

    // 批量插入（通过循环调用）
    for i in 0..10 {
        repo.insert(&db, &UserInsert {
            name: format!("批量用户{}", i),
            age: 20 + i,
            email: None,
            status: 1,
        }).await.expect("插入失败");
    }

    // 验证
    let users = repo.find_all(&db).await.expect("查询失败");
    assert_eq!(users.len(), 10);

    println!("✅ 批量插入测试通过");
}

// ============================================================================
// 事务测试
// ============================================================================

#[tokio::test]
async fn test_transaction_commit() {
    let db = setup_database().await;
    let repo = get_repo();

    // 开启事务
    let mut tx = repo.begin_transaction(&db).await.expect("开启事务失败");

    // 在事务中插入
    repo.insert_tx(&mut tx, &UserInsert {
        name: "事务用户1".to_string(),
        age: 25,
        email: None,
        status: 1,
    }).await.expect("事务插入失败");

    repo.insert_tx(&mut tx, &UserInsert {
        name: "事务用户2".to_string(),
        age: 30,
        email: None,
        status: 1,
    }).await.expect("事务插入失败");

    // 提交事务
    tx.commit().await.expect("提交事务失败");

    // 验证
    let users = repo.find_all(&db).await.expect("查询失败");
    assert_eq!(users.len(), 2, "应该有 2 条数据");

    println!("✅ 事务提交测试通过");
}

#[tokio::test]
async fn test_transaction_rollback() {
    let db = setup_database().await;
    let repo = get_repo();

    // 先插入一条数据
    repo.insert(&db, &UserInsert {
        name: "初始用户".to_string(),
        age: 20,
        email: None,
        status: 1,
    }).await.expect("插入失败");

    // 开启事务
    let mut tx = repo.begin_transaction(&db).await.expect("开启事务失败");

    // 在事务中插入
    repo.insert_tx(&mut tx, &UserInsert {
        name: "事务用户".to_string(),
        age: 25,
        email: None,
        status: 1,
    }).await.expect("事务插入失败");

    // 回滚事务
    tx.rollback().await.expect("回滚事务失败");

    // 验证（只有初始的 1 条数据）
    let users = repo.find_all(&db).await.expect("查询失败");
    assert_eq!(users.len(), 1, "回滚后应该只有 1 条数据");
    assert_eq!(users[0].name, "初始用户");

    println!("✅ 事务回滚测试通过");
}

#[tokio::test]
async fn test_transaction_query() {
    let db = setup_database().await;
    let repo = get_repo();

    // 开启事务
    let mut tx = repo.begin_transaction(&db).await.expect("开启事务失败");

    // 插入数据
    repo.insert_tx(&mut tx, &UserInsert {
        name: "事务查询用户".to_string(),
        age: 25,
        email: None,
        status: 1,
    }).await.expect("插入失败");

    // 在事务中查询（应该能看到未提交的数据）
    let users = repo.find_all_tx(&mut tx).await.expect("事务查询失败");
    assert_eq!(users.len(), 1, "事务中应该能看到插入的数据");

    // 提交
    tx.commit().await.expect("提交失败");

    println!("✅ 事务查询测试通过");
}

// ============================================================================
// #[transactional] 自动事务测试
// ============================================================================

/// 定义带 #[transactional] 的 Repository
#[repository(sql_file = "tests/sql/FeatureTestRepository.md", db_type = "sqlite")]
pub trait TransactionalTestRepository {
    /// 自动事务方法：批量插入
    #[transactional]
    async fn insert(&self, user: &UserInsert) -> Result<u64, MarkdownSqlError>;

    /// 查询所有
    async fn find_all(&self) -> Result<Vec<User>, MarkdownSqlError>;
}

fn get_transactional_repo() -> TransactionalTestRepositoryImpl {
    TransactionalTestRepositoryImpl::new(&SQL_MANAGER)
}

#[tokio::test]
async fn test_transactional_attribute() {
    let db = setup_database().await;
    let repo = get_transactional_repo();

    // 使用 #[transactional] 标记的方法（自动事务）
    repo.insert(&db, &UserInsert {
        name: "自动事务用户".to_string(),
        age: 30,
        email: None,
        status: 1,
    }).await.expect("自动事务插入失败");

    // 验证
    let users = repo.find_all(&db).await.expect("查询失败");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "自动事务用户");

    println!("✅ #[transactional] 自动事务测试通过");
}
