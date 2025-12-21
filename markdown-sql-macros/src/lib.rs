//! # markdown-sql-macros
//!
//! markdown-sql 的过程宏支持。
//!
//! ## #[repository] 宏
//!
//! 将 Repository trait 与 Markdown SQL 文件关联，自动生成实现。
//!
//! ```ignore
//! use markdown_sql_macros::repository;
//!
//! #[repository(sql_file = "sql/UserRepository.md")]
//! pub trait UserRepository {
//!     async fn find_by_id(&self, id: i64) -> Option<User>;
//!     async fn insert(&self, name: &str, age: i32) -> u64;
//! }
//! ```

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::path::PathBuf;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Ident, ItemTrait, LitStr, Token, TraitItem,
};

mod parser;
mod safety_checker;

use parser::parse_content;
use safety_checker::SafetyChecker;

/// 检查 SQL 文件安全性
///
/// 在编译时读取 SQL 文件，检测不安全的模式。
fn check_sql_file_safety(sql_file: &str) -> Result<(), String> {
    // 获取项目根目录（CARGO_MANIFEST_DIR）
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "无法获取 CARGO_MANIFEST_DIR".to_string())?;

    let file_path = PathBuf::from(&manifest_dir).join(sql_file);

    // 读取文件内容
    let content = std::fs::read_to_string(&file_path).map_err(|e| {
        format!(
            "无法读取 SQL 文件 '{}': {}\n  完整路径: {}",
            sql_file,
            e,
            file_path.display()
        )
    })?;

    // 解析 SQL 块
    let blocks = parse_content(&content);

    if blocks.is_empty() {
        return Err(format!(
            "SQL 文件 '{}' 中没有找到有效的 SQL 块\n  请确保 SQL 块使用 ```sql 和 -- sqlId 格式",
            sql_file
        ));
    }

    // 收集所有错误
    let mut errors = Vec::new();

    for (sql_id, block) in &blocks {
        if let Err(e) = SafetyChecker::check(sql_id, &block.content) {
            errors.push(format!(
                "\n[{}] SQL 注入风险!\n  位置: {} 第 {} 行\n  内容: {}\n  建议: {}",
                sql_file, e.sql_id, e.line, e.content, e.suggestion
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "SQL 安全检查失败，发现 {} 处不安全代码:{}",
            errors.len(),
            errors.join("")
        ))
    }
}

/// Repository 属性参数
struct RepositoryArgs {
    sql_file: String,
}

impl Parse for RepositoryArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut sql_file = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if ident == "sql_file" {
                let lit: LitStr = input.parse()?;
                sql_file = Some(lit.value());
            }

            // 跳过可能的逗号
            let _ = input.parse::<Token![,]>();
        }

        Ok(RepositoryArgs {
            sql_file: sql_file.ok_or_else(|| {
                syn::Error::new(input.span(), "缺少 sql_file 属性")
            })?,
        })
    }
}

/// 方法信息
#[allow(dead_code)]
struct MethodInfo {
    /// 方法名（Rust 风格，snake_case）
    name: String,
    /// SQL ID（camelCase）
    sql_id: String,
    /// 是否异步
    is_async: bool,
    /// 参数列表（名称, 类型）
    params: Vec<(String, String)>,
    /// 返回类型
    return_type: String,
    /// 是否返回列表
    is_list: bool,
    /// 是否返回 Option
    is_option: bool,
    /// 是否批量操作
    is_batch: bool,
}

/// 将 snake_case 转换为 camelCase
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else if i == 0 {
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }

    result
}

