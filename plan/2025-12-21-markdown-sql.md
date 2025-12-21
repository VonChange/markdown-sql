# markdown-sql: Rust 版 Markdown SQL 框架

> 参考 spring-data-jdbc-mybatis，实现 Rust 版本的 Markdown SQL 管理框架

---

## 📋 需求背景

### 痛点

1. SQL 散落在代码中，难以管理和维护
2. 动态 SQL 拼接繁琐，容易出错
3. 缺乏统一的 SQL 文档化方案

### 目标

参考 Java 的 [spring-data-jdbc-mybatis](https://github.com/VonChange/spring-data-jdbc-mybatis)，实现 Rust 版本：

- **Markdown SQL** - 把 SQL 写在 Markdown 文件里，可读性好，便于文档化
- **模板引擎** - 使用 MiniJinja 实现动态 SQL（条件、循环等）
- **接口映射** - 通过 trait + 宏，方法名自动映射到 Markdown 中的 SQL ID
- **外部包形式** - 可独立发布，供其他项目引入

### 范围

| 包含 | 不包含 |
|-----|-------|
| Markdown SQL 解析 | MyBatis XML 语法 |
| MiniJinja 模板渲染 | ORM 自动生成 insert/update |
| trait 方法映射 | 方法名查询（findByXxx） |
| sqlx 执行封装 | 多数据源支持（初期） |

---

## 🛠️ 技术选型

| 组件 | 选择 | 理由 |
|-----|------|------|
| **模板引擎** | **MiniJinja** | Jinja2 语法、运行时灵活、高性能、`{% include %}` 原生支持 |
| SQL 执行 | sqlx | Rust 标准、异步、多数据库支持 |
| Markdown 解析 | **无依赖（字符串解析）** | 参考 spring-data-jdbc-mybatis，简单高效 |
| 正则表达式 | regex + once_cell | 用于 include 命名空间处理 |
| 过程宏 | syn + quote | Rust 标准方案 |

### 设计理念：AI 友好 + 安全

采用 **MiniJinja 标准语法 + 参数绑定**：

| 功能 | 说明 | 语法 |
|-----|------|------|
| SQL 片段引用 | MiniJinja 原生 | `{% include "columns" %}` |
| 动态条件 | MiniJinja 原生 | `{% if param %}AND col = #{param}{% endif %}` |
| 参数占位 | 防 SQL 注入 | `#{param}` → `?` |
| IN 查询 | 自定义过滤器 | `{{ ids \| bind_join(",") }}` → `?,?,?` |

**优势**：
- ✅ AI 天然理解 Jinja2 语法，可直接生成
- ✅ **参数绑定防止 SQL 注入**（`#{param}` → sqlx 参数绑定）
- ✅ 标准语法 + 少量扩展，易于学习

### ⚠️ 安全设计：参数绑定 + 编译时强制检查

**绝不使用字符串拼接！** 所有参数都通过 sqlx 参数绑定。

#### 语法安全白名单

| 语法 | 状态 | 说明 |
|-----|------|------|
| `#{param}` | ✅ 安全 | 参数绑定 |
| `{{ list \| bind_join(",") }}` | ✅ 安全 | IN 查询 |
| `{% if %}` / `{% for %}` | ✅ 安全 | 动态逻辑 |
| `{% include %}` | ✅ 安全 | SQL 片段引用 |
| `{{ param }}` | ❌ **禁止** | 直接拼接，**编译失败** |
| `{{ list \| join(",") }}` | ❌ **禁止** | 直接拼接，**编译失败** |
| `{{ param \| raw_safe }}` | ⚠️ 豁免 | 显式声明安全 |

#### 编译时强制检查（过程宏）

**在 `#[repository]` 宏展开时检查 SQL 文件**，检测到危险语法则**编译失败**：

```rust
#[repository(sql_file = "sql/UserRepository.md")]  // 编译时读取并检查
pub trait UserRepository {
    async fn find_user_list(&self, ...) -> ...;
}
```

**编译失败示例**：

```
error: SQL 安全检查失败
  --> src/repository/user.rs:3:1
   |
3  | #[repository(sql_file = "sql/UserRepository.md")]
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: sql/UserRepository.md 第 5 行
   = note: 检测到不安全语法: {{ user_name }}
   = help: 请改为: #{user_name} (参数绑定)
   = help: 如需字符串拼接，请使用: {{ user_name | raw_safe }}
```

**优势**：
- ✅ **最早发现问题**：编译阶段就拦截，CI 直接失败
- ✅ **错误信息清晰**：直接指向 Rust 代码位置
- ✅ **无运行时开销**：不影响程序启动速度

#### 安全豁免：`raw_safe` 过滤器

如果**确实需要**字符串拼接（如动态表名、动态排序字段），使用 `raw_safe` 过滤器显式声明：

```sql
-- 动态表名（已审核安全：值来自枚举，非用户输入）
SELECT * FROM {{ table_name | raw_safe }}

-- 动态排序（已审核安全：值来自预定义列表）
ORDER BY {{ order_column | raw_safe }} {{ order_dir | raw_safe }}
```

**使用 `raw_safe` 的注意事项**：
- ⚠️ 只用于**确定安全**的场景（值来自枚举/预定义列表）
- ⚠️ **绝不**用于用户输入
- ⚠️ 使用前需在 Rust 代码中**验证值的合法性**

#### 处理流程

```
cargo build
    ↓
展开 #[repository(sql_file = "xxx.md")] 宏
    ↓
┌─────────────────────────────────────────────┐
│ 编译时安全检查：扫描 {{ }} 语法               │
│                                             │
│ - {{ param }}        → ❌ 编译失败           │
│ - {{ x | join() }}   → ❌ 编译失败           │
│ - {{ x | bind_join }}→ ✅ 通过               │
│ - {{ x | raw_safe }} → ⚠️ 通过（已豁免）     │
└─────────────────────────────────────────────┘
    ↓（全部通过）
生成 Repository 实现代码
    ↓
编译成功

---

运行时
    ↓
SqlManager::render("findUserList", context)
    ↓
阶段一：MiniJinja 渲染（处理动态逻辑）
┌─────────────────────────────────────┐
│ {% if user_name %}                   │
│ AND user_name = #{user_name}         │  →  AND user_name = #{user_name}
│ {% endif %}                          │
└─────────────────────────────────────┘
    ↓
阶段二：参数提取（转换为 ? 占位符）
┌─────────────────────────────────────┐
│ AND user_name = #{user_name}         │  →  SqlResult {
│                                      │       sql: "AND user_name = ?",
│                                      │       params: ["user_name"]
│                                      │     }
└─────────────────────────────────────┘
    ↓
阶段三：sqlx 参数绑定执行
```

### Markdown 解析策略

参考 spring-data-jdbc-mybatis 的 `MarkdownUtil.java` 实现，**不使用任何 Markdown 解析库**，直接用字符串操作：

1. 查找 \`\`\` 代码块起止位置
2. 提取第一行（语言标识，如 `sql`）
3. 提取第二行的 `-- sqlId` 作为 SQL ID
4. 提取代码块内容

**优势**：
- 零依赖，无需引入 pulldown-cmark
- 实现简单（约 50 行 Rust 代码）
- 性能最优（直接字符串操作）

### 为什么选 MiniJinja？

| 对比项 | MiniJinja | Tera | Askama |
|-------|-----------|------|--------|
| 运行时加载 | ✅ | ✅ | ❌ |
| 语法 | Jinja2 | Jinja2 | Jinja2 |
| 性能 | 快 | 中等 | 最快 |
| 动态 SQL 支持 | ✅ 完美 | ✅ | ❌ 需编译时确定 |
| 作者 | Armin Ronacher (Flask) | - | - |

**选择 MiniJinja 的核心原因**：
1. 支持运行时动态加载模板（Markdown SQL 场景必需）
2. Jinja2 语法成熟，学习成本低
3. 高性能，作者是 Flask/Jinja2 创始人

---

## 📦 项目结构

```
markdown-sql/
├── Cargo.toml                    # workspace 配置
├── README.md                     # 项目说明
├── plan/                         # 开发计划文档
│   └── 2025-12-21-markdown-sql.md
├── markdown-sql/                 # 核心库
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                # 入口，导出公共 API
│       ├── parser.rs             # Markdown 解析（纯字符串操作，无依赖）
│       ├── manager.rs            # SQL 管理器（启动时注册 + 缓存）
│       ├── param_extractor.rs    # 参数提取器（#{param} → ? + 参数列表）
│       ├── executor.rs           # SQL 执行器（封装 sqlx，参数绑定）
│       └── error.rs              # 错误定义
├── markdown-sql-macros/          # 过程宏
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                # #[repository] 宏
│       └── safety_checker.rs     # 安全检查器（编译时检测危险语法）
└── examples/
    └── demo/                     # 示例项目
        ├── Cargo.toml
        ├── src/
        │   └── main.rs
        └── sql/
            └── UserRepository.md
```

### Cargo.toml 依赖示例

**markdown-sql/Cargo.toml**（核心库）：

```toml
[package]
name = "markdown-sql"
version = "0.1.0"
edition = "2021"

[dependencies]
minijinja = "2"           # 模板引擎
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
regex = "1"               # 正则表达式（include 命名空间处理）
once_cell = "1"           # 全局缓存
thiserror = "2"           # 错误处理
tracing = "0.1"           # 日志
```

**使用方项目 Cargo.toml**：

```toml
[dependencies]
markdown-sql = { path = "../markdown-sql" }
# 或发布后
# markdown-sql = "0.1"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
tokio = { version = "1", features = ["full"] }
```

---

## 🔄 事务设计（SeaORM 风格）

借鉴 SeaORM 的设计思路，通过 **泛型执行器** 实现一套方法同时支持普通查询和事务。

### 核心设计

```rust
use sqlx::{Database, Executor, Pool, Transaction};

/// 抽象执行器 trait
/// 
/// 让 Repository 方法可以接受 Pool 或 Transaction
pub trait SqlExecutor<'e, DB: Database>: Executor<'e, Database = DB> {}

// 为各种执行器类型实现 trait
impl<'e, DB: Database> SqlExecutor<'e, DB> for &'e Pool<DB> {}
impl<'e, DB: Database> SqlExecutor<'e, DB> for &'e mut sqlx::pool::PoolConnection<DB> {}
impl<'e, DB: Database> SqlExecutor<'e, DB> for &'e mut Transaction<'_, DB> {}
```

### Repository 接口设计

```rust
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    /// 所有方法接受泛型执行器
    /// 可传入 Pool（普通查询）或 Transaction（事务）
    async fn find_by_id<'e, E>(&self, exec: E, id: i64) -> Option<User>
    where
        E: SqlExecutor<'e, Postgres>;
    
    async fn insert<'e, E>(&self, exec: E, user: &User) -> i64
    where
        E: SqlExecutor<'e, Postgres>;
    
    async fn update<'e, E>(&self, exec: E, user: &User) -> u64
    where
        E: SqlExecutor<'e, Postgres>;
}
```

### 使用示例

```rust
// 创建 Repository
let user_repo = UserRepositoryImpl::new(&sql_manager);

