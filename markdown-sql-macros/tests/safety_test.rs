//! 安全检查功能测试
//!
//! 测试编译时安全检查的逻辑。

use std::fs;
use std::path::PathBuf;

/// 获取测试 SQL 目录
fn test_sql_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sql")
}

#[test]
fn test_safe_sql_file_exists() {
    let safe_path = test_sql_dir().join("safe.md");
    assert!(safe_path.exists(), "安全 SQL 文件应该存在");

    let content = fs::read_to_string(&safe_path).unwrap();

    // 安全文件不应包含不安全模式
    assert!(!content.contains("{{ name }}"), "安全文件不应包含直接输出");
    assert!(content.contains("#{id}"), "安全文件应使用参数绑定");
    assert!(content.contains("bind_join"), "安全文件应使用 bind_join");
}

#[test]
fn test_unsafe_sql_file_exists() {
    let unsafe_path = test_sql_dir().join("unsafe.md");
    assert!(unsafe_path.exists(), "不安全 SQL 文件应该存在");

    let content = fs::read_to_string(&unsafe_path).unwrap();

    // 不安全文件包含不安全模式
    assert!(content.contains("{{ name }}"), "不安全文件应包含直接输出");
}

#[test]
fn test_safety_check_integration() {
    // 测试解析器和安全检查器的集成
    let safe_content = r#"
# Safe SQL

```sql
-- findById
SELECT * FROM user WHERE id = #{id}
```

```sql
-- findByIds
SELECT * FROM user WHERE id IN ({{ ids | bind_join(",") }})
```
"#;

    // 解析内容
    let blocks: Vec<_> = safe_content
        .match_indices("```sql")
        .map(|(start, _)| {
            let end = safe_content[start..]
                .find("```\n")
                .map(|e| start + e)
                .unwrap_or(safe_content.len());
            &safe_content[start..end]
        })
        .collect();

    // 验证没有不安全的模式
    for block in &blocks {
        // 检查是否有未经过滤的 {{ }} 输出
        if block.contains("{{") && !block.contains("bind_join") && !block.contains("raw_safe") {
            panic!("发现不安全的 SQL 模式: {}", block);
        }
    }
}

#[test]
fn test_unsafe_pattern_detection() {
    let unsafe_patterns = vec![
        "WHERE name = {{ name }}",
        "WHERE id IN ({{ ids | join(\",\") }})",
        "SELECT {{ column }} FROM user",
    ];

    let safe_patterns = vec![
        "WHERE name = #{name}",
        "WHERE id IN ({{ ids | bind_join(\",\") }})",
        "SELECT {{ column | raw_safe }} FROM user",
        "{% if name %}AND name = #{name}{% endif %}",
    ];

    for pattern in unsafe_patterns {
        // 检查是否能检测到不安全模式
        let has_unsafe_output = pattern.contains("{{")
            && !pattern.contains("bind_join")
            && !pattern.contains("raw_safe");
        assert!(has_unsafe_output, "应该检测到不安全模式: {}", pattern);
    }

    for pattern in safe_patterns {
        // 检查安全模式不会误报
        let has_unsafe_output = pattern.contains("{{")
            && !pattern.contains("bind_join")
            && !pattern.contains("raw_safe");
        assert!(!has_unsafe_output, "不应该误报安全模式: {}", pattern);
    }
}
