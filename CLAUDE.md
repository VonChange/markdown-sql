# markdown-sql AI 协作规范

## 核心原则

1. **SQL 必须写在 Markdown 文件中，禁止硬编码！**
2. **数据库操作必须通过 `#[repository]` 宏 + trait 定义，禁止直接调用底层函数！**

这是本框架的核心设计理念，任何情况下都不能违反。

---

## 🔒 框架强制约束

### 1. `load_content()` 已被移除公开接口

```rust
// ❌ 编译失败：load_content 是 pub(crate)，外部无法调用
manager.load_content(sql_content, "User");  // error[E0624]: method is private
```

### 2. `#[repository]` 宏强制要求 `sql_file` 参数

```rust
// ❌ 编译失败：缺少 sql_file 参数
#[repository]  // error: missing required argument `sql_file`
pub trait UserRepository { ... }

// ✅ 必须指定 SQL 文件路径
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository { ... }
```

### 3. 底层函数已移到 `__internal` 模块，禁止直接调用

```rust
// ❌ 编译失败：底层函数不再公开导出
use markdown_sql::query_list;  // error: no `query_list` in the root
use markdown_sql::execute;     // error: no `execute` in the root

// ❌ 严禁！即使能访问 __internal 也不能直接调用
markdown_sql::__internal::sqlite::query_list(...);  // 违反规范！

// ✅ 必须通过 Repository trait 方法调用
let repo = UserRepositoryImpl::new(&SQL_MANAGER);
let users = repo.find_all(&db).await?;
```

### 4. 多数据库支持通过 `db_type` 参数

```rust
// SQLite（默认）
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository { ... }

// MySQL
#[repository(sql_file = "sql/UserRepository.md", db_type = "mysql")]
pub trait UserRepository { ... }

// PostgreSQL
#[repository(sql_file = "sql/UserRepository.md", db_type = "postgres")]
pub trait UserRepository { ... }
```

---

## ❌ 禁止的做法

### 1. 直接使用 sqlx 硬编码 SQL（最重要！）

```rust
// ❌ 严禁！绕过框架直接硬编码 SQL
sqlx::query("SELECT * FROM users WHERE id = ?")
    .bind(id)
    .fetch_one(&pool)
    .await?;

// ❌ 严禁！query_as 也不行
sqlx::query_as::<_, User>("SELECT * FROM users")
    .fetch_all(&pool)
    .await?;

// ❌ 严禁！字符串拼接更危险
let sql = format!("SELECT * FROM {} WHERE id = {}", table, id);
sqlx::query(&sql).execute(&pool).await?;
```

### 2. 直接调用底层函数（绕过 Repository）

```rust
// ❌ 严禁！底层函数不再公开导出
markdown_sql::query_list(&manager, &db, "findAll", &params).await?;
markdown_sql::execute(&manager, &db, "insert", &user).await?;

// ❌ 严禁！通过 __internal 调用也不行
markdown_sql::__internal::sqlite::query_list(...).await?;
markdown_sql::__internal::mysql::execute(...).await?;

// ✅ 必须通过 Repository trait 方法
let repo = UserRepositoryImpl::new(&SQL_MANAGER);
let users = repo.find_all(&db).await?;
let affected = repo.insert(&db, &user).await?;
```

**必须使用 `#[repository]` 宏 + trait 方式！**

### 3. 代码中硬编码 SQL 字符串

```rust
// ❌ 错误：框架已禁止此方式
let sql_content = r#"..."#;
manager.load_content(sql_content, "User");  // 编译失败
```

### 3. 测试中硬编码 SQL

```rust
// ❌ 错误：测试代码中也不能硬编码
fn setup_manager() -> SqlManager {
    let sql = r#"-- findById SELECT * FROM users"#;
    manager.load_content(sql, "Test");  // 编译失败
}
```

### 4. 任何形式的 SQL 字符串拼接

```rust
// ❌ 错误：字符串拼接 SQL（SQL 注入风险）
let sql = format!("SELECT * FROM {} WHERE id = {}", table, id);
```

---

## ✅ 正确的做法

### 1. SQL 写在 Markdown 文件中

