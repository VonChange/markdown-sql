# markdown-sql

A Rust framework for storing SQL in Markdown files with dynamic SQL and parameter binding support.

[中文文档](README.zh-CN.md)

## ✨ Features

- 📝 **Markdown SQL**: Write SQL in Markdown code blocks for better readability
- 🔒 **Security**: Compile-time SQL injection checks, all parameters are bound
- 🎨 **Dynamic SQL**: MiniJinja template syntax with conditionals and loops
- 🔗 **SQL Reuse**: `{% include %}` to reference other SQL fragments
- 🚀 **High Performance**: Pre-compiled templates at startup, zero parsing overhead at runtime
- 🔄 **Transaction Support**: SeaORM-style generic executor
- 📦 **Batch Operations**: Prepared statement reuse for batch execution

## 📦 Installation

```toml
[dependencies]
markdown-sql = { git = "https://github.com/VonChange/markdown-sql.git", branch = "main" }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
tokio = { version = "1", features = ["full"] }
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

## IN Query

​```sql
-- findByIds
SELECT {% include "columns" %}
FROM user
WHERE id IN ({{ ids | bind_join(",") }})
​```

## Insert User

​```sql
-- insert
INSERT INTO user (name, age, status)
VALUES (#{name}, #{age}, #{status})
​```
```

### 2. Use SqlManager

```rust
use markdown_sql::{DbType, ParamExtractor, SqlManager};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create SQL manager
    let mut manager = SqlManager::builder()
        .db_type(DbType::Postgres)
        .debug(true)
        .build()?;

    // 2. Load SQL file
    manager.load_file("sql/UserRepository.md")?;

    // 3. Render SQL
    let sql = manager.render("findById", &json!({"id": 1}))?;
    // Output: SELECT id, name, age, status, create_time FROM user WHERE id = #{id}

    // 4. Extract parameters
    let result = ParamExtractor::extract(&sql, DbType::Postgres);
    // result.sql: "SELECT ... WHERE id = $1"
    // result.params: ["id"]

    // 5. Dynamic SQL
    let sql = manager.render("findByCondition", &json!({
        "name": "%test%",
        "status": 1
    }))?;
    // Output: SELECT ... WHERE 1=1 AND name LIKE #{name} AND status = #{status}

    // 6. IN query
    let sql = manager.render("findByIds", &json!({"ids": [1, 2, 3]}))?;
    // Output: SELECT ... WHERE id IN (#{__bind_0},#{__bind_1},#{__bind_2})

    Ok(())
}
```

## 📝 SQL Syntax

### Parameter Binding

```sql
-- Use #{param} syntax, auto-converts to $1 (Postgres) or ? (MySQL/SQLite)
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

### IN Query

```sql
-- Use bind_join filter to safely expand lists
WHERE id IN ({{ ids | bind_join(",") }})
```

## 🔒 Security Checks

Compile-time detection of unsafe SQL patterns:

| Syntax | Status | Description |
|--------|--------|-------------|
| `#{param}` | ✅ Safe | Parameter binding |
| `{{ list \| bind_join(",") }}` | ✅ Safe | IN query |
| `{% if %}` / `{% for %}` | ✅ Safe | Dynamic logic |
| `{{ param }}` | ❌ Compile Error | SQL injection risk |
| `{{ list \| join(",") }}` | ❌ Compile Error | SQL injection risk |
| `{{ param \| raw_safe }}` | ⚠️ Exempt | Explicitly declared safe |

## 🗄️ Multi-Database Support

```rust
// PostgreSQL: #{id} → $1
let result = ParamExtractor::extract(&sql, DbType::Postgres);

// MySQL: #{id} → ?
let result = ParamExtractor::extract(&sql, DbType::Mysql);

// SQLite: #{id} → ?
let result = ParamExtractor::extract(&sql, DbType::Sqlite);
```

## 🤖 AI/Vibe Coding Friendly

This framework is designed with AI-assisted programming in mind:

### Why Markdown SQL?

| Traditional Approach | markdown-sql |
|---------------------|--------------|
| SQL embedded in code, hard for AI to understand context | SQL in Markdown with clear structure and comments |
| Magic strings scattered across files | Centralized, well-documented SQL files |
| No clear relationship between SQL and business logic | Markdown headings describe intent |

### AI Benefits

1. **Clear Context**: SQL blocks have descriptive headings
   ```markdown
   ## Find active users by department
   ​```sql
   -- findActiveUsersByDept
   SELECT * FROM user WHERE status = 1 AND dept_id = #{deptId}
   ​```
   ```

2. **Self-Documenting**: AI can understand what each SQL does from the Markdown structure

3. **Easy Generation**: AI can generate new SQL blocks following the established pattern

4. **Safe by Default**: `#{param}` syntax prevents AI from accidentally generating SQL injection vulnerabilities

5. **Reusable Fragments**: `{% include %}` helps AI understand and reuse common patterns

### Vibe Coding Workflow

```
Human: "Add a query to find users by email"

AI: Creates in UserRepository.md:

## Find user by email

​```sql
-- findByEmail
SELECT {% include "columns" %}
FROM user
WHERE email = #{email}
​```
```

## 📖 Documentation

For detailed design documentation, see [plan/2025-12-21-markdown-sql.md](plan/2025-12-21-markdown-sql.md)

## 📜 License

MIT
