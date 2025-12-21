# markdown-sql

A Rust framework for storing SQL in Markdown files, with dynamic SQL and parameter binding support.

[中文文档](README.zh-CN.md)

## ✨ Features

- 📝 **Markdown SQL**: SQL in Markdown code blocks, highly readable
- 🔒 **Safe**: Compile-time SQL injection check, all parameters bound
- 🎨 **Dynamic SQL**: MiniJinja template syntax, supports conditions and loops
- 🔗 **SQL Reuse**: `{% include %}` references other SQL fragments
- 🚀 **High Performance**: Templates pre-compiled at startup, zero runtime parsing overhead
- 🎯 **Trait-based**: Define trait interface, macro auto-generates implementation

## 📦 Installation

```toml
[dependencies]
markdown-sql = { git = "https://github.com/VonChange/markdown-sql.git", branch = "main" }
markdown-sql-macros = { git = "https://github.com/VonChange/markdown-sql.git", branch = "main" }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

## 🚀 Quick Start

### 1. Create SQL File

`sql/UserRepository.md`:

```markdown
# User Repository SQL

## Common Columns

​```sql
-- columns
id, name, age, status, create_time
​```

## Find User

​```sql
-- findById
SELECT {% include "columns" %}
FROM user
WHERE id = #{id}
​```

## Conditional Query

​```sql
-- findByCondition
SELECT {% include "columns" %}
FROM user
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
​```
```

### 2. Define Repository Trait

```rust
use markdown_sql_macros::repository;
use serde::Serialize;

// Parameter structs
#[derive(Serialize)]
pub struct IdParams {
    pub id: i64,
}

// Define Repository trait
// Method names auto-map to SQL IDs (snake_case → camelCase)
// find_by_id → findById
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    /// Find user by ID
    async fn find_by_id(
        &self,
        db: &sqlx::Pool<sqlx::Sqlite>,
        params: &IdParams,
    ) -> Result<Option<User>, AppError>;

    /// Get total count
    async fn get_count(&self, db: &sqlx::Pool<sqlx::Sqlite>) -> Result<i64, AppError>;
}
```

### 3. Use Repository

```rust
use include_dir::{include_dir, Dir};
use markdown_sql::{DbType, SqlManager};
use once_cell::sync::Lazy;

// Embed SQL directory
static SQL_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/sql");

// Global SQL manager
static SQL_MANAGER: Lazy<SqlManager> = Lazy::new(|| {
    let mut manager = SqlManager::builder()
        .db_type(DbType::Sqlite)
        .debug(true)
        .build()
        .expect("Failed to create SqlManager");

    manager
        .load_embedded_dir(&SQL_DIR)
        .expect("Failed to load SQL directory");

    manager
});

// Get Repository instance
pub fn get_user_repo() -> UserRepositoryImpl {
    UserRepositoryImpl::new(&*SQL_MANAGER)
}

// Usage
async fn example(db: &Pool<Sqlite>) {
    let repo = get_user_repo();

    // Query single
    let user = repo.find_by_id(db, &IdParams { id: 1 }).await?;

    // Get count
    let count = repo.get_count(db).await?;
}
```

## 📝 SQL Syntax

### Parameter Binding

```sql
-- Use #{param} syntax, auto-converts to ? (SQLite/MySQL) or $1 (PostgreSQL)
SELECT * FROM user WHERE id = #{id} AND name = #{name}
```

### Dynamic SQL

```sql
-- Conditionals
{% if name %}AND name = #{name}{% endif %}

-- Loops
{% for status in statuses %}
  #{status}{% if not loop.last %},{% endif %}
{% endfor %}
```

### SQL Fragment Reuse

```sql
-- Define fragment
-- columns
id, name, age, status

-- Reference fragment
SELECT {% include "columns" %} FROM user
```

## 🔒 Safety Check

Compile-time detection of unsafe SQL patterns:

| Syntax | Status | Description |
|--------|--------|-------------|
| `#{param}` | ✅ Safe | Parameter binding |
| `{{ list \| bind_join(",") }}` | ✅ Safe | IN query |
| `{% if %}` / `{% for %}` | ✅ Safe | Dynamic logic |
| `{{ param }}` | ❌ Compile error | SQL injection risk |
| `{{ list \| join(",") }}` | ❌ Compile error | SQL injection risk |
| `{{ param \| raw_safe }}` | ⚠️ Exempt | Explicit safe declaration |

## 🗄️ Return Type Mapping

| Return Type | Execution | Description |
|------------|-----------|-------------|
| `Vec<T>` | fetch_all | Query list |
| `Option<T>` | fetch_optional | Query single (optional) |
| `T` | fetch_one | Query single (required) |
| `i64` | Scalar query | e.g., COUNT |
| `u64` | execute | INSERT/UPDATE/DELETE affected rows |

## 🤖 AI/Vibe Coding Friendly

This framework is designed with AI-assisted programming in mind:

### Why Markdown SQL?

| Traditional | markdown-sql |
|-------------|--------------|
| SQL embedded in code, hard for AI to understand context | SQL in Markdown, clear structure with comments |
| Magic strings scattered everywhere | SQL centralized, documented |
| SQL-business logic relationship unclear | Markdown titles describe intent |

### Advantages for AI

1. **Clear context**: SQL blocks have descriptive titles
2. **Self-documenting**: AI can understand each SQL's purpose from Markdown structure
3. **Easy to generate**: AI can generate new SQL blocks following existing patterns
4. **Safe by default**: `#{param}` syntax prevents AI from accidentally generating SQL injection vulnerabilities
5. **Trait-based**: AI only needs to define interface, no execution code required

## 📖 Documentation

For detailed design documentation, see [plan/2025-12-21-markdown-sql.md](plan/2025-12-21-markdown-sql.md)

## 📜 License

MIT
