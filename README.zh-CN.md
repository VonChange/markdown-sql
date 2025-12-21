# markdown-sql

将 SQL 存储在 Markdown 文件中的 Rust 框架，支持动态 SQL 和参数绑定。

## ✨ 特性

- 📝 **Markdown SQL**：SQL 写在 Markdown 代码块中，可读性强
- 🔒 **安全**：编译时检查 SQL 注入风险，所有参数都通过绑定传入
- 🎨 **动态 SQL**：使用 MiniJinja 模板语法，支持条件、循环
- 🔗 **SQL 复用**：`{% include %}` 引用其他 SQL 片段
- 🚀 **高性能**：启动时预编译模板，运行时零解析开销
- 🎯 **trait 方式**：定义 trait 接口，宏自动生成实现
- 🔄 **事务支持**：支持手动事务和闭包事务
- 📦 **批量操作**：一条 SQL + 多组参数，预编译复用

## 📦 安装

```toml
[dependencies]
markdown-sql = { git = "https://github.com/VonChange/markdown-sql.git", branch = "main" }
markdown-sql-macros = { git = "https://github.com/VonChange/markdown-sql.git", branch = "main" }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
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

### 2. 定义 Repository trait

```rust
use markdown_sql_macros::repository;
use serde::Serialize;

// 参数结构体
#[derive(Serialize)]
pub struct IdParams {
    pub id: i64,
}

#[derive(Serialize)]
pub struct ConditionParams {
    pub name: Option<String>,
    pub status: Option<i32>,
}

// 定义 Repository trait
// 方法名自动映射到 SQL ID（snake_case → camelCase）
// find_by_id → findById
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    /// 根据 ID 查询用户
    async fn find_by_id(
        &self,
        db: &impl DbPool,
        params: &IdParams,
    ) -> Result<Option<User>, AppError>;

    /// 条件查询
    async fn find_by_condition(
        &self,
        db: &impl DbPool,
        params: &ConditionParams,
    ) -> Result<Vec<User>, AppError>;

    /// 获取总数
    async fn get_count(&self, db: &impl DbPool) -> Result<i64, AppError>;

    /// 插入用户
    async fn insert(
        &self,
        db: &impl DbPool,
        user: &User,
    ) -> Result<u64, AppError>;
}
```

### 3. 使用 Repository

```rust
use include_dir::{include_dir, Dir};
use markdown_sql::{DbPool, DbType, SqlManager};
use once_cell::sync::Lazy;

// 嵌入 SQL 目录
static SQL_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/sql");

// 全局 SQL 管理器
static SQL_MANAGER: Lazy<SqlManager> = Lazy::new(|| {
    let mut manager = SqlManager::builder()
        .db_type(DbType::Sqlite)
        .debug(true)
        .build()
        .expect("创建 SqlManager 失败");

    manager
        .load_embedded_dir(&SQL_DIR)
        .expect("加载 SQL 目录失败");

    manager
});

// 获取 Repository 实例
pub fn get_user_repo() -> UserRepositoryImpl {
    UserRepositoryImpl::new(&*SQL_MANAGER)
}

// 使用
async fn example(db: &impl DbPool) {
    let repo = get_user_repo();

    // 查询单条
    let user = repo.find_by_id(db, &IdParams { id: 1 }).await?;

    // 条件查询
    let users = repo.find_by_condition(db, &ConditionParams {
        name: Some("%张%".to_string()),
        status: None,
    }).await?;

    // 获取总数
    let count = repo.get_count(db).await?;

    // 插入
    let affected = repo.insert(db, &new_user).await?;
}
```

## 🔄 事务支持

### 手动事务

```rust
use markdown_sql::{begin_transaction, execute_tx, query_list_tx};

// 开启事务
let mut tx = begin_transaction(&db).await?;

// 在事务中执行操作
execute_tx(&manager, &mut tx, "insert", &user1).await?;
execute_tx(&manager, &mut tx, "insert", &user2).await?;

// 查询也可以在事务中
let users: Vec<User> = query_list_tx(&manager, &mut tx, "findAll", &json!({})).await?;

// 提交事务
tx.commit().await?;

// 如果不调用 commit()，事务会在 tx drop 时自动回滚
```

### 闭包事务

```rust
use markdown_sql::with_transaction;

with_transaction(&db, |tx| Box::pin(async move {
    execute_tx(&manager, tx, "insert", &user1).await?;
    execute_tx(&manager, tx, "update", &user2).await?;
    Ok(())
})).await?;
// 成功则自动 commit，失败则自动 rollback
```

## 📦 批量操作

一条 SQL + 多组参数，预编译复用，在事务内执行：

```rust
use markdown_sql::batch_execute;

// 准备数据
let users = vec![
    UserInsert { name: "用户1".into(), age: 25, status: 1 },
    UserInsert { name: "用户2".into(), age: 30, status: 1 },
    UserInsert { name: "用户3".into(), age: 28, status: 1 },
];

