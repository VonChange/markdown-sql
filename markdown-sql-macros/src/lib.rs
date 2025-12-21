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

/// 数据库类型
#[derive(Debug, Clone, Copy, Default)]
enum DbTypeArg {
    #[default]
    Sqlite,
    Mysql,
    Postgres,
}

impl DbTypeArg {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sqlite" => Some(DbTypeArg::Sqlite),
            "mysql" => Some(DbTypeArg::Mysql),
            "postgres" | "postgresql" | "pg" => Some(DbTypeArg::Postgres),
            _ => None,
        }
    }

    /// 返回内部模块路径
    fn internal_module(&self) -> proc_macro2::TokenStream {
        match self {
            DbTypeArg::Sqlite => quote! { markdown_sql::__internal::sqlite },
            DbTypeArg::Mysql => quote! { markdown_sql::__internal::mysql },
            DbTypeArg::Postgres => quote! { markdown_sql::__internal::postgres },
        }
    }

    /// 返回 DbPool trait
    fn db_pool_trait(&self) -> proc_macro2::TokenStream {
        match self {
            DbTypeArg::Sqlite => quote! { markdown_sql::SqliteDbPool },
            DbTypeArg::Mysql => quote! { markdown_sql::MySqlDbPool },
            DbTypeArg::Postgres => quote! { markdown_sql::PgDbPool },
        }
    }
}

/// Repository 属性参数
struct RepositoryArgs {
    sql_file: String,
    db_type: DbTypeArg,
}

impl Parse for RepositoryArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut sql_file = None;
        let mut db_type = DbTypeArg::default();

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if ident == "sql_file" {
                let lit: LitStr = input.parse()?;
                sql_file = Some(lit.value());
            } else if ident == "db_type" {
                let lit: LitStr = input.parse()?;
                db_type = DbTypeArg::from_str(&lit.value()).ok_or_else(|| {
                    syn::Error::new(
                        lit.span(),
                        "无效的 db_type，支持: sqlite, mysql, postgres",
                    )
                })?;
            }

            let _ = input.parse::<Token![,]>();
        }

        Ok(RepositoryArgs {
            sql_file: sql_file
                .ok_or_else(|| syn::Error::new(input.span(), "缺少 sql_file 属性"))?,
            db_type,
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
    /// 是否使用事务（标记了 #[transactional]）
    is_transactional: bool,
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

/// 将 snake_case 转换为 PascalCase（用于结构体名）
fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true; // 首字母大写

    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
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

            // 检查是否有 #[transactional] 属性
            let is_transactional = method.attrs.iter().any(|attr| {
                attr.path().is_ident("transactional")
            });

            methods.push(MethodInfo {
                name,
                sql_id,
                is_async,
                params,
                return_kind,
                return_type,
                is_transactional,
            });
        }
    }

    methods
}

