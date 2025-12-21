//! 错误类型定义
//!
//! 定义 markdown-sql 框架的所有错误类型
//!
//! ## 错误分类
//!
//! | 类型 | 说明 | 场景 |
//! |------|------|------|
//! | `FileNotFound` | 文件未找到 | SQL 文件不存在 |
//! | `SqlNotFound` | SQL 未找到 | sqlId 不存在 |
//! | `ParamMissing` | 参数缺失 | 模板需要的参数未提供 |
//! | `RenderError` | 渲染错误 | 模板渲染失败 |
//! | `SqlxError` | SQL 执行错误 | 数据库操作失败 |
//! | `TransactionError` | 事务错误 | 事务操作失败 |

use thiserror::Error;

/// markdown-sql 错误类型
#[derive(Debug, Error)]
pub enum MarkdownSqlError {
    // ============ 文件相关 ============

    /// 文件未找到
    #[error("文件未找到: {path}")]
    FileNotFound {
        /// 文件路径
        path: String,
    },

    /// 无效的文件路径
    #[error("无效的文件路径: {path} - {reason}")]
    InvalidPath {
        /// 文件路径
        path: String,
        /// 原因
        reason: String,
    },

    // ============ SQL 相关 ============

    /// SQL 未找到
    #[error("SQL 未找到: {sql_id} (文件: {file})")]
    SqlNotFound {
        /// SQL ID
        sql_id: String,
        /// 所在文件
        file: String,
    },

    /// 模板解析错误
    #[error("模板解析错误: {sql_id} - {message}")]
    TemplateError {
        /// SQL ID
        sql_id: String,
        /// 错误信息
        message: String,
    },

    /// 模板渲染错误
    #[error("模板渲染错误: {sql_id} - {message}")]
    RenderError {
        /// SQL ID
        sql_id: String,
        /// 错误信息
        message: String,
    },

    // ============ 参数相关 ============

    /// 参数缺失
    #[error("参数缺失: SQL '{sql_id}' 需要参数 '{param}'")]
    ParamMissing {
        /// SQL ID
        sql_id: String,
        /// 缺失的参数名
        param: String,
    },

    /// 参数类型错误
    #[error("参数类型错误: 参数 '{param}' 期望 {expected}，实际 {actual}")]
    ParamTypeMismatch {
        /// 参数名
        param: String,
        /// 期望类型
        expected: String,
        /// 实际类型
        actual: String,
    },

    /// 参数错误（通用）
    #[error("参数错误: {0}")]
    ParamError(String),

    // ============ 数据库相关 ============

    /// SQL 执行错误
    #[error("SQL 执行错误: {0}")]
    SqlxError(#[from] sqlx::Error),

    /// 事务错误
    #[error("事务错误: {operation} - {message}")]
    TransactionError {
        /// 操作类型（begin/commit/rollback）
        operation: String,
        /// 错误信息
        message: String,
    },

    /// 连接错误
    #[error("数据库连接错误: {0}")]
    ConnectionError(String),

    // ============ 查询结果相关 ============

    /// 记录不存在
    #[error("记录不存在: {sql_id}")]
    NotFound {
        /// SQL ID
        sql_id: String,
    },

    /// 结果为空（用于 query_one）
    #[error("查询结果为空: {sql_id}")]
    EmptyResult {
        /// SQL ID
        sql_id: String,
    },

    // ============ 安全相关 ============

    /// 安全检查错误（编译时检测到不安全语法）
    #[error("SQL 安全检查失败: {sql_id} 第 {line} 行\n  内容: {content}\n  建议: {suggestion}")]
    UnsafeSql {
        /// SQL ID
        sql_id: String,
        /// 行号
        line: usize,
        /// 不安全内容
        content: String,
        /// 修复建议
        suggestion: String,
    },

    // ============ 其他 ============

    /// IO 错误
    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),

    /// 功能不支持
    #[error("功能不支持: {feature} - {reason}")]
    NotSupported {
        /// 功能名称
        feature: String,
        /// 原因
        reason: String,
    },

    /// 内部错误（不应该发生）
    #[error("内部错误: {0}")]
    Internal(String),
}

// ============ 便捷构造函数 ============

impl MarkdownSqlError {
    /// 创建文件未找到错误
    pub fn file_not_found(path: impl Into<String>) -> Self {
        Self::FileNotFound { path: path.into() }
    }

    /// 创建 SQL 未找到错误
    pub fn sql_not_found(sql_id: impl Into<String>, file: impl Into<String>) -> Self {
        Self::SqlNotFound {
            sql_id: sql_id.into(),
            file: file.into(),
        }
    }

    /// 创建参数缺失错误
    pub fn param_missing(sql_id: impl Into<String>, param: impl Into<String>) -> Self {
        Self::ParamMissing {
            sql_id: sql_id.into(),
            param: param.into(),
        }
    }

    /// 创建渲染错误
    pub fn render_error(sql_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::RenderError {
            sql_id: sql_id.into(),
            message: message.into(),
        }
    }

    /// 创建记录不存在错误
    pub fn not_found(sql_id: impl Into<String>) -> Self {
        Self::NotFound {
            sql_id: sql_id.into(),
        }
    }

    /// 创建功能不支持错误
    pub fn not_supported(feature: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::NotSupported {
            feature: feature.into(),
            reason: reason.into(),
        }
    }
}

/// 结果类型别名
pub type Result<T> = std::result::Result<T, MarkdownSqlError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        // 使用便捷构造函数
        let err = MarkdownSqlError::file_not_found("test.md");
        assert_eq!(err.to_string(), "文件未找到: test.md");

        let err = MarkdownSqlError::sql_not_found("findById", "user.md");
        assert!(err.to_string().contains("SQL 未找到: findById"));
        assert!(err.to_string().contains("user.md"));

        // 直接使用结构体
        let err = MarkdownSqlError::UnsafeSql {
            sql_id: "findUserList".to_string(),
            line: 5,
            content: "{{ user_name }}".to_string(),
            suggestion: "请改为: #{user_name}".to_string(),
        };
        assert!(err.to_string().contains("SQL 安全检查失败"));

        // 测试参数缺失
        let err = MarkdownSqlError::param_missing("insertUser", "name");
        assert!(err.to_string().contains("参数缺失"));
        assert!(err.to_string().contains("name"));

        // 测试记录不存在
        let err = MarkdownSqlError::not_found("findUserById");
        assert!(err.to_string().contains("记录不存在"));
    }
}
