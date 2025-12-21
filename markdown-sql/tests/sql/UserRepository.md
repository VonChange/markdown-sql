# UserRepository SQL

用于测试的用户相关 SQL 定义。

## 查询全部字段

```sql
-- columns
id, name, age, status
```

## 公共条件

```sql
-- commonCondition
{% if status %}AND status = #{status}{% endif %}
{% if name %}AND name LIKE #{name}{% endif %}
{% if minAge %}AND age >= #{minAge}{% endif %}
```

## 根据 ID 查询

```sql
-- findById
SELECT {% include "columns" %}
FROM user_info
WHERE id = #{id}
```

## 查询所有用户

```sql
-- findAll
SELECT {% include "columns" %}
FROM user_info
ORDER BY id
```

## 根据条件查询

```sql
-- findByCondition
SELECT {% include "columns" %}
FROM user_info
WHERE 1=1
{% include "commonCondition" %}
ORDER BY id
```

## 根据 ID 列表查询

```sql
-- findByIds
SELECT {% include "columns" %}
FROM user_info
WHERE id IN ({{ ids | bind_join(",") }})
ORDER BY id
```

## 插入用户

```sql
-- insert
INSERT INTO user_info (name, age, status)
VALUES (#{name}, #{age}, #{status})
```

## 更新用户

```sql
-- update
UPDATE user_info
SET name = #{name},
    age = #{age},
    status = #{status}
WHERE id = #{id}
```

## 删除用户

```sql
-- deleteById
DELETE FROM user_info WHERE id = #{id}
```

## 统计用户数量

```sql
-- count
SELECT COUNT(*) as count FROM user_info
WHERE 1=1
{% include "commonCondition" %}
```
