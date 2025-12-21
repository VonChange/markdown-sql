# Unsafe SQL

不安全的 SQL 定义（用于测试编译失败）。

## 不安全的直接输出

```sql
-- findByName
SELECT * FROM user WHERE name = {{ name }}
```