/// 生成参数表达式
///
/// - 0 个参数：使用 EmptyParams
/// - 1 个参数：直接传递
/// - 多个参数：生成内部结构体（避免使用 json!）
fn build_params_code(
    method: &MethodInfo,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let param_names: Vec<_> = method.params.iter().map(|(name, _, _)| name).collect();

    if param_names.is_empty() {
        // 无参数
        (quote! {}, quote! { &markdown_sql::EmptyParams })
    } else if param_names.len() == 1 {
        // 单参数：直接传递
        let param = &param_names[0];
        (quote! {}, quote! { #param })
    } else {
        // 多参数：生成内部结构体
        // 结构体名：__方法名Params（避免命名冲突）
        let struct_name = format_ident!("__{}Params", to_pascal_case(&method.name.to_string()));

        // 生成字段定义（保留原始类型，支持引用）
        let field_defs: Vec<_> = method
            .params
            .iter()
            .map(|(name, ty, _)| {
                quote! { #name: #ty }
            })
            .collect();

        // 生成字段赋值
        let field_assigns: Vec<_> = param_names
            .iter()
            .map(|name| {
                quote! { #name }
            })
            .collect();

        // 结构体定义（局部作用域内）
        let struct_def = quote! {
            #[derive(serde::Serialize)]
            struct #struct_name<'__p> {
                #[serde(skip)]
                __marker: std::marker::PhantomData<&'__p ()>,
                #(#field_defs,)*
            }
            let __params = #struct_name {
                __marker: std::marker::PhantomData,
                #(#field_assigns,)*
            };
        };

        (struct_def, quote! { &__params })
    }
}

/// 生成方法实现（简化版，使用 build_params_code）
fn generate_method_impl(method: &MethodInfo, db_type: DbTypeArg) -> proc_macro2::TokenStream {
    let method_name = &method.name;
    let sql_id = &method.sql_id;
    let internal_mod = db_type.internal_module();
    let db_pool_trait = db_type.db_pool_trait();

    // 生成参数列表
    let param_defs: Vec<_> = method
        .params
        .iter()
        .map(|(name, ty, _)| {
            quote! { #name: #ty }
        })
        .collect();

    // 生成返回类型
    let return_type = method.return_type.as_ref().map(|ty| quote! { -> #ty });

    // 构建参数代码
    let (params_struct, params_expr) = build_params_code(method);

    // 根据返回类型生成调用代码
    let call_expr = match &method.return_kind {
        ReturnKind::List(inner_ty) => {
            quote! {
                #internal_mod::query_list::<#inner_ty, _, _>(
                    &self.manager,
                    db,
                    #sql_id,
                    #params_expr,
                ).await.map_err(|e| e.into())
            }
        }
        ReturnKind::Optional(inner_ty) => {
            quote! {
                #internal_mod::query_optional::<#inner_ty, _, _>(
                    &self.manager,
                    db,
                    #sql_id,
                    #params_expr,
                ).await.map_err(|e| e.into())
            }
        }
        ReturnKind::One(inner_ty) => {
            quote! {
                #internal_mod::query_one::<#inner_ty, _, _>(
                    &self.manager,
                    db,
                    #sql_id,
                    #params_expr,
                ).await.map_err(|e| e.into())
            }
        }
        ReturnKind::Scalar => {
            quote! {
                #internal_mod::query_scalar(
                    &self.manager,
                    db,
                    #sql_id,
                    #params_expr,
                ).await.map_err(|e| e.into())
            }
        }
        ReturnKind::Affected => {
            quote! {
                #internal_mod::execute(
                    &self.manager,
                    db,
                    #sql_id,
                    #params_expr,
                ).await.map_err(|e| e.into())
            }
        }
        ReturnKind::Unit => {
            quote! { Ok(()) }
        }
    };

    // 组合最终方法体
    let body = quote! {
        #params_struct
        #call_expr
    };

    // 生成异步标记
    let async_token = if method.is_async {
        quote! { async }
    } else {
        quote! {}
    };

    // 如果标记了 #[transactional]，生成事务包装代码
    if method.is_transactional {
        // 生成事务版本的 body
        let tx_body = generate_tx_body_for_transactional(method, db_type);
        
        quote! {
            /// 自动事务方法：在事务中执行，成功自动提交，失败自动回滚
            pub #async_token fn #method_name<D: #db_pool_trait>(
                &self,
                db: &D,
                #(#param_defs),*
            ) #return_type {
                // 开启事务
                let mut tx = #internal_mod::begin_transaction(db).await?;
                
                // 执行操作
                let result = { #tx_body };
                
                // 根据结果提交或回滚
                match result {
                    Ok(value) => {
                        tx.commit().await.map_err(markdown_sql::MarkdownSqlError::from)?;
                        Ok(value)
                    }
                    Err(e) => {
                        // 事务会在 drop 时自动回滚
                        Err(e)
                    }
                }
            }
        }
    } else {
        quote! {
            pub #async_token fn #method_name<D: #db_pool_trait>(
                &self,
                db: &D,
                #(#param_defs),*
            ) #return_type {
                #body
            }
        }
    }
}

