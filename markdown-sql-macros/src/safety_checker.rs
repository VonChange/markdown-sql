//! SQL 安全检查器
//!
//! 编译时检测 SQL 模板中的不安全语法。
//!
//! ## 安全规则
//!
//! | 语法 | 状态 | 说明 |
//! |-----|------|------|
//! | `#{param}` | ✅ 安全 | 参数绑定 |
//! | `{{ list \| bind_join(",") }}` | ✅ 安全 | IN 查询 |
//! | `{% if %}` / `{% for %}` | ✅ 安全 | 动态逻辑 |
//! | `{% include %}` | ✅ 安全 | SQL 片段引用 |
//! | `{{ param }}` | ❌ 禁止 | 直接拼接，编译失败 |
//! | `{{ list \| join(",") }}` | ❌ 禁止 | 直接拼接，编译失败 |
//! | `{{ param \| raw_safe }}` | ⚠️ 豁免 | 显式声明安全 |

use once_cell::sync::Lazy;
use regex::Regex;

/// 安全过滤器白名单
#[allow(dead_code)]
const SAFE_FILTERS: &[&str] = &["bind_join", "raw_safe"];

/// 不安全的 {{ }} 语法正则
/// 匹配 {{ xxx }} 模式
static UNSAFE_OUTPUT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\{\{\s*[^}]+\s*\}\}").unwrap()
});

/// 安全过滤器正则
static SAFE_FILTER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\|\s*(bind_join|raw_safe)").unwrap()
});

/// 安全检查错误
#[derive(Debug, Clone)]
pub struct SafetyError {
    /// SQL ID
    pub sql_id: String,
    /// 行号
    pub line: usize,
    /// 不安全内容
    pub content: String,
    /// 修复建议
    pub suggestion: String,
}

impl std::fmt::Display for SafetyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SQL 安全检查失败: {} 第 {} 行\n  内容: {}\n  建议: {}",
            self.sql_id, self.line, self.content, self.suggestion
        )
    }
}

/// SQL 安全检查器
pub struct SafetyChecker;

impl SafetyChecker {
    /// 检查 SQL 模板是否安全
    ///
    /// # 参数
    /// - `sql_id`: SQL ID（用于错误提示）
    /// - `content`: SQL 模板内容
    ///
    /// # 返回
    /// - `Ok(())` 如果安全
    /// - `Err(SafetyError)` 如果检测到不安全语法
    pub fn check(sql_id: &str, content: &str) -> Result<(), SafetyError> {
        // 查找所有 {{ }} 输出
        for mat in UNSAFE_OUTPUT_RE.find_iter(content) {
            let output = mat.as_str();

            // 检查是否使用了安全过滤器
            if !SAFE_FILTER_RE.is_match(output) {
                // 计算行号
                let line_num = content[..mat.start()].matches('\n').count() + 1;

                return Err(SafetyError {
                    sql_id: sql_id.to_string(),
                    line: line_num,
                    content: output.to_string(),
                    suggestion: Self::get_suggestion(output),
                });
            }
        }

        Ok(())
    }

    /// 检查多个 SQL 块
    #[allow(dead_code)]
    pub fn check_all(
        blocks: &[(String, String)],
    ) -> Result<(), Vec<SafetyError>> {
        let mut errors = Vec::new();

        for (sql_id, content) in blocks {
            if let Err(e) = Self::check(sql_id, content) {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// 生成修复建议
    fn get_suggestion(unsafe_output: &str) -> String {
        // 提取变量名
        if let Some(var) = Self::extract_var_name(unsafe_output) {
            if unsafe_output.contains("join") {
                format!("请改为: {{{{ {} | bind_join(\",\") }}}}", var)
            } else {
                format!("请改为: #{{{}}} (参数绑定)", var)
            }
        } else {
            "请使用 #{param} 参数绑定语法".to_string()
        }
    }

    /// 提取变量名
    fn extract_var_name(output: &str) -> Option<String> {
        // 简单提取 {{ var }} 中的 var
        let trimmed = output
            .trim_start_matches("{{")
            .trim_end_matches("}}")
            .trim();
        let var = trimmed.split('|').next()?.trim();
        if !var.is_empty() {
            Some(var.to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsafe_direct_output() {
        let result = SafetyChecker::check("test", "WHERE name = {{ name }}");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.suggestion.contains("#{name}"));
    }

    #[test]
    fn test_unsafe_join() {
        let result = SafetyChecker::check(
            "test",
            "WHERE id IN ({{ ids | join(\",\") }})",
        );
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.suggestion.contains("bind_join"));
    }

    #[test]
    fn test_safe_bind_join() {
        let result = SafetyChecker::check(
            "test",
            "WHERE id IN ({{ ids | bind_join(\",\") }})",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_safe_raw_safe() {
        let result = SafetyChecker::check(
            "test",
            "SELECT * FROM {{ table | raw_safe }}",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_safe_param_binding() {
        let result = SafetyChecker::check(
            "test",
            "WHERE name = #{name} AND age = #{age}",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_safe_dynamic_sql() {
        let result = SafetyChecker::check(
            "test",
            r#"
SELECT * FROM user
WHERE 1=1
{% if name %}AND name = #{name}{% endif %}
{% for status in statuses %}
  {% if loop.first %}AND status IN ({% endif %}
  #{status}{% if not loop.last %},{% endif %}
  {% if loop.last %}){% endif %}
{% endfor %}
"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_all() {
        let blocks = vec![
            ("safe".to_string(), "WHERE id = #{id}".to_string()),
            ("unsafe".to_string(), "WHERE name = {{ name }}".to_string()),
        ];

        let result = SafetyChecker::check_all(&blocks);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].sql_id, "unsafe");
    }
}
