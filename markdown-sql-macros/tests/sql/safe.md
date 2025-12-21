# Safe SQL

安全的 SQL 定义。

## 根据 ID 查询

```sql
-- findById
SELECT * FROM user WHERE id = #{id}
```

## IN 查询

```sql
-- findByIds
SELECT * FROM user WHERE id IN ({{ ids | bind_join(",") }})
```

## 动态 SQL

```sql
-- findByCondition
SELECT * FROM user
WHERE 1=1
{% if name %}AND name = #{name}{% endif %}
{% if status %}AND status = #{status}{% endif %}
```
