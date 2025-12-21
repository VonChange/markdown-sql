# markdown-sql

将 SQL 存储在 Markdown 文件中的 Rust 框架，支持动态 SQL 和参数绑定。

## ✨ 特性

- 📝 **Markdown SQL**：SQL 写在 Markdown 代码块中，可读性强
- 🔒 **安全**：编译时检查 SQL 注入风险，所有参数都通过绑定传入
- 🎨 **动态 SQL**：使用 MiniJinja 模板语法，支持条件、循环
- 🔗 **SQL 复用**：`{% include %}` 引用其他 SQL 片段
- 🚀 **高性能**：启动时预编译模板，运行时零解析开销
- 🔄 **事务支持**：SeaORM 风格的泛型执行器
- 📦 **批量操作**：预编译复用，一条 SQL 批量执行

## 📦 安装

```toml
[dependencies]
markdown-sql = "0.1"
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
tokio = { version = "1", features = ["full"] }
```

## 🚀 快速开始

### 1. 创建 SQL 文件

`sql/UserRepository.md`:

```markdown
# 用户 Repository SQL

## 公共字段

​```sql
-- columns
id, name, age, status, create_time
​```

## 查询用户

​```sql
-- findById
SELECT {% include "columns" %}
FROM user
WHERE id = #{id}
​```

## 条件查询

​```sql
-- findByCondition
SELECT {% include "columns" %}
FROM user
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
​```

## 插入用户

​```sql
-- insert
INSERT INTO user (name, age, status)
VALUES (#{name}, #{age}, #{status})
​```
```

### 2. 定义 Repository

```rust
use markdown_sql::repository;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub age: i32,
    pub status: i32,
    pub create_time: String,
}

#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    async fn find_by_id(&self, id: i64) -> Option<User>;
    async fn find_by_condition(&self, name: Option<String>, status: Option<i32>) -> Vec<User>;
    async fn insert(&self, name: &str, age: i32, status: i32) -> u64;
}
```

### 3. 使用

```rust
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect("postgres://...").await?;
    
    let user_repo = UserRepositoryImpl::new(&pool);
    
    // 查询单个用户
    if let Some(user) = user_repo.find_by_id(&pool, 1).await? {
        println!("用户: {:?}", user);
    }
    
    // 条件查询
    let users = user_repo.find_by_condition(
        &pool,
        Some("张%".to_string()),
        Some(1),
    ).await?;
    
    // 插入
    let affected = user_repo.insert(&pool, "新用户", 25, 1).await?;
    
    Ok(())
}
```

## 📖 文档

详细文档请查看 [plan/2025-12-21-markdown-sql.md](plan/2025-12-21-markdown-sql.md)

## 📜 License

MIT
