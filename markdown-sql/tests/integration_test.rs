//! SQLite 集成测试
//!
//! 使用 SQLite 内存数据库进行完整的端到端测试。
//! 包含连接池并发测试。

use markdown_sql::{DbType, ParamExtractor, SqlManager};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{FromRow, Row, SqlitePool};
use std::sync::Arc;
use std::time::Duration;

/// 用户实体
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
struct User {
    id: i64,
    name: String,
    age: i32,
    status: i32,
}

/// 创建测试数据库和表
async fn setup_database() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("创建 SQLite 连接失败");

    // 创建用户表
    sqlx::query(
        r#"
        CREATE TABLE user_info (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            age INTEGER NOT NULL,
            status INTEGER NOT NULL DEFAULT 1
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("创建表失败");

    pool
}

/// 创建 SQL 管理器并加载 SQL（从 Markdown 文件）
fn setup_manager() -> SqlManager {
    let mut manager = SqlManager::builder()
        .db_type(DbType::Sqlite)
        .debug(true)
        .build()
        .expect("创建 SqlManager 失败");

    // 从 Markdown 文件加载 SQL
    manager
        .load_file("tests/sql/UserRepository.md")
        .expect("加载 SQL 文件失败");

    manager
}

/// 执行带参数的查询，返回多行
async fn query_all<T: for<'r> FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin>(
    pool: &SqlitePool,
    manager: &SqlManager,
    sql_id: &str,
    params: &serde_json::Value,
) -> Vec<T> {
    // 1. 渲染 SQL
    let sql = manager.render(sql_id, params).expect("渲染 SQL 失败");

    // 2. 提取参数
    let result = ParamExtractor::extract(&sql, DbType::Sqlite);

    // 3. 构建查询
    let mut query = sqlx::query_as::<_, T>(&result.sql);

    // 4. 绑定参数
    for param_name in &result.params {
        if param_name.starts_with("__bind_") {
            // IN 查询的展开参数
            let idx: usize = param_name
                .strip_prefix("__bind_")
                .unwrap()
                .parse()
                .unwrap();
            if let Some(arr) = params.get("ids").and_then(|v| v.as_array()) {
                if let Some(val) = arr.get(idx) {
                    query = query.bind(val.as_i64().unwrap());
                }
            }
        } else if let Some(val) = params.get(param_name) {
            if val.is_i64() {
                query = query.bind(val.as_i64().unwrap());
            } else if val.is_string() {
                query = query.bind(val.as_str().unwrap().to_string());
            }
        }
    }

    // 5. 执行查询
    query.fetch_all(pool).await.expect("查询失败")
}

/// 执行带参数的修改操作，返回影响行数
async fn execute(
    pool: &SqlitePool,
    manager: &SqlManager,
    sql_id: &str,
    params: &serde_json::Value,
) -> u64 {
    // 1. 渲染 SQL
    let sql = manager.render(sql_id, params).expect("渲染 SQL 失败");

    // 2. 提取参数
    let result = ParamExtractor::extract(&sql, DbType::Sqlite);

    // 3. 构建查询
    let mut query = sqlx::query(&result.sql);

    // 4. 绑定参数
    for param_name in &result.params {
        if let Some(val) = params.get(param_name) {
            if val.is_i64() {
                query = query.bind(val.as_i64().unwrap());
            } else if val.is_string() {
                query = query.bind(val.as_str().unwrap().to_string());
            }
        }
    }

    // 5. 执行
    query
        .execute(pool)
        .await
        .expect("执行失败")
        .rows_affected()
}

/// 执行 COUNT 查询
async fn query_count(
    pool: &SqlitePool,
    manager: &SqlManager,
    sql_id: &str,
    params: &serde_json::Value,
) -> i64 {
    let sql = manager.render(sql_id, params).expect("渲染 SQL 失败");
    let result = ParamExtractor::extract(&sql, DbType::Sqlite);

    let mut query = sqlx::query(&result.sql);

    for param_name in &result.params {
        if let Some(val) = params.get(param_name) {
            if val.is_i64() {
                query = query.bind(val.as_i64().unwrap());
            } else if val.is_string() {
                query = query.bind(val.as_str().unwrap().to_string());
            }
        }
    }

    let row = query.fetch_one(pool).await.expect("查询失败");
    row.get::<i64, _>("count")
}

