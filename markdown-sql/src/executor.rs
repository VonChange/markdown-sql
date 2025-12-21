//! SQL 执行器
//!
//! 封装 sqlx 执行逻辑，提供参数绑定和批量操作支持。
//!
//! ## 核心功能
//!
//! - 渲染 SQL 模板
//! - 提取参数并绑定到 sqlx
//! - 支持查询（Vec/Option）和更新（影响行数）
//! - 批量操作（预编译复用）
//! - Debug 日志

use std::time::Instant;

use serde::Serialize;
use tracing::debug;

use crate::error::Result;
use crate::manager::SqlManager;
use crate::param_extractor::{ParamExtractor, SqlResult};

/// SQL 执行上下文
///
/// 封装一次 SQL 执行的上下文信息
#[derive(Debug)]
pub struct ExecuteContext {
    /// SQL ID
    pub sql_id: String,
    /// 渲染后的 SQL（带 #{param}）
    pub rendered_sql: String,
    /// 提取后的 SQL（带 ? 或 $1）
    pub final_sql: String,
    /// 参数名列表
    pub param_names: Vec<String>,
    /// 执行时间
    pub duration: Option<std::time::Duration>,
    /// 影响行数
    pub rows_affected: Option<u64>,
}

impl ExecuteContext {
    /// 创建执行上下文
    pub fn new(sql_id: &str, rendered_sql: String, sql_result: SqlResult) -> Self {
        Self {
            sql_id: sql_id.to_string(),
            rendered_sql,
            final_sql: sql_result.sql,
            param_names: sql_result.params,
            duration: None,
            rows_affected: None,
        }
    }

    /// 记录执行时间
    pub fn set_duration(&mut self, duration: std::time::Duration) {
        self.duration = Some(duration);
    }

    /// 记录影响行数
    pub fn set_rows_affected(&mut self, rows: u64) {
        self.rows_affected = Some(rows);
    }

    /// 生成 Debug 日志
    pub fn log(&self) {
        debug!(
            "Executing: {}\n  SQL: {}\n  Params: {:?}\n  Duration: {:?}\n  Rows: {:?}",
            self.sql_id,
            self.final_sql,
            self.param_names,
            self.duration,
            self.rows_affected
        );
    }
}

/// SQL 执行器
///
/// 负责执行 SQL 并处理结果
pub struct SqlExecutor<'a> {
    manager: &'a SqlManager,
}

impl<'a> SqlExecutor<'a> {
    /// 创建执行器
    pub fn new(manager: &'a SqlManager) -> Self {
        Self { manager }
    }

    /// 准备 SQL 执行
    ///
    /// 渲染模板并提取参数
    pub fn prepare<T: Serialize>(&self, sql_id: &str, params: &T) -> Result<ExecuteContext> {
        // 渲染模板
        let rendered_sql = self.manager.render(sql_id, params)?;

        // 提取参数
        let sql_result = ParamExtractor::extract(&rendered_sql, self.manager.db_type());

        Ok(ExecuteContext::new(sql_id, rendered_sql, sql_result))
    }

    /// 获取管理器
    pub fn manager(&self) -> &SqlManager {
        self.manager
    }
}

/// 批量执行器
///
/// 支持一条 SQL + 多组参数的批量执行
#[allow(dead_code)]
pub struct BatchExecutor {
    /// SQL 模板（带 #{param}）用于调试
    sql_template: String,
    /// 提取后的 SQL（带占位符）
    final_sql: String,
    /// 参数名列表
    param_names: Vec<String>,
    /// 是否开启 Debug
    debug: bool,
}

impl BatchExecutor {
    /// 创建批量执行器
    pub fn new(sql_template: String, final_sql: String, param_names: Vec<String>) -> Self {
        Self {
            sql_template,
            final_sql,
            param_names,
            debug: false,
        }
    }

    /// 从 SQL 管理器创建批量执行器
    pub fn from_manager<T: Serialize>(
        manager: &SqlManager,
        sql_id: &str,
        sample_params: &T,
    ) -> Result<Self> {
        // 使用示例参数渲染 SQL（确保条件正确）
        let rendered = manager.render(sql_id, sample_params)?;
        let sql_result = ParamExtractor::extract(&rendered, manager.db_type());

        Ok(Self {
            sql_template: rendered,
            final_sql: sql_result.sql,
            param_names: sql_result.params,
            debug: manager.is_debug(),
        })
    }

