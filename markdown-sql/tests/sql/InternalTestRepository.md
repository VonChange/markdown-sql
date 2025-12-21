# InternalTestRepository SQL

框架内部集成测试用 SQL。

## 表管理

```sql
-- createUserTable
CREATE TABLE user_info (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    age INTEGER NOT NULL,
    status INTEGER NOT NULL DEFAULT 1
)
```

```sql
-- createStressTable
CREATE TABLE stress_test (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    value TEXT NOT NULL
)
```

```sql
-- truncateUserTable
DELETE FROM user_info
```

## 查询

```sql
-- columns
id, name, age, status
```

```sql
-- commonCondition
{% if status %}AND status = #{status}{% endif %}
{% if name %}AND name LIKE #{name}{% endif %}
{% if minAge %}AND age >= #{minAge}{% endif %}
```

```sql
-- findById
SELECT {% include "columns" %}
FROM user_info
WHERE id = #{id}
```

```sql
-- findAll
SELECT {% include "columns" %}
FROM user_info
ORDER BY id
```

```sql
-- findByCondition
SELECT {% include "columns" %}
FROM user_info
WHERE 1=1
{% include "commonCondition" %}
ORDER BY id
```

```sql
-- findByIds
SELECT {% include "columns" %}
FROM user_info
WHERE id IN ({{ ids | bind_join(",") }})
ORDER BY id
```

## 写入

```sql
-- insert
INSERT INTO user_info (name, age, status)
VALUES (#{name}, #{age}, #{status})
```

```sql
-- update
UPDATE user_info
SET name = #{name},
    age = #{age},
    status = #{status}
WHERE id = #{id}
```

```sql
-- deleteById
DELETE FROM user_info WHERE id = #{id}
```

## 统计

```sql
-- count
SELECT COUNT(*) as count FROM user_info
WHERE 1=1
{% include "commonCondition" %}
```

## 压力测试

```sql
-- stressInsert
INSERT INTO stress_test (value) VALUES (#{value})
```

```sql
-- stressSelect
SELECT id, value FROM stress_test
```

```sql
-- stressCount
SELECT COUNT(*) FROM stress_test
```