// ====== 普通查询（直接用 Pool）======
let user = user_repo.find_by_id(&pool, 1).await?;
user_repo.insert(&pool, &new_user).await?;

// ====== 事务操作 ======
// 方式一：手动事务
let mut tx = pool.begin().await?;
user_repo.insert(&mut tx, &user1).await?;
user_repo.insert(&mut tx, &user2).await?;
user_repo.update(&mut tx, &user3).await?;
tx.commit().await?;  // 不 commit 会自动 rollback

// 方式二：闭包事务（可选封装）
pool.transaction(|tx| async {
    user_repo.insert(tx, &user1).await?;
    user_repo.update(tx, &user2).await?;
    Ok(())
}).await?;
```

### 事务工具函数（可选）

```rust
/// 事务辅助宏 / 函数
pub async fn with_transaction<F, T, E>(
    pool: &Pool<Postgres>,
    f: F,
) -> Result<T, E>
where
    F: for<'c> FnOnce(&'c mut Transaction<'_, Postgres>) -> BoxFuture<'c, Result<T, E>>,
    E: From<sqlx::Error>,
{
    let mut tx = pool.begin().await?;
    let result = f(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}

// 使用
with_transaction(&pool, |tx| Box::pin(async move {
    user_repo.insert(tx, &user1).await?;
    order_repo.insert(tx, &order).await?;
    Ok(())
})).await?;
```

### 设计优势

| 特性 | 说明 |
|-----|------|
| **统一接口** | 一套方法定义，同时支持 Pool 和 Transaction |
| **零额外成本** | 泛型在编译时单态化，无运行时开销 |
| **类型安全** | 编译器确保事务边界正确 |
| **SeaORM 风格** | 熟悉 SeaORM 的用户无学习成本 |
| **灵活控制** | 用户完全控制事务的 begin/commit/rollback |

---

## 📤 返回值类型约定

Repository 方法的返回值类型决定 sqlx 的调用方式。

### 类型规则

| 返回值类型 | sqlx 方法 | 说明 |
|-----------|----------|------|
| `Vec<T>` | `fetch_all()` | 查询列表 |
| `Option<T>` | `fetch_all().first()` | 查询单条，取第一行 |
| `T`（实体） | `fetch_all().first().unwrap()` | 查询单条，取第一行（确保有值） |
| `u64` / `i64` | `execute().rows_affected()` | 返回影响行数（INSERT/UPDATE/DELETE） |

### 示例

```rust
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    // Vec<T> → fetch_all
    async fn find_all<'e, E>(&self, exec: E) -> Vec<User>
    where E: SqlExecutor<'e, Postgres>;
    
    // Option<T> → fetch_all().first()，取第一行
    async fn find_by_id<'e, E>(&self, exec: E, id: i64) -> Option<User>
    where E: SqlExecutor<'e, Postgres>;
    
    // u64 → execute().rows_affected()
    async fn insert<'e, E>(&self, exec: E, user: &User) -> u64
    where E: SqlExecutor<'e, Postgres>;
    
    async fn update<'e, E>(&self, exec: E, user: &User) -> u64
    where E: SqlExecutor<'e, Postgres>;
    
    async fn delete<'e, E>(&self, exec: E, id: i64) -> u64
    where E: SqlExecutor<'e, Postgres>;
}
```

### 设计说明

**为什么单条查询也用 `fetch_all().first()` 而不是 `fetch_one()`？**

- 统一处理逻辑，简化宏实现
- 避免 `fetch_one` 在无结果时抛出错误
- 与 spring-data-jdbc-mybatis 保持一致

---

## 🔢 多数据库占位符

不同数据库使用不同的参数占位符格式。

### 占位符对照

| 数据库 | 占位符格式 | 示例 |
|-------|----------|------|
| MySQL | `?` | `WHERE id = ?` |
| SQLite | `?` | `WHERE id = ?` |
| PostgreSQL | `$1, $2, $3...` | `WHERE id = $1` |

### 实现方案

```rust
/// 数据库类型
#[derive(Debug, Clone, Copy)]
pub enum DbType {
    Mysql,
    Sqlite,
    Postgres,
}

impl ParamExtractor {
    /// 根据数据库类型生成占位符
    pub fn extract(sql: &str, db_type: DbType) -> SqlResult {
        let mut params = Vec::new();
        let mut index = 0;
        
        let new_sql = PARAM_RE.replace_all(sql, |caps: &regex::Captures| {
            let param_name = caps[1].to_string();
            params.push(param_name);
            index += 1;
            
            match db_type {
                DbType::Mysql | DbType::Sqlite => "?".to_string(),
                DbType::Postgres => format!("${}", index),
            }
        }).to_string();
        
        SqlResult { sql: new_sql, params }
    }
}
```

### 配置方式

```rust
// 初始化时指定数据库类型
let sql_manager = SqlManager::builder()
    .db_type(DbType::Postgres)
    .load_file("sql/UserRepository.md")?
    .build()?;
