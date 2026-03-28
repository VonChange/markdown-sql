//! Demo 测试
//!
//! 演示 markdown-sql 的各项功能：
//! - SQL 模板解析
//! - 动态 SQL 渲染
//! - 参数占位符转换
//! - 多数据库支持

use markdown_sql::{DbType, ParamExtractor, SqlManager};
use serde_json::json;

/// 创建 SQL 管理器并加载 SQL
fn setup_manager() -> SqlManager {
    let mut manager = SqlManager::builder()
        .db_type(DbType::Postgres)
        .debug(true)
        .build()
        .expect("创建 SqlManager 失败");

    manager
        .load_file("tests/sql/UserRepository.md")
        .expect("加载 SQL 文件失败");

    manager
}

#[test]
fn test_load_sql_file() {
    let manager = setup_manager();

    // 验证 SQL 已加载
    assert!(manager.contains("findById"));
    assert!(manager.contains("findAll"));
    assert!(manager.contains("insert"));
    assert!(manager.contains("update"));
    assert!(manager.contains("deleteById"));
    assert!(manager.contains("count"));

    println!("✅ SQL 文件加载成功");
}

#[test]
fn test_simple_query() {
    let manager = setup_manager();

    // 渲染简单查询
    let sql = manager.render("findById", &json!({"id": 1})).unwrap();
    println!("渲染后 SQL:\n{}\n", sql);

    // 提取参数（PostgreSQL）
    let result = ParamExtractor::extract(&sql, DbType::Postgres);
    println!("PostgreSQL SQL: {}", result.sql);
    println!("参数列表: {:?}\n", result.params);

    assert!(result.sql.contains("$1"));
    assert_eq!(result.params, vec!["id"]);
}

#[test]
fn test_dynamic_sql_with_conditions() {
    let manager = setup_manager();

    // 测试动态 SQL - 有条件
    let sql = manager
        .render(
            "findByCondition",
            &json!({
                "name": "%test%",
                "status": 1,
                "minAge": 18
            }),
        )
        .unwrap();
    println!("动态 SQL（有条件）:\n{}\n", sql);

    assert!(sql.contains("AND name LIKE"));
    assert!(sql.contains("AND status ="));
    assert!(sql.contains("AND age >="));

    let result = ParamExtractor::extract(&sql, DbType::Postgres);
    println!("最终 SQL: {}", result.sql);
    println!("参数列表: {:?}\n", result.params);

    assert_eq!(result.params.len(), 3);
}

#[test]
fn test_dynamic_sql_without_conditions() {
    let manager = setup_manager();

    // 测试动态 SQL - 无条件
    let sql = manager.render("findByCondition", &json!({})).unwrap();
    println!("动态 SQL（无条件）:\n{}\n", sql);

    // 不应该包含 AND 条件
    assert!(!sql.contains("AND name"));
    assert!(!sql.contains("AND status"));
    assert!(!sql.contains("AND age"));
}

#[test]
fn test_in_query_with_bind_join() {
    let manager = setup_manager();

    // 测试 IN 查询（使用 bind_join 过滤器）
    let sql = manager
        .render("findByIds", &json!({"ids": [1, 2, 3, 4, 5]}))
        .unwrap();
    println!("IN 查询 SQL:\n{}\n", sql);

    // 应该展开为多个 #{__bind_N}
    assert!(sql.contains("#{__bind_0}"));
    assert!(sql.contains("#{__bind_4}"));

    let result = ParamExtractor::extract(&sql, DbType::Postgres);
    println!("最终 SQL: {}", result.sql);
    println!("参数列表: {:?}\n", result.params);

    // 应该有 5 个占位符
    assert!(result.sql.contains("$1,$2,$3,$4,$5"));
    assert_eq!(result.params.len(), 5);
}

#[test]
fn test_insert_sql() {
    let manager = setup_manager();

    // 测试插入语句
    let sql = manager
        .render(
            "insert",
            &json!({
                "name": "新用户",
                "age": 25,
                "status": 1
            }),
        )
        .unwrap();
    println!("INSERT SQL:\n{}\n", sql);

    let result = ParamExtractor::extract(&sql, DbType::Postgres);
    println!("最终 SQL: {}", result.sql);
    println!("参数列表: {:?}\n", result.params);

    assert!(result.sql.contains("$1, $2, $3"));
    assert_eq!(result.params, vec!["name", "age", "status"]);
}

#[test]
fn test_update_sql() {
    let manager = setup_manager();

    // 测试更新语句
    let sql = manager
        .render(
            "update",
            &json!({
                "id": 1,
                "name": "更新名称",
                "age": 30,
                "status": 2
            }),
        )
        .unwrap();
    println!("UPDATE SQL:\n{}\n", sql);

    let result = ParamExtractor::extract(&sql, DbType::Postgres);
    println!("最终 SQL: {}", result.sql);
    println!("参数列表: {:?}\n", result.params);

    assert_eq!(result.params, vec!["name", "age", "status", "id"]);
}

#[test]
fn test_mysql_placeholder() {
    let manager = setup_manager();

    // 测试 MySQL 占位符（使用 ? 而不是 $N）
    let sql = manager.render("findById", &json!({"id": 1})).unwrap();
    let result = ParamExtractor::extract(&sql, DbType::Mysql);

    println!("MySQL SQL: {}", result.sql);
    println!("参数列表: {:?}\n", result.params);

    assert!(result.sql.contains("?"));
    assert!(!result.sql.contains("$1"));
}

#[test]
fn test_sqlite_placeholder() {
    let manager = setup_manager();

    // 测试 SQLite 占位符（使用 ? 而不是 $N）
    let sql = manager.render("findById", &json!({"id": 1})).unwrap();
    let result = ParamExtractor::extract(&sql, DbType::Sqlite);

    println!("SQLite SQL: {}", result.sql);
    println!("参数列表: {:?}\n", result.params);

    assert!(result.sql.contains("?"));
    assert!(!result.sql.contains("$1"));
}

#[test]
fn test_include_directive() {
    let manager = setup_manager();

    // 测试 {% include %} 指令
    // findById 使用了 {% include "columns" %}
    let sql = manager.render("findById", &json!({"id": 1})).unwrap();

    // 应该包含 columns 定义的字段
    assert!(sql.contains("id, name, age, status"));
    println!("Include 测试通过: SQL 包含引用的字段\n{}", sql);
}

#[test]
fn test_count_query() {
    let manager = setup_manager();

    // 测试 COUNT 查询 - 无条件
    let sql = manager.render("count", &json!({})).unwrap();
    println!("COUNT SQL（无条件）:\n{}\n", sql);
    assert!(sql.contains("COUNT(*)"));

    // 测试 COUNT 查询 - 有条件
    let sql = manager.render("count", &json!({"status": 1})).unwrap();
    println!("COUNT SQL（有条件）:\n{}\n", sql);
    assert!(sql.contains("AND status ="));
}

#[test]
fn test_param_extractor_no_params() {
    // 测试无参数的 SQL
    let sql = "SELECT * FROM user_info";
    let result = ParamExtractor::extract(sql, DbType::Postgres);

    assert_eq!(result.sql, sql);
    assert!(result.params.is_empty());
}

#[test]
fn test_param_extractor_multiple_same_params() {
    // 测试重复参数
    let sql = "SELECT * FROM user_info WHERE name = #{name} OR alias = #{name}";
    let result = ParamExtractor::extract(sql, DbType::Postgres);

    println!("重复参数 SQL: {}", result.sql);
    println!("参数列表: {:?}", result.params);

    // 参数应该按出现顺序列出
    assert_eq!(result.params, vec!["name", "name"]);
}
