//! # markdown-sql
//!
//! 将 SQL 存储在 Markdown 文件中的 Rust 框架，支持动态 SQL 和参数绑定。
//!
//! ## ✨ 特性
//!
//! - 📝 **Markdown SQL**：SQL 写在 Markdown 代码块中，可读性强
//! - 🔒 **安全**：编译时检查 SQL 注入风险，所有参数都通过绑定传入
//! - 🎨 **动态 SQL**：使用 MiniJinja 模板语法，支持条件、循环
//! - 🔗 **SQL 复用**：`{% include %}` 引用其他 SQL 片段
//! - 🚀 **高性能**：启动时预编译模板，运行时零解析开销
//!
//! ## 快速开始
//!
//! ### 1. 创建 SQL 文件
//!
//! `sql/UserRepository.md`:
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
//! ### 2. 加载并使用
//!
//! ```ignore
//! use markdown_sql::{SqlManager, DbType};
//! use serde_json::json;
//!
//! // 创建管理器
//! let mut manager = SqlManager::builder()
//!     .db_type(DbType::Postgres)
//!     .debug(true)
//!     .load_file("sql/UserRepository.md")
//!     .build()?;
//!
//! // 渲染 SQL
//! let sql = manager.render("findById", &json!({"id": 1}))?;
//! // sql = "SELECT * FROM user WHERE id = #{id}"
//!
//! // 提取参数
//! use markdown_sql::ParamExtractor;
//! let result = ParamExtractor::extract(&sql, DbType::Postgres);
//! // result.sql = "SELECT * FROM user WHERE id = $1"
//! // result.params = ["id"]
//! ```
//!
//! ## 动态 SQL
//!
//! 使用 MiniJinja 模板语法：
//!
//! ```markdown
//! ```sql
//! -- findByCondition
//! SELECT * FROM user
//! WHERE 1=1
//! {% if name %}AND name LIKE #{name}{% endif %}
//! {% if status %}AND status = #{status}{% endif %}
//! ```
//! ```
//!
//! ## SQL 复用
//!
//! 使用 `{% include %}` 引用其他 SQL 片段：
//!
//! ```markdown
//! ```sql
//! -- columns
//! id, name, age
//! ```
//!
//! ```sql
//! -- findAll
//! SELECT {% include "columns" %}
//! FROM user
//! ```
//! ```
//!
//! ## 参数绑定
//!
//! - `#{param}` - 安全的参数绑定（转换为 `?` 或 `$1`）
//! - `{{ list | bind_join(",") }}` - IN 查询展开
//! - `{{ value | raw_safe }}` - 显式声明安全的字符串拼接

pub mod error;
pub mod executor;
pub mod manager;
pub mod param_extractor;
pub mod parser;

// 重新导出常用类型
pub use error::{MarkdownSqlError, Result};
pub use executor::{BatchExecutor, ExecuteContext, ParamBinder, SqlExecutor, Timer};
pub use manager::{init, render, set_db_type, set_debug, SqlManager, SqlManagerBuilder};
pub use param_extractor::{DbType, ParamExtractor, SqlResult};
pub use parser::{MarkdownParser, SqlBlock};

// 当启用 embed feature 时，重新导出 include_dir
#[cfg(feature = "embed")]
pub use include_dir::{include_dir, Dir};

/// 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_full_workflow() {
        // 1. 创建管理器
        let mut manager = SqlManager::new();
        manager.set_db_type(DbType::Postgres);

        // 2. 加载 SQL
        let content = r#"
```sql
-- columns
id, name, age
```

```sql
-- findById
SELECT {% include "columns" %}
FROM user
WHERE id = #{id}
```

```sql
-- findByCondition
SELECT {% include "columns" %}
FROM user
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
```

```sql
-- findByIds
SELECT * FROM user
WHERE id IN ({{ ids | bind_join(",") }})
```

```sql
-- insert
INSERT INTO user (name, age)
VALUES (#{name}, #{age})
```
"#;

        manager.load_content(content, "User").unwrap();

        // 3. 测试简单查询
        let sql = manager.render("findById", &json!({"id": 1})).unwrap();
        assert!(sql.contains("id, name, age"));
        assert!(sql.contains("#{id}"));

        let result = ParamExtractor::extract(&sql, DbType::Postgres);
        assert!(result.sql.contains("$1"));
        assert_eq!(result.params, vec!["id"]);

        // 4. 测试动态 SQL
        let sql = manager
            .render("findByCondition", &json!({"name": "%test%"}))
            .unwrap();
        assert!(sql.contains("AND name LIKE #{name}"));
        assert!(!sql.contains("AND status"));

        // 5. 测试 IN 查询
        let sql = manager
            .render("findByIds", &json!({"ids": [1, 2, 3]}))
            .unwrap();
        assert!(sql.contains("#{__bind_0}"));
        assert!(sql.contains("#{__bind_1}"));
        assert!(sql.contains("#{__bind_2}"));

        // 6. 测试插入
        let sql = manager
            .render("insert", &json!({"name": "test", "age": 25}))
            .unwrap();
        let result = ParamExtractor::extract(&sql, DbType::Postgres);
        assert!(result.sql.contains("$1"));
        assert!(result.sql.contains("$2"));
        assert_eq!(result.params, vec!["name", "age"]);
    }

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