```

---

## 📄 分页查询简化

分页查询通常需要两个 SQL：查询数据 + 查询总数。通过 `{% include %}` 复用条件。

### 推荐写法

```markdown
# UserRepository SQL

## 公共查询条件

​```sql
-- whereCondition
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
​```

## 分页查询用户

​```sql
-- findUserPage
SELECT id, name, age, status
FROM user
WHERE 1=1
{% include "whereCondition" %}
ORDER BY create_time DESC
LIMIT #{size} OFFSET #{offset}
​```

## 统计用户总数

​```sql
-- countUserPage
SELECT COUNT(*) FROM user
WHERE 1=1
{% include "whereCondition" %}
​```
```

### Repository 接口

```rust
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    /// 分页查询
    async fn find_user_page<'e, E>(
        &self, exec: E,
        name: Option<String>,
        status: Option<i32>,
        size: i64,
        offset: i64,
    ) -> Vec<User>
    where E: SqlExecutor<'e, Postgres>;
    
    /// 统计总数
    async fn count_user_page<'e, E>(
        &self, exec: E,
        name: Option<String>,
        status: Option<i32>,
    ) -> i64
    where E: SqlExecutor<'e, Postgres>;
}
```

### 命名约定

| SQL ID | 方法名 | 用途 |
|--------|-------|------|
| `findXxxPage` | `find_xxx_page` | 分页查询数据 |
| `countXxxPage` | `count_xxx_page` | 统计总数 |

---

## 🐛 Debug 模式

开启 Debug 模式后，输出 SQL 执行日志，方便调试。

### 开启方式

```rust
// 方式一：代码配置
let sql_manager = SqlManager::builder()
    .debug(true)
    .build()?;

// 方式二：环境变量
// MARKDOWN_SQL_DEBUG=true cargo run
```

### 日志输出示例

```
[DEBUG markdown-sql] Executing: UserRepository.findById
  SQL: SELECT id, name, age FROM user WHERE id = $1
  Params: [123]
  Duration: 2.3ms
  Rows: 1

[DEBUG markdown-sql] Executing: UserRepository.findUserPage
  SQL: SELECT id, name, age FROM user WHERE 1=1 AND status = $1 LIMIT $2 OFFSET $3
  Params: [1, 10, 0]
  Duration: 5.1ms
  Rows: 10
```

### 实现方案

```rust
use tracing::{debug, instrument};

impl SqlManager {
    #[instrument(skip(self, exec, params))]
    pub async fn execute<'e, E, T>(
        &self,
        exec: E,
        sql_id: &str,
        params: &impl Serialize,
    ) -> Result<Vec<T>, MarkdownSqlError>
    where
        E: SqlExecutor<'e, Postgres>,
        T: for<'r> FromRow<'r, PgRow>,
    {
        let start = std::time::Instant::now();
        
        // 渲染 SQL
        let rendered = self.render(sql_id, params)?;
        let sql_result = ParamExtractor::extract(&rendered, self.db_type);
        
        if self.debug {
            debug!(
                "Executing: {}\n  SQL: {}\n  Params: {:?}",
                sql_id, sql_result.sql, sql_result.params
            );
        }
        
        // 执行查询
        let rows = sqlx::query_as::<_, T>(&sql_result.sql)
            // ... 绑定参数
            .fetch_all(exec)
            .await?;
        
        if self.debug {
            debug!(
                "  Duration: {:?}\n  Rows: {}",
                start.elapsed(), rows.len()
            );
        }
        
        Ok(rows)
    }
}
```

---

## 📦 批量操作（预编译复用）

借鉴 spring-data-jdbc-mybatis，实现 **一条 SQL + 数组参数** 的批量操作。

### 设计理念

| 对比 | `{% for %}` 拼接 | 预编译复用 |
|-----|-----------------|-----------|
| SQL 写法 | 复杂，需要循环 | **简单，和单条一样** |
| 性能 | 一条大 SQL | **预编译复用，减少解析** |
| 安全 | 需要小心处理 | **每个参数都绑定** |
| 可读性 | 差 | **好** |

### SQL 写法（和单条一样！）

```markdown
## 批量插入用户

​```sql
-- batchInsert
INSERT INTO user (name, age, status) VALUES (#{name}, #{age}, #{status})
​```

## 批量更新用户

​```sql
-- batchUpdate
UPDATE user SET name = #{name}, age = #{age} WHERE id = #{id}
​```
```

**注意**：SQL 写法和单条操作完全一样，无需 `{% for %}` 循环！

### Repository 接口

```rust
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    /// 批量插入 - 参数类型为 &[Entity]，自动识别为批量操作
    async fn batch_insert<'e, E>(&self, exec: E, users: &[User]) -> u64
    where E: SqlExecutor<'e, Postgres>;
    
    /// 批量更新
    async fn batch_update<'e, E>(&self, exec: E, users: &[User]) -> u64
    where E: SqlExecutor<'e, Postgres>;
}
```

### 核心实现：BatchExecutor

```rust
use sqlx::{Database, Executor, Transaction, Encode, Type};

/// 批量执行器（类似 JDBC 的 BatchPreparedStatement）
pub struct BatchExecutor<'q, DB: Database> {
    /// 预编译 SQL（带占位符）
    sql: String,
    /// 参数名列表（从 SQL 提取）
    param_names: Vec<String>,
    /// 待执行的参数批次
    batches: Vec<Vec<Box<dyn Encode<'q, DB> + Send + 'q>>>,
}

impl<'q, DB: Database> BatchExecutor<'q, DB> {
    /// 创建批量执行器
    pub fn new(sql: String, param_names: Vec<String>) -> Self {
        Self {
            sql,
            param_names,
            batches: Vec::new(),
        }
    }
    
    /// 添加一批参数（类似 JDBC addBatch）
    pub fn add_batch<T: Serialize>(&mut self, entity: &T) {
        // 从实体提取参数值
        let params = self.extract_params(entity);
        self.batches.push(params);
    }
    
    /// 执行批量操作（类似 JDBC executeBatch）
    /// 
    /// 使用预编译语句复用 + 事务保证原子性
    pub async fn execute<'e>(
        self,
        tx: &mut Transaction<'e, DB>,
    ) -> Result<u64, sqlx::Error> {
        let mut total_affected = 0u64;
        
        // 预编译语句（仅解析一次）
        // sqlx 会自动缓存预编译语句
        for params in self.batches {
            let mut query = sqlx::query(&self.sql);
            
            // 绑定参数
            for param in params {
                query = query.bind(param);
            }
            
            // 执行
            let result = query.execute(&mut **tx).await?;
            total_affected += result.rows_affected();
        }
        
        Ok(total_affected)
    }
}
```

### 宏生成的代码

```rust
// #[repository] 宏展开后的批量方法实现
impl UserRepositoryImpl {
    pub async fn batch_insert<'e, E>(
        &self,
        exec: E,
        users: &[User],
    ) -> Result<u64, MarkdownSqlError>
    where
        E: SqlExecutor<'e, Postgres>,
    {
        if users.is_empty() {
            return Ok(0);
        }
        
        // 1. 获取 SQL 模板
        let sql_template = self.sql_manager.get("batchInsert")?;
        
        // 2. 渲染 SQL（对于批量操作，使用空参数渲染，因为参数通过绑定传入）
        let rendered = self.sql_manager.render("batchInsert", &serde_json::json!({}))?;
        
        // 3. 提取参数占位符
        let sql_result = ParamExtractor::extract(&rendered, DbType::Postgres);
        // sql_result.sql = "INSERT INTO user (name, age, status) VALUES ($1, $2, $3)"
        // sql_result.params = ["name", "age", "status"]
        
        // 4. 开启事务
        let mut tx = exec.begin().await?;
        let mut total = 0u64;
        
        // 5. 预编译复用 + 循环执行
        for user in users {
            let result = sqlx::query(&sql_result.sql)
                .bind(&user.name)
                .bind(&user.age)
                .bind(&user.status)
                .execute(&mut *tx)
                .await?;
            total += result.rows_affected();
        }
        
        // 6. 提交事务
        tx.commit().await?;
        
        Ok(total)
    }
}
```

### 使用示例

```rust
// 准备数据
let users = vec![
    User { name: "Alice".into(), age: 25, status: 1 },
    User { name: "Bob".into(), age: 30, status: 1 },
    User { name: "Charlie".into(), age: 28, status: 1 },
];

