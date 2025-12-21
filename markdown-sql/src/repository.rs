//! Repository 执行辅助模块
//!
//! 为 `#[repository]` 宏生成的代码提供运行时支持。
//!
//! ## 核心功能
//!
//! - 渲染 SQL 模板
//! - 从 serde_json::Value 动态绑定参数
//! - 支持多种返回类型（Vec、Option、标量、影响行数）
//! - 支持 Pool 和 Transaction 两种执行方式
//! - 批量操作支持

use serde::Serialize;
use serde_json::Value;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Arguments, FromRow, Row, Sqlite, Transaction};

use crate::database::DbPool;
use crate::error::{MarkdownSqlError, Result};
use crate::manager::SqlManager;
use crate::param_extractor::ParamExtractor;

/// Repository 基础 trait
///
/// 所有生成的 Repository 都实现此 trait
pub trait Repository {
    /// 获取 SQL 管理器
    fn sql_manager(&self) -> &SqlManager;
}

// ============================================================================
// Pool 版本（使用 DbPool trait）
// ============================================================================

/// 查询列表
///
/// 返回 `Vec<T>`
pub async fn query_list<T, P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<Vec<T>>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    P: Serialize,
    D: DbPool,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let rows = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_all(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(rows)
}

/// 查询单条（可选）
///
/// 返回 `Option<T>`
pub async fn query_optional<T, P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<Option<T>>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    P: Serialize,
    D: DbPool,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let row = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_optional(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row)
}

/// 查询单条（必须存在）
///
/// 返回 `T`，不存在则报错
pub async fn query_one<T, P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<T>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    P: Serialize,
    D: DbPool,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let row = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_one(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row)
}

/// 查询标量值（如 COUNT）
///
/// 返回 `i64`
pub async fn query_scalar<P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<i64>
where
    P: Serialize,
    D: DbPool,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let row = sqlx::query_with(&sql, args)
        .fetch_one(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row.get::<i64, _>(0))
}

/// 执行更新（INSERT/UPDATE/DELETE）
///
/// 返回影响行数
pub async fn execute<P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<u64>
where
    P: Serialize,
    D: DbPool,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let result = sqlx::query_with(&sql, args)
        .execute(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(result.rows_affected())
}

// ============================================================================
// Transaction 版本（直接使用 &mut Transaction）
// ============================================================================

/// 在事务中查询列表
pub async fn query_list_tx<'t, T, P>(
    manager: &SqlManager,
    tx: &mut Transaction<'t, Sqlite>,
    sql_id: &str,
    params: &P,
) -> Result<Vec<T>>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    P: Serialize,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let rows = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_all(&mut **tx)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(rows)
}

/// 在事务中查询单条（可选）
pub async fn query_optional_tx<'t, T, P>(
    manager: &SqlManager,
    tx: &mut Transaction<'t, Sqlite>,
    sql_id: &str,
    params: &P,
) -> Result<Option<T>>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    P: Serialize,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let row = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_optional(&mut **tx)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row)
}

/// 在事务中查询单条（必须存在）
pub async fn query_one_tx<'t, T, P>(
    manager: &SqlManager,
    tx: &mut Transaction<'t, Sqlite>,
    sql_id: &str,
    params: &P,
) -> Result<T>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    P: Serialize,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let row = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_one(&mut **tx)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row)
}

/// 在事务中查询标量值
pub async fn query_scalar_tx<'t, P>(
    manager: &SqlManager,
    tx: &mut Transaction<'t, Sqlite>,
    sql_id: &str,
    params: &P,
) -> Result<i64>
where
    P: Serialize,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let row = sqlx::query_with(&sql, args)
        .fetch_one(&mut **tx)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row.get::<i64, _>(0))
}

/// 在事务中执行更新
pub async fn execute_tx<'t, P>(
    manager: &SqlManager,
    tx: &mut Transaction<'t, Sqlite>,
    sql_id: &str,
    params: &P,
) -> Result<u64>
where
    P: Serialize,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let result = sqlx::query_with(&sql, args)
        .execute(&mut **tx)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(result.rows_affected())
}

// ============================================================================
// 批量操作
// ============================================================================

