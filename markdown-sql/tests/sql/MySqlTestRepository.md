# MySQL 测试 SQL

## 表管理

```sql
-- dropTable
DROP TABLE IF EXISTS mysql_test_users
```

```sql
-- createTable
CREATE TABLE mysql_test_users (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    age INT NOT NULL,
    status INT NOT NULL DEFAULT 1
)
```

```sql
-- truncateTable
TRUNCATE TABLE mysql_test_users
```

## 查询所有

```sql
-- findAll
SELECT id, name, age, status
FROM mysql_test_users
```

## 根据 ID 查询

```sql
-- findById
SELECT id, name, age, status
FROM mysql_test_users
WHERE id = #{id}
```

## 插入

```sql
-- insert
INSERT INTO mysql_test_users (name, age, status)
VALUES (#{name}, #{age}, #{status})
```

## 更新

```sql
-- update
UPDATE mysql_test_users
SET name = #{name}, age = #{age}, status = #{status}
WHERE id = #{id}
```

## 删除

```sql
-- delete
DELETE FROM mysql_test_users
WHERE id = #{id}
```

## 统计

```sql
-- count
SELECT COUNT(*) FROM mysql_test_users
{% if status %}WHERE status = #{status}{% endif %}
```
