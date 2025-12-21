//! Repository 执行辅助模块
//!
//! 为 `#[repository]` 宏生成的代码提供运行时支持。
//!
//! ## 核心功能
//!
//! - 渲染 SQL 模板
//! - 从 serde_json::Value 动态绑定参数
//! - 支持多种返回类型（Vec、Option、标量、影响行数）

use serde::Serialize;
use serde_json::Value;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Arguments, FromRow, Pool, Row, Sqlite};

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

/// 查询列表
///
/// 返回 `Vec<T>`
pub async fn query_list<T, P>(
    manager: &SqlManager,
    pool: &Pool<Sqlite>,
    sql_id: &str,
    params: &P,
) -> Result<Vec<T>>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    P: Serialize,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;

    // 构建动态参数
    let args = build_arguments(&param_names, &json_value)?;

    // 执行查询
    let rows = sqlx::query_as_with::<_, T, _>(&sql, args)
        .fetch_all(pool)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(rows)
}

/// 查询单条（可选）
///
/// 返回 `Option<T>`
pub async fn query_optional<T, P>(
    manager: &SqlManager,
    pool: &Pool<Sqlite>,
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
        .fetch_optional(pool)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row)
}

/// 查询单条（必须存在）
///
/// 返回 `T`，不存在则报错
pub async fn query_one<T, P>(
    manager: &SqlManager,
    pool: &Pool<Sqlite>,
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
        .fetch_one(pool)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row)
}

/// 查询标量值（如 COUNT）
///
/// 返回 `i64`
pub async fn query_scalar<P>(
    manager: &SqlManager,
    pool: &Pool<Sqlite>,
    sql_id: &str,
    params: &P,
) -> Result<i64>
where
    P: Serialize,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let row = sqlx::query_with(&sql, args)
        .fetch_one(pool)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(row.get::<i64, _>(0))
}

/// 执行更新（INSERT/UPDATE/DELETE）
///
/// 返回影响行数
pub async fn execute<P>(
    manager: &SqlManager,
    pool: &Pool<Sqlite>,
    sql_id: &str,
    params: &P,
) -> Result<u64>
where
    P: Serialize,
{
    let (sql, param_names, json_value) = prepare_sql(manager, sql_id, params)?;
    let args = build_arguments(&param_names, &json_value)?;

    let result = sqlx::query_with(&sql, args)
        .execute(pool)
        .await
        .map_err(MarkdownSqlError::from)?;

    Ok(result.rows_affected())
}

/// 准备 SQL
///
/// 渲染模板、提取参数、序列化参数值
fn prepare_sql<P: Serialize>(
    manager: &SqlManager,
    sql_id: &str,
    params: &P,
) -> Result<(String, Vec<String>, Value)> {
    // 序列化参数为 JSON
    let json_value = serde_json::to_value(params)
        .map_err(|e| MarkdownSqlError::ParamError(format!("参数序列化失败: {}", e)))?;

    // 渲染 SQL 模板
    let rendered = manager.render(sql_id, &json_value)?;

    // 提取参数
    let result = ParamExtractor::extract(&rendered, manager.db_type());

    // Debug 日志
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
        // 处理 __bind_N 格式的参数（bind_join 生成的）
        let value = if name.starts_with("__bind_") {
            // 从原始数组中获取
            // __bind_0 对应第一个数组元素
            if let Some(idx_str) = name.strip_prefix("__bind_") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    // 查找包含数组的字段
                    find_array_element(json_value, idx)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            // 普通参数
            get_nested_value(json_value, name)
        };

        let value = value.unwrap_or(Value::Null);
        add_json_value_to_args(&mut args, &value)?;
    }

    Ok(args)
}

/// 获取嵌套 JSON 值
///
/// 支持 `user.name` 格式
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
///
/// 用于处理 bind_join 生成的 __bind_N 参数
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
            // 需要 clone 以满足 'static 生命周期
            args.add(s.clone())
                .map_err(|e| MarkdownSqlError::ParamError(format!("绑定 String 失败: {}", e)))?;
        }
        Value::Array(_) | Value::Object(_) => {
            // 数组和对象序列化为 JSON 字符串
            let json_str = serde_json::to_string(value)
                .map_err(|e| MarkdownSqlError::ParamError(format!("序列化 JSON 失败: {}", e)))?;
            args.add(json_str)
                .map_err(|e| MarkdownSqlError::ParamError(format!("绑定 JSON 失败: {}", e)))?;
        }
    }
    Ok(())
}

/// 空参数结构体
///
/// 用于无参数的查询
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
