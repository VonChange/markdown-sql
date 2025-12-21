# markdown-sql

A Rust framework for storing SQL in Markdown files with dynamic SQL and parameter binding support.

## ✨ Features

- 📝 **Markdown SQL**: Write SQL in Markdown code blocks for better readability
- 🔒 **Safe**: Compile-time SQL injection checks, all parameters go through binding
- 🎨 **Dynamic SQL**: MiniJinja template syntax with conditions and loops
- 🔗 **SQL Reuse**: `{% include %}` to reference other SQL fragments
- 🚀 **High Performance**: Templates pre-compiled at startup, zero parsing overhead at runtime
- 🎯 **Trait-based**: Define trait interfaces, macros generate implementations
- 🔄 **Transaction Support**: Manual and closure-based transactions
- 📦 **Batch Operations**: One SQL + multiple parameter sets, prepared statement reuse

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
id, name, age, status, created_at
​```

## Find User by ID

​```sql
-- findById
SELECT {% include "columns" %}
FROM users
WHERE id = #{id}
​```

## Conditional Query

​```sql
-- findByCondition
SELECT {% include "columns" %}
FROM users
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
​```

## Insert User

​```sql
-- insert
INSERT INTO users (name, age, status)
VALUES (#{name}, #{age}, #{status})
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

#[derive(Serialize)]
pub struct ConditionParams {
    pub name: Option<String>,
    pub status: Option<i32>,
}

// Define Repository trait
// Method names auto-map to SQL IDs (snake_case → camelCase)
// find_by_id → findById
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    async fn find_by_id(
        &self,
        db: &impl DbPool,
        params: &IdParams,
    ) -> Result<Option<User>, AppError>;

    async fn find_by_condition(
        &self,
        db: &impl DbPool,
        params: &ConditionParams,
    ) -> Result<Vec<User>, AppError>;

    async fn get_count(&self, db: &impl DbPool) -> Result<i64, AppError>;

    async fn insert(
        &self,
        db: &impl DbPool,
        user: &User,
    ) -> Result<u64, AppError>;
}
```

### 3. Use the Repository

```rust
use include_dir::{include_dir, Dir};
use markdown_sql::{DbPool, DbType, SqlManager};
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

pub fn get_user_repo() -> UserRepositoryImpl {
    UserRepositoryImpl::new(&*SQL_MANAGER)
}

async fn example(db: &impl DbPool) {
    let repo = get_user_repo();

    // Query single
    let user = repo.find_by_id(db, &IdParams { id: 1 }).await?;

    // Conditional query
    let users = repo.find_by_condition(db, &ConditionParams {
        name: Some("%John%".to_string()),
        status: None,
    }).await?;

    // Get count
    let count = repo.get_count(db).await?;

    // Insert
    let affected = repo.insert(db, &new_user).await?;
}
```

## 🔄 Transaction Support

### Manual Transaction

```rust
use markdown_sql::{begin_transaction, execute_tx, query_list_tx};

// Begin transaction
let mut tx = begin_transaction(&db).await?;

// Execute operations in transaction
execute_tx(&manager, &mut tx, "insert", &user1).await?;
execute_tx(&manager, &mut tx, "insert", &user2).await?;

// Query in transaction
let users: Vec<User> = query_list_tx(&manager, &mut tx, "findAll", &json!({})).await?;

// Commit
tx.commit().await?;

// If commit() is not called, transaction auto-rollbacks on drop
```

### Closure Transaction

```rust
use markdown_sql::with_transaction;

with_transaction(&db, |tx| Box::pin(async move {
    execute_tx(&manager, tx, "insert", &user1).await?;
    execute_tx(&manager, tx, "update", &user2).await?;
    Ok(())
})).await?;
// Auto-commit on success, auto-rollback on failure
```

## 📦 Batch Operations

One SQL + multiple parameter sets, prepared statement reuse, executed in transaction:

```rust
use markdown_sql::batch_execute;

