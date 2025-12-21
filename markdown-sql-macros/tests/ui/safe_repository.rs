//! 安全的 Repository 定义（应该编译成功）

use markdown_sql_macros::repository;

#[repository(sql_file = "tests/sql/safe.md")]
pub trait SafeUserRepository {
    async fn find_by_id(&self, id: i64);
    async fn find_by_ids(&self, ids: Vec<i64>);
    async fn find_by_condition(&self, name: Option<String>, status: Option<i32>);
}

fn main() {
    println!("编译成功！");
}