#[tokio::test]
async fn test_insert_and_query() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 1. 插入用户
    let affected = execute(
        &pool,
        &manager,
        "insert",
        &json!({
            "name": "张三",
            "age": 25,
            "status": 1
        }),
    )
    .await;
    assert_eq!(affected, 1);

    // 2. 查询用户
    let users: Vec<User> = query_all(&pool, &manager, "findById", &json!({"id": 1})).await;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "张三");
    assert_eq!(users[0].age, 25);
}

#[tokio::test]
async fn test_update() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 1. 插入用户
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "李四", "age": 30, "status": 1}),
    )
    .await;

    // 2. 更新用户
    let affected = execute(
        &pool,
        &manager,
        "update",
        &json!({
            "id": 1,
            "name": "李四改名",
            "age": 31,
            "status": 2
        }),
    )
    .await;
    assert_eq!(affected, 1);

    // 3. 验证更新
    let users: Vec<User> = query_all(&pool, &manager, "findById", &json!({"id": 1})).await;
    assert_eq!(users[0].name, "李四改名");
    assert_eq!(users[0].age, 31);
    assert_eq!(users[0].status, 2);
}

#[tokio::test]
async fn test_delete() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 1. 插入用户
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "王五", "age": 28, "status": 1}),
    )
    .await;

    // 2. 删除用户
    let affected = execute(&pool, &manager, "deleteById", &json!({"id": 1})).await;
    assert_eq!(affected, 1);

    // 3. 验证删除
    let users: Vec<User> = query_all(&pool, &manager, "findById", &json!({"id": 1})).await;
    assert!(users.is_empty());
}

#[tokio::test]
async fn test_dynamic_sql() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 插入多个用户
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "张三", "age": 20, "status": 1}),
    )
    .await;
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "张四", "age": 25, "status": 1}),
    )
    .await;
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "李五", "age": 30, "status": 2}),
    )
    .await;

    // 1. 无条件查询
    let users: Vec<User> = query_all(&pool, &manager, "findByCondition", &json!({})).await;
    assert_eq!(users.len(), 3);

    // 2. 按状态查询
    let users: Vec<User> =
        query_all(&pool, &manager, "findByCondition", &json!({"status": 1})).await;
    assert_eq!(users.len(), 2);

    // 3. 按名称模糊查询
    let users: Vec<User> =
        query_all(&pool, &manager, "findByCondition", &json!({"name": "张%"})).await;
    assert_eq!(users.len(), 2);

    // 4. 按年龄范围查询
    let users: Vec<User> =
        query_all(&pool, &manager, "findByCondition", &json!({"minAge": 25})).await;
    assert_eq!(users.len(), 2);

    // 5. 组合条件查询
    let users: Vec<User> = query_all(
        &pool,
        &manager,
        "findByCondition",
        &json!({"status": 1, "minAge": 22}),
    )
    .await;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "张四");
}

#[tokio::test]
async fn test_in_query() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 插入多个用户
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "用户1", "age": 20, "status": 1}),
    )
    .await;
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "用户2", "age": 25, "status": 1}),
    )
    .await;
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "用户3", "age": 30, "status": 1}),
    )
    .await;

    // IN 查询
    let users: Vec<User> =
        query_all(&pool, &manager, "findByIds", &json!({"ids": [1, 3]})).await;
    assert_eq!(users.len(), 2);
    assert_eq!(users[0].name, "用户1");
    assert_eq!(users[1].name, "用户3");
}

#[tokio::test]
async fn test_count() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 插入用户
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "A", "age": 20, "status": 1}),
    )
    .await;
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "B", "age": 25, "status": 1}),
    )
    .await;
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "C", "age": 30, "status": 2}),
    )
    .await;

    // 1. 总数
    let count = query_count(&pool, &manager, "count", &json!({})).await;
    assert_eq!(count, 3);

    // 2. 按状态统计
    let count = query_count(&pool, &manager, "count", &json!({"status": 1})).await;
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_find_all() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 插入用户
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "A", "age": 20, "status": 1}),
    )
    .await;
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "B", "age": 25, "status": 1}),
    )
    .await;

    // 查询全部
    let users: Vec<User> = query_all(&pool, &manager, "findAll", &json!({})).await;
    assert_eq!(users.len(), 2);
}