// 批量插入（内部自动开启事务）
let affected = user_repo.batch_insert(&pool, &users).await?;
println!("插入 {} 条记录", affected);

// 批量更新
let affected = user_repo.batch_update(&pool, &users).await?;
println!("更新 {} 条记录", affected);
```

### 性能优化：大批量拼接 VALUES

对于超大批量（> 1000 条），可选择拼接成一条 SQL：

```rust
/// 高性能批量插入（拼接 VALUES）
/// 
/// 适用场景：大批量插入（> 1000 条）
/// 注意：PostgreSQL 参数上限 32767，需分批处理
pub async fn batch_insert_fast<'e, E>(
    &self,
    exec: E,
    users: &[User],
) -> Result<u64, sqlx::Error>
where
    E: SqlExecutor<'e, Postgres>,
{
    if users.is_empty() {
        return Ok(0);
    }
    
    // 分批处理（每批最多 1000 条，避免超出参数限制）
    const BATCH_SIZE: usize = 1000;
    let mut total = 0u64;
    
    for chunk in users.chunks(BATCH_SIZE) {
        // 拼接 SQL: INSERT INTO user (...) VALUES ($1,$2,$3), ($4,$5,$6), ...
        let mut sql = String::from("INSERT INTO user (name, age, status) VALUES ");
        let mut args = sqlx::postgres::PgArguments::default();
        
        for (i, user) in chunk.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            let base = i * 3;
            sql.push_str(&format!("(${}, ${}, ${})", base + 1, base + 2, base + 3));
            args.add(&user.name);
            args.add(&user.age);
            args.add(&user.status);
        }
        
        let result = sqlx::query_with(&sql, args)
            .execute(&exec)
            .await?;
        
        total += result.rows_affected();
    }
    
    Ok(total)
}
```

### 方案对比

| 方案 | 适用场景 | 性能 | 复杂度 |
|-----|---------|-----|-------|
| **预编译复用** | 通用场景 | ⭐⭐⭐ | 低 |
| **拼接 VALUES** | 大批量插入 | ⭐⭐⭐⭐ | 中 |
| **COPY/LOAD DATA** | 超大批量 | ⭐⭐⭐⭐⭐ | 高（数据库特定） |

### 识别规则

宏根据参数类型自动识别批量操作：

| 参数类型 | 操作类型 |
|---------|---------|
| `&User` / `User` | 单条操作 |
| `&[User]` / `Vec<User>` | 批量操作 |

---

## 🎯 核心设计

### 1. Markdown SQL 格式（MiniJinja + 参数绑定）

采用 **MiniJinja 语法 + `#{param}` 参数绑定**，AI 友好且安全。

`sql/UserRepository.md`:

```markdown
# UserRepository SQL

## 公共字段定义

​```sql
-- columns
id, user_code, user_name, mobile_no, create_time
​```

## 公共查询条件

​```sql
-- commonCondition
AND is_delete = 0
{% if status %}AND status = #{status}{% endif %}
​```

## 查询用户列表

​```sql
-- findUserList
SELECT {% include "columns" %} FROM user_info
WHERE 1=1
{% include "commonCondition" %}
{% if user_name %}AND user_name LIKE CONCAT(#{user_name}, '%'){% endif %}
ORDER BY create_time DESC
​```

## 插入用户

​```sql
-- insertUser
INSERT INTO user_info (user_code, user_name, mobile_no, create_time)
VALUES (#{user_code}, #{user_name}, #{mobile_no}, #{create_time})
​```

## 更新用户

​```sql
-- updateUser
UPDATE user_info
SET
{% if user_name %}user_name = #{user_name},{% endif %}
{% if mobile_no %}mobile_no = #{mobile_no},{% endif %}
updated_at = datetime('now')
WHERE id = #{id}
​```

## 删除用户

​```sql
-- deleteById
DELETE FROM user_info WHERE id = #{id}
​```

## IN 查询示例

​```sql
-- findByIds
SELECT * FROM user_info
WHERE id IN ({{ ids | bind_join(",") }})
​```
```

**格式规则：**

- SQL 代码块必须是 \`\`\`sql ... \`\`\`
- SQL ID 通过 `-- sqlId` 注释标识（必须是代码块第一行）
- **参数占位**：使用 `#{param}` 语法（转换为 sqlx 参数绑定）
- **IN 查询**：使用 `{{ list | bind_join(",") }}` 自定义过滤器

### SQL 片段引用：`{% include "sqlId" %}`

使用 MiniJinja 原生的 `{% include %}` 语法复用 SQL 片段。

#### ⚠️ 命名空间规则（重要）

由于多个 Markdown 文件可能有相同的 sqlId（如 `findById`、`insert`），采用**命名空间**避免冲突：

**模板注册规则**：`文件名.sqlId`

| Markdown 文件 | sqlId | 注册为 |
|--------------|-------|--------|
| `UserRepository.md` | `findById` | `UserRepository.findById` |
| `UserRepository.md` | `columns` | `UserRepository.columns` |
| `OrderRepository.md` | `findById` | `OrderRepository.findById` |

**include 引用规则**：

| 场景 | 写法 | 解析为 |
|-----|-----|-------|
| 同文件引用 | `{% include "columns" %}` | 自动补全为 `当前文件名.columns` |
| 跨文件引用 | `{% include "UserRepository.columns" %}` | 完整命名空间，原样使用 |

#### 同文件引用示例

`sql/UserRepository.md`:

```markdown
​```sql
-- columns
id, user_code, user_name
​```

​```sql
-- baseCondition
AND is_delete = 0
{% include "statusCondition" %}
​```

​```sql
-- statusCondition
{% if status %}AND status = {{ status }}{% endif %}
​```

​```sql
-- findUserList
SELECT {% include "columns" %} FROM user_info
WHERE 1=1
{% include "baseCondition" %}
ORDER BY id DESC
​```
```

> 同文件内的 `{% include "columns" %}` 自动解析为 `{% include "UserRepository.columns" %}`

#### 跨文件引用示例

`sql/OrderRepository.md`:

```markdown
​```sql
-- columns
id, order_no, user_id, amount, create_time
​```

​```sql
-- findWithUser
SELECT 
  {% include "columns" %},
  {% include "UserRepository.columns" %}
FROM order_info o
JOIN user_info u ON o.user_id = u.id
WHERE o.id = #{id}
​```
```

> 跨文件引用必须使用完整命名空间：`{% include "UserRepository.columns" %}`

### 动态 SQL：MiniJinja 标准语法

**不使用自定义简化语法**，直接使用 MiniJinja 标准语法（AI 友好）：

#### 条件判断

```sql
{% if user_name %}AND user_name = {{ user_name }}{% endif %}
{% if status %}AND status = {{ status }}{% endif %}
```

#### Like 模糊查询

```sql
-- 右模糊
{% if user_name %}AND user_name LIKE CONCAT({{ user_name }}, '%'){% endif %}

-- 左模糊
{% if user_name %}AND user_name LIKE CONCAT('%', {{ user_name }}){% endif %}

-- 全模糊
{% if user_name %}AND user_name LIKE CONCAT('%', {{ user_name }}, '%'){% endif %}
```

#### IN 查询

```sql
{% if ids %}
AND id IN ({{ ids | join(",") }})
{% endif %}
```

#### 比较运算

```sql
{% if min_age %}AND age > {{ min_age }}{% endif %}
{% if max_age %}AND age < {{ max_age }}{% endif %}
{% if start_time %}AND create_time >= {{ start_time }}{% endif %}
```

#### 完整示例

```sql
-- findUserList
SELECT {% include "columns" %} FROM user_info
WHERE 1=1
{% include "commonCondition" %}
{% if user_name %}AND user_name LIKE CONCAT({{ user_name }}, '%'){% endif %}
{% if status %}AND status = {{ status }}{% endif %}
{% if ids %}AND id IN ({{ ids | join(",") }}){% endif %}
ORDER BY id DESC
```

**优势**：
- ✅ AI 可直接生成，无需学习自定义语法
- ✅ 标准 Jinja2 语法，通用性强
- ✅ 无需预处理器，架构更简单

### 2. Repository Trait 定义

```rust
use markdown_sql::prelude::*;

/// 用户 Repository
/// 
/// sql_file 属性指定对应的 Markdown 文件路径
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    /// 查询用户列表
    /// 方法名 find_user_list -> Markdown 中的 -- findUserList
    async fn find_user_list(
        &self,
        user_name: Option<String>,
        status: Option<i32>,
    ) -> Result<Vec<UserInfo>, MarkdownSqlError>;

    /// 插入用户
    async fn insert_user(
        &self,
        user_code: &str,
        user_name: &str,
        mobile_no: &str,
        create_time: &str,
    ) -> Result<u64, MarkdownSqlError>;

    /// 更新用户
    async fn update_user(
        &self,
        id: i64,
        user_name: Option<String>,
        mobile_no: Option<String>,
    ) -> Result<u64, MarkdownSqlError>;

    /// 删除用户
    async fn delete_by_id(&self, id: i64) -> Result<u64, MarkdownSqlError>;
}
```

### 3. 宏展开结果（参考）

```rust
// #[repository] 宏会生成以下代码：

pub struct UserRepositoryImpl<'a> {
    pool: &'a SqlitePool,
    sql_loader: SqlLoader,  // Markdown SQL 加载器
}

impl<'a> UserRepositoryImpl<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        let sql_loader = SqlLoader::from_file("sql/UserRepository.md").unwrap();
        Self { pool, sql_loader }
    }
}

impl<'a> UserRepository for UserRepositoryImpl<'a> {
    async fn find_user_list(
        &self,
        user_name: Option<String>,
        status: Option<i32>,
    ) -> Result<Vec<UserInfo>, MarkdownSqlError> {
        // 1. 获取 SQL 模板
        let template = self.sql_loader.get("findUserList")?;
        
        // 2. 渲染模板（MiniJinja）
        let context = minijinja::context! {
            user_name => user_name,
            status => status,
        };
        let sql = template.render(context)?;
        
        // 3. 执行 SQL
        let result = sqlx::query_as::<_, UserInfo>(&sql)
            .fetch_all(self.pool)
            .await?;
        
        Ok(result)
    }
    
    // ... 其他方法类似
}
```

### 4. 核心模块

#### 4.1 Markdown 解析器 (parser.rs)

参考 spring-data-jdbc-mybatis 的 `MarkdownUtil.java`，使用纯字符串操作：

```rust
use std::collections::HashMap;

const CODE_BLOCK_MARKER: &str = "```";
const SQL_ID_PREFIX: &str = "--";

/// 解析 Markdown 文件，提取 SQL 代码块
/// 
/// 解析规则：
/// 1. 查找 ``` 代码块
/// 2. 第一行为语言标识（如 sql）
/// 3. 第二行以 -- 开头，则提取为 SQL ID
/// 4. 剩余内容为 SQL 模板
pub fn parse_markdown_sql(content: &str) -> HashMap<String, String> {
    let mut sql_map = HashMap::new();
    let mut pos = 0;
    let len = content.len();
    let marker_len = CODE_BLOCK_MARKER.len();

    while pos < len {
        // 查找代码块开始位置
        let Some(start) = content[pos..].find(CODE_BLOCK_MARKER) else {
            break;
        };
        let block_start = pos + start + marker_len;

        // 提取第一行（语言标识）
        let first_line_end = content[block_start..].find('\n').unwrap_or(0);
        let lang = content[block_start..block_start + first_line_end].trim();
        
        // 跳过非 sql 代码块
        let content_start = block_start + first_line_end + 1;

        // 查找代码块结束位置
        let Some(end_offset) = content[content_start..].find(CODE_BLOCK_MARKER) else {
            break;
        };
        let block_end = content_start + end_offset;

        // 提取代码块内容
        let block_content = &content[content_start..block_end];
        
        // 提取 SQL ID（第一行以 -- 开头）
        if let Some(first_line) = block_content.lines().next() {
            let trimmed = first_line.trim();
            if trimmed.starts_with(SQL_ID_PREFIX) {
                let sql_id = trimmed[SQL_ID_PREFIX.len()..].trim().to_string();
                if !sql_id.is_empty() {
                    // SQL 内容为第一行之后的部分
                    let sql_content: String = block_content
                        .lines()
                        .skip(1)
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim()
                        .to_string();
                    sql_map.insert(sql_id, sql_content);
                }
            } else if !lang.is_empty() {
                // 如果没有 -- sqlId，使用语言标识作为 key（兼容模式）
                sql_map.insert(lang.to_string(), block_content.trim().to_string());
            }
        }

        pos = block_end + marker_len;
    }

    sql_map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_sql() {
        let content = r#"
# User SQL

​```sql
-- findUserList
SELECT * FROM user_info
WHERE 1=1
​```

​```sql
-- insertUser
INSERT INTO user_info (name) VALUES ('test')
​```
"#;
        let sql_map = parse_markdown_sql(content);
        assert!(sql_map.contains_key("findUserList"));
        assert!(sql_map.contains_key("insertUser"));
    }
}
```

**实现说明**：
- 无任何外部依赖，纯 Rust 标准库
- 约 60 行代码，简洁高效
- 支持 `-- sqlId` 格式的 SQL ID 提取
- 自动跳过语言标识行

#### 4.2 SQL 管理器 (manager.rs)

**核心模块**：启动时将所有 SQL 片段注册到 MiniJinja Environment，**使用命名空间避免冲突**。

```rust
use minijinja::Environment;
use once_cell::sync::Lazy;
use std::sync::RwLock;
use std::path::Path;
use std::fs;
use serde::Serialize;
use regex::Regex;

/// 全局 MiniJinja 环境（包含所有 SQL 模板）
static ENV: Lazy<RwLock<Environment<'static>>> = Lazy::new(|| {
    RwLock::new(Environment::new())
});

/// include 引用正则（匹配 {% include "xxx" %}）
static INCLUDE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\{%\s*include\s+"([^"]+)"\s*%\}"#).unwrap()
});

/// SQL 管理器
pub struct SqlManager;

impl SqlManager {
    /// 初始化 Repository（在启动时调用）
    /// 
    /// 将 Markdown 文件中的所有 SQL 片段注册到 MiniJinja Environment
    /// **使用 `文件名.sqlId` 作为模板名，避免多文件冲突**
    pub fn init(sql_file: &str) -> Result<(), MarkdownSqlError> {
        let path = Path::new(sql_file);
        let content = fs::read_to_string(path)
            .map_err(|_| MarkdownSqlError::FileNotFound(sql_file.to_string()))?;
        
        // 提取命名空间（文件名，不含扩展名）
        // 例如：sql/UserRepository.md -> UserRepository
        let namespace = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| MarkdownSqlError::InvalidPath(sql_file.to_string()))?;
        
        // 解析 Markdown，提取 SQL 片段
        let sql_map = crate::parser::parse_markdown_sql(&content);
        
