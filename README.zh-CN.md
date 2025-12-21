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
- 🗄️ **多数据库**：支持 SQLite、MySQL、PostgreSQL

## 📦 安装

### SQLite（默认）

```toml
[dependencies]
markdown-sql = { git = "https://github.com/VonChange/markdown-sql.git", branch = "main" }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
once_cell = "1"
```

### MySQL

```toml
[dependencies]
markdown-sql = { git = "https://github.com/VonChange/markdown-sql.git", branch = "main", features = ["mysql"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "mysql"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
once_cell = "1"
```

### PostgreSQL

```toml
[dependencies]
markdown-sql = { git = "https://github.com/VonChange/markdown-sql.git", branch = "main", features = ["postgres"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
once_cell = "1"
```

> **注意**：`#[repository]` 宏已被 `markdown-sql` 重新导出，无需单独引用 `markdown-sql-macros`。

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
use markdown_sql::repository;  // 宏已被重新导出
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

每个 Repository 方法都会自动生成 `_tx` 事务版本：

- `insert(&db, &user)` → `insert_tx(&mut tx, &user)`
- `find_all(&db)` → `find_all_tx(&mut tx)`
- `update(&db, &user)` → `update_tx(&mut tx, &user)`

### `#[transactional]` 自动事务

对于需要自动事务的方法，使用 `#[transactional]` 注解：

```rust
use markdown_sql::{repository, transactional};

#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    // 普通方法（无事务）
    async fn insert(&self, user: &UserInsert) -> Result<u64>;
    
    // 自动事务方法
    #[transactional]
    async fn batch_insert(&self, user: &UserInsert) -> Result<u64>;
}

// 调用时自动：开启事务 → 执行 → 成功提交/失败回滚
repo.batch_insert(&db, &user).await?;
```

### 手动事务

```rust
let repo = get_user_repo();

// 开启事务
let mut tx = repo.begin_transaction(&db).await?;

// 在事务中执行多个操作
repo.insert_tx(&mut tx, &user1).await?;
repo.insert_tx(&mut tx, &user2).await?;

// 在事务中查询（能看到未提交的数据）
let users = repo.find_all_tx(&mut tx).await?;

// 提交事务
tx.commit().await?;

// 如果不调用 commit()，事务会在 tx drop 时自动回滚
```

### 事务回滚

```rust
let mut tx = repo.begin_transaction(&db).await?;

repo.insert_tx(&mut tx, &user).await?;

// 显式回滚
tx.rollback().await?;
```

### 业务服务层事务示例

```rust
pub struct OrderService {
    order_repo: OrderRepositoryImpl,
    item_repo: OrderItemRepositoryImpl,
}

impl OrderService {
    /// 创建订单（多个 Repository 在同一事务中）
    pub async fn create_order(
        &self,
        db: &impl SqliteDbPool,
        order: &Order,
        items: &[OrderItem],
    ) -> Result<(), AppError> {
        // 开启事务
        let mut tx = self.order_repo.begin_transaction(db).await?;
        
        // 插入订单
        self.order_repo.insert_tx(&mut tx, order).await?;
        
        // 插入订单项
        for item in items {
            self.item_repo.insert_tx(&mut tx, item).await?;
        }
        
        // 提交事务
        tx.commit().await?;
        Ok(())
    }
}
```

## 📦 批量操作

通过事务 + 循环调用实现批量操作：

```rust
let repo = get_user_repo();

// 准备数据
let users = vec![
    UserInsert { name: "用户1".into(), age: 25, status: 1 },
    UserInsert { name: "用户2".into(), age: 30, status: 1 },
    UserInsert { name: "用户3".into(), age: 28, status: 1 },
];

// 开启事务批量插入
let mut tx = repo.begin_transaction(&db).await?;
for user in &users {
    repo.insert_tx(&mut tx, &user).await?;
}
tx.commit().await?;
```

> **注意**：所有数据库操作必须通过 Repository 方法（普通版或 `_tx` 版本）调用。

