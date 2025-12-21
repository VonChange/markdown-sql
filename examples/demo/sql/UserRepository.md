# UserRepository SQL

用户数据访问层的所有 SQL 定义。

## 公共字段

```sql
-- columns
id, name, age, email, status, created_at
```

## 查询所有用户

```sql
-- findAll
SELECT {% include "columns" %}
FROM users
ORDER BY id DESC
```

## 根据 ID 查询

```sql
-- findById
SELECT {% include "columns" %}
FROM users
WHERE id = #{id}
```

## 条件查询

```sql
-- findByCondition
SELECT {% include "columns" %}
FROM users
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
{% if min_age %}AND age >= #{min_age}{% endif %}
ORDER BY id DESC
```

## 统计用户数量

```sql
-- count
SELECT COUNT(*) as count
FROM users
WHERE 1=1
{% if status %}AND status = #{status}{% endif %}
```

## 插入用户

```sql
-- insert
INSERT INTO users (name, age, email, status, created_at)
VALUES (#{name}, #{age}, #{email}, #{status}, datetime('now'))
```

## 更新用户

```sql
-- update
UPDATE users
SET
{% if name %}name = #{name},{% endif %}
{% if age %}age = #{age},{% endif %}
{% if email %}email = #{email},{% endif %}
{% if status %}status = #{status},{% endif %}
updated_at = datetime('now')
WHERE id = #{id}
```

## 删除用户

```sql
-- deleteById
DELETE FROM users WHERE id = #{id}
```

## IN 查询

```sql
-- findByIds
SELECT {% include "columns" %}
FROM users
WHERE id IN ({{ ids | bind_join(",") }})
```
