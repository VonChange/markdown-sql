//! SQL 管理器
//!
//! 负责加载、注册和管理 SQL 模板。
//!
//! ## 核心功能
//!
//! - 从 Markdown 文件加载 SQL
//! - 使用命名空间管理 SQL ID（避免冲突）
//! - 注册到 MiniJinja 模板引擎
//! - 支持 `{% include %}` 引用其他 SQL 片段
//! - 渲染动态 SQL
//!
//! ## 使用示例
//!
//! ```ignore
//! use markdown_sql::SqlManager;
//!
//! // 创建管理器
//! let mut manager = SqlManager::new();
//!
//! // 加载 SQL 文件
//! manager.load_file("sql/UserRepository.md")?;
//!
//! // 渲染 SQL
//! let sql = manager.render("UserRepository.findById", &params)?;
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use minijinja::{Environment, Value};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;

use crate::error::{MarkdownSqlError, Result};
use crate::param_extractor::DbType;
use crate::parser::{MarkdownParser, SqlBlock};

/// include 命名空间正则
/// 匹配 `{% include "sqlId" %}` 中的 sqlId
static INCLUDE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\{%\s*include\s*"([^"]+)"\s*%\}"#).unwrap()
});

/// SQL 模板存储（使用 Arc 共享）
type TemplateStore = Arc<RwLock<HashMap<String, String>>>;

/// SQL 管理器
pub struct SqlManager {
    /// SQL 模板存储
    templates: TemplateStore,
    /// 原始 SQL 块（用于调试）
    sql_blocks: HashMap<String, SqlBlock>,
    /// 数据库类型
    db_type: DbType,
    /// 是否开启 Debug 模式
    debug: bool,
}

impl SqlManager {
    /// 创建新的 SQL 管理器
    pub fn new() -> Self {
        Self {
            templates: Arc::new(RwLock::new(HashMap::new())),
            sql_blocks: HashMap::new(),
            db_type: DbType::default(),
            debug: false,
        }
    }

    /// 创建构建器
    pub fn builder() -> SqlManagerBuilder {
        SqlManagerBuilder::new()
    }

    /// 创建 MiniJinja 环境
    fn create_env(&self) -> Environment<'static> {
        let mut env = Environment::new();
        
        // 注册自定义过滤器
        Self::register_filters(&mut env);
        
        // 设置模板加载器
        let templates = self.templates.clone();
        env.set_loader(move |name| {
            let store = templates.read().unwrap();
            Ok(store.get(name).cloned())
        });
        