    /// 设置 Debug 模式
    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }

    /// 获取 SQL
    pub fn sql(&self) -> &str {
        &self.final_sql
    }

    /// 获取参数名列表
    pub fn param_names(&self) -> &[String] {
        &self.param_names
    }

    /// 记录批量执行日志
    pub fn log_batch(&self, batch_size: usize, total_affected: u64, duration: std::time::Duration) {
        if self.debug {
            debug!(
                "Batch Execute:\n  SQL: {}\n  Batch Size: {}\n  Total Affected: {}\n  Duration: {:?}",
                self.final_sql, batch_size, total_affected, duration
            );
        }
    }
}

/// 参数绑定辅助结构
///
/// 用于从实体提取参数值
pub struct ParamBinder<'a> {
    /// 参数名列表
    param_names: &'a [String],
}

impl<'a> ParamBinder<'a> {
    /// 创建参数绑定器
    pub fn new(param_names: &'a [String]) -> Self {
        Self { param_names }
    }

    /// 获取参数名列表
    pub fn param_names(&self) -> &[String] {
        self.param_names
    }

    /// 从 JSON 值提取参数
    ///
    /// 根据参数名列表从 JSON 对象中提取对应的值
    pub fn extract_from_json(
        &self,
        value: &serde_json::Value,
    ) -> Result<Vec<serde_json::Value>> {
        let mut values = Vec::new();

        for name in self.param_names {
            // 支持嵌套属性：user.name -> value["user"]["name"]
            let val = if name.contains('.') {
                let parts: Vec<&str> = name.split('.').collect();
                let mut current = value;
                for part in parts {
                    current = current.get(part).unwrap_or(&serde_json::Value::Null);
                }
                current.clone()
            } else {
                value
                    .get(name)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null)
            };

            values.push(val);
        }

        Ok(values)
    }
}

/// 计时工具
pub struct Timer {
    start: Instant,
}

impl Timer {
    /// 开始计时
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// 获取已过时间
    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_execute_context() {
        let sql_result = SqlResult {
            sql: "SELECT * FROM user WHERE id = $1".to_string(),
            params: vec!["id".to_string()],
        };

        let mut ctx = ExecuteContext::new(
            "findById",
            "SELECT * FROM user WHERE id = #{id}".to_string(),
            sql_result,
        );

        assert_eq!(ctx.sql_id, "findById");
        assert!(ctx.final_sql.contains("$1"));

        ctx.set_duration(std::time::Duration::from_millis(5));
        ctx.set_rows_affected(1);

        assert!(ctx.duration.is_some());
        assert_eq!(ctx.rows_affected, Some(1));
    }

    #[test]
    fn test_param_binder() {
        let names = vec!["id".to_string(), "name".to_string()];
        let binder = ParamBinder::new(&names);

        let value = json!({
            "id": 1,
            "name": "test",
            "extra": "ignored"
        });

        let values = binder.extract_from_json(&value).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], json!(1));
        assert_eq!(values[1], json!("test"));
    }

    #[test]
    fn test_param_binder_nested() {
        let names = vec!["user.id".to_string(), "user.name".to_string()];
        let binder = ParamBinder::new(&names);

        let value = json!({
            "user": {
                "id": 1,
                "name": "test"
            }
        });

        let values = binder.extract_from_json(&value).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], json!(1));
        assert_eq!(values[1], json!("test"));
    }

    #[test]
    fn test_batch_executor() {
        let executor = BatchExecutor::new(
            "INSERT INTO user (name) VALUES (#{name})".to_string(),
            "INSERT INTO user (name) VALUES ($1)".to_string(),
            vec!["name".to_string()],
        );

        assert!(executor.sql().contains("$1"));
        assert_eq!(executor.param_names(), &["name".to_string()]);
    }

    #[test]
    fn test_timer() {
        let timer = Timer::start();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed();
        assert!(elapsed.as_millis() >= 10);
    }
}
