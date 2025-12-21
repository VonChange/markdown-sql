//! 错误类型定义
//!
//! 定义 markdown-sql 框架的所有错误类型

use thiserror::Error;

/// markdown-sql 错误类型
#[derive(Debug, Error)]
pub enum MarkdownSqlError {
    /// 文件未找到
    #[error("文件未找到: {0}")]
    FileNotFound(String),

    /// 无效的文件路径
    #[error("无效的文件路径: {0}")]
    InvalidPath(String),

    /// SQL 未找到
    #[error("SQL 未找到: {0}")]
    SqlNotFound(String),

    /// 模板解析错误
    #[error("模板解析错误: {0}")]
    TemplateError(String),

    /// 模板渲染错误
    #[error("模板渲染错误: {0}")]
    RenderError(String),

    /// SQL 执行错误
    #[error("SQL 执行错误: {0}")]
    SqlxError(#[from] sqlx::Error),

    /// IO 错误
    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),

    /// 参数错误
    #[error("参数错误: {0}")]
    ParamError(String),

    /// 安全检查错误（编译时检测到不安全语法）
    #[error("SQL 安全检查失败: {sql_id} 第 {line} 行\n  内容: {content}\n  建议: {suggestion}")]
    UnsafeSql {
        sql_id: String,
        line: usize,
        content: String,
        suggestion: String,
    },
}

/// 结果类型别名
pub type Result<T> = std::result::Result<T, MarkdownSqlError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MarkdownSqlError::FileNotFound("test.md".to_string());
        assert_eq!(err.to_string(), "文件未找到: test.md");

        let err = MarkdownSqlError::SqlNotFound("findById".to_string());
        assert_eq!(err.to_string(), "SQL 未找到: findById");

        let err = MarkdownSqlError::UnsafeSql {
            sql_id: "findUserList".to_string(),
            line: 5,
            content: "{{ user_name }}".to_string(),
            suggestion: "请改为: #{user_name}".to_string(),
        };
        assert!(err.to_string().contains("SQL 安全检查失败"));
    }
}