/// 批量执行（INSERT/UPDATE/DELETE）
///
/// 一条 SQL + 多组参数，在事务内执行
///
/// ## 示例
///
/// ```ignore
/// let users = vec![user1, user2, user3];
/// let affected = batch_execute(&manager, &pool, "insert", &users).await?;
/// ```
pub async fn batch_execute<P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    items: &[P],
) -> Result<u64>
where
    P: Serialize,
    D: DbPool,
{
    if items.is_empty() {
        return Ok(0);
    }

    // 使用第一个元素渲染 SQL（获取参数结构）
    let (sql, param_names, _) = prepare_sql(manager, sql_id, &items[0])?;

    let mut total_affected = 0u64;

    // 开启事务
    let mut tx = db.pool().begin().await.map_err(MarkdownSqlError::from)?;

    // 预编译复用：循环执行每个参数
    for item in items {
        let json_value = serde_json::to_value(item)
            .map_err(|e| MarkdownSqlError::ParamError(format!("参数序列化失败: {}", e)))?;

        let args = build_arguments(&param_names, &json_value)?;

        let result = sqlx::query_with(&sql, args)
            .execute(&mut *tx)
            .await
            .map_err(MarkdownSqlError::from)?;

        total_affected += result.rows_affected();
    }

    // 提交事务
    tx.commit().await.map_err(MarkdownSqlError::from)?;

    if manager.is_debug() {
        tracing::debug!(
            "[SQL] 批量执行 {} 完成，共 {} 条，影响 {} 行",
            sql_id,
            items.len(),
            total_affected
        );
    }

    Ok(total_affected)
}

/// 在事务中批量执行
///
/// 使用外部事务，不自动 commit
pub async fn batch_execute_tx<'t, P>(
    manager: &SqlManager,
    tx: &mut Transaction<'t, Sqlite>,
    sql_id: &str,
    items: &[P],
) -> Result<u64>
where
    P: Serialize,
{
    if items.is_empty() {
        return Ok(0);
    }

    // 使用第一个元素渲染 SQL
    let (sql, param_names, _) = prepare_sql(manager, sql_id, &items[0])?;

    let mut total_affected = 0u64;

    for item in items {
        let json_value = serde_json::to_value(item)
            .map_err(|e| MarkdownSqlError::ParamError(format!("参数序列化失败: {}", e)))?;

        let args = build_arguments(&param_names, &json_value)?;

        let result = sqlx::query_with(&sql, args)
            .execute(&mut **tx)
            .await
            .map_err(MarkdownSqlError::from)?;

        total_affected += result.rows_affected();
    }

    if manager.is_debug() {
        tracing::debug!(
            "[SQL] 批量执行(事务) {} 完成，共 {} 条，影响 {} 行",
            sql_id,
            items.len(),
            total_affected
        );
    }

    Ok(total_affected)
}

// ============================================================================
// 事务辅助函数
// ============================================================================

/// 开启事务
///
/// ## 示例
///
/// ```ignore
/// let mut tx = begin_transaction(&db).await?;
/// // 执行操作...
/// tx.commit().await?;
/// ```
pub async fn begin_transaction<D: DbPool>(db: &D) -> Result<Transaction<'static, Sqlite>> {
    db.pool()
        .begin()
        .await
        .map_err(MarkdownSqlError::from)
}

/// 事务闭包执行
///
/// 自动处理 commit/rollback
///
/// ## 示例
///
/// ```ignore
/// with_transaction(&db, |tx| Box::pin(async move {
///     execute_tx(&manager, tx, "insert", &user).await?;
///     execute_tx(&manager, tx, "update", &order).await?;
///     Ok(())
/// })).await?;
/// ```
pub async fn with_transaction<D, F, T>(db: &D, f: F) -> Result<T>
where
    D: DbPool,
    F: for<'t> FnOnce(
        &'t mut Transaction<'static, Sqlite>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send + 't>>,
{
    let mut tx = db.pool().begin().await.map_err(MarkdownSqlError::from)?;

    let result = f(&mut tx).await;

    match result {
        Ok(value) => {
            tx.commit().await.map_err(MarkdownSqlError::from)?;
            Ok(value)
        }
        Err(e) => {
            // Transaction 会在 drop 时自动 rollback
            Err(e)
        }
    }
}