        env
    }

    /// 注册自定义过滤器
    fn register_filters(env: &mut Environment<'static>) {
        // bind_join 过滤器：安全的 IN 查询展开
        // 用法：{{ ids | bind_join(",") }}
        // 效果：生成 #{__bind_0},#{__bind_1},#{__bind_2}
        env.add_filter("bind_join", |value: Value, separator: String| -> String {
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
        });

        // raw_safe 过滤器：显式声明安全的字符串拼接
        // 用法：{{ table_name | raw_safe }}
        // 注意：仅用于已验证安全的值（如枚举、预定义列表）
        env.add_filter("raw_safe", |value: Value| -> String {
            value.to_string()
        });
    }

    /// 设置数据库类型
    pub fn set_db_type(&mut self, db_type: DbType) {
        self.db_type = db_type;
    }

    /// 获取数据库类型
    pub fn db_type(&self) -> DbType {
        self.db_type
    }

    /// 设置 Debug 模式
    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }

    /// 是否开启 Debug 模式
    pub fn is_debug(&self) -> bool {
        self.debug
    }

    /// 从文件加载 SQL
    ///
    /// # 参数
    /// - `path`: Markdown 文件路径
    ///
    /// # 返回
    /// - 加载的 SQL ID 数量
    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> Result<usize> {
        let path = path.as_ref();
        let namespace = MarkdownParser::extract_namespace(path);
        let sql_blocks = MarkdownParser::parse_file(path)?;

        let count = sql_blocks.len();
        self.register_blocks(sql_blocks, &namespace)?;

        if self.debug {
            tracing::debug!("加载 SQL 文件: {} ({} 个 SQL)", path.display(), count);
        }

        Ok(count)
    }

    /// 从内容加载 SQL
    ///
    /// # 参数
    /// - `content`: Markdown 内容
    /// - `namespace`: 命名空间
    pub fn load_content(&mut self, content: &str, namespace: &str) -> Result<usize> {
        let sql_blocks = MarkdownParser::parse_content(content)?;
        let count = sql_blocks.len();
        self.register_blocks(sql_blocks, namespace)?;
        Ok(count)
    }

    /// 从嵌入的目录加载所有 SQL 文件
    ///
    /// 需要启用 `embed` feature。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use include_dir::{include_dir, Dir};
    /// use markdown_sql::SqlManager;
    ///
    /// // 编译时嵌入 sql 目录
    /// static SQL_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/sql");
    ///
    /// let mut manager = SqlManager::new();
    /// manager.load_embedded_dir(&SQL_DIR)?;
    /// ```
    #[cfg(feature = "embed")]
    pub fn load_embedded_dir(&mut self, dir: &include_dir::Dir) -> Result<usize> {
        let mut total = 0;

        for entry in dir.files() {
            let path = entry.path();
            
            // 只处理 .md 文件
            if path.extension().map_or(false, |ext| ext == "md") {
                let content = entry.contents_utf8().ok_or_else(|| {
                    MarkdownSqlError::InvalidPath(format!(
                        "文件 {} 不是有效的 UTF-8",
                        path.display()
                    ))
                })?;

                let namespace = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("default");

                let count = self.load_content(content, namespace)?;
                total += count;

                if self.debug {
                    tracing::debug!(
                        "加载嵌入 SQL 文件: {} ({} 个 SQL)",
                        path.display(),
                        count
                    );
                }
            }
        }

        // 递归处理子目录
        for subdir in dir.dirs() {
            total += self.load_embedded_dir(subdir)?;
        }

        Ok(total)
    }

    /// 注册 SQL 块到模板存储
    fn register_blocks(
        &mut self,
        blocks: HashMap<String, SqlBlock>,
        namespace: &str,
    ) -> Result<()> {
        let mut store = self.templates.write().unwrap();
        
        for (id, block) in blocks {
            // 使用命名空间：Namespace.sqlId
            let full_id = format!("{}.{}", namespace, id);

            // 展开 include 命名空间
            // 将 {% include "columns" %} 转换为 {% include "Namespace.columns" %}
            let content = self.expand_include_namespace(&block.content, namespace);

            // 注册到模板存储
            store.insert(full_id.clone(), content.clone());

            // 同时注册短名称（用于同文件引用）
            // 注意：后注册的会覆盖先注册的
            store.insert(id.clone(), content);

            // 保存原始块（用于调试）
            self.sql_blocks.insert(full_id, block);
        }

        Ok(())
    }

    /// 展开 include 的命名空间
    ///
    /// 将 `{% include "sqlId" %}` 转换为 `{% include "Namespace.sqlId" %}`
    /// 但保留已有命名空间的引用不变
    fn expand_include_namespace(&self, content: &str, namespace: &str) -> String {
        INCLUDE_RE
            .replace_all(content, |caps: &regex::Captures| {
                let ref_id = &caps[1];
                // 如果已经有命名空间（包含 .），则保持不变
                if ref_id.contains('.') {
                    return caps[0].to_string();
                }
                // 添加当前命名空间
                format!("{{% include \"{}.{}\" %}}", namespace, ref_id)
            })
            .to_string()
    }

    /// 渲染 SQL 模板
    ///
    /// # 参数
    /// - `sql_id`: SQL ID（可以是短名称或完整名称）
    /// - `params`: 模板参数
    ///
    /// # 返回
    /// - 渲染后的 SQL（仍包含 #{param} 占位符）
    pub fn render<T: Serialize>(&self, sql_id: &str, params: &T) -> Result<String> {
        // 创建环境（带加载器）
        let env = self.create_env();
        
        let template = env.get_template(sql_id).map_err(|_| {
            MarkdownSqlError::SqlNotFound(sql_id.to_string())
        })?;

        let context = Value::from_serialize(params);
        let rendered = template
            .render(&context)
            .map_err(|e| MarkdownSqlError::RenderError(e.to_string()))?;

        // 清理多余空白行
        let cleaned = Self::clean_sql(&rendered);

        if self.debug {
            tracing::debug!("渲染 SQL: {}\n{}", sql_id, cleaned);
        }

        Ok(cleaned)
    }

    /// 清理 SQL 中的多余空白
    fn clean_sql(sql: &str) -> String {
        sql.lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<&str>>()
            .join("\n")
    }

    /// 获取原始 SQL 块（用于调试）
    pub fn get_block(&self, sql_id: &str) -> Option<&SqlBlock> {
        self.sql_blocks.get(sql_id)
    }

    /// 获取所有已注册的 SQL ID
    pub fn sql_ids(&self) -> Vec<String> {
        self.sql_blocks.keys().cloned().collect()
    }

    /// 检查 SQL ID 是否存在
    pub fn contains(&self, sql_id: &str) -> bool {
        let store = self.templates.read().unwrap();
        store.contains_key(sql_id)
    }
}

