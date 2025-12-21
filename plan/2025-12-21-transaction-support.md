# 事务支持设计

## 需求

1. **Repository 级事务**：`#[transactional]` 注解，用于批量操作
2. **业务服务层事务**：多个 Repository 操作在同一事务中
3. **手动事务管理**：业务层可以手动控制 begin/commit/rollback

---

## 设计方案

### 1. Repository 方法支持事务参数

Repository 方法需要能够接受 Transaction 作为执行上下文：

```rust
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    // 普通方法（使用连接池，autocommit）
    async fn find_all(&self) -> Result<Vec<User>>;
    async fn insert(&self, user: &UserInsert) -> Result<u64>;
}
```

生成的方法签名支持两种调用方式：
- `repo.insert(&db, &user)` - 使用连接池，autocommit
- `repo.insert_tx(&mut tx, &user)` - 使用事务

### 2. 手动事务 API

```rust
use markdown_sql::Transaction;

// 获取事务（通过 Repository 或独立函数）
let mut tx = db.begin_transaction().await?;

// 在事务中执行操作
repo.insert_tx(&mut tx, &user1).await?;
repo.insert_tx(&mut tx, &user2).await?;

// 提交
tx.commit().await?;

// 或回滚
// tx.rollback().await?;
```

### 3. 闭包事务 API

```rust
use markdown_sql::with_transaction;

with_transaction(&db, |tx| async move {
    repo.insert_tx(tx, &user1).await?;
    repo.insert_tx(tx, &user2).await?;
    Ok(())
}).await?;
// 成功自动 commit，失败自动 rollback
```

### 4. `#[transactional]` 宏（Repository 级）

```rust
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    // 自动在事务中执行（主要用于批量操作）
    #[transactional]
    async fn batch_insert(&self, users: &[UserInsert]) -> Result<u64>;
}
```

---

## 实现步骤

### 阶段一：扩展宏生成事务版本方法 ✅

- [x] 为每个方法生成 `_tx` 版本
- [x] `insert` → 生成 `insert` + `insert_tx`
- [x] `_tx` 版本接受 `&mut Transaction` 参数

### 阶段二：提供手动事务 API ✅

- [x] Repository 添加 `begin_transaction()` 方法
- [x] 导出 `Transaction` 类型
- [x] `with_transaction` 闭包 API（已有）

### 阶段三：`#[transactional]` 属性支持 ✅

- [x] 解析 `#[transactional]` 属性
- [x] 自动包装事务逻辑

### 阶段四：测试和文档 ✅

- [x] 添加事务测试用例（test_transaction_commit、test_transaction_rollback、test_transaction_query）
- [x] 更新 README
- [x] 更新 CLAUDE.md

---

## API 示例

### 示例 1：手动事务（最灵活）

```rust
// 开启事务
let mut tx = db.begin_transaction().await?;

// 在事务中执行多个操作
let order_repo = get_order_repo();
let item_repo = get_order_item_repo();

order_repo.insert_tx(&mut tx, &order).await?;
for item in &items {
    item_repo.insert_tx(&mut tx, item).await?;
}

// 提交
tx.commit().await?;
```

### 示例 2：闭包事务（推荐）

```rust
with_transaction(&db, |tx| async move {
    order_repo.insert_tx(tx, &order).await?;
    for item in &items {
        item_repo.insert_tx(tx, item).await?;
    }
    Ok(())
}).await?;
```

### 示例 3：Repository 批量事务

```rust
#[repository(sql_file = "sql/UserRepository.md")]
pub trait UserRepository {
    #[transactional]
    async fn batch_insert(&self, users: &[UserInsert]) -> Result<u64>;
}

// 使用
repo.batch_insert(&db, &users).await?;  // 自动在事务中执行
```

---

## 验收标准

- [x] Repository 方法自动生成 `_tx` 版本
- [x] 手动事务 API 可用（`begin_transaction`）
- [x] 闭包事务 API 可用（`with_transaction`）
- [x] `#[transactional]` 注解可用
- [x] 测试覆盖所有场景
- [x] 文档更新完成