## 🗃️ DbPool trait

所有 Repository 方法的 `db` 参数接受实现了对应数据库 `DbPool` trait 的类型：

```rust
use markdown_sql::SqliteDbPool;  // SQLite
// use markdown_sql::MySqlDbPool;   // MySQL
// use markdown_sql::PgDbPool;      // PostgreSQL

// 自定义数据库封装
pub struct AppDb {
    pub sqlite: Pool<Sqlite>,
}

impl SqliteDbPool for AppDb {
    fn pool(&self) -> &Pool<Sqlite> {
        &self.sqlite
    }
}

// 使用时直接传 &db，不需要 &db.sqlite
repo.find_by_id(&db, &params).await?;
```

框架已内置实现：
- `Pool<DB>` （直接传连接池）
- `&Pool<DB>`（传连接池引用）
- `Arc<T>` where `T: DbPool`
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

## 🗄️ 多数据库支持

通过 `#[repository]` 宏的 `db_type` 参数指定数据库类型：

### SQLite（默认）

```rust
use markdown_sql::{repository, SqliteDbPool, DbType, SqlManager};

// 定义数据库封装
struct AppDb {
    pool: sqlx::Pool<sqlx::Sqlite>,
}

impl SqliteDbPool for AppDb {
    fn pool(&self) -> &sqlx::Pool<sqlx::Sqlite> {
        &self.pool
    }
}

// db_type 默认为 "sqlite"，可省略
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    async fn find_all(&self) -> Result<Vec<User>, MarkdownSqlError>;
    async fn insert(&self, user: &UserInsert) -> Result<u64, MarkdownSqlError>;
}

// 使用
let repo = UserRepositoryImpl::new(&SQL_MANAGER);
let users = repo.find_all(&db).await?;
```

### MySQL

```rust
use markdown_sql::{repository, MySqlDbPool, DbType, SqlManager};

// 定义数据库封装
struct AppDb {
    pool: sqlx::Pool<sqlx::MySql>,
}

impl MySqlDbPool for AppDb {
    fn pool(&self) -> &sqlx::Pool<sqlx::MySql> {
        &self.pool
    }
}

// 指定 db_type = "mysql"
#[repository(sql_file = "sql/UserRepository.md", db_type = "mysql")]
pub trait UserRepository {
    async fn find_all(&self) -> Result<Vec<User>, MarkdownSqlError>;
    async fn insert(&self, user: &UserInsert) -> Result<u64, MarkdownSqlError>;
}

// 使用
let repo = UserRepositoryImpl::new(&SQL_MANAGER);
let users = repo.find_all(&db).await?;
```

### PostgreSQL

```rust
use markdown_sql::{repository, PgDbPool, DbType, SqlManager};

// 定义数据库封装
struct AppDb {
    pool: sqlx::Pool<sqlx::Postgres>,
}

impl PgDbPool for AppDb {
    fn pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        &self.pool
    }
}

// 指定 db_type = "postgres"
#[repository(sql_file = "sql/UserRepository.md", db_type = "postgres")]
pub trait UserRepository {
    async fn find_all(&self) -> Result<Vec<User>, MarkdownSqlError>;
    async fn insert(&self, user: &UserInsert) -> Result<u64, MarkdownSqlError>;
}

// 使用
let repo = UserRepositoryImpl::new(&SQL_MANAGER);
let users = repo.find_all(&db).await?;
```

### 数据库类型配置

SqlManager 需要配置对应的 `DbType`：

```rust
// SQLite/MySQL 使用 ? 占位符
let manager = SqlManager::builder()
    .db_type(DbType::Sqlite)  // 或 DbType::Mysql
    .build()?;

// PostgreSQL 使用 $1, $2, ... 占位符
let manager = SqlManager::builder()
    .db_type(DbType::Postgres)
    .build()?;
```

> **注意**：`db_type` 参数支持 `"sqlite"`、`"mysql"`、`"postgres"`（或 `"postgresql"`、`"pg"`）。