        // 将每个 SQL 片段注册为 MiniJinja 模板
        let mut env = ENV.write().unwrap();
        for (sql_id, sql_content) in sql_map {
            // 1. 处理 include 引用：补全命名空间
            //    {% include "columns" %} -> {% include "UserRepository.columns" %}
            //    {% include "OtherRepo.columns" %} -> 保持不变
            let processed = Self::expand_include_namespace(&sql_content, namespace);
            
            // 2. 使用命名空间注册模板
            //    columns -> UserRepository.columns
            let full_id = format!("{}.{}", namespace, sql_id);
            
            env.add_template_owned(full_id.clone(), processed)
                .map_err(|e| MarkdownSqlError::TemplateError(e.to_string()))?;
            
            tracing::debug!("[SqlManager] 注册模板: {}", full_id);
        }
        
        tracing::info!("[SqlManager] 初始化完成: {}, 命名空间: {}", sql_file, namespace);
        Ok(())
    }
    
    /// 展开 include 引用的命名空间
    /// 
    /// - 同文件引用：`{% include "columns" %}` -> `{% include "命名空间.columns" %}`
    /// - 跨文件引用：`{% include "OtherRepo.columns" %}` -> 保持不变
    fn expand_include_namespace(content: &str, namespace: &str) -> String {
        INCLUDE_RE.replace_all(content, |caps: &regex::Captures| {
            let ref_id = &caps[1];
            
            // 如果已包含点号，说明是跨文件引用，保持不变
            if ref_id.contains('.') {
                return caps[0].to_string();
            }
            
            // 同文件引用，补全命名空间
            format!("{{% include \"{}.{}\" %}}", namespace, ref_id)
        }).to_string()
    }
    
    /// 渲染 SQL 模板
    /// 
    /// sql_id 格式：`命名空间.sqlId`，例如 `UserRepository.findById`
    pub fn render<T: Serialize>(sql_id: &str, context: T) -> Result<String, MarkdownSqlError> {
        let env = ENV.read().unwrap();
        
        let template = env.get_template(sql_id)
            .map_err(|_| MarkdownSqlError::SqlNotFound(sql_id.to_string()))?;
        
        let rendered = template.render(&context)
            .map_err(|e| MarkdownSqlError::RenderError(e.to_string()))?;
        
        Ok(rendered.trim().to_string())
    }
    
    /// 检查模板是否存在
    pub fn has(sql_id: &str) -> bool {
        let env = ENV.read().unwrap();
        env.get_template(sql_id).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_include_namespace() {
        // 同文件引用
        let sql = r#"SELECT {% include "columns" %} FROM user"#;
        let result = SqlManager::expand_include_namespace(sql, "UserRepository");
        assert_eq!(result, r#"SELECT {% include "UserRepository.columns" %} FROM user"#);
        
        // 跨文件引用（保持不变）
        let sql = r#"SELECT {% include "OtherRepo.columns" %} FROM user"#;
        let result = SqlManager::expand_include_namespace(sql, "UserRepository");
        assert_eq!(result, r#"SELECT {% include "OtherRepo.columns" %} FROM user"#);
        
        // 混合情况
        let sql = r#"SELECT {% include "columns" %}, {% include "OtherRepo.fields" %} FROM user"#;
        let result = SqlManager::expand_include_namespace(sql, "UserRepository");
        assert_eq!(result, r#"SELECT {% include "UserRepository.columns" %}, {% include "OtherRepo.fields" %} FROM user"#);
    }
}
```

**处理流程**：

```
程序启动
    ↓
SqlManager::init("sql/UserRepository.md")
    ↓
1. 提取命名空间：UserRepository
2. 解析 Markdown（提取 SQL 块）
3. 处理 include 引用：
   - {% include "columns" %} -> {% include "UserRepository.columns" %}
   - {% include "OtherRepo.x" %} -> 保持不变
4. 使用 命名空间.sqlId 注册模板
    ↓
程序运行中
    ↓
SqlManager::render("UserRepository.findUserList", context)
    ↓
MiniJinja 渲染（自动展开 include）→ sqlx 执行
```

**初始化示例**：

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::init();
    
    // 初始化 SQL 管理器（启动时注册所有模板）
    SqlManager::init("sql/UserRepository.md")?;   // 注册 UserRepository.xxx
    SqlManager::init("sql/OrderRepository.md")?;  // 注册 OrderRepository.xxx
    
    // 启动 Web 服务器
    // ...
    Ok(())
}
```

**使用示例**：

```rust
// 渲染并执行 SQL（使用完整命名空间）
let sql = SqlManager::render("UserRepository.findUserList", minijinja::context! {
    user_name => Some("张"),
    status => Some(1),
})?;

let users: Vec<UserInfo> = sqlx::query_as(&sql)
    .fetch_all(&pool)
    .await?;

// 另一个 Repository
let result = SqlManager::render("OrderRepository.findById", minijinja::context! {
    id => 123,
})?;
// result.sql = "SELECT ... WHERE id = ?"
// result.params = ["id"]
```

#### 4.3 安全检查器 (markdown-sql-macros/src/safety_checker.rs)

**编译时强制检查**：在 `#[repository]` 宏展开时检测危险语法，**编译失败**。

```rust
use regex::Regex;
use once_cell::sync::Lazy;

/// 安全过滤器白名单
const SAFE_FILTERS: &[&str] = &["bind_join", "raw_safe"];

/// 不安全的 {{ }} 语法正则
/// 匹配 {{ xxx }} 但排除 {{ xxx | safe_filter }}
static UNSAFE_OUTPUT_RE: Lazy<Regex> = Lazy::new(|| {
    // 匹配 {{ ... }} 模式
    Regex::new(r"\{\{\s*[^}]+\s*\}\}").unwrap()
});

/// 安全过滤器正则
static SAFE_FILTER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\|\s*(bind_join|raw_safe)\s*[\(\)]?").unwrap()
});

/// 安全检查器
pub struct SafetyChecker;

impl SafetyChecker {
    /// 检查 SQL 模板是否安全
    /// 
    /// 返回 Err 如果检测到不安全语法
    pub fn check(sql_id: &str, content: &str) -> Result<(), MarkdownSqlError> {
        // 查找所有 {{ }} 输出
        for mat in UNSAFE_OUTPUT_RE.find_iter(content) {
            let output = mat.as_str();
            
            // 检查是否使用了安全过滤器
            if !SAFE_FILTER_RE.is_match(output) {
                // 计算行号
                let line_num = content[..mat.start()].matches('\n').count() + 1;
                
                return Err(MarkdownSqlError::UnsafeSql {
                    sql_id: sql_id.to_string(),
                    line: line_num,
                    content: output.to_string(),
                    suggestion: Self::get_suggestion(output),
                });
            }
        }
        
        Ok(())
    }
    
    /// 生成修复建议
    fn get_suggestion(unsafe_output: &str) -> String {
        // 提取变量名
        if let Some(var) = Self::extract_var_name(unsafe_output) {
            if unsafe_output.contains("join") {
                format!("请改为: {{{{ {} | bind_join(\",\") }}}}", var)
            } else {
                format!("请改为: #{{{}}} (参数绑定)", var)
            }
        } else {
            "请使用 #{param} 参数绑定语法".to_string()
        }
    }
    
    fn extract_var_name(output: &str) -> Option<String> {
        // 简单提取 {{ var }} 中的 var
        let trimmed = output.trim_start_matches("{{").trim_end_matches("}}").trim();
        let var = trimmed.split('|').next()?.trim();
        if !var.is_empty() {
            Some(var.to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsafe_direct_output() {
        let result = SafetyChecker::check("test", "WHERE name = {{ name }}");
        assert!(result.is_err());
    }

    #[test]
    fn test_unsafe_join() {
        let result = SafetyChecker::check("test", "WHERE id IN ({{ ids | join(\",\") }})");
        assert!(result.is_err());
    }

    #[test]
    fn test_safe_bind_join() {
        let result = SafetyChecker::check("test", "WHERE id IN ({{ ids | bind_join(\",\") }})");
        assert!(result.is_ok());
    }

    #[test]
    fn test_safe_raw_safe() {
        let result = SafetyChecker::check("test", "SELECT * FROM {{ table | raw_safe }}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_safe_param_binding() {
        let result = SafetyChecker::check("test", "WHERE name = #{name}");
        assert!(result.is_ok());
    }
}
```

**编译时错误**（在宏中使用 `syn::Error`）：

```rust
// 在宏中生成编译错误
if let Err(e) = SafetyChecker::check(&sql_file, &content) {
    return syn::Error::new(
        Span::call_site(),
        format!(
            "SQL 安全检查失败: {} 第 {} 行\n  内容: {}\n  建议: {}",
            e.sql_id, e.line, e.content, e.suggestion
        )
    ).to_compile_error().into();
}
```

**检查结果类型**：

```rust
pub struct SafetyError {
    pub sql_id: String,
    pub line: usize,
    content: String,
    suggestion: String,
},
```

#### 4.4 参数提取器 (param_extractor.rs)

**核心模块**：将 `#{param}` 转换为 `?` 占位符，并收集参数列表。

```rust
use regex::Regex;
use once_cell::sync::Lazy;

/// 参数占位符正则：匹配 #{param_name}
static PARAM_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"#\{(\w+)\}").unwrap()
});

/// SQL 渲染结果（包含参数列表）
#[derive(Debug, Clone)]
pub struct SqlResult {
    /// 带 ? 占位符的 SQL
    pub sql: String,
    /// 参数名列表（按出现顺序）
    pub params: Vec<String>,
}

/// 参数提取器
pub struct ParamExtractor;

impl ParamExtractor {
    /// 将 #{param} 转换为 ? 并提取参数列表
    /// 
    /// 示例：
    /// - 输入：`WHERE user_name = #{user_name} AND status = #{status}`
    /// - 输出：SqlResult {
    ///     sql: "WHERE user_name = ? AND status = ?",
    ///     params: ["user_name", "status"]
    ///   }
    pub fn extract(sql: &str) -> SqlResult {
        let mut params = Vec::new();
        
        let new_sql = PARAM_RE.replace_all(sql, |caps: &regex::Captures| {
            let param_name = caps[1].to_string();
            params.push(param_name);
            "?".to_string()
        }).to_string();
        
        SqlResult {
            sql: new_sql,
            params,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_param() {
        let result = ParamExtractor::extract("WHERE id = #{id}");
        assert_eq!(result.sql, "WHERE id = ?");
        assert_eq!(result.params, vec!["id"]);
    }

    #[test]
    fn test_multiple_params() {
        let result = ParamExtractor::extract(
            "WHERE user_name = #{user_name} AND status = #{status}"
        );
        assert_eq!(result.sql, "WHERE user_name = ? AND status = ?");
        assert_eq!(result.params, vec!["user_name", "status"]);
    }

    #[test]
    fn test_no_params() {
        let result = ParamExtractor::extract("SELECT * FROM user");
        assert_eq!(result.sql, "SELECT * FROM user");
        assert!(result.params.is_empty());
    }
}
```

#### 4.4 自定义过滤器：bind_join

**用于 IN 查询**：将数组转换为 `?,?,?` 占位符。

```rust
use minijinja::{Environment, Value};

/// 注册自定义过滤器
pub fn register_filters(env: &mut Environment) {
    // bind_join 过滤器：用于 IN 查询
    // {{ ids | bind_join(",") }} → ?,?,? 并记录参数
    env.add_filter("bind_join", bind_join_filter);
}

/// bind_join 过滤器实现
/// 
/// 将数组转换为占位符列表，同时生成参数标记
/// 输入：[1, 2, 3]
/// 输出：#{__bind_0},#{__bind_1},#{__bind_2}
/// 
/// 后续由 ParamExtractor 处理为 ?,?,?
fn bind_join_filter(value: Value, separator: String) -> String {
    if let Ok(seq) = value.try_iter() {
        let placeholders: Vec<String> = seq
            .enumerate()
            .map(|(i, _)| format!("#{{__bind_{}}}", i))
            .collect();
        placeholders.join(&separator)
    } else {
        // 单个值
        "#{__bind_0}".to_string()
    }
}
```

**使用示例**：

```sql
-- Markdown SQL
WHERE id IN ({{ ids | bind_join(",") }})

-- MiniJinja 渲染后
WHERE id IN (#{__bind_0},#{__bind_1},#{__bind_2})

-- ParamExtractor 处理后
SqlResult {
    sql: "WHERE id IN (?,?,?)",
    params: ["__bind_0", "__bind_1", "__bind_2"]
}

-- 执行时从 context 获取对应值绑定
```

#### 4.5 错误定义 (error.rs)

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MarkdownSqlError {
    #[error("文件未找到: {0}")]
    FileNotFound(String),
    
    #[error("无效的文件路径: {0}")]
    InvalidPath(String),
    
    #[error("SQL 未找到: {0}")]
    SqlNotFound(String),
    
    #[error("模板解析错误: {0}")]
    TemplateError(String),
    
    #[error("模板渲染错误: {0}")]
    RenderError(String),
    
    #[error("SQL 执行错误: {0}")]
    SqlxError(#[from] sqlx::Error),
}
```

---

## 📝 使用示例

### 完整示例

```rust
use sqlx::SqlitePool;
use markdown_sql::SqlManager;

// 1. 定义实体
#[derive(Debug, sqlx::FromRow)]
pub struct UserInfo {
    pub id: i64,
    pub user_code: String,
    pub user_name: String,
    pub mobile_no: Option<String>,
    pub create_time: String,
}

// 2. 初始化并使用
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 SQL 管理器（启动时注册模板，使用命名空间）
    SqlManager::init("sql/UserRepository.md")?;   // 注册 UserRepository.xxx
    SqlManager::init("sql/OrderRepository.md")?;  // 注册 OrderRepository.xxx
    
    // 连接数据库
    let pool = SqlitePool::connect("sqlite:data.db").await?;
    
    // 渲染 SQL（使用完整命名空间：文件名.sqlId）
    let sql = SqlManager::render("UserRepository.findUserList", minijinja::context! {
        user_name => Some("张"),
        status => Some(1),
    })?;
    
    // 执行查询
    let users: Vec<UserInfo> = sqlx::query_as(&sql)
        .fetch_all(&pool)
        .await?;
    
    println!("查询到 {} 个用户", users.len());
    Ok(())
}
```

### Markdown SQL 文件（MiniJinja 标准语法）

`sql/UserRepository.md`:

```markdown
# 用户 Repository SQL

## 公共字段

​```sql
-- columns
id, user_code, user_name, mobile_no, create_time
​```

## 公共条件

​```sql
-- commonCondition
AND is_delete = 0
{% if status %}AND status = {{ status }}{% endif %}
​```

## 查询用户列表

​```sql
-- findUserList
SELECT {% include "columns" %} FROM user_info
WHERE 1=1
{% include "commonCondition" %}
{% if user_name %}AND user_name LIKE CONCAT({{ user_name }}, '%'){% endif %}
ORDER BY id DESC
​```

## 插入用户

​```sql
-- insertUser
INSERT INTO user_info (user_code, user_name, mobile_no, create_time)
VALUES ({{ user_code }}, {{ user_name }}, {{ mobile_no }}, datetime('now'))
​```

## 按 ID 列表查询

​```sql
-- findByIds
SELECT {% include "columns" %} FROM user_info
WHERE id IN ({{ ids | join(",") }})
​```
```

### 跨文件引用示例

`sql/OrderRepository.md`:

```markdown
# 订单 Repository SQL

## 公共字段

​```sql
-- columns
id, order_no, user_id, amount, status, create_time
​```

## 查询订单详情（关联用户）

​```sql
-- findWithUser
SELECT 
  o.{% include "columns" %},
  u.{% include "UserRepository.columns" %}
FROM order_info o
LEFT JOIN user_info u ON o.user_id = u.id
WHERE o.id = #{id}
​```
```

> 注意：同文件引用 `{% include "columns" %}` 自动解析为 `OrderRepository.columns`
> 跨文件引用需要完整命名空间 `{% include "UserRepository.columns" %}`

---

## ✅ 任务清单

### 阶段一：核心框架（MVP）

- [ ] 创建 `markdown-sql` 项目结构
- [ ] 实现 Markdown SQL 解析器 (parser.rs) - 纯字符串操作
- [ ] 实现 SQL 管理器 (manager.rs) - 启动时注册模板到 MiniJinja
- [ ] 实现参数提取器 (param_extractor.rs) - `#{param}` → `?/$1` + 参数列表
- [ ] **实现多数据库占位符** - MySQL/SQLite 用 `?`，PostgreSQL 用 `$1`
- [ ] 实现自定义过滤器 `bind_join` - IN 查询支持
- [ ] 实现自定义过滤器 `raw_safe` - 显式安全豁免
- [ ] 实现 SQL 执行器 (executor.rs) - sqlx 参数绑定执行
- [ ] **实现返回值类型约定** - `Vec<T>` / `Option<T>` / `u64` 自动映射
- [ ] **实现 Debug 模式** - 输出 SQL 执行日志
- [ ] **实现批量操作（BatchExecutor）** - 预编译复用 + 事务
- [ ] 基础错误处理 (error.rs)
- [ ] 编写单元测试

### 阶段二：宏支持 + 编译时安全检查

- [ ] 创建 `markdown-sql-macros` 子 crate
- [ ] 实现 `#[repository]` 属性宏
- [ ] **实现编译时安全检查 (safety_checker.rs)** - 检测 `{{ }}` 禁止语法
- [ ] 方法名 -> SQL ID 转换（snake_case -> camelCase）
- [ ] 自动生成 Impl 结构体
- [ ] 编写宏测试

### 阶段三：事务支持（SeaORM 风格）

- [ ] 实现 `SqlExecutor` trait 抽象
- [ ] Repository 方法支持泛型执行器
- [ ] 支持手动事务（`begin` / `commit` / `rollback`）
- [ ] 编写事务测试

### 阶段四：完善

- [ ] 支持多数据库（SQLite、MySQL、PostgreSQL）
- [ ] 编写示例项目
- [ ] 编写文档

---

## 🎯 验收标准

1. **功能验收**
   - [ ] 能正确解析 Markdown 文件中的 SQL 代码块
   - [ ] 能根据 SQL ID 注册 MiniJinja 模板
   - [ ] `{% include "sqlId" %}` 能正确引用其他 SQL 片段
   - [ ] MiniJinja 能正确渲染动态 SQL（条件、循环）
   - [ ] `#{param}` 能正确转换为 `?`（MySQL/SQLite）或 `$1`（PostgreSQL）
   - [ ] `{{ list | bind_join(",") }}` 能正确生成 IN 查询占位符
   - [ ] sqlx 参数绑定执行（防止 SQL 注入）
   - [ ] **返回值类型**：`Vec<T>` 返回列表，`Option<T>` 返回单条（取第一行）
   - [ ] **多数据库**：支持 MySQL、SQLite、PostgreSQL 的占位符差异
   - [ ] **Debug 模式**：开启后输出 SQL、参数、执行时间日志

2. **安全验收（编译时）**
   - [ ] 编译时检测 `{{ param }}` 直接输出语法，**编译失败**
   - [ ] 编译时检测 `{{ list | join() }}` 不安全过滤器，**编译失败**
   - [ ] `{{ param | raw_safe }}` 能通过安全检查（显式豁免）
   - [ ] 编译错误信息清晰，指向 Rust 代码位置，包含 SQL 文件名、行号、建议

3. **事务验收**
   - [ ] Repository 方法支持传入 `&Pool` 执行普通查询
   - [ ] Repository 方法支持传入 `&mut Transaction` 执行事务操作
   - [ ] 手动事务：`begin` → 多次操作 → `commit` 能正常工作
   - [ ] 事务回滚：不调用 `commit`，Transaction drop 时自动回滚

4. **批量操作验收**
   - [ ] 批量插入：一条 SQL + `&[Entity]` 参数，预编译复用执行
   - [ ] 批量更新：一条 SQL + `&[Entity]` 参数，预编译复用执行
   - [ ] 自动识别：参数为 `&[T]` / `Vec<T>` 时自动使用批量模式
   - [ ] 事务保证：批量操作在事务内执行，保证原子性

5. **质量门**
   - [ ] `cargo build` 编译通过
   - [ ] `cargo clippy` 无警告
   - [ ] `cargo test` 测试通过
   - [ ] 关键代码有中文注释

6. **使用验收**
   - [ ] 示例项目能正常运行
   - [ ] API 简洁易用
   - [ ] AI 可直接生成 Markdown SQL（无需学习自定义语法）

---

## 📚 参考资料

- [spring-data-jdbc-mybatis](https://github.com/VonChange/spring-data-jdbc-mybatis) - Java 参考实现
  - 特别参考 `MarkdownUtil.java` 的简洁解析实现
- [MiniJinja](https://github.com/mitsuhiko/minijinja) - Rust 运行时模板引擎
- [sqlx](https://github.com/launchbadge/sqlx) - Rust 异步 SQL 工具包

---

## 💡 语法速查

### 参数绑定（防 SQL 注入）

```sql
-- ✅ 安全写法：使用 #{param}
WHERE user_name = #{user_name}
WHERE id = #{id}

-- ❌ 危险写法：直接拼接（禁止使用！）
WHERE user_name = {{ user_name }}
```

### SQL 片段引用（include）

```jinja
-- 引用其他 SQL 片段
SELECT {% include "columns" %} FROM user_info
WHERE 1=1
{% include "commonCondition" %}
```

### 条件语句

```jinja
{% if user_name %}AND user_name = #{user_name}{% endif %}

{% if status == 1 %}
  AND status = 1
{% elif status == 2 %}
  AND status = 2
{% else %}
  AND status = 0
{% endif %}
```

### IN 查询（bind_join 过滤器）

```jinja
-- 推荐：使用 bind_join 过滤器（安全 + 简洁）
AND id IN ({{ ids | bind_join(",") }})

-- 或者：使用 loop + #{} 判断（繁琐，但也是安全的）
AND id IN ({% for id in ids %}#{id}{% if not loop.last %},{% endif %}{% endfor %})
```

### 循环变量

```jinja
{% for item in items %}
  {{ loop.index }}     -- 当前索引（从 1 开始）
  {{ loop.index0 }}    -- 当前索引（从 0 开始）
  {{ loop.first }}     -- 是否第一个元素
  {{ loop.last }}      -- 是否最后一个元素
  {{ loop.length }}    -- 总元素数量
{% endfor %}
```

### Like 查询

```jinja
-- 右模糊
{% if user_name %}AND user_name LIKE CONCAT(#{user_name}, '%'){% endif %}

-- 左模糊
{% if user_name %}AND user_name LIKE CONCAT('%', #{user_name}){% endif %}

-- 全模糊
{% if user_name %}AND user_name LIKE CONCAT('%', #{user_name}, '%'){% endif %}
```

### 过滤器

```jinja
{{ ids | bind_join(",") }}       -- IN 查询专用（安全）
{{ table | raw_safe }}           -- 显式豁免（仅用于确定安全的场景）
{{ user_name | upper }}          -- 转大写（用于条件判断，非参数）
{{ user_name | lower }}          -- 转小写
{{ user_name | default("") }}    -- 默认值
```

### 安全豁免（raw_safe）

```sql
-- ⚠️ 仅用于确定安全的场景（值来自枚举/预定义列表，非用户输入）
SELECT * FROM {{ table_name | raw_safe }}
ORDER BY {{ order_column | raw_safe }} {{ order_dir | raw_safe }}
```

**使用前提**：
- ✅ 值来自枚举或预定义常量
- ✅ 在 Rust 代码中已验证值的合法性
- ❌ **绝不用于用户输入**

### 方法名转换规则

| Rust 方法名 (snake_case) | SQL ID (camelCase) |
|-------------------------|-------------------|
| `find_user_list` | `findUserList` |
| `insert_user` | `insertUser` |
| `delete_by_id` | `deleteById` |