impl Default for SqlManager {
    fn default() -> Self {
        Self::new()
    }
}

/// SQL 管理器构建器
pub struct SqlManagerBuilder {
    db_type: DbType,
    debug: bool,
    files: Vec<String>,
}

impl SqlManagerBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            db_type: DbType::default(),
            debug: false,
            files: Vec::new(),
        }
    }

    /// 设置数据库类型
    pub fn db_type(mut self, db_type: DbType) -> Self {
        self.db_type = db_type;
        self
    }

    /// 设置 Debug 模式
    pub fn debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// 添加 SQL 文件
    pub fn load_file<S: Into<String>>(mut self, path: S) -> Self {
        self.files.push(path.into());
        self
    }

    /// 构建 SQL 管理器
    pub fn build(self) -> Result<SqlManager> {
        let mut manager = SqlManager::new();
        manager.set_db_type(self.db_type);
        manager.set_debug(self.debug);

        for file in self.files {
            manager.load_file(&file)?;
        }

        Ok(manager)
    }
}

impl Default for SqlManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局 SQL 管理器
static GLOBAL_MANAGER: Lazy<RwLock<SqlManager>> = Lazy::new(|| {
    RwLock::new(SqlManager::new())
});

/// 初始化全局 SQL 管理器
pub fn init<P: AsRef<Path>>(path: P) -> Result<usize> {
    let mut manager = GLOBAL_MANAGER.write().unwrap();
    manager.load_file(path)
}

/// 设置全局数据库类型
pub fn set_db_type(db_type: DbType) {
    let mut manager = GLOBAL_MANAGER.write().unwrap();
    manager.set_db_type(db_type);
}

/// 设置全局 Debug 模式
pub fn set_debug(debug: bool) {
    let mut manager = GLOBAL_MANAGER.write().unwrap();
    manager.set_debug(debug);
}