/// 生成 #[transactional] 方法内部的事务操作代码
fn generate_tx_body_for_transactional(method: &MethodInfo, db_type: DbTypeArg) -> proc_macro2::TokenStream {
    let sql_id = &method.sql_id;
    let internal_mod = db_type.internal_module();

    // 构建参数代码（使用结构体）
    let (params_struct, params_expr) = build_params_code(method);

    let call_expr = match &method.return_kind {
        ReturnKind::List(inner_ty) => {
            quote! {
                #internal_mod::query_list_tx::<#inner_ty, _>(
                    &self.manager,
                    &mut tx,
                    #sql_id,
                    #params_expr,
                ).await.map_err(|e| e.into())
            }
        }
        ReturnKind::Optional(inner_ty) => {
            match db_type {
                DbTypeArg::Sqlite => quote! {
                    #internal_mod::query_optional_tx::<#inner_ty, _>(
                        &self.manager,
                        &mut tx,
                        #sql_id,
                        #params_expr,
                    ).await.map_err(|e| e.into())
                },
                _ => quote! {
                    let result: Vec<#inner_ty> = #internal_mod::query_list_tx(
                        &self.manager,
                        &mut tx,
                        #sql_id,
                        #params_expr,
                    ).await.map_err(|e| e.into())?;
                    Ok(result.into_iter().next())
                },
            }
        }
        ReturnKind::One(inner_ty) => {
            match db_type {
                DbTypeArg::Sqlite => quote! {
                    #internal_mod::query_one_tx::<#inner_ty, _>(
                        &self.manager,
                        &mut tx,
                        #sql_id,
                        #params_expr,
                    ).await.map_err(|e| e.into())
                },
                _ => quote! {
                    let result: Vec<#inner_ty> = #internal_mod::query_list_tx(
                        &self.manager,
                        &mut tx,
                        #sql_id,
                        #params_expr,
                    ).await.map_err(|e| e.into())?;
                    result.into_iter().next()
                        .ok_or_else(|| markdown_sql::MarkdownSqlError::not_found(#sql_id))
                },
            }
        }
        ReturnKind::Scalar => {
            match db_type {
                DbTypeArg::Sqlite => quote! {
                    #internal_mod::query_scalar_tx(
                        &self.manager,
                        &mut tx,
                        #sql_id,
                        #params_expr,
                    ).await.map_err(|e| e.into())
                },
                _ => quote! {
                    Err(markdown_sql::MarkdownSqlError::not_supported(
                        "query_scalar_tx",
                        "事务中的标量查询暂不支持此数据库"
                    ))
                },
            }
        }
        ReturnKind::Affected => {
            quote! {
                #internal_mod::execute_tx(
                    &self.manager,
                    &mut tx,
                    #sql_id,
                    #params_expr,
                ).await.map_err(|e| e.into())
            }
        }
        ReturnKind::Unit => {
            quote! { Ok(()) }
        }
    };

    // 组合参数结构体和调用
    quote! {
        #params_struct
        #call_expr
    }
}