```markdown
# UserRepository SQL

## 根据 ID 查询

​```sql
-- findById
SELECT * FROM users WHERE id = #{id}
​```

## 查询所有

​```sql
-- findAll
SELECT * FROM users ORDER BY id
​```

## 插入用户

​```sql
-- insert
INSERT INTO users (name, age, status) VALUES (#{name}, #{age}, #{status})
​```
```

### 2. 定义 Repository trait（唯一入口！）

```rust
use markdown_sql::{repository, SqlManager, SqliteDbPool};
use once_cell::sync::Lazy;

// 全局 SqlManager
static SQL_MANAGER: Lazy<SqlManager> = Lazy::new(|| {
    let mut manager = SqlManager::builder()
        .db_type(DbType::Sqlite)
        .debug(true)
        .build()
        .expect("创建 SqlManager 失败");
    
    manager.load_file("sql/UserRepository.md").expect("加载失败");
    manager
});

// 定义 Repository trait
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    async fn find_all(&self) -> Result<Vec<User>, MarkdownSqlError>;
    async fn find_by_id(&self, params: &IdParams) -> Result<Option<User>, MarkdownSqlError>;
    async fn insert(&self, params: &UserInsert) -> Result<u64, MarkdownSqlError>;
}

// 获取 Repository 实例
fn get_user_repo() -> UserRepositoryImpl {
    UserRepositoryImpl::new(&SQL_MANAGER)
}
```

### 3. 使用 Repository

```rust
// ✅ 正确：通过 Repository 方法调用
let repo = get_user_repo();
let users = repo.find_all(&db).await?;
let user = repo.find_by_id(&db, &IdParams { id: 1 }).await?;
let affected = repo.insert(&db, &user_insert).await?;
```

### 4. 事务操作

#### 方式一：`#[transactional]` 自动事务（推荐用于批量操作）

```rust
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    // 自动事务方法：自动开启、提交、回滚
    #[transactional]
    async fn batch_insert(&self, user: &UserInsert) -> Result<u64, MarkdownSqlError>;
}

// 调用时自动处理事务
repo.batch_insert(&db, &user).await?;
```

#### 方式二：手动事务（用于跨 Repository 操作）

每个 Repository 方法都有 `_tx` 事务版本：

```rust
// ✅ 正确：通过 Repository 事务方法
let repo = get_user_repo();

// 开启事务
let mut tx = repo.begin_transaction(&db).await?;

// 使用 _tx 版本方法
repo.insert_tx(&mut tx, &user1).await?;
repo.insert_tx(&mut tx, &user2).await?;
let users = repo.find_all_tx(&mut tx).await?;

// 提交事务
tx.commit().await?;
```

### 5. 测试也使用 Repository

```rust
// ✅ 正确：测试代码也必须使用 Repository trait
#[repository(sql_file = "tests/sql/TestRepository.md")]
pub trait TestRepository {
    async fn create_table(&self) -> Result<u64, MarkdownSqlError>;
    async fn find_all(&self) -> Result<Vec<User>, MarkdownSqlError>;
}

#[tokio::test]
async fn test_query() {
    let repo = get_test_repo();
    repo.create_table(&db).await.expect("创建表失败");
    let users = repo.find_all(&db).await.expect("查询失败");
}
```

---

## 目录结构规范

```
项目根目录/
├── sql/                          # SQL 文件目录
│   ├── UserRepository.md         # 用户相关 SQL
│   ├── OrderRepository.md        # 订单相关 SQL
│   └── ...
├── src/
│   └── repository/               # Repository 代码
│       ├── user.rs
│       └── order.rs
└── tests/
    └── sql/                      # 测试用 SQL 文件
        └── TestRepository.md
```

---

## SQL 文件命名规范

| 类型 | 命名 | 示例 |
|-----|------|------|
| Repository SQL | `{Entity}Repository.md` | `UserRepository.md` |
| 公共 SQL 片段 | `Common.md` 或 `Shared.md` | `CommonConditions.md` |
| 测试 SQL | `{Test}Repository.md` | `FeatureTestRepository.md` |

---

## SQL ID 命名规范

| 操作 | 前缀 | 示例 |
|-----|------|------|
| 查询单条 | `findBy` | `findById`, `findByEmail` |
| 查询列表 | `findAll`, `find` | `findAll`, `findByStatus` |
| 插入 | `insert` | `insert`, `insertBatch` |
| 更新 | `update` | `update`, `updateStatus` |
| 删除 | `delete` | `deleteById`, `deleteByIds` |
| 统计 | `count` | `count`, `countByStatus` |
| 片段 | 名词 | `columns`, `commonCondition` |

---

## 参数绑定规范

| 语法 | 用途 | 安全性 |
|-----|------|--------|
| `#{param}` | 参数绑定 | ✅ 安全 |
| `{{ list \| bind_join(",") }}` | IN 查询 | ✅ 安全 |
| `{{ param }}` | 直接输出 | ❌ 禁止 |
| `{{ param \| raw_safe }}` | 豁免 | ⚠️ 仅限预定义值 |

---

## ⚠️ 防止全表查询

当所有动态条件都为空时，会导致全表扫描：

```sql
-- ❌ 危险：所有参数为空时变成 SELECT * FROM users WHERE 1=1
SELECT * FROM users
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
```

### 必须遵守的规则

#### 1. 条件查询必须带分页

```sql
-- ✅ 正确：强制分页
SELECT * FROM users
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
LIMIT #{page_size} OFFSET #{offset}
```

#### 2. 业务层必须验证参数

```rust
// ✅ 正确：调用前验证
if params.name.is_none() && params.status.is_none() {
    return Err(AppError::InvalidParams("至少需要一个查询条件"));
}
let users = repo.find_by_condition(&db, &params).await?;
```

#### 3. 设计必填条件

```sql
-- ✅ 正确：status 必填，不用 if
SELECT * FROM users
WHERE status = #{status}
{% if name %}AND name LIKE #{name}{% endif %}
```

#### 4. 列表查询强制 LIMIT

```sql
-- ✅ 正确：添加 LIMIT 保护
SELECT * FROM users
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
LIMIT {{ limit | default(100) }}
```

### 场景策略

| 场景 | 策略 |
|-----|------|
| 后台管理查询 | 业务验证 + 强制分页 |
| 精确查询 | 必填参数设计 |
| 导出/报表 | 业务验证 + LIMIT 保护 |
| 统计查询 | 必填时间范围 |

---

## 提交检查清单

- [ ] **是否有 `sqlx::query` 直接调用？**（最重要！）
- [ ] **是否有直接调用 `__internal` 模块？**（必须用 Repository）
- [ ] **是否通过 `#[repository]` 宏定义 trait？**（唯一入口）
- [ ] **条件查询是否有分页或 LIMIT？**（防全表扫描）
- [ ] **全动态条件是否有业务验证？**（防空条件查询）
- [ ] SQL 是否写在 Markdown 文件中？
- [ ] 是否有硬编码 SQL 字符串？
- [ ] 测试是否从文件加载 SQL？
- [ ] SQL ID 命名是否规范？
- [ ] 是否使用 `#{param}` 参数绑定？

### 自查命令

```bash
# 检测是否有直接使用 sqlx::query 的代码
rg 'sqlx::query' --type rust src/

# 检测是否有直接调用 __internal 模块
rg '__internal' --type rust src/

# 检测是否有直接调用底层函数（应该没有输出）
rg 'markdown_sql::(query_list|execute|batch_execute|begin_transaction)' --type rust src/

# 如果有输出，说明违反规范，必须修改！
```

---

## 常见错误修复

### 错误 1：直接使用 sqlx::query（最常见！）

**错误代码**：
```rust
// ❌ 错误
let users: Vec<User> = sqlx::query_as("SELECT * FROM users WHERE status = ?")
    .bind(status)
    .fetch_all(&pool)
    .await?;
```

**修复步骤**：

1. 在 `sql/UserRepository.md` 中添加 SQL
2. 定义 Repository trait
3. 使用 Repository 方法调用

### 错误 2：直接调用底层函数

**错误代码**：
```rust
// ❌ 错误：直接调用底层函数
let users: Vec<User> = markdown_sql::query_list(&manager, &db, "findAll", &params).await?;
```

**修复步骤**：

1. 定义 Repository trait：
```rust
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    async fn find_all(&self) -> Result<Vec<User>, MarkdownSqlError>;
}
```

2. 使用 Repository：
```rust
let repo = UserRepositoryImpl::new(&SQL_MANAGER);
let users = repo.find_all(&db).await?;
```

### 错误 3：测试中硬编码 SQL

**修复步骤**：
1. 创建 `tests/sql/XxxRepository.md` 文件
2. 把 SQL（包括建表 DDL）移到 Markdown 文件中
3. 定义测试用 Repository trait
4. 测试代码通过 Repository 方法操作数据库

### 错误 4：使用 `{{ param }}` 直接输出

**修复步骤**：
1. 改为 `#{param}` 使用参数绑定
2. 如果是 IN 查询，使用 `{{ list | bind_join(",") }}`
3. 如果确定安全（值来自枚举），使用 `{{ param | raw_safe }}`
