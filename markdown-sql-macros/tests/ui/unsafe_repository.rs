//! 不安全的 Repository 定义（应该编译失败）

use markdown_sql_macros::repository;

#[repository(sql_file = "tests/sql/unsafe.md")]
pub trait UnsafeUserRepository {
    async fn find_by_name(&self, name: String);
}

fn main() {}