/// 生成事务版本方法实现（方法名_tx）- 简化版
fn generate_method_impl_tx(method: &MethodInfo, db_type: DbTypeArg) -> proc_macro2::TokenStream {
    let method_name = &method.name;
    let tx_method_name = format_ident!("{}_tx", method_name);
    let sql_id = &method.sql_id;
    let internal_mod = db_type.internal_module();

    // 生成参数列表
    let param_defs: Vec<_> = method
        .params
        .iter()
        .map(|(name, ty, _)| {
            quote! { #name: #ty }
        })
        .collect();

    // 生成返回类型
    let return_type = method.return_type.as_ref().map(|ty| quote! { -> #ty });

    // 构建参数代码（使用结构体）
    let (params_struct, params_expr) = build_params_code(method);

    // 根据返回类型生成调用代码
    let call_expr = match &method.return_kind {
        ReturnKind::List(inner_ty) => {
            quote! {
                #internal_mod::query_list_tx::<#inner_ty, _>(
                    &self.manager,
                    tx,
                    #sql_id,
                    #params_expr,
                ).await.map_err(|e| e.into())
            }
        }
        ReturnKind::Optional(inner_ty) => {
            match db_type {
                DbTypeArg::Sqlite => quote! {
                    #internal_mod::query_optional_tx::<#inner_ty, _>(
                        &self.manager,
                        tx,
                        #sql_id,
                        #params_expr,
                    ).await.map_err(|e| e.into())
                },
                _ => quote! {
                    let result: Vec<#inner_ty> = #internal_mod::query_list_tx(
                        &self.manager,
                        tx,
                        #sql_id,
                        #params_expr,
                    ).await.map_err(|e| e.into())?;
                    Ok(result.into_iter().next())
                },
            }
        }
        ReturnKind::One(inner_ty) => {
            match db_type {
                DbTypeArg::Sqlite => quote! {
                    #internal_mod::query_one_tx::<#inner_ty, _>(
                        &self.manager,
                        tx,
                        #sql_id,
                        #params_expr,
                    ).await.map_err(|e| e.into())
                },
                _ => quote! {
                    let result: Vec<#inner_ty> = #internal_mod::query_list_tx(
                        &self.manager,
                        tx,
                        #sql_id,
                        #params_expr,
                    ).await.map_err(|e| e.into())?;
                    result.into_iter().next()
                        .ok_or_else(|| markdown_sql::MarkdownSqlError::not_found(#sql_id))
                },
            }
        }
        ReturnKind::Scalar => {
            match db_type {
                DbTypeArg::Sqlite => quote! {
                    #internal_mod::query_scalar_tx(
                        &self.manager,
                        tx,
                        #sql_id,
                        #params_expr,
                    ).await.map_err(|e| e.into())
                },
                _ => quote! {
                    // MySQL/PG 暂不支持 query_scalar_tx
                    Err(markdown_sql::MarkdownSqlError::not_supported(
                        "query_scalar_tx",
                        "事务中的标量查询暂不支持此数据库"
                    ))
                },
            }
        }
        ReturnKind::Affected => {
            quote! {
                #internal_mod::execute_tx(
                    &self.manager,
                    tx,
                    #sql_id,
                    #params_expr,
                ).await.map_err(|e| e.into())
            }
        }
        ReturnKind::Unit => {
            quote! { Ok(()) }
        }
    };

    // 组合参数结构体和调用
    let body = quote! {
        #params_struct
        #call_expr
    };

    // 生成异步标记
    let async_token = if method.is_async {
        quote! { async }
    } else {
        quote! {}
    };

    quote! {
        /// 事务版本：在事务中执行此操作
        pub #async_token fn #tx_method_name<'t>(
            &self,
            tx: &mut #internal_mod::Transaction<'t>,
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
/// - `sql_file`: SQL 文件路径（相对于项目根目录）**【必需】**
/// - `db_type`: 数据库类型，支持 `"sqlite"`、`"mysql"`、`"postgres"`（默认: `"sqlite"`）
///
/// ## 示例
///
/// ### SQLite（默认）
///
/// ```ignore
/// #[repository(sql_file = "sql/UserRepository.md")]
/// pub trait UserRepository {
///     async fn find_by_id(&self, params: &IdParams) -> Result<Option<User>>;
///     async fn find_all(&self) -> Result<Vec<User>>;
///     async fn insert(&self, user: &UserInput) -> Result<u64>;
/// }
/// ```
///
/// ### MySQL
///
/// ```ignore
/// #[repository(sql_file = "sql/UserRepository.md", db_type = "mysql")]
/// pub trait UserRepository {
///     async fn find_by_id(&self, params: &IdParams) -> Result<Option<User>>;
/// }
/// ```
///
/// ### PostgreSQL
///
/// ```ignore
/// #[repository(sql_file = "sql/UserRepository.md", db_type = "postgres")]
/// pub trait UserRepository {
///     async fn find_by_id(&self, params: &IdParams) -> Result<Option<User>>;
/// }
/// ```
///
/// ## 方法命名规则
///
/// - 方法名自动映射到 SQL ID：`snake_case` → `camelCase`
/// - `find_by_id` → `findById`
/// - `find_all` → `findAll`
/// - `get_count` → `getCount`
///
/// ## 返回类型映射
///
/// | 返回类型 | 调用函数 | 说明 |
/// |---------|---------|------|
/// | `Vec<T>` | `query_list` | 查询列表 |
/// | `Option<T>` | `query_optional` | 查询单条（可选） |
/// | `T` | `query_one` | 查询单条（必须） |
/// | `i64` | `query_scalar` | 标量查询（COUNT） |
/// | `u64` | `execute` | 影响行数（INSERT/UPDATE/DELETE） |
///
/// ## 生成的代码
///
/// ```ignore
/// pub struct UserRepositoryImpl {
///     manager: &'static markdown_sql::SqlManager,
/// }
///
/// impl UserRepositoryImpl {
///     pub fn new(manager: &'static markdown_sql::SqlManager) -> Self {
///         Self { manager }
///     }
///
///     pub async fn find_by_id<D: markdown_sql::SqliteDbPool>(
///         &self,
///         db: &D,
///         params: &IdParams
///     ) -> Result<Option<User>> {
///         markdown_sql::__internal::sqlite::query_optional::<User, _, _>(...)
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
    if let Err(e) = check_sql_file_safety(sql_file) {
        return syn::Error::new_spanned(&trait_item.ident, e)
            .to_compile_error()
            .into();
    }

    let db_type = args.db_type;

    // 生成普通方法实现
    let method_impls: Vec<_> = methods
        .iter()
        .map(|m| generate_method_impl(m, db_type))
        .collect();

    // 生成事务版本方法实现
    let tx_method_impls: Vec<_> = methods
        .iter()
        .map(|m| generate_method_impl_tx(m, db_type))
        .collect();

    let internal_mod = db_type.internal_module();
    let db_pool_trait = db_type.db_pool_trait();

    // 生成完整代码
    let expanded = quote! {
        // 保留原始 trait 定义
        #trait_item

        /// 自动生成的 Repository 实现
        ///
        /// 每个方法都有两个版本：
        /// - 普通版本：`find_all(&db)` - 使用连接池
        /// - 事务版本：`find_all_tx(&mut tx)` - 在事务中执行
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

            /// 开启事务
            ///
            /// ## 示例
            ///
            /// ```ignore
            /// let mut tx = repo.begin_transaction(&db).await?;
            /// repo.insert_tx(&mut tx, &user1).await?;
            /// repo.insert_tx(&mut tx, &user2).await?;
            /// tx.commit().await?;
            /// ```
            pub async fn begin_transaction<D: #db_pool_trait>(
                &self,
                db: &D,
            ) -> markdown_sql::Result<#internal_mod::Transaction<'static>> {
                #internal_mod::begin_transaction(db).await
            }

            // ==================== 普通方法 ====================
            #(#method_impls)*

            // ==================== 事务版本方法 ====================
            #(#tx_method_impls)*
        }
    };

    TokenStream::from(expanded)
}

/// 事务标记属性
///
/// 在 Repository trait 方法上使用此属性，方法调用时会自动：
/// 1. 开启事务
/// 2. 执行操作
/// 3. 成功时提交，失败时自动回滚
///
/// ## 示例
///
/// ```ignore
/// #[repository(sql_file = "sql/UserRepository.md")]
/// pub trait UserRepository {
///     // 普通方法（无事务）
///     async fn insert(&self, user: &UserInsert) -> Result<u64>;
///
///     // 自动事务方法
///     #[transactional]
///     async fn batch_insert(&self, users: &[UserInsert]) -> Result<u64>;
/// }
///
/// // 调用时自动在事务中执行
/// repo.batch_insert(&db, &users).await?;
/// ```
#[proc_macro_attribute]
pub fn transactional(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // 这是一个标记属性，由 #[repository] 宏解析
    // 此宏直接返回原始内容，不做任何修改
    item
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
