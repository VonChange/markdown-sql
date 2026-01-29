//! 类型感知的参数绑定
//!
//! 通过 `#[derive(TypedParams)]` 自动为参数结构体生成类型感知的绑定代码，
//! 避免通过 JSON 序列化丢失类型信息。
//!
//! ## 背景
//!
//! 原有的参数绑定流程：
//! ```text
//! NaiveDateTime → serde 序列化 → "2026-01-29T20:00:00" → 绑定为 String → 类型错误
//! ```
//!
//! 使用 TypedParams 后：
//! ```text
//! NaiveDateTime → 直接绑定 → 数据库 TIMESTAMP 类型 → 正确
//! ```
//!
//! ## 使用方式
//!
//! ```ignore
//! use markdown_sql::TypedParams;
//! use chrono::NaiveDateTime;
//!
//! #[derive(Serialize, TypedParams)]
//! struct LogInsert {
//!     log_path: String,
//!     expires_date: Option<NaiveDateTime>,  // 类型保留
//! }
//! ```

use crate::error::Result;

// ============================================================================
// PostgreSQL
// ============================================================================

#[cfg(feature = "postgres")]
use sqlx::postgres::PgArguments;

/// PostgreSQL 类型感知参数绑定
///
/// 通过 `#[derive(TypedParams)]` 自动实现
#[cfg(feature = "postgres")]
pub trait TypedParamsPg {
    /// 根据参数名列表，将字段值绑定到 PgArguments
    ///
    /// # 参数
    /// - `param_names`: SQL 中提取的参数名列表（如 `["log_path", "expires_date"]`）
    /// - `args`: sqlx 的参数容器
    ///
    /// # 返回
    /// - `Ok(())`: 绑定成功
    /// - `Err`: 绑定失败（如参数类型不支持）
    fn bind_to_pg_args(&self, param_names: &[String], args: &mut PgArguments) -> Result<()>;
}

// ============================================================================
// MySQL
// ============================================================================

#[cfg(feature = "mysql")]
use sqlx::mysql::MySqlArguments;

/// MySQL 类型感知参数绑定
#[cfg(feature = "mysql")]
pub trait TypedParamsMySql {
    fn bind_to_mysql_args(&self, param_names: &[String], args: &mut MySqlArguments) -> Result<()>;
}

// ============================================================================
// SQLite
// ============================================================================

#[cfg(feature = "sqlite")]
use sqlx::sqlite::SqliteArguments;

/// SQLite 类型感知参数绑定
#[cfg(feature = "sqlite")]
pub trait TypedParamsSqlite {
    fn bind_to_sqlite_args<'q>(
        &'q self,
        param_names: &[String],
        args: &mut SqliteArguments<'q>,
    ) -> Result<()>;
}

// ============================================================================
// EmptyParams 定义和实现
// ============================================================================

/// 空参数结构体（用于无参数的查询）
///
/// 由 `#[repository]` 宏在无参数方法中自动使用
#[derive(Debug, Clone, Default)]
pub struct EmptyParams;

// 为 EmptyParams 实现 Serialize（serde）
impl serde::Serialize for EmptyParams {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let map = serializer.serialize_map(Some(0))?;
        map.end()
    }
}

/// PostgreSQL TypedParams 实现
#[cfg(feature = "postgres")]
impl TypedParamsPg for EmptyParams {
    fn bind_to_pg_args(&self, _param_names: &[String], _args: &mut PgArguments) -> Result<()> {
        // 空实现：无参数需要绑定
        Ok(())
    }
}

/// MySQL TypedParams 实现
#[cfg(feature = "mysql")]
impl TypedParamsMySql for EmptyParams {
    fn bind_to_mysql_args(
        &self,
        _param_names: &[String],
        _args: &mut MySqlArguments,
    ) -> Result<()> {
        Ok(())
    }
}

/// SQLite TypedParams 实现
#[cfg(feature = "sqlite")]
impl TypedParamsSqlite for EmptyParams {
    fn bind_to_sqlite_args<'q>(
        &'q self,
        _param_names: &[String],
        _args: &mut SqliteArguments<'q>,
    ) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_empty_params() {
        // EmptyParams 实现测试会在集成测试中进行
    }
}