let users = vec![
    UserInsert { name: "User1".into(), age: 25, status: 1 },
    UserInsert { name: "User2".into(), age: 30, status: 1 },
    UserInsert { name: "User3".into(), age: 28, status: 1 },
];

// Batch insert (auto-transaction internally)
let affected = batch_execute(&manager, &db, "insert", &users).await?;
println!("Inserted {} rows", affected);
```

### Batch in Transaction

```rust
use markdown_sql::{begin_transaction, batch_execute_tx};

let mut tx = begin_transaction(&db).await?;

batch_execute_tx(&manager, &mut tx, "insertUser", &users).await?;
batch_execute_tx(&manager, &mut tx, "updateOrder", &orders).await?;

tx.commit().await?;
```

## 🗃️ DbPool Trait

All Repository method `db` parameters accept types implementing `DbPool` trait:

```rust
use markdown_sql::DbPool;

pub struct AppDb {
    pub sqlite: Pool<Sqlite>,
}

impl DbPool for AppDb {
    fn pool(&self) -> &Pool<Sqlite> {
        &self.sqlite
    }
}

// Use &db directly, no need for &db.sqlite
repo.find_by_id(&db, &params).await?;
```

Built-in implementations:
- `Pool<Sqlite>`
- `&Pool<Sqlite>`
- `Arc<T>` where T: DbPool

## 📝 SQL Syntax

### Parameter Binding

```sql
-- Use #{param} syntax, auto-converts to ? (SQLite/MySQL) or $1 (PostgreSQL)
SELECT * FROM users WHERE id = #{id} AND name = #{name}
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
SELECT {% include "columns" %} FROM users
```

### IN Queries

```sql
-- Use bind_join filter for safe list expansion
WHERE id IN ({{ ids | bind_join(",") }})
```

## 🔒 Safety Checks

Compile-time detection of unsafe SQL patterns:

| Syntax | Status | Description |
|--------|--------|-------------|
| `#{param}` | ✅ Safe | Parameter binding |
| `{{ list \| bind_join(",") }}` | ✅ Safe | IN queries |
| `{% if %}` / `{% for %}` | ✅ Safe | Dynamic logic |
| `{{ param }}` | ❌ Compile error | SQL injection risk |
| `{{ list \| join(",") }}` | ❌ Compile error | SQL injection risk |
| `{{ param \| raw_safe }}` | ⚠️ Exempt | Explicitly declared safe |

## 🗄️ Return Type Mapping

| Return Type | Execution | Description |
|------------|-----------|-------------|
| `Vec<T>` | fetch_all | Query list |
| `Option<T>` | fetch_optional | Query single (optional) |
| `T` | fetch_one | Query single (required) |
| `i64` | Scalar query | e.g., COUNT |
| `u64` | execute | INSERT/UPDATE/DELETE affected rows |

## 🤖 AI / Vibe Coding Friendly

This framework is designed with AI-assisted programming in mind:

### Why Markdown SQL?

| Traditional | markdown-sql |
|-------------|--------------|
| SQL embedded in code, AI struggles with context | SQL in Markdown, clear structure with comments |
| Magic strings scattered everywhere | Centralized SQL management, documented |
| SQL-business relationship unclear | Markdown headings describe intent |

### AI Advantages

1. **Clear Context**: SQL blocks have descriptive headings
2. **Self-Documenting**: AI understands each SQL's purpose from Markdown structure
3. **Easy Generation**: AI can generate new SQL blocks following existing patterns
4. **Safe by Default**: `#{param}` syntax prevents AI from accidentally generating SQL injection vulnerabilities
5. **Trait-based**: AI only defines interfaces, no execution code needed

## 📖 Examples

Run the demo project:

```bash
cd examples/demo
cargo run
```

## 📖 Documentation

See detailed design document: [plan/2025-12-21-markdown-sql.md](plan/2025-12-21-markdown-sql.md)

## 📜 License

MIT
