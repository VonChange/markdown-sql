//! PostgreSQL 数据库支持模块
//!
//! 提供 PostgreSQL 特定的查询和执行函数。

use serde::Serialize;
use serde_json::Value;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::{Arguments, FromRow, Postgres, Row, Transaction};

use super::traits::PgDbPool;
use crate::error::{MarkdownSqlError, Result};
use crate::manager::SqlManager;
use crate::param_extractor::ParamExtractor;

// ============================================================================
// Pool 版本
// ============================================================================

/// 查询列表（PostgreSQL）
pub async fn query_list<T, P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<Vec<T>>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
    P: Serialize,
    D: PgDbPool,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let rows = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_all(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(rows)
}

/// 查询单条（PostgreSQL，可选）
pub async fn query_optional<T, P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<Option<T>>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
    P: Serialize,
    D: PgDbPool,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let row = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_optional(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row)
}

/// 查询单条（PostgreSQL，必须存在）
pub async fn query_one<T, P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<T>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
    P: Serialize,
    D: PgDbPool,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let row = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_one(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row)
}

/// 查询标量值（PostgreSQL）
pub async fn query_scalar<P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<i64>
where
    P: Serialize,
    D: PgDbPool,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let row = sqlx::query_with(&sql, args)
        .fetch_one(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row.get::<i64, _>(0))
}

/// 执行更新（PostgreSQL）
pub async fn execute<P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<u64>
where
    P: Serialize,
    D: PgDbPool,
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
// Typed Pool 版本（类型感知参数绑定）
// ============================================================================

use crate::typed_params::TypedParamsPg;

/// 查询列表（PostgreSQL，类型感知）
pub async fn query_list_typed<T, P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<Vec<T>>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
    P: Serialize + TypedParamsPg,
    D: PgDbPool,
{
    // 1. 用 JSON 渲染模板、提取参数名
    let (sql, param_names, _json_value) = prepare_sql(manager, sql_id, params)?;

    // 2. 使用 TypedParams 绑定（保留类型）
    let mut args = PgArguments::default();
    params.bind_to_pg_args(&param_names, &mut args)?;

    // 3. 执行查询
    let rows = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_all(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(rows)
}

/// 查询单条（PostgreSQL，可选，类型感知）
pub async fn query_optional_typed<T, P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<Option<T>>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
    P: Serialize + TypedParamsPg,
    D: PgDbPool,
{
    let (sql, param_names, _json_value) = prepare_sql(manager, sql_id, params)?;

    let mut args = PgArguments::default();
    params.bind_to_pg_args(&param_names, &mut args)?;

    let row = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_optional(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row)
}

/// 查询单条（PostgreSQL，必须存在，类型感知）
pub async fn query_one_typed<T, P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<T>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
    P: Serialize + TypedParamsPg,
    D: PgDbPool,
{
    let (sql, param_names, _json_value) = prepare_sql(manager, sql_id, params)?;

    let mut args = PgArguments::default();
    params.bind_to_pg_args(&param_names, &mut args)?;

    let row = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_one(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row)
}

/// 查询标量值（PostgreSQL，类型感知）
pub async fn query_scalar_typed<P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<i64>
where
    P: Serialize + TypedParamsPg,
    D: PgDbPool,
{
    let (sql, param_names, _json_value) = prepare_sql(manager, sql_id, params)?;

    let mut args = PgArguments::default();
    params.bind_to_pg_args(&param_names, &mut args)?;

    let row = sqlx::query_with(&sql, args)
        .fetch_one(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row.get::<i64, _>(0))
}

/// 执行更新（PostgreSQL，类型感知）
pub async fn execute_typed<P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<u64>
where
    P: Serialize + TypedParamsPg,
    D: PgDbPool,
{
    let (sql, param_names, _json_value) = prepare_sql(manager, sql_id, params)?;

    let mut args = PgArguments::default();
    params.bind_to_pg_args(&param_names, &mut args)?;

    let result = sqlx::query_with(&sql, args)
        .execute(db.pool())
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(result.rows_affected())
}

// ============================================================================
// Typed Transaction 版本（类型感知参数绑定）
// ============================================================================

/// 在事务中查询列表（PostgreSQL，类型感知）
pub async fn query_list_typed_tx<'t, T, P>(
    manager: &SqlManager,
    tx: &mut Transaction<'t, Postgres>,
    sql_id: &str,
    params: &P,
) -> Result<Vec<T>>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
    P: Serialize + TypedParamsPg,
{
    let (sql, param_names, _json_value) = prepare_sql(manager, sql_id, params)?;

    let mut args = PgArguments::default();
    params.bind_to_pg_args(&param_names, &mut args)?;

    let rows = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_all(&mut **tx)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(rows)
}

/// 在事务中执行更新（PostgreSQL，类型感知）
pub async fn execute_typed_tx<'t, P>(
    manager: &SqlManager,
    tx: &mut Transaction<'t, Postgres>,
    sql_id: &str,
    params: &P,
) -> Result<u64>
where
    P: Serialize + TypedParamsPg,
{
    let (sql, param_names, _json_value) = prepare_sql(manager, sql_id, params)?;

    let mut args = PgArguments::default();
    params.bind_to_pg_args(&param_names, &mut args)?;

    let result = sqlx::query_with(&sql, args)
        .execute(&mut **tx)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(result.rows_affected())
}

// ============================================================================
// Transaction 版本
// ============================================================================

/// 在事务中查询列表（PostgreSQL）
pub async fn query_list_tx<'t, T, P>(
    manager: &SqlManager,
    tx: &mut Transaction<'t, Postgres>,
    sql_id: &str,
    params: &P,
) -> Result<Vec<T>>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
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

/// 在事务中执行更新（PostgreSQL）
pub async fn execute_tx<'t, P>(
    manager: &SqlManager,
    tx: &mut Transaction<'t, Postgres>,
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

/// 批量执行（PostgreSQL）
pub async fn batch_execute<P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    items: &[P],
) -> Result<u64>
where
    P: Serialize,
    D: PgDbPool,
{
    if items.is_empty() {
        return Ok(0);
    }

    let (sql, param_names, _) = prepare_sql(manager, sql_id, &items[0])?;
    let mut total_affected = 0u64;
    let mut tx = db.pool().begin().await.map_err(MarkdownSqlError::from)?;

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

    tx.commit().await.map_err(MarkdownSqlError::from)?;
    Ok(total_affected)
}

// ============================================================================
// 事务辅助
// ============================================================================

/// 开启事务（PostgreSQL）
pub async fn begin_transaction<D: PgDbPool>(db: &D) -> Result<Transaction<'static, Postgres>> {
    db.pool()
        .begin()
        .await
        .map_err(MarkdownSqlError::from)
}

// ============================================================================
// 内部辅助函数
// ============================================================================

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
            "[PostgreSQL] {} → {}\n  参数: {:?}\n  值: {}",
            sql_id,
            result.sql,
            result.params,
            json_value
        );
    }

    Ok((result.sql, result.params, json_value))
}

fn build_arguments(param_names: &[String], json_value: &Value) -> Result<PgArguments> {
    let mut args = PgArguments::default();

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

fn add_json_value_to_args(args: &mut PgArguments, value: &Value) -> Result<()> {
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
