//! Markdown SQL 解析器
//!
//! 从 Markdown 文件中提取 SQL 代码块。
//! 参考 spring-data-jdbc-mybatis 的实现，使用纯字符串操作，无依赖。
//!
//! ## SQL 格式
//!
//! ```markdown
//! ## 查询用户
//!
//! ```sql
//! -- findById
//! SELECT * FROM user WHERE id = #{id}
//! ```
//! ```
//!
//! - SQL ID 在代码块第一行，以 `-- sqlId` 格式定义
//! - 代码块使用 ``` 或 ```sql 标记

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::error::{MarkdownSqlError, Result};

/// SQL 代码块
#[derive(Debug, Clone)]
pub struct SqlBlock {
    /// SQL ID（如 findById、insert）
    pub id: String,
    /// SQL 内容（不含 ID 注释行）
    pub content: String,
    /// 在文件中的行号（用于错误提示）
    pub line_number: usize,
}

/// Markdown SQL 解析器
pub struct MarkdownParser;

impl MarkdownParser {
    /// 从文件路径解析 SQL 代码块
    ///
    /// # 参数
    /// - `path`: Markdown 文件路径
    ///
    /// # 返回
    /// - `HashMap<String, SqlBlock>`: SQL ID -> SQL 代码块
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<HashMap<String, SqlBlock>> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                MarkdownSqlError::FileNotFound(path.display().to_string())
            } else {
                MarkdownSqlError::IoError(e)
            }
        })?;

        Self::parse_content(&content)
    }

    /// 解析 Markdown 内容
    ///
    /// # 参数
    /// - `content`: Markdown 文件内容
    ///
    /// # 返回
    /// - `HashMap<String, SqlBlock>`: SQL ID -> SQL 代码块
    pub fn parse_content(content: &str) -> Result<HashMap<String, SqlBlock>> {
        let mut sql_blocks = HashMap::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // 检测代码块开始：``` 或 ```sql 或 ```SQL
            if line.starts_with("```") {
                let block_start = i;
                i += 1;

                // 收集代码块内容
                let mut block_lines = Vec::new();
                while i < lines.len() && !lines[i].trim().starts_with("```") {
                    block_lines.push(lines[i]);
                    i += 1;
                }

                // 解析 SQL 代码块
                if let Some(sql_block) = Self::parse_sql_block(&block_lines, block_start + 1) {
                    sql_blocks.insert(sql_block.id.clone(), sql_block);
                }
            }

            i += 1;
        }

        Ok(sql_blocks)
    }

    /// 解析单个 SQL 代码块
    ///
    /// # 参数
    /// - `lines`: 代码块内的行（不含 ``` 标记）
    /// - `start_line`: 代码块开始行号
    ///
    /// # 返回
    /// - `Option<SqlBlock>`: 如果是有效的 SQL 代码块则返回
    fn parse_sql_block(lines: &[&str], start_line: usize) -> Option<SqlBlock> {
        if lines.is_empty() {
            return None;
        }

        // 第一行应该是 SQL ID 注释：-- sqlId
        let first_line = lines[0].trim();
        let sql_id = Self::extract_sql_id(first_line)?;

        // 剩余内容作为 SQL
        let content: String = if lines.len() > 1 {
            lines[1..]
                .iter()
                .map(|s| *s)
                .collect::<Vec<&str>>()
                .join("\n")
                .trim()
                .to_string()
        } else {
            String::new()
        };

        Some(SqlBlock {
            id: sql_id,
            content,
            line_number: start_line,
        })
    }

    /// 从注释行提取 SQL ID
    ///
    /// 支持格式：
    /// - `-- sqlId`
    /// - `--sqlId`
    /// - `-- sql_id`
    fn extract_sql_id(line: &str) -> Option<String> {
        let line = line.trim();

        // 必须以 -- 开头
        if !line.starts_with("--") {
            return None;
        }

        // 移除 -- 前缀，提取 ID
        let id = line.trim_start_matches('-').trim();

        // ID 不能为空，且不能包含空格（避免匹配普通注释）
        if id.is_empty() || id.contains(' ') {
            return None;
        }

        Some(id.to_string())
    }

    /// 从文件路径提取命名空间
    ///
    /// 例如：`sql/UserRepository.md` -> `UserRepository`
    pub fn extract_namespace<P: AsRef<Path>>(path: P) -> String {
        let path = path.as_ref();
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("default")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_sql() {
        let content = r#"
# UserRepository SQL

## 查询用户

```sql
-- findById
SELECT * FROM user WHERE id = #{id}
```
"#;

        let blocks = MarkdownParser::parse_content(content).unwrap();
        assert_eq!(blocks.len(), 1);

        let block = blocks.get("findById").unwrap();
        assert_eq!(block.id, "findById");
        assert!(block.content.contains("SELECT * FROM user"));
    }

    #[test]
    fn test_parse_multiple_sql() {
        let content = r#"
# SQL 定义

```sql
-- columns
id, name, age
```

```sql
-- findAll
SELECT {% include "columns" %} FROM user
```

```sql
-- insert
INSERT INTO user (name, age) VALUES (#{name}, #{age})
```
"#;

        let blocks = MarkdownParser::parse_content(content).unwrap();
        assert_eq!(blocks.len(), 3);
        assert!(blocks.contains_key("columns"));
        assert!(blocks.contains_key("findAll"));
        assert!(blocks.contains_key("insert"));
    }

    #[test]
    fn test_parse_dynamic_sql() {
        let content = r#"
```sql
-- findByCondition
SELECT * FROM user
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
```
"#;

        let blocks = MarkdownParser::parse_content(content).unwrap();
        let block = blocks.get("findByCondition").unwrap();
        assert!(block.content.contains("{% if name %}"));
        assert!(block.content.contains("{% if status %}"));
    }

    #[test]
    fn test_extract_sql_id() {
        assert_eq!(
            MarkdownParser::extract_sql_id("-- findById"),
            Some("findById".to_string())
        );
        assert_eq!(
            MarkdownParser::extract_sql_id("--findById"),
            Some("findById".to_string())
        );
        assert_eq!(
            MarkdownParser::extract_sql_id("  -- find_by_id  "),
            Some("find_by_id".to_string())
        );

        // 无效格式
        assert_eq!(MarkdownParser::extract_sql_id("findById"), None);
        assert_eq!(MarkdownParser::extract_sql_id("-- find by id"), None);
        assert_eq!(MarkdownParser::extract_sql_id("--"), None);
    }

    #[test]
    fn test_extract_namespace() {
        assert_eq!(
            MarkdownParser::extract_namespace("sql/UserRepository.md"),
            "UserRepository"
        );
        assert_eq!(
            MarkdownParser::extract_namespace("/path/to/OrderRepository.md"),
            "OrderRepository"
        );
        assert_eq!(MarkdownParser::extract_namespace("test.md"), "test");
    }

    #[test]
    fn test_parse_code_block_without_sql_id() {
        let content = r#"
# 说明文档

这是一段代码示例：

```rust
fn main() {
    println!("Hello");
}
```

## SQL 定义

```sql
-- findById
SELECT * FROM user WHERE id = #{id}
```
"#;

        let blocks = MarkdownParser::parse_content(content).unwrap();
        // Rust 代码块没有 -- sqlId，不会被解析
        assert_eq!(blocks.len(), 1);
        assert!(blocks.contains_key("findById"));
    }
}
