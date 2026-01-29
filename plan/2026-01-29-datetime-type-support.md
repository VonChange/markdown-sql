# DateTime 类型原生支持

## 问题

当前框架通过 `serde_json::to_value()` 序列化参数，导致 `NaiveDateTime` 等时间类型被转换为 ISO 8601 字符串：

```
NaiveDateTime → serde 序列化 → "2026-01-29T20:00:00" → 绑定为 String → PostgreSQL 报错
```

PostgreSQL 期望 TIMESTAMP 类型，但收到的是 TEXT。

## 根本原因

`postgres.rs` 的 `prepare_sql` 函数：

```rust
fn prepare_sql<P: Serialize>(...) -> Result<(String, Vec<String>, Value)> {
    // 问题在这里：序列化丢失类型信息
    let json_value = serde_json::to_value(params)?;
    // ...
}
```

## 解决方案：TypedParams Trait

**核心思路**：
1. 模板渲染阶段仍用 JSON（提取参数名、条件判断）
2. 参数绑定阶段直接从原始结构体取值，保留类型

### 1. 新增 `TypedParams` trait

```rust
// markdown-sql/src/typed_params.rs

use sqlx::postgres::PgArguments;
use sqlx::mysql::MySqlArguments;
use sqlx::sqlite::SqliteArguments;

/// 类型感知的参数绑定
/// 
/// 通过 derive macro 自动实现，保留字段原始类型
pub trait TypedParamsPg {
    /// 根据参数名列表，将字段值绑定到 PgArguments
    fn bind_to_args(&self, param_names: &[String], args: &mut PgArguments);
}

pub trait TypedParamsMySql {
    fn bind_to_args(&self, param_names: &[String], args: &mut MySqlArguments);
}

pub trait TypedParamsSqlite {
    fn bind_to_args(&self, param_names: &[String], args: &mut SqliteArguments<'_>);
}
```

### 2. 新增 derive macro

```rust
// markdown-sql-macros/src/lib.rs

/// 为参数结构体生成类型感知的绑定代码
/// 
/// ## 示例
/// 
/// ```rust
/// #[derive(Serialize, TypedParams)]
/// struct LogRestoreInsert {
///     log_path: String,
///     expires_date: Option<NaiveDateTime>,
/// }
/// ```
/// 
/// 生成的代码：
/// 
/// ```rust
/// impl TypedParamsPg for LogRestoreInsert {
///     fn bind_to_args(&self, param_names: &[String], args: &mut PgArguments) {
///         for name in param_names {
///             match name.as_str() {
///                 "log_path" => { args.add(&self.log_path); }
///                 "expires_date" => { args.add(&self.expires_date); }
///                 _ => {}
///             }
///         }
///     }
/// }
/// ```
#[proc_macro_derive(TypedParams)]
pub fn derive_typed_params(input: TokenStream) -> TokenStream {
    // ... 解析结构体字段，生成 match 分支
}
```

### 3. 修改底层执行函数

```rust
// postgres.rs

// 新增：类型感知版本
pub async fn query_list_typed<T, P, D>(
    manager: &SqlManager,
    db: &D,
    sql_id: &str,
    params: &P,
) -> Result<Vec<T>>
where
    T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
    P: Serialize + TypedParamsPg,  // 要求实现 TypedParamsPg
    D: PgDbPool,
{
    // 1. 用 JSON 渲染模板、提取参数名（保持不变）
    let json_value = serde_json::to_value(params)?;
    let rendered = manager.render(sql_id, &json_value)?;
    let result = ParamExtractor::extract(&rendered, manager.db_type());
    
    // 2. 使用 TypedParams 绑定（新增：保留类型）
    let mut args = PgArguments::default();
    params.bind_to_args(&result.params, &mut args);
    
    // 3. 执行查询
    let rows = sqlx::query_as_with::<_, T, _>(&result.sql, args)
        .fetch_all(db.pool())
        .await?;
    
    Ok(rows)
}
```

### 4. 修改 `#[repository]` 宏

生成的代码调用 `query_list_typed` 而不是 `query_list`：

```rust
// 当前生成
#internal_mod::query_list::<#inner_ty, _, _>(...)

// 改为
#internal_mod::query_list_typed::<#inner_ty, _, _>(...)
```

## 用户使用方式

```rust
// 参数结构体需要同时 derive Serialize 和 TypedParams
#[derive(Debug, Clone, Serialize, TypedParams)]
pub struct LogRestoreInsert {
    pub log_path: String,
    pub expires_date: Option<NaiveDateTime>,  // ✅ 类型保留
}

// Repository 使用不变
#[repository(sql_file = "sql/LogRestoreRepository.md", db_type = "postgres")]
pub trait LogRestoreRepository {
    async fn insert(&self, params: &LogRestoreInsert) -> Result<u64, MarkdownSqlError>;
}
```

