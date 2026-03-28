//! 参数提取器
//!
//! 将 `#{param}` 语法转换为数据库占位符（`?` 或 `$1`），并提取参数列表。
//!
//! ## 示例
//!
//! ```text
//! 输入: SELECT * FROM user WHERE id = #{id} AND name = #{name}
//! 输出（MySQL）: SELECT * FROM user WHERE id = ? AND name = ?
//!       params: ["id", "name"]
//! 输出（PostgreSQL）: SELECT * FROM user WHERE id = $1 AND name = $2
//!       params: ["id", "name"]
//! ```

use once_cell::sync::Lazy;
use regex::Regex;

/// 数据库类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DbType {
    /// MySQL 数据库
    #[default]
    Mysql,
    /// SQLite 数据库
    Sqlite,
    /// PostgreSQL 数据库
    Postgres,
}

impl DbType {
    /// 获取占位符格式
    fn placeholder(&self, index: usize) -> String {
        match self {
            DbType::Mysql | DbType::Sqlite => "?".to_string(),
            DbType::Postgres => format!("${}", index),
        }
    }
}

/// SQL 渲染结果
#[derive(Debug, Clone)]
pub struct SqlResult {
    /// 带占位符的 SQL（`?` 或 `$1`）
    pub sql: String,
    /// 参数名列表（按出现顺序）
    pub params: Vec<String>,
}

/// 参数占位符正则：匹配 `#{param_name}`
/// 支持：`#{id}`, `#{user_name}`, `#{user.name}`
static PARAM_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"#\{(\w+(?:\.\w+)*)\}").unwrap());

/// 参数提取器
pub struct ParamExtractor;

impl ParamExtractor {
    /// 将 `#{param}` 转换为占位符并提取参数列表
    ///
    /// # 参数
    /// - `sql`: 包含 `#{param}` 的 SQL
    /// - `db_type`: 数据库类型
    ///
    /// # 返回
    /// - `SqlResult`: 包含转换后的 SQL 和参数列表
    ///
    /// # 示例
    ///
    /// ```
    /// use markdown_sql::param_extractor::{ParamExtractor, DbType};
    ///
    /// let result = ParamExtractor::extract(
    ///     "SELECT * FROM user WHERE id = #{id}",
    ///     DbType::Postgres
    /// );
    /// assert_eq!(result.sql, "SELECT * FROM user WHERE id = $1");
    /// assert_eq!(result.params, vec!["id"]);
    /// ```
    pub fn extract(sql: &str, db_type: DbType) -> SqlResult {
        let mut params = Vec::new();
        let mut index = 0;

        let new_sql = PARAM_RE
            .replace_all(sql, |caps: &regex::Captures| {
                let param_name = caps[1].to_string();
                params.push(param_name);
                index += 1;
                db_type.placeholder(index)
            })
            .to_string();

        SqlResult {
            sql: new_sql,
            params,
        }
    }

    /// 检查 SQL 中是否包含参数占位符
    pub fn has_params(sql: &str) -> bool {
        PARAM_RE.is_match(sql)
    }

    /// 统计 SQL 中的参数数量
    pub fn count_params(sql: &str) -> usize {
        PARAM_RE.find_iter(sql).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mysql() {
        let result = ParamExtractor::extract(
            "SELECT * FROM user WHERE id = #{id} AND name = #{name}",
            DbType::Mysql,
        );
        assert_eq!(result.sql, "SELECT * FROM user WHERE id = ? AND name = ?");
        assert_eq!(result.params, vec!["id", "name"]);
    }

    #[test]
    fn test_extract_postgres() {
        let result = ParamExtractor::extract(
            "SELECT * FROM user WHERE id = #{id} AND name = #{name}",
            DbType::Postgres,
        );
        assert_eq!(result.sql, "SELECT * FROM user WHERE id = $1 AND name = $2");
        assert_eq!(result.params, vec!["id", "name"]);
    }

    #[test]
    fn test_extract_nested_param() {
        let result = ParamExtractor::extract(
            "SELECT * FROM user WHERE name = #{user.name}",
            DbType::Mysql,
        );
        assert_eq!(result.sql, "SELECT * FROM user WHERE name = ?");
        assert_eq!(result.params, vec!["user.name"]);
    }

    #[test]
    fn test_extract_no_params() {
        let result = ParamExtractor::extract("SELECT * FROM user", DbType::Mysql);
        assert_eq!(result.sql, "SELECT * FROM user");
        assert!(result.params.is_empty());
    }

    #[test]
    fn test_extract_duplicate_params() {
        let result = ParamExtractor::extract(
            "SELECT * FROM user WHERE id = #{id} OR parent_id = #{id}",
            DbType::Postgres,
        );
        assert_eq!(
            result.sql,
            "SELECT * FROM user WHERE id = $1 OR parent_id = $2"
        );
        // 每次出现都是一个新参数
        assert_eq!(result.params, vec!["id", "id"]);
    }

    #[test]
    fn test_extract_insert() {
        let result = ParamExtractor::extract(
            "INSERT INTO user (name, age, status) VALUES (#{name}, #{age}, #{status})",
            DbType::Postgres,
        );
        assert_eq!(
            result.sql,
            "INSERT INTO user (name, age, status) VALUES ($1, $2, $3)"
        );
        assert_eq!(result.params, vec!["name", "age", "status"]);
    }

    #[test]
    fn test_has_params() {
        assert!(ParamExtractor::has_params("WHERE id = #{id}"));
        assert!(!ParamExtractor::has_params("WHERE id = 1"));
    }

    #[test]
    fn test_count_params() {
        assert_eq!(
            ParamExtractor::count_params("WHERE id = #{id} AND name = #{name}"),
            2
        );
        assert_eq!(ParamExtractor::count_params("SELECT * FROM user"), 0);
    }
}