## 📋 参数传递规范

### 推荐方式

**1. 无参数**：

```rust
async fn find_all(&self) -> Result<Vec<User>, MarkdownSqlError>;
```

**2. 单参数对象（推荐）**：

```rust
#[derive(Serialize)]
pub struct IdParams { pub id: i64 }

async fn find_by_id(&self, params: &IdParams) -> Result<Option<User>, MarkdownSqlError>;

// 使用
let params = IdParams { id: 1 };
let user = repo.find_by_id(&db, &params).await?;
```

**3. 多条件查询对象（推荐）**：

```rust
#[derive(Serialize)]
pub struct UserQuery {
    pub name: Option<String>,
    pub status: Option<i32>,
    pub min_age: Option<i32>,
}

async fn find_by_condition(&self, params: &UserQuery) -> Result<Vec<User>, MarkdownSqlError>;

// 使用：按状态查询
let query = UserQuery {
    name: None,
    status: Some(1),
    min_age: None,
};
let users = repo.find_by_condition(&db, &query).await?;

// 使用：组合条件查询
let query = UserQuery {
    name: Some("张%".to_string()),  // LIKE 模糊匹配
    status: Some(1),
    min_age: Some(18),
};
let users = repo.find_by_condition(&db, &query).await?;
```

**4. 插入/更新对象**：

```rust
#[derive(Serialize)]
pub struct UserInsert {
    pub name: String,
    pub age: i32,
    pub email: Option<String>,
    pub status: i32,
}

async fn insert(&self, params: &UserInsert) -> Result<u64, MarkdownSqlError>;

// 使用
let user = UserInsert {
    name: "张三".to_string(),
    age: 25,
    email: Some("zhangsan@test.com".to_string()),
    status: 1,
};
repo.insert(&db, &user).await?;
```

**5. 列表参数对象（IN 查询）**：

```rust
#[derive(Serialize)]
pub struct IdsParams {
    pub ids: Vec<i64>,
}

async fn find_by_ids(&self, params: &IdsParams) -> Result<Vec<User>, MarkdownSqlError>;

// 使用
let params = IdsParams { ids: vec![1, 3, 5] };
let users = repo.find_by_ids(&db, &params).await?;
```

### 对应 SQL 示例

```sql
-- findByCondition
SELECT * FROM users
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
{% if min_age %}AND age >= #{min_age}{% endif %}
ORDER BY id

-- findByIds
SELECT * FROM users
WHERE id IN ({{ ids | bind_join(",") }})
```

### 禁止方式

```rust
// ❌ 禁止：使用 serde_json::json!()
let params = serde_json::json!({ "name": name });

// ❌ 禁止：使用 HashMap
let mut params = HashMap::new();
```

## 📋 错误类型

框架提供细粒度的错误类型便于精准处理：

| 错误类型 | 说明 |
|---------|------|
| `FileNotFound` | SQL 文件未找到 |
| `SqlNotFound` | sqlId 不存在 |
| `ParamMissing` | 模板参数缺失 |
| `RenderError` | 模板渲染失败 |
| `SqlxError` | 数据库执行错误 |
| `TransactionError` | 事务操作失败 |
| `NotFound` | 记录不存在（query_one） |
| `UnsafeSql` | 编译时安全检查失败 |

```rust
use markdown_sql::MarkdownSqlError;

match result {
    Err(MarkdownSqlError::NotFound { sql_id }) => {
        HttpResponse::NotFound().body(format!("未找到: {}", sql_id))
    }
    Err(MarkdownSqlError::SqlxError(e)) => {
        tracing::error!("数据库错误: {}", e);
        HttpResponse::InternalServerError().finish()
    }
    Ok(data) => HttpResponse::Ok().json(data),
}
```

## 📖 文档

详细设计文档请查看 [plan/2025-12-21-markdown-sql.md](plan/2025-12-21-markdown-sql.md)

## 📜 License

MIT