#[tokio::test]
async fn test_transaction() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 开启事务
    let mut tx = pool.begin().await.expect("开启事务失败");

    // 在事务中插入
    let sql = manager
        .render("insert", &json!({"name": "事务用户", "age": 22, "status": 1}))
        .unwrap();
    let result = ParamExtractor::extract(&sql, DbType::Sqlite);

    sqlx::query(&result.sql)
        .bind("事务用户")
        .bind(22i32)
        .bind(1i32)
        .execute(&mut *tx)
        .await
        .expect("事务插入失败");

    // 提交事务
    tx.commit().await.expect("提交事务失败");

    // 验证数据已提交
    let users: Vec<User> = query_all(&pool, &manager, "findById", &json!({"id": 1})).await;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "事务用户");
}

#[tokio::test]
async fn test_transaction_rollback() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 先插入一条数据
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "原始用户", "age": 20, "status": 1}),
    )
    .await;

    // 开启事务并回滚
    {
        let mut tx = pool.begin().await.expect("开启事务失败");

        let sql = manager
            .render(
                "insert",
                &json!({"name": "回滚用户", "age": 25, "status": 1}),
            )
            .unwrap();
        let result = ParamExtractor::extract(&sql, DbType::Sqlite);

        sqlx::query(&result.sql)
            .bind("回滚用户")
            .bind(25i32)
            .bind(1i32)
            .execute(&mut *tx)
            .await
            .expect("事务插入失败");

        // 不调用 commit，事务会自动回滚
        tx.rollback().await.expect("回滚失败");
    }

    // 验证数据已回滚
    let count = query_count(&pool, &manager, "count", &json!({})).await;
    assert_eq!(count, 1); // 只有原始用户
}

// ==================== 连接池测试 ====================

/// 创建带连接池配置的数据库
async fn setup_database_with_pool() -> SqlitePool {
    // 使用文件数据库而不是内存数据库，以便多个连接共享
    let pool = SqlitePoolOptions::new()
        .max_connections(10) // 最大 10 个连接
        .min_connections(2) // 最少保持 2 个连接
        .acquire_timeout(Duration::from_secs(5)) // 获取连接超时
        .idle_timeout(Duration::from_secs(60)) // 空闲连接超时
        .connect("sqlite::memory:?mode=rwc&cache=shared")
        .await
        .expect("创建连接池失败");

    // 创建用户表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_info (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            age INTEGER NOT NULL,
            status INTEGER NOT NULL DEFAULT 1
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("创建表失败");

    pool
}

#[tokio::test]
async fn test_connection_pool_basic() {
    let pool = setup_database_with_pool().await;
    let manager = setup_manager();

    // 测试连接池基本功能
    assert!(pool.size() >= 1, "连接池应至少有 1 个连接");

    // 插入数据
    execute(
        &pool,
        &manager,
        "insert",
        &json!({"name": "池用户", "age": 25, "status": 1}),
    )
    .await;

    // 查询数据
    let users: Vec<User> = query_all(&pool, &manager, "findById", &json!({"id": 1})).await;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "池用户");

    println!("连接池大小: {}", pool.size());
}

#[tokio::test]
async fn test_connection_pool_concurrent() {
    let pool = setup_database_with_pool().await;
    let manager = Arc::new(setup_manager());

    // 先插入一些测试数据
    for i in 0..10 {
        execute(
            &pool,
            &manager,
            "insert",
            &json!({"name": format!("用户{}", i), "age": 20 + i, "status": 1}),
        )
        .await;
    }

    // 并发查询测试
    let mut handles = vec![];
    let pool = Arc::new(pool);

    for i in 1..=10 {
        let pool = pool.clone();
        let manager = manager.clone();

        let handle = tokio::spawn(async move {
            // 每个任务执行多次查询
            for _ in 0..5 {
                let sql = manager.render("findById", &json!({"id": i})).unwrap();
                let result = ParamExtractor::extract(&sql, DbType::Sqlite);

                let rows: Vec<User> = sqlx::query_as(&result.sql)
                    .bind(i as i64)
                    .fetch_all(&*pool)
                    .await
                    .expect("并发查询失败");

                assert!(!rows.is_empty(), "应该查询到数据");
            }
        });

        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.expect("任务执行失败");
    }

    println!("并发测试完成，连接池大小: {}", pool.size());
}

