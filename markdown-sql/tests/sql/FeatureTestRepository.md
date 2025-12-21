# FeatureTestRepository SQL

功能测试 Repository 的 SQL 定义。

## 表管理

```sql
-- createTable
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    age INTEGER NOT NULL,
    email TEXT,
    status INTEGER NOT NULL DEFAULT 1,
    created_at TEXT,
    updated_at TEXT
)
```

```sql
-- dropTable
DROP TABLE IF EXISTS users
```

```sql
-- truncateTable
DELETE FROM users
```

## 通用字段

```sql
-- columns
id, name, age, email, status, created_at
```

## 公共条件

```sql
-- commonCondition
{% if status %}AND status = #{status}{% endif %}
{% if name %}AND name LIKE #{name}{% endif %}
{% if min_age %}AND age >= #{min_age}{% endif %}
```

## 查询

```sql
-- findAll
SELECT {% include "columns" %}
FROM users
ORDER BY id
```

```sql
-- findById
SELECT {% include "columns" %}
FROM users
WHERE id = #{id}
```

```sql
-- findByCondition
SELECT {% include "columns" %}
FROM users
WHERE 1=1
{% include "commonCondition" %}
ORDER BY id
```

```sql
-- findByIds
SELECT {% include "columns" %}
FROM users
WHERE id IN ({{ ids | bind_join(",") }})
ORDER BY id
```

## 写入

```sql
-- insert
INSERT INTO users (name, age, email, status, created_at)
VALUES (#{name}, #{age}, #{email}, #{status}, datetime('now'))
```

```sql
-- update
UPDATE users
SET name = #{name},
    age = #{age},
    email = #{email},
    status = #{status},
    updated_at = datetime('now')
WHERE id = #{id}
```

```sql
-- deleteById
DELETE FROM users WHERE id = #{id}
```

## 统计

```sql
-- count
SELECT COUNT(*) FROM users
WHERE 1=1
{% include "commonCondition" %}
```
