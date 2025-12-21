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
//!     async fn find_by_id(&self, id: i64) -> Result<Option<User>>;
//!     async fn find_all(&self) -> Result<Vec<User>>;
//!     async fn get_count(&self) -> Result<i64>;
//!     async fn insert(&self, user: &UserInput) -> Result<u64>;
//! }
//! ```

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::path::PathBuf;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, GenericArgument, Ident, ItemTrait, LitStr, PathArguments, ReturnType, Token,
    TraitItem, Type,
};

mod parser;
mod safety_checker;

use parser::parse_content;
use safety_checker::SafetyChecker;

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

            let _ = input.parse::<Token![,]>();
        }

        Ok(RepositoryArgs {
            sql_file: sql_file
                .ok_or_else(|| syn::Error::new(input.span(), "缺少 sql_file 属性"))?,
        })
    }
}

/// 返回类型信息
#[derive(Debug, Clone)]
enum ReturnKind {
    /// Vec<T> - 列表查询
    List(Type),
    /// Option<T> - 可选单条
    Optional(Type),
    /// T（非 Vec/Option）- 必须单条
    One(Type),
    /// i64 - 标量查询（如 COUNT）
    Scalar,
    /// u64 - 影响行数（INSERT/UPDATE/DELETE）
    Affected,
    /// () - 无返回
    Unit,
}

/// 方法信息
struct MethodInfo {
    /// 方法名（snake_case）
    name: Ident,
    /// SQL ID（camelCase）
    sql_id: String,
    /// 是否异步
    is_async: bool,
    /// 参数列表（名称, 类型, 是否引用）
    params: Vec<(Ident, Type, bool)>,
    /// 返回类型
    return_kind: ReturnKind,
    /// 完整返回类型（用于签名）
    return_type: Option<Type>,
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

/// 检查 SQL 文件安全性
fn check_sql_file_safety(sql_file: &str) -> Result<(), String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "无法获取 CARGO_MANIFEST_DIR".to_string())?;

    let file_path = PathBuf::from(&manifest_dir).join(sql_file);

    let content = std::fs::read_to_string(&file_path).map_err(|e| {
        format!(
            "无法读取 SQL 文件 '{}': {}\n  完整路径: {}",
            sql_file,
            e,
            file_path.display()
        )
    })?;

    let blocks = parse_content(&content);

    if blocks.is_empty() {
        return Err(format!(
            "SQL 文件 '{}' 中没有找到有效的 SQL 块",
            sql_file
        ));
    }

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

/// 解析返回类型
fn parse_return_kind(ty: &Type) -> ReturnKind {
    let _type_str = quote!(#ty).to_string().replace(' ', "");

    // 检查 Result<T, E> 包装
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Result" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return parse_inner_return_kind(inner_ty);
                    }
                }
            }
        }
    }

    parse_inner_return_kind(ty)
}

/// 解析内部返回类型（去掉 Result 包装后）
fn parse_inner_return_kind(ty: &Type) -> ReturnKind {
    let type_str = quote!(#ty).to_string().replace(' ', "");

    // Vec<T>
    if type_str.starts_with("Vec<") {
        if let Type::Path(type_path) = ty {
            if let Some(segment) = type_path.path.segments.last() {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return ReturnKind::List(inner_ty.clone());
                    }
                }
            }
        }
    }

    // Option<T>
    if type_str.starts_with("Option<") {
        if let Type::Path(type_path) = ty {
            if let Some(segment) = type_path.path.segments.last() {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return ReturnKind::Optional(inner_ty.clone());
                    }
                }
            }
        }
    }

    // i64 - 标量
    if type_str == "i64" {
        return ReturnKind::Scalar;
    }

    // u64 - 影响行数
    if type_str == "u64" {
        return ReturnKind::Affected;
    }

    // () - 无返回
    if type_str == "()" {
        return ReturnKind::Unit;
    }

    // 其他类型 - 单条查询
    ReturnKind::One(ty.clone())
}

/// 解析参数（跳过 &self 和 db 参数）
fn parse_params(method: &syn::TraitItemFn) -> Vec<(Ident, Type, bool)> {
    method
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let param_name = pat_ident.ident.to_string();

                    // 跳过 self 和 db 参数
                    if param_name == "self" || param_name == "db" || param_name == "pool" {
                        return None;
                    }

                    let is_ref = matches!(&*pat_type.ty, Type::Reference(_));
                    return Some((pat_ident.ident.clone(), (*pat_type.ty).clone(), is_ref));
                }
            }
            None
        })
        .collect()
}