#[tokio::test]
async fn test_connection_pool_mixed_operations() {
    let pool = setup_database_with_pool().await;
    let manager = Arc::new(setup_manager());
    let pool = Arc::new(pool);

    // 混合并发操作：插入、查询、更新、删除
    let mut handles = vec![];

    // 插入任务
    for i in 0..5 {
        let pool = pool.clone();
        let manager = manager.clone();
        let handle = tokio::spawn(async move {
            let sql = manager
                .render(
                    "insert",
                    &json!({"name": format!("并发用户{}", i), "age": 20 + i, "status": 1}),
                )
                .unwrap();
            let result = ParamExtractor::extract(&sql, DbType::Sqlite);

            sqlx::query(&result.sql)
                .bind(format!("并发用户{}", i))
                .bind((20 + i) as i32)
                .bind(1i32)
                .execute(&*pool)
                .await
                .expect("并发插入失败");
        });
        handles.push(handle);
    }

    // 等待插入完成
    for handle in handles {
        handle.await.expect("插入任务失败");
    }

    // 验证插入结果
    let sql = manager.render("count", &json!({})).unwrap();
    let result = ParamExtractor::extract(&sql, DbType::Sqlite);
    let row = sqlx::query(&result.sql)
        .fetch_one(&*pool)
        .await
        .expect("查询失败");
    let count: i64 = row.get("count");
    assert_eq!(count, 5, "应该插入 5 条数据");

    // 并发查询任务
    let mut query_handles = vec![];
    for _ in 0..10 {
        let pool = pool.clone();
        let manager = manager.clone();
        let handle = tokio::spawn(async move {
            let sql = manager.render("findAll", &json!({})).unwrap();
            let result = ParamExtractor::extract(&sql, DbType::Sqlite);

            let rows: Vec<User> = sqlx::query_as(&result.sql)
                .fetch_all(&*pool)
                .await
                .expect("并发查询失败");

            assert_eq!(rows.len(), 5, "应该查询到 5 条数据");
        });
        query_handles.push(handle);
    }

    for handle in query_handles {
        handle.await.expect("查询任务失败");
    }

    println!("混合操作测试完成，连接池大小: {}", pool.size());
}

#[tokio::test]
async fn test_connection_pool_stress() {
    let pool = SqlitePoolOptions::new()
        .max_connections(5) // 限制最大连接数
        .connect("sqlite::memory:?mode=rwc&cache=shared")
        .await
        .expect("创建连接池失败");

    // 创建表
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS stress_test (id INTEGER PRIMARY KEY, value TEXT)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let pool = Arc::new(pool);
    let mut handles = vec![];

    // 启动 20 个并发任务，超过连接池大小
    for i in 0..20 {
        let pool = pool.clone();
        let handle = tokio::spawn(async move {
            // 插入
            sqlx::query("INSERT INTO stress_test (value) VALUES (?)")
                .bind(format!("value_{}", i))
                .execute(&*pool)
                .await
                .expect("压力测试插入失败");

            // 查询
            let _: Vec<(i64, String)> =
                sqlx::query_as("SELECT id, value FROM stress_test")
                    .fetch_all(&*pool)
                    .await
                    .expect("压力测试查询失败");
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.expect("压力测试任务失败");
    }

    // 验证结果
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM stress_test")
        .fetch_one(&*pool)
        .await
        .unwrap();
    assert_eq!(count.0, 20, "应该插入 20 条数据");

    println!("压力测试完成，连接池最大连接数: 5，并发任务: 20");
}

// ============================================================================
// 新增：使用 repository 模块的事务和批量操作测试
// ============================================================================

use markdown_sql::{batch_execute, begin_transaction, query_list_tx, query_scalar_tx};

/// 测试使用 _tx 函数的手动事务
#[tokio::test]
async fn test_transaction_with_tx_functions() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 开启事务
    let mut tx = begin_transaction(&pool).await.expect("开启事务失败");

    // 在事务中插入数据
    let insert_sql = manager
        .render(
            "insert",
            &json!({"name": "事务用户1", "age": 25, "status": 1}),
        )
        .unwrap();
    let result = ParamExtractor::extract(&insert_sql, DbType::Sqlite);

    sqlx::query(&result.sql)
        .bind("事务用户1")
        .bind(25i32)
        .bind(1i32)
        .execute(&mut *tx)
        .await
        .expect("事务插入失败");

    // 在事务中查询
    let users: Vec<User> = query_list_tx(&manager, &mut tx, "findAll", &json!({}))
        .await
        .expect("事务查询失败");

    assert_eq!(users.len(), 1, "事务中应该有 1 条数据");

    // 提交事务
    tx.commit().await.expect("提交事务失败");

    // 提交后验证
    let count_sql = manager.render("count", &json!({})).unwrap();
    let result = ParamExtractor::extract(&count_sql, DbType::Sqlite);
    let row = sqlx::query(&result.sql)
        .fetch_one(&pool)
        .await
        .expect("查询失败");
    let count: i64 = row.get("count");
    assert_eq!(count, 1, "提交后应该有 1 条数据");

    println!("✅ 手动事务测试通过");
}

/// 测试事务回滚
#[tokio::test]
async fn test_transaction_rollback_with_tx_functions() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 开启事务
    let mut tx = begin_transaction(&pool).await.expect("开启事务失败");

    // 在事务中插入数据
    let insert_sql = manager
        .render(
            "insert",
            &json!({"name": "回滚用户", "age": 30, "status": 1}),
        )
        .unwrap();
    let result = ParamExtractor::extract(&insert_sql, DbType::Sqlite);

    sqlx::query(&result.sql)
        .bind("回滚用户")
        .bind(30i32)
        .bind(1i32)
        .execute(&mut *tx)
        .await
        .expect("事务插入失败");

    // 在事务中验证数据存在
    let count: i64 = query_scalar_tx(&manager, &mut tx, "count", &json!({}))
        .await
        .expect("事务查询失败");
    assert_eq!(count, 1, "事务中应该有 1 条数据");

    // 不提交，直接 drop（自动回滚）
    drop(tx);

    // 验证数据已回滚
    let count_sql = manager.render("count", &json!({})).unwrap();
    let result = ParamExtractor::extract(&count_sql, DbType::Sqlite);
    let row = sqlx::query(&result.sql)
        .fetch_one(&pool)
        .await
        .expect("查询失败");
    let count: i64 = row.get("count");
    assert_eq!(count, 0, "回滚后应该没有数据");

    println!("✅ 事务回滚测试通过");
}