// ============================================================================
// 内部辅助函数
// ============================================================================

/// 准备 SQL
fn prepare_sql<P: Serialize>(
    manager: &SqlManager,
    sql_id: &str,
    params: &P,
) -> Result<(String, Vec<String>, Value)> {
    let json_value = serde_json::to_value(params)
        .map_err(|e| MarkdownSqlError::ParamError(format!("参数序列化失败: {}", e)))?;

    let rendered = manager.render(sql_id, &json_value)?;
    let result = ParamExtractor::extract(&rendered, manager.db_type());

    if manager.is_debug() {
        tracing::debug!(
            "[SQL] {} → {}\n  参数: {:?}\n  值: {}",
            sql_id,
            result.sql,
            result.params,
            json_value
        );
    }

    Ok((result.sql, result.params, json_value))
}

/// 从 JSON 构建 SQLite 参数
fn build_arguments(param_names: &[String], json_value: &Value) -> Result<SqliteArguments<'static>> {
    let mut args = SqliteArguments::default();

    for name in param_names {
        let value = if name.starts_with("__bind_") {
            if let Some(idx_str) = name.strip_prefix("__bind_") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    find_array_element(json_value, idx)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            get_nested_value(json_value, name)
        };

        let value = value.unwrap_or(Value::Null);
        add_json_value_to_args(&mut args, &value)?;
    }

    Ok(args)
}

/// 获取嵌套 JSON 值
fn get_nested_value(value: &Value, path: &str) -> Option<Value> {
    if path.contains('.') {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;
        for part in parts {
            current = current.get(part)?;
        }
        Some(current.clone())
    } else {
        value.get(path).cloned()
    }
}

/// 从 JSON 中查找数组元素
fn find_array_element(value: &Value, idx: usize) -> Option<Value> {
    if let Some(obj) = value.as_object() {
        for (_, v) in obj {
            if let Some(arr) = v.as_array() {
                if idx < arr.len() {
                    return arr.get(idx).cloned();
                }
            }
        }
    }
    None
}

/// 将 JSON 值添加到参数列表
fn add_json_value_to_args(args: &mut SqliteArguments<'static>, value: &Value) -> Result<()> {
    match value {
        Value::Null => {
            args.add(Option::<String>::None)
                .map_err(|e| MarkdownSqlError::ParamError(format!("绑定 NULL 失败: {}", e)))?;
        }
        Value::Bool(b) => {
            args.add(*b)
                .map_err(|e| MarkdownSqlError::ParamError(format!("绑定 Bool 失败: {}", e)))?;
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                args.add(i)
                    .map_err(|e| MarkdownSqlError::ParamError(format!("绑定 i64 失败: {}", e)))?;
            } else if let Some(f) = n.as_f64() {
                args.add(f)
                    .map_err(|e| MarkdownSqlError::ParamError(format!("绑定 f64 失败: {}", e)))?;
            }
        }
        Value::String(s) => {
            args.add(s.clone())
                .map_err(|e| MarkdownSqlError::ParamError(format!("绑定 String 失败: {}", e)))?;
        }
        Value::Array(_) | Value::Object(_) => {
            let json_str = serde_json::to_string(value)
                .map_err(|e| MarkdownSqlError::ParamError(format!("序列化 JSON 失败: {}", e)))?;
            args.add(json_str)
                .map_err(|e| MarkdownSqlError::ParamError(format!("绑定 JSON 失败: {}", e)))?;
        }
    }
    Ok(())
}

/// 空参数结构体
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct EmptyParams;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_nested_value() {
        let value = json!({
            "user": {
                "name": "test",
                "age": 25
            },
            "id": 1
        });

        assert_eq!(get_nested_value(&value, "id"), Some(json!(1)));
        assert_eq!(get_nested_value(&value, "user.name"), Some(json!("test")));
        assert_eq!(get_nested_value(&value, "user.age"), Some(json!(25)));
        assert_eq!(get_nested_value(&value, "unknown"), None);
    }

    #[test]
    fn test_build_arguments() {
        let names = vec!["name".to_string(), "age".to_string()];
        let value = json!({
            "name": "test",
            "age": 25
        });

        let result = build_arguments(&names, &value);
        assert!(result.is_ok());
    }
}
