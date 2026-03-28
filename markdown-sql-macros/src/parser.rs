//! Markdown SQL 解析器（宏专用）
//!
//! 简化版解析器，用于编译时检查。

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

/// SQL 代码块正则
static SQL_BLOCK_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"```sql\s*\n([\s\S]*?)```").unwrap());

/// SQL ID 正则（-- sqlId 格式）
static SQL_ID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^--\s*(\w+)\s*$").unwrap());

/// SQL 块
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SqlBlock {
    /// SQL ID
    pub id: String,
    /// SQL 内容
    pub content: String,
}

/// 解析 Markdown 内容，提取 SQL 块
pub fn parse_content(content: &str) -> HashMap<String, SqlBlock> {
    let mut blocks = HashMap::new();

    for cap in SQL_BLOCK_RE.captures_iter(content) {
        if let Some(block_content) = cap.get(1) {
            let content = block_content.as_str();

            // 提取 SQL ID
            if let Some(sql_id) = extract_sql_id(content) {
                // 去掉第一行（SQL ID 注释）
                let sql_content = content
                    .lines()
                    .skip(1)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .to_string();

                blocks.insert(
                    sql_id.clone(),
                    SqlBlock {
                        id: sql_id,
                        content: sql_content,
                    },
                );
            }
        }
    }

    blocks
}

/// 提取 SQL ID
fn extract_sql_id(content: &str) -> Option<String> {
    let first_line = content.lines().next()?.trim();
    SQL_ID_RE
        .captures(first_line)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content() {
        let content = r#"
# Test SQL

```sql
-- findById
SELECT * FROM user WHERE id = #{id}
```

```sql
-- insert
INSERT INTO user (name) VALUES (#{name})
```
"#;
        let blocks = parse_content(content);
        assert_eq!(blocks.len(), 2);
        assert!(blocks.contains_key("findById"));
        assert!(blocks.contains_key("insert"));
    }
}