/// 用于批量插入的参数结构体
#[derive(Debug, Serialize)]
struct InsertParams {
    name: String,
    age: i32,
    status: i32,
}

/// 测试批量操作
#[tokio::test]
async fn test_batch_execute() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 准备批量数据
    let users: Vec<InsertParams> = (0..5)
        .map(|i| InsertParams {
            name: format!("批量用户{}", i),
            age: 20 + i,
            status: 1,
        })
        .collect();

    // 批量插入
    let affected = batch_execute(&manager, &pool, "insert", &users)
        .await
        .expect("批量插入失败");

    assert_eq!(affected, 5, "应该插入 5 条数据");

    // 验证结果
    let count_sql = manager.render("count", &json!({})).unwrap();
    let result = ParamExtractor::extract(&count_sql, DbType::Sqlite);
    let row = sqlx::query(&result.sql)
        .fetch_one(&pool)
        .await
        .expect("查询失败");
    let count: i64 = row.get("count");
    assert_eq!(count, 5, "数据库中应该有 5 条数据");

    println!("✅ 批量操作测试通过");
}

/// 测试批量操作空数组
#[tokio::test]
async fn test_batch_execute_empty() {
    let pool = setup_database().await;
    let manager = setup_manager();

    // 空数组批量插入
    let users: Vec<InsertParams> = vec![];
    let affected = batch_execute(&manager, &pool, "insert", &users)
        .await
        .expect("批量插入失败");

    assert_eq!(affected, 0, "空数组应该返回 0");

    println!("✅ 空批量操作测试通过");
}

/// 测试事务内批量操作
#[tokio::test]
async fn test_batch_execute_in_transaction() {
    use markdown_sql::batch_execute_tx;

    let pool = setup_database().await;
    let manager = setup_manager();

    let mut tx = begin_transaction(&pool).await.expect("开启事务失败");

    // 准备批量数据
    let users: Vec<InsertParams> = (0..3)
        .map(|i| InsertParams {
            name: format!("事务批量用户{}", i),
            age: 25 + i,
            status: 1,
        })
        .collect();

    // 在事务内批量插入
    let affected = batch_execute_tx(&manager, &mut tx, "insert", &users)
        .await
        .expect("事务批量插入失败");

    assert_eq!(affected, 3, "应该插入 3 条数据");

    // 提交事务
    tx.commit().await.expect("提交事务失败");

    // 验证结果
    let count_sql = manager.render("count", &json!({})).unwrap();
    let result = ParamExtractor::extract(&count_sql, DbType::Sqlite);
    let row = sqlx::query(&result.sql)
        .fetch_one(&pool)
        .await
        .expect("查询失败");
    let count: i64 = row.get("count");
    assert_eq!(count, 3, "数据库中应该有 3 条数据");

    println!("✅ 事务内批量操作测试通过");
}