## 需要处理的边界情况

### 1. 嵌套字段（`user.name`）

当前框架支持 `#{user.name}` 访问嵌套属性，TypedParams 也需要支持：

```rust
// 生成的代码需要处理嵌套
match name.as_str() {
    "user.name" => { args.add(&self.user.name); }
    // ...
}
```

### 2. EmptyParams（无参数查询）

框架有 `EmptyParams` 用于无参数的查询，需要为它实现 TypedParams：

```rust
impl TypedParamsPg for EmptyParams {
    fn bind_to_args(&self, _param_names: &[String], _args: &mut PgArguments) {
        // 空实现
    }
}
```

### 3. IN 查询（bind_join）

`{{ ids | bind_join(",") }}` 生成的参数名是 `__bind_0`, `__bind_1` 等，需要特殊处理：

```rust
// 需要从数组字段按索引取值
match name.as_str() {
    n if n.starts_with("__bind_") => {
        let idx: usize = n.strip_prefix("__bind_").unwrap().parse().unwrap();
        if let Some(val) = self.ids.get(idx) {
            args.add(val);
        }
    }
    // ...
}
```

### 4. 事务版本函数

除了 `query_list_typed`，还需要修改事务版本：
- `query_list_tx` → `query_list_typed_tx`
- `execute_tx` → `execute_typed_tx`
- 等等

### 5. 未知字段处理

当 SQL 中的参数名在结构体中不存在时，需要记录警告或报错：

```rust
_ => {
    tracing::warn!("TypedParams: 未知参数 '{}'", name);
}
```

## 文件修改清单

| 文件 | 修改内容 |
|------|---------|
| `markdown-sql/src/typed_params.rs` | 新增 TypedParams trait 定义 + EmptyParams 实现 |
| `markdown-sql/src/lib.rs` | 导出 TypedParams trait 和 derive macro |
| `markdown-sql-macros/src/lib.rs` | 新增 `#[derive(TypedParams)]` 宏 |
| `markdown-sql-macros/src/typed_params.rs` | derive macro 实现逻辑 |
| `markdown-sql/src/db/postgres.rs` | 新增 `query_list_typed` / `execute_typed` / `*_typed_tx` 等函数 |
| `markdown-sql/src/db/mysql.rs` | 同上 |
| `markdown-sql/src/db/sqlite.rs` | 同上 |
| `markdown-sql-macros/src/lib.rs` | 修改 `#[repository]` 宏调用 typed 版本 |

## 兼容性

- **向后兼容**：保留原有 `query_list` 函数（仅 Serialize）
- **新功能**：`query_list_typed` 要求同时实现 `Serialize + TypedParams`
- **宏生成**：默认使用 typed 版本，用户参数结构体需加 `#[derive(TypedParams)]`

## 测试用例

```rust
#[derive(Serialize, TypedParams)]
struct TestParams {
    name: String,
    created_at: NaiveDateTime,
    expires_at: Option<NaiveDateTime>,
}

// SQL: INSERT INTO test (name, created_at, expires_at) VALUES (#{name}, #{created_at}, #{expires_at})
// 预期：created_at 和 expires_at 正确绑定为 TIMESTAMP 类型（无需 SQL 类型转换）
```

## 开发步骤

1. **Phase 1: 核心 trait 和 derive macro**
   - 新增 `typed_params.rs`，定义三个 trait
   - 实现 `#[derive(TypedParams)]` 宏
   - 为 `EmptyParams` 实现 trait
   - 单元测试

2. **Phase 2: 底层函数**
   - `postgres.rs` 新增 `query_list_typed` / `execute_typed` 等
   - `mysql.rs` / `sqlite.rs` 同步
   - 新增事务版本 `*_typed_tx`

3. **Phase 3: 修改 repository 宏**
   - 宏生成代码调用 typed 版本
   - 保持向后兼容

4. **Phase 4: 集成测试**
   - 在 marmot-api 中实际使用 NaiveDateTime 字段测试

## 验收标准

- [ ] `#[derive(TypedParams)]` 宏能正确解析结构体字段
- [ ] `NaiveDateTime` 字段能正确绑定到 PostgreSQL TIMESTAMP 列
- [ ] `NaiveDate` 字段能正确绑定到 PostgreSQL DATE 列
- [ ] `Option<NaiveDateTime>` 为 None 时正确绑定为 NULL
- [ ] 普通 String / i32 / i64 / bool 字段正确绑定
- [ ] 嵌套字段 `user.name` 正确处理
- [ ] IN 查询 `bind_join` 参数正确处理
- [ ] EmptyParams 正常工作
- [ ] MySQL 和 SQLite 同样支持
- [ ] 事务版本函数正常工作
- [ ] 向后兼容：不使用 TypedParams 的代码仍能编译运行