/// 解析返回类型
fn parse_return_type(ty: &syn::Type) -> (String, bool, bool) {
    let type_str = quote!(#ty).to_string();
    let is_list = type_str.contains("Vec <") || type_str.contains("Vec<");
    let is_option = type_str.contains("Option <") || type_str.contains("Option<");
    (type_str, is_list, is_option)
}

/// 检查是否是批量操作（参数包含 slice 或 Vec）
fn is_batch_operation(params: &[(String, String)]) -> bool {
    params.iter().any(|(_, ty)| {
        ty.contains("& [") || ty.contains("&[") || ty.contains("Vec <") || ty.contains("Vec<")
    })
}

/// 从 trait 解析方法信息
fn parse_methods(trait_item: &ItemTrait) -> Vec<MethodInfo> {
    let mut methods = Vec::new();

    for item in &trait_item.items {
        if let TraitItem::Fn(method) = item {
            let name = method.sig.ident.to_string();
            let sql_id = to_camel_case(&name);
            let is_async = method.sig.asyncness.is_some();

            // 解析参数（跳过 &self 和执行器参数）
            let params: Vec<(String, String)> = method
                .sig
                .inputs
                .iter()
                .filter_map(|arg| {
                    if let syn::FnArg::Typed(pat_type) = arg {
                        if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                            let param_name = pat_ident.ident.to_string();
                            // 跳过 self 和 exec 参数
                            if param_name != "self" && param_name != "exec" {
                                let param_type = quote!(#pat_type.ty).to_string();
                                return Some((param_name, param_type));
                            }
                        }
                    }
                    None
                })
                .collect();

            // 解析返回类型
            let (return_type, is_list, is_option) = if let syn::ReturnType::Type(_, ty) =
                &method.sig.output
            {
                parse_return_type(ty)
            } else {
                ("()".to_string(), false, false)
            };

            let is_batch = is_batch_operation(&params);

            methods.push(MethodInfo {
                name,
                sql_id,
                is_async,
                params,
                return_type,
                is_list,
                is_option,
                is_batch,
            });
        }
    }

    methods
}

/// Repository 属性宏
///
/// 将 trait 与 Markdown SQL 文件关联，生成实现结构体。
///
/// ## 属性
///
/// - `sql_file`: SQL 文件路径（相对于项目根目录）
///
/// ## 示例
///
/// ```ignore
/// #[repository(sql_file = "sql/UserRepository.md")]
/// pub trait UserRepository {
///     async fn find_by_id(&self, id: i64) -> Option<User>;
/// }
/// ```
///
/// ## 生成的代码
///
/// ```ignore
/// pub struct UserRepositoryImpl<'a> {
///     manager: &'a SqlManager,
/// }
///
/// impl<'a> UserRepositoryImpl<'a> {
///     pub fn new(manager: &'a SqlManager) -> Self {
///         Self { manager }
///     }
///     
///     pub async fn find_by_id(&self, id: i64) -> Option<User> {
///         // 自动生成的实现
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn repository(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as RepositoryArgs);
    let trait_item = parse_macro_input!(item as ItemTrait);

    let sql_file = &args.sql_file;
    let trait_name = &trait_item.ident;
    let impl_name = format_ident!("{}Impl", trait_name);
    let vis = &trait_item.vis;

    // 解析方法
    let methods = parse_methods(&trait_item);

    // ========== 编译时安全检查 ==========
    // 读取 SQL 文件并检查不安全的模式
    if let Err(e) = check_sql_file_safety(sql_file) {
        return syn::Error::new_spanned(&trait_item.ident, e)
            .to_compile_error()
            .into();
    }

    // 生成方法实现
    let method_impls: Vec<_> = methods
        .iter()
        .map(|m| {
            let method_name = format_ident!("{}", m.name);
            let sql_id = &m.sql_id;

            // 生成参数列表
            let param_names: Vec<_> = m.params.iter().map(|(n, _)| format_ident!("{}", n)).collect();

            // 生成 JSON 参数构建
            let json_fields: Vec<_> = m
                .params
                .iter()
                .map(|(n, _)| {
                    let name = format_ident!("{}", n);
                    let key = n.as_str();
                    quote! { #key: #name }
                })
                .collect();

            // 根据返回类型生成不同的实现
            if m.is_batch {
                // 批量操作
                quote! {
                    pub async fn #method_name<'e, E>(
                        &self,
                        exec: E,
                        #(#param_names: impl serde::Serialize + Clone),*
                    ) -> Result<u64, markdown_sql::MarkdownSqlError>
                    where
                        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
                    {
                        // 批量操作实现
                        let sql = self.manager.render(#sql_id, &serde_json::json!({}))?;
                        let sql_result = markdown_sql::ParamExtractor::extract(&sql, self.manager.db_type());
                        
                        // TODO: 实现批量执行
                        Ok(0)
                    }
                }
            } else if m.return_type.contains("u64") || m.return_type.contains("i64") && !m.is_option {
                // 返回影响行数（INSERT/UPDATE/DELETE）
                quote! {
                    pub async fn #method_name<'e, E>(
                        &self,
                        exec: E,
                        #(#param_names: impl serde::Serialize),*
                    ) -> Result<u64, markdown_sql::MarkdownSqlError>
                    where
                        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
                    {
                        let params = serde_json::json!({ #(#json_fields),* });
                        let sql = self.manager.render(#sql_id, &params)?;
                        let sql_result = markdown_sql::ParamExtractor::extract(&sql, self.manager.db_type());
                        
                        if self.manager.is_debug() {
                            tracing::debug!("Executing: {}\n  SQL: {}", #sql_id, sql_result.sql);
                        }
                        
                        let result = sqlx::query(&sql_result.sql)
                            .execute(exec)
                            .await
                            .map_err(markdown_sql::MarkdownSqlError::from)?;
                        
                        Ok(result.rows_affected())
                    }
                }
            } else {
                // 查询操作
                quote! {
                    pub async fn #method_name<'e, E, T>(
                        &self,
                        exec: E,
                        #(#param_names: impl serde::Serialize),*
                    ) -> Result<Vec<T>, markdown_sql::MarkdownSqlError>
                    where
                        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
                        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
                    {
                        let params = serde_json::json!({ #(#json_fields),* });
                        let sql = self.manager.render(#sql_id, &params)?;
                        let sql_result = markdown_sql::ParamExtractor::extract(&sql, self.manager.db_type());
                        
                        if self.manager.is_debug() {
                            tracing::debug!("Executing: {}\n  SQL: {}", #sql_id, sql_result.sql);
                        }
                        
                        let rows = sqlx::query_as::<_, T>(&sql_result.sql)
                            .fetch_all(exec)
                            .await
                            .map_err(markdown_sql::MarkdownSqlError::from)?;
                        
                        Ok(rows)
                    }
                }
            }
        })
        .collect();

    // 生成完整代码
    let expanded = quote! {
        // 保留原始 trait 定义
        #trait_item

        /// 自动生成的 Repository 实现
        #vis struct #impl_name<'a> {
            manager: &'a markdown_sql::SqlManager,
        }

        impl<'a> #impl_name<'a> {
            /// 创建新的 Repository 实例
            pub fn new(manager: &'a markdown_sql::SqlManager) -> Self {
                Self { manager }
            }

            /// 获取 SQL 文件路径
            pub fn sql_file() -> &'static str {
                #sql_file
            }

            #(#method_impls)*
        }
    };

    TokenStream::from(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("find_by_id"), "findById");
        assert_eq!(to_camel_case("find_all"), "findAll");
        assert_eq!(to_camel_case("insert"), "insert");
        assert_eq!(to_camel_case("batch_insert"), "batchInsert");
        assert_eq!(to_camel_case("find_user_by_name_and_age"), "findUserByNameAndAge");
    }
}
