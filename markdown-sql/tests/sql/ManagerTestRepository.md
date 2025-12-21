# ManagerTestRepository SQL

manager.rs 单元测试用的 SQL 定义。

## 公共字段

```sql
-- columns
id, name, age
```

## 根据 ID 查询

```sql
-- findById
SELECT * FROM user WHERE id = #{id}
```

## 查询所有

```sql
-- findAll
SELECT {% include "columns" %}
FROM user
```

## 条件查询

```sql
-- findByCondition
SELECT * FROM user
WHERE 1=1
{% if name %}AND name LIKE #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
```

## IN 查询

```sql
-- findByIds
SELECT * FROM user
WHERE id IN ({{ ids | bind_join(",") }})
```
