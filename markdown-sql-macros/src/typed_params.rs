//! TypedParams derive macro 实现
//!
//! 为参数结构体自动生成类型感知的参数绑定代码。

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type};

/// 生成 TypedParams 实现
pub fn derive_typed_params_impl(input: DeriveInput) -> TokenStream {
    let struct_name = &input.ident;

    // 只支持结构体
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields.named.iter().collect::<Vec<_>>(),
            Fields::Unit => vec![], // 支持无字段结构体（如 EmptyParams）
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "TypedParams 只支持具名字段或无字段的结构体",
                )
                .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "TypedParams 只支持结构体")
                .to_compile_error();
        }
    };

    // 收集字段信息
    let field_names: Vec<_> = fields
        .iter()
        .filter_map(|f| f.ident.as_ref())
        .collect();

    let field_name_strs: Vec<_> = field_names.iter().map(|n| n.to_string()).collect();

    // 检查是否有 Vec 类型的字段（用于 IN 查询）
    let vec_fields: Vec<_> = fields
        .iter()
        .filter_map(|f| {
            let name = f.ident.as_ref()?;
            if is_vec_type(&f.ty) {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    // 生成普通字段的 match 分支（跳过 Vec 类型，Vec 通过 __bind_N 处理）
    let match_arms: Vec<_> = fields
        .iter()
        .filter(|f| !is_vec_type(&f.ty))  // 跳过 Vec 类型
        .filter_map(|f| {
            let name = f.ident.as_ref()?;
            let name_str = name.to_string();
            Some(quote! {
                #name_str => {
                    args.add(&self.#name)
                        .map_err(|e| markdown_sql::MarkdownSqlError::ParamError(
                            format!("绑定参数 '{}' 失败: {}", #name_str, e)
                        ))?;
                }
            })
        })
        .collect();

    // 生成 __bind_N 处理代码（用于 IN 查询）
    let bind_handlers: Vec<_> = vec_fields
        .iter()
        .map(|name| {
            let name_str = name.to_string();
            quote! {
                // 检查是否是 __bind_N 格式，且对应当前 Vec 字段
                if let Some(idx_str) = __name.strip_prefix("__bind_") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < self.#name.len() {
                            args.add(&self.#name[idx])
                                .map_err(|e| markdown_sql::MarkdownSqlError::ParamError(
                                    format!("绑定参数 '{}[{}]' 失败: {}", #name_str, idx, e)
                                ))?;
                            continue;
                        }
                    }
                }
            }
        })
        .collect();

    // 生成 PostgreSQL 实现
    let pg_impl = quote! {
        #[cfg(feature = "postgres")]
        impl markdown_sql::TypedParamsPg for #struct_name {
            fn bind_to_pg_args(
                &self,
                param_names: &[String],
                args: &mut sqlx::postgres::PgArguments,
            ) -> markdown_sql::Result<()> {
                use sqlx::Arguments;
                
                for __name in param_names {
                    // 处理 IN 查询的 __bind_N 参数
                    #(#bind_handlers)*
                    
                    // 处理普通字段
                    match __name.as_str() {
                        #(#match_arms)*
                        _ => {
                            // 未知参数，记录警告但不报错（可能是嵌套字段等）
                            tracing::warn!("TypedParams: 未知参数 '{}'", __name);
                        }
                    }
                }
                Ok(())
            }
        }
    };

    // 生成 SQLite 实现
    let sqlite_impl = quote! {
        #[cfg(feature = "sqlite")]
        impl markdown_sql::TypedParamsSqlite for #struct_name {
            fn bind_to_sqlite_args<'__q>(
                &'__q self,
                param_names: &[String],
                args: &mut sqlx::sqlite::SqliteArguments<'__q>,
            ) -> markdown_sql::Result<()> {
                use sqlx::Arguments;
                
                for __name in param_names {
                    // 处理 IN 查询的 __bind_N 参数
                    #(#bind_handlers)*
                    
                    // 处理普通字段
                    match __name.as_str() {
                        #(#match_arms)*
                        _ => {
                            tracing::warn!("TypedParams: 未知参数 '{}'", __name);
                        }
                    }
                }
                Ok(())
            }
        }
    };

    // 生成 MySQL 实现
    let mysql_impl = quote! {
        #[cfg(feature = "mysql")]
        impl markdown_sql::TypedParamsMySql for #struct_name {
            fn bind_to_mysql_args(
                &self,
                param_names: &[String],
                args: &mut sqlx::mysql::MySqlArguments,
            ) -> markdown_sql::Result<()> {
                use sqlx::Arguments;
                
                for __name in param_names {
                    // 处理 IN 查询的 __bind_N 参数
                    #(#bind_handlers)*
                    
                    // 处理普通字段
                    match __name.as_str() {
                        #(#match_arms)*
                        _ => {
                            tracing::warn!("TypedParams: 未知参数 '{}'", __name);
                        }
                    }
                }
                Ok(())
            }
        }
    };

    quote! {
        #pg_impl
        #sqlite_impl
        #mysql_impl
    }
}

/// 检查类型是否是 Vec<T>
fn is_vec_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Vec";
        }
    }
    false
}
