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
markdown-sql = { git = "https://github.com/VonChange/markdown-sql.git", branch = "main" }
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

## IN 查询

​```sql
-- findByIds
SELECT {% include "columns" %}
FROM user
WHERE id IN ({{ ids | bind_join(",") }})
​```

## 插入用户

​```sql
-- insert
INSERT INTO user (name, age, status)
VALUES (#{name}, #{age}, #{status})
​```
```

### 2. 使用 SqlManager

```rust
use markdown_sql::{DbType, ParamExtractor, SqlManager};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建 SQL 管理器
    let mut manager = SqlManager::builder()
        .db_type(DbType::Postgres)
        .debug(true)
        .build()?;

    // 2. 加载 SQL 文件
    manager.load_file("sql/UserRepository.md")?;

    // 3. 渲染 SQL
    let sql = manager.render("findById", &json!({"id": 1}))?;
    // 输出: SELECT id, name, age, status, create_time FROM user WHERE id = #{id}

    // 4. 提取参数
    let result = ParamExtractor::extract(&sql, DbType::Postgres);
    // result.sql: "SELECT ... WHERE id = $1"
    // result.params: ["id"]

    // 5. 动态 SQL
    let sql = manager.render("findByCondition", &json!({
        "name": "%张%",
        "status": 1
    }))?;
    // 输出: SELECT ... WHERE 1=1 AND name LIKE #{name} AND status = #{status}

    // 6. IN 查询
    let sql = manager.render("findByIds", &json!({"ids": [1, 2, 3]}))?;
    // 输出: SELECT ... WHERE id IN (#{__bind_0},#{__bind_1},#{__bind_2})

    Ok(())
}
```

## 📝 SQL 语法

### 参数绑定

```sql
-- 使用 #{param} 语法，自动转换为 $1 (Postgres) 或 ? (MySQL/SQLite)
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

## 🗄️ 多数据库支持

```rust
// PostgreSQL: #{id} → $1
let result = ParamExtractor::extract(&sql, DbType::Postgres);

// MySQL: #{id} → ?
let result = ParamExtractor::extract(&sql, DbType::Mysql);

// SQLite: #{id} → ?
let result = ParamExtractor::extract(&sql, DbType::Sqlite);
```

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
   ```markdown
   ## 按部门查询活跃用户
   ​```sql
   -- findActiveUsersByDept
   SELECT * FROM user WHERE status = 1 AND dept_id = #{deptId}
   ​```
   ```

2. **自文档化**：AI 可以从 Markdown 结构理解每个 SQL 的作用

3. **易于生成**：AI 可以按照已有模式生成新的 SQL 块

4. **默认安全**：`#{param}` 语法防止 AI 意外生成 SQL 注入漏洞

5. **可复用片段**：`{% include %}` 帮助 AI 理解和复用通用模式

### Vibe Coding 工作流

```
用户: "添加一个按邮箱查询用户的方法"

AI: 在 UserRepository.md 中生成:

## 按邮箱查询用户

​```sql
-- findByEmail
SELECT {% include "columns" %}
FROM user
WHERE email = #{email}
​```
```

### 设计理念

- **让 AI 更好理解**：SQL 不再是散落的字符串，而是有结构、有注释的文档
- **减少 AI 出错**：强制参数绑定，编译时安全检查
- **提高 AI 效率**：清晰的模式让 AI 能快速学习并生成正确的代码

## 📖 文档

详细设计文档请查看 [plan/2025-12-21-markdown-sql.md](plan/2025-12-21-markdown-sql.md)

## 📜 License

MIT