/// 渲染 SQL（使用全局管理器）
pub fn render<T: Serialize>(sql_id: &str, params: &T) -> Result<String> {
    let manager = GLOBAL_MANAGER.read().unwrap();
    manager.render(sql_id, params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_load_content() {
        let mut manager = SqlManager::new();

        let content = r#"
```sql
-- columns
id, name, age
```

```sql
-- findById
SELECT {% include "columns" %}
FROM user
WHERE id = #{id}
```
"#;

        let count = manager.load_content(content, "User").unwrap();
        assert_eq!(count, 2);

        // 使用完整名称
        assert!(manager.contains("User.columns"));
        assert!(manager.contains("User.findById"));

        // 使用短名称
        assert!(manager.contains("columns"));
        assert!(manager.contains("findById"));
    }

    #[test]
    fn test_render_simple() {
        let mut manager = SqlManager::new();

        let content = r#"
```sql
-- findById
SELECT * FROM user WHERE id = #{id}
```
"#;

        manager.load_content(content, "User").unwrap();

        let sql = manager.render("findById", &json!({"id": 1})).unwrap();
        assert!(sql.contains("SELECT * FROM user"));
        assert!(sql.contains("#{id}"));
    }

    #[test]
    fn test_render_with_include() {
        let mut manager = SqlManager::new();

        let content = r#"
```sql
-- columns
id, name, age
```

```sql
-- findAll
SELECT {% include "columns" %}
FROM user
```
"#;

        manager.load_content(content, "User").unwrap();

        let sql = manager.render("findAll", &json!({})).unwrap();
        assert!(sql.contains("id, name, age"));
    }

    #[test]
    fn test_render_dynamic_sql() {
        let mut manager = SqlManager::new();

        let content = r#"
```sql
-- findByCondition
SELECT * FROM user
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
```
"#;

        manager.load_content(content, "User").unwrap();

        // 带条件
        let sql = manager
            .render("findByCondition", &json!({"name": "%test%", "status": 1}))
            .unwrap();
        assert!(sql.contains("AND name LIKE #{name}"));
        assert!(sql.contains("AND status = #{status}"));

        // 无条件
        let sql = manager
            .render("findByCondition", &json!({}))
            .unwrap();
        assert!(!sql.contains("AND name"));
        assert!(!sql.contains("AND status"));
    }

    #[test]
    fn test_bind_join_filter() {
        let mut manager = SqlManager::new();

        let content = r#"
```sql
-- findByIds
SELECT * FROM user
WHERE id IN ({{ ids | bind_join(",") }})
```
"#;

        manager.load_content(content, "User").unwrap();

        let sql = manager
            .render("findByIds", &json!({"ids": [1, 2, 3]}))
            .unwrap();
        // 应该生成 #{__bind_0},#{__bind_1},#{__bind_2}
        assert!(sql.contains("#{__bind_0}"));
        assert!(sql.contains("#{__bind_1}"));
        assert!(sql.contains("#{__bind_2}"));
    }

    #[test]
    fn test_namespace_isolation() {
        let mut manager = SqlManager::new();

        // 加载两个文件，都有 columns
        let user_content = r#"
```sql
-- columns
id, user_name, age
```
"#;

        let order_content = r#"
```sql
-- columns
id, order_no, amount
```
"#;

        manager.load_content(user_content, "User").unwrap();
        manager.load_content(order_content, "Order").unwrap();

        // 使用完整名称可以区分
        let user_sql = manager.render("User.columns", &json!({})).unwrap();
        assert!(user_sql.contains("user_name"));

        let order_sql = manager.render("Order.columns", &json!({})).unwrap();
        assert!(order_sql.contains("order_no"));
    }

    #[test]
    fn test_cross_file_include() {
        let mut manager = SqlManager::new();

        let user_content = r#"
```sql
-- columns
id, name
```
"#;

        let order_content = r#"
```sql
-- findWithUser
SELECT o.*, u.{% include "User.columns" %}
FROM orders o
JOIN user u ON o.user_id = u.id
```
"#;

        manager.load_content(user_content, "User").unwrap();
        manager.load_content(order_content, "Order").unwrap();

        let sql = manager.render("Order.findWithUser", &json!({})).unwrap();
        assert!(sql.contains("id, name"));
    }

    #[test]
    fn test_builder() {
        let manager = SqlManager::builder()
            .db_type(DbType::Postgres)
            .debug(true)
            .build()
            .unwrap();

        assert_eq!(manager.db_type(), DbType::Postgres);
        assert!(manager.is_debug());
    }
}
