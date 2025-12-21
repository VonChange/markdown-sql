# PostgreSQL 测试 SQL

## 表管理

```sql
-- dropTable
DROP TABLE IF EXISTS pg_test_users
```

```sql
-- createTable
CREATE TABLE pg_test_users (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    age INT NOT NULL,
    status INT NOT NULL DEFAULT 1
)
```

```sql
-- truncateTable
TRUNCATE TABLE pg_test_users RESTART IDENTITY
```

## 查询所有

```sql
-- findAll
SELECT id, name, age, status
FROM pg_test_users
```

## 根据 ID 查询

```sql
-- findById
SELECT id, name, age, status
FROM pg_test_users
WHERE id = #{id}
```

## 插入

```sql
-- insert
INSERT INTO pg_test_users (name, age, status)
VALUES (#{name}, #{age}, #{status})
```

## 更新

```sql
-- update
UPDATE pg_test_users
SET name = #{name}, age = #{age}, status = #{status}
WHERE id = #{id}
```

## 删除

```sql
-- delete
DELETE FROM pg_test_users
WHERE id = #{id}
```

## 统计

```sql
-- count
SELECT COUNT(*) FROM pg_test_users
{% if status %}WHERE status = #{status}{% endif %}
```