// 批量插入（内部自动开启事务）
let affected = batch_execute(&manager, &db, "insert", &users).await?;
println!("插入 {} 条", affected);
```

### 在事务中批量操作

```rust
use markdown_sql::{begin_transaction, batch_execute_tx};

let mut tx = begin_transaction(&db).await?;

// 批量插入
batch_execute_tx(&manager, &mut tx, "insertUser", &users).await?;

// 批量更新
batch_execute_tx(&manager, &mut tx, "updateOrder", &orders).await?;

tx.commit().await?;
```

## 🗃️ DbPool trait

所有 Repository 方法的 `db` 参数接受实现了 `DbPool` trait 的类型：

```rust
use markdown_sql::DbPool;

// 自定义数据库封装
pub struct AppDb {
    pub sqlite: Pool<Sqlite>,
}

impl DbPool for AppDb {
    fn pool(&self) -> &Pool<Sqlite> {
        &self.sqlite
    }
}

// 使用时直接传 &db，不需要 &db.sqlite
repo.find_by_id(&db, &params).await?;
```

框架已内置实现：
- `Pool<Sqlite>`
- `&Pool<Sqlite>`
- `Arc<T>` where T: DbPool

## 📝 SQL 语法

### 参数绑定

```sql
-- 使用 #{param} 语法，自动转换为 ? (SQLite/MySQL) 或 $1 (PostgreSQL)
SELECT * FROM user WHERE id = #{id} AND name = #{name}
```

### 动态 SQL

```sql
-- 条件判断
{% if name %}AND name = #{name}{% endif %}

-- 循环
{% for status in statuses %}
  #{status}{% if not loop.last %},{% endif %}
{% endfor %}
```

### SQL 片段复用

```sql
-- 定义片段
-- columns
id, name, age, status

-- 引用片段
SELECT {% include "columns" %} FROM user
```

### IN 查询

```sql
-- 使用 bind_join 过滤器，安全展开列表
WHERE id IN ({{ ids | bind_join(",") }})
```

## 🔒 安全检查

编译时自动检测不安全的 SQL 模式：

| 语法 | 状态 | 说明 |
|-----|------|------|
| `#{param}` | ✅ 安全 | 参数绑定 |
| `{{ list \| bind_join(",") }}` | ✅ 安全 | IN 查询 |
| `{% if %}` / `{% for %}` | ✅ 安全 | 动态逻辑 |
| `{{ param }}` | ❌ 编译失败 | SQL 注入风险 |
| `{{ list \| join(",") }}` | ❌ 编译失败 | SQL 注入风险 |
| `{{ param \| raw_safe }}` | ⚠️ 豁免 | 显式声明安全 |

## 🗄️ 返回类型映射

| 返回类型 | 执行方式 | 说明 |
|---------|---------|------|
| `Vec<T>` | fetch_all | 查询列表 |
| `Option<T>` | fetch_optional | 查询单条（可选） |
| `T` | fetch_one | 查询单条（必须存在） |
| `i64` | 标量查询 | 如 COUNT |
| `u64` | execute | INSERT/UPDATE/DELETE 影响行数 |

## 🤖 AI 编程 / Vibe Coding 友好

本框架在设计时充分考虑了 AI 辅助编程的场景：

### 为什么用 Markdown SQL？

| 传统方式 | markdown-sql |
|---------|--------------|
| SQL 嵌入代码中，AI 难以理解上下文 | SQL 在 Markdown 中，结构清晰有注释 |
| 魔法字符串散落各处 | SQL 集中管理，文档化 |
| SQL 与业务逻辑关系不明确 | Markdown 标题描述意图 |

### 对 AI 的优势

1. **清晰的上下文**：SQL 块有描述性标题
2. **自文档化**：AI 可以从 Markdown 结构理解每个 SQL 的作用
3. **易于生成**：AI 可以按照已有模式生成新的 SQL 块
4. **默认安全**：`#{param}` 语法防止 AI 意外生成 SQL 注入漏洞
5. **trait 方式**：AI 只需定义接口，无需写执行代码

### Vibe Coding 工作流

```
用户: "添加一个按邮箱查询用户的方法"

AI:

1. 在 UserRepository.md 中添加:
   ## 按邮箱查询用户
   ​```sql
   -- findByEmail
   SELECT {% include "columns" %}
   FROM user
   WHERE email = #{email}
   ​```

2. 在 trait 中添加方法:
   async fn find_by_email(&self, db: &impl DbPool, params: &EmailParams)
       -> Result<Option<User>, AppError>;

3. 添加参数结构体:
   #[derive(Serialize)]
   pub struct EmailParams {
       pub email: String,
   }

完成！无需写任何执行代码。
```

## 📖 示例

运行示例项目：

```bash
cd examples/demo
cargo run
```

## 📖 文档

详细设计文档请查看 [plan/2025-12-21-markdown-sql.md](plan/2025-12-21-markdown-sql.md)

## 📜 License

MIT