/// 从 trait 解析方法信息
fn parse_methods(trait_item: &ItemTrait) -> Vec<MethodInfo> {
    let mut methods = Vec::new();

    for item in &trait_item.items {
        if let TraitItem::Fn(method) = item {
            let name = method.sig.ident.clone();
            let sql_id = to_camel_case(&name.to_string());
            let is_async = method.sig.asyncness.is_some();

            let params = parse_params(method);

            let (return_kind, return_type) = match &method.sig.output {
                ReturnType::Type(_, ty) => (parse_return_kind(ty), Some((**ty).clone())),
                ReturnType::Default => (ReturnKind::Unit, None),
            };

            methods.push(MethodInfo {
                name,
                sql_id,
                is_async,
                params,
                return_kind,
                return_type,
            });
        }
    }

    methods
}

/// 生成方法实现
fn generate_method_impl(method: &MethodInfo) -> proc_macro2::TokenStream {
    let method_name = &method.name;
    let sql_id = &method.sql_id;

    // 生成参数列表
    let param_defs: Vec<_> = method
        .params
        .iter()
        .map(|(name, ty, _)| {
            quote! { #name: #ty }
        })
        .collect();

    // 生成参数传递（用于构造参数结构体或直接传递）
    let param_names: Vec<_> = method.params.iter().map(|(name, _, _)| name).collect();

    // 生成返回类型
    let return_type = method.return_type.as_ref().map(|ty| quote! { -> #ty });

    // 根据返回类型生成不同的实现
    let body = match &method.return_kind {
        ReturnKind::List(inner_ty) => {
            if param_names.is_empty() {
                quote! {
                    markdown_sql::query_list::<#inner_ty, _, _>(
                        &self.manager,
                        db,
                        #sql_id,
                        &markdown_sql::EmptyParams,
                    ).await.map_err(|e| e.into())
                }
            } else if param_names.len() == 1 {
                let param = &param_names[0];
                quote! {
                    markdown_sql::query_list::<#inner_ty, _, _>(
                        &self.manager,
                        db,
                        #sql_id,
                        #param,
                    ).await.map_err(|e| e.into())
                }
            } else {
                // 多个参数，使用 serde_json::json! 构造
                let json_fields: Vec<_> = param_names
                    .iter()
                    .map(|name| {
                        let key = name.to_string();
                        quote! { #key: #name }
                    })
                    .collect();
                quote! {
                    markdown_sql::query_list::<#inner_ty, _, _>(
                        &self.manager,
                        db,
                        #sql_id,
                        &serde_json::json!({ #(#json_fields),* }),
                    ).await.map_err(|e| e.into())
                }
            }
        }
        ReturnKind::Optional(inner_ty) => {
            if param_names.is_empty() {
                quote! {
                    markdown_sql::query_optional::<#inner_ty, _, _>(
                        &self.manager,
                        db,
                        #sql_id,
                        &markdown_sql::EmptyParams,
                    ).await.map_err(|e| e.into())
                }
            } else if param_names.len() == 1 {
                let param = &param_names[0];
                quote! {
                    markdown_sql::query_optional::<#inner_ty, _, _>(
                        &self.manager,
                        db,
                        #sql_id,
                        #param,
                    ).await.map_err(|e| e.into())
                }
            } else {
                let json_fields: Vec<_> = param_names
                    .iter()
                    .map(|name| {
                        let key = name.to_string();
                        quote! { #key: #name }
                    })
                    .collect();
                quote! {
                    markdown_sql::query_optional::<#inner_ty, _, _>(
                        &self.manager,
                        db,
                        #sql_id,
                        &serde_json::json!({ #(#json_fields),* }),
                    ).await.map_err(|e| e.into())
                }
            }
        }
        ReturnKind::One(inner_ty) => {
            if param_names.is_empty() {
                quote! {
                    markdown_sql::query_one::<#inner_ty, _, _>(
                        &self.manager,
                        db,
                        #sql_id,
                        &markdown_sql::EmptyParams,
                    ).await.map_err(|e| e.into())
                }
            } else if param_names.len() == 1 {
                let param = &param_names[0];
                quote! {
                    markdown_sql::query_one::<#inner_ty, _, _>(
                        &self.manager,
                        db,
                        #sql_id,
                        #param,
                    ).await.map_err(|e| e.into())
                }
            } else {
                let json_fields: Vec<_> = param_names
                    .iter()
                    .map(|name| {
                        let key = name.to_string();
                        quote! { #key: #name }
                    })
                    .collect();
                quote! {
                    markdown_sql::query_one::<#inner_ty, _, _>(
                        &self.manager,
                        db,
                        #sql_id,
                        &serde_json::json!({ #(#json_fields),* }),
                    ).await.map_err(|e| e.into())
                }
            }
        }
        ReturnKind::Scalar => {
            if param_names.is_empty() {
                quote! {
                    markdown_sql::query_scalar(
                        &self.manager,
                        db,
                        #sql_id,
                        &markdown_sql::EmptyParams,
                    ).await.map_err(|e| e.into())
                }
            } else if param_names.len() == 1 {
                let param = &param_names[0];
                quote! {
                    markdown_sql::query_scalar(
                        &self.manager,
                        db,
                        #sql_id,
                        #param,
                    ).await.map_err(|e| e.into())
                }
            } else {
                let json_fields: Vec<_> = param_names
                    .iter()
                    .map(|name| {
                        let key = name.to_string();
                        quote! { #key: #name }
                    })
                    .collect();
                quote! {
                    markdown_sql::query_scalar(
                        &self.manager,
                        db,
                        #sql_id,
                        &serde_json::json!({ #(#json_fields),* }),
                    ).await.map_err(|e| e.into())
                }
            }
        }
        ReturnKind::Affected => {
            if param_names.is_empty() {
                quote! {
                    markdown_sql::execute(
                        &self.manager,
                        db,
                        #sql_id,
                        &markdown_sql::EmptyParams,
                    ).await.map_err(|e| e.into())
                }
            } else if param_names.len() == 1 {
                let param = &param_names[0];
                quote! {
                    markdown_sql::execute(
                        &self.manager,
                        db,
                        #sql_id,
                        #param,
                    ).await.map_err(|e| e.into())
                }
            } else {
                let json_fields: Vec<_> = param_names
                    .iter()
                    .map(|name| {
                        let key = name.to_string();
                        quote! { #key: #name }
                    })
                    .collect();
                quote! {
                    markdown_sql::execute(
                        &self.manager,
                        db,
                        #sql_id,
                        &serde_json::json!({ #(#json_fields),* }),
                    ).await.map_err(|e| e.into())
                }
            }
        }
        ReturnKind::Unit => {
            quote! {
                Ok(())
            }
        }
    };

    // 生成异步标记
    let async_token = if method.is_async {
        quote! { async }
    } else {
        quote! {}
    };

    quote! {
        pub #async_token fn #method_name<D: markdown_sql::DbPool>(
            &self,
            db: &D,
            #(#param_defs),*
        ) #return_type {
            #body
        }
    }
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
///     // 方法名自动映射到 SQL ID（snake_case → camelCase）
///     // find_by_id → findById
///     // db 参数接受任何实现 DbPool 的类型
///     async fn find_by_id(&self, db: &impl DbPool, params: &IdParams) -> Result<Option<User>>;
///
///     // 无参数查询
///     async fn get_count(&self, db: &impl DbPool) -> Result<i64>;
///
///     // 返回列表
///     async fn find_all(&self, db: &impl DbPool) -> Result<Vec<User>>;
///
///     // 返回影响行数
///     async fn insert(&self, db: &Pool<Sqlite>, user: &User) -> Result<u64>;
/// }
/// ```
///
/// ## 生成的代码
///
/// ```ignore
/// pub struct UserRepositoryImpl {
///     manager: markdown_sql::SqlManager,
/// }
///
/// impl UserRepositoryImpl {
///     pub fn new(manager: markdown_sql::SqlManager) -> Self {
///         Self { manager }
///     }
///
///     pub async fn find_by_id(&self, db: &Pool<Sqlite>, params: &IdParams) -> Result<Option<User>> {
///         markdown_sql::query_optional::<User, _>(&self.manager, db, "findById", params)
///             .await
///             .map_err(|e| e.into())
///     }
///     // ...
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
    if let Err(e) = check_sql_file_safety(sql_file) {
        return syn::Error::new_spanned(&trait_item.ident, e)
            .to_compile_error()
            .into();
    }

    // 生成方法实现
    let method_impls: Vec<_> = methods.iter().map(generate_method_impl).collect();

    // 生成完整代码
    let expanded = quote! {
        // 保留原始 trait 定义
        #trait_item

        /// 自动生成的 Repository 实现
        #vis struct #impl_name {
            manager: &'static markdown_sql::SqlManager,
        }

        impl #impl_name {
            /// 创建新的 Repository 实例
            ///
            /// 接受静态引用（通常来自 `Lazy<SqlManager>`）
            pub fn new(manager: &'static markdown_sql::SqlManager) -> Self {
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
        assert_eq!(to_camel_case("get_daily_stats"), "getDailyStats");
        assert_eq!(to_camel_case("get_total_count"), "getTotalCount");
    }
}
