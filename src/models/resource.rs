#[derive(Debug, serde::Serialize)]
pub struct ResourceGroup {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub is_alpha: bool,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Endpoint {
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub is_alpha: bool,
    pub external_docs: Option<ExternalDoc>,
    pub parameters: Vec<Parameter>,
    pub request_body: Option<RequestBody>,
    pub responses: Vec<Response>,
    pub security_schemes: Vec<String>,
    pub callbacks: Vec<CallbackEntry>,
    #[serde(skip_serializing)]
    pub links: Vec<ResponseLink>,
    pub drill_deeper: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Parameter {
    pub name: String,
    pub location: ParameterLocation,
    pub required: bool,
    pub schema_type: String,
    pub format: Option<String>,
    pub description: Option<String>,
    pub enum_values: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RequestBody {
    pub content_type: String,
    pub fields: Vec<Field>,
    /// Schema names for oneOf/anyOf request bodies (fields will be empty in this case)
    pub options: Vec<String>,
    pub schema_ref: Option<String>,
    pub example: Option<serde_json::Value>,
    /// When the body is `type: array` with `items.$ref`, this holds the item type name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array_item_type: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Response {
    pub status_code: String,
    pub description: String,
    pub schema_ref: Option<String>,
    pub example: Option<serde_json::Value>,
    pub headers: Vec<ResponseHeader>,
    pub links: Vec<ResponseLink>,
    /// Expanded schema fields, populated when --expand is used on endpoint detail
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExternalDoc {
    pub url: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResponseHeader {
    pub name: String,
    pub type_display: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResponseLink {
    pub name: String,
    pub operation_id: String,
    pub parameters: Vec<String>,
    pub description: Option<String>,
    pub drill_command: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CallbackResponse {
    pub status_code: String,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CallbackOperation {
    pub method: String,
    pub url_expression: String,
    pub summary: Option<String>,
    pub body_schema: Option<String>,
    pub responses: Vec<CallbackResponse>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CallbackEntry {
    pub name: String,
    pub defined_on_operation_id: Option<String>,
    pub defined_on_method: String,
    pub defined_on_path: String,
    pub operations: Vec<CallbackOperation>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Field {
    pub name: String,
    pub type_display: String,
    pub required: bool,
    pub optional: bool,
    pub nullable: bool,
    pub read_only: bool,
    pub write_only: bool,
    pub deprecated: bool,
    pub description: Option<String>,
    pub enum_values: Vec<String>,
    pub constraints: Vec<String>,
    pub default_value: Option<serde_json::Value>,
    pub example: Option<serde_json::Value>,
    pub nested_schema_name: Option<String>,
    pub nested_fields: Vec<Field>,
}

/// Convert a PascalCase or raw tag name to a human-readable display name.
/// Tags that already contain spaces are returned unchanged (after stripping status qualifiers).
/// PascalCase names get spaces inserted before each uppercase-after-lowercase transition.
/// Example: "DiscoveryIntegration" → "Discovery Integration"
pub fn humanize_tag_name(tag_name: &str) -> String {
    let mut s = tag_name.to_string();

    // Strip trailing status qualifiers (same logic as slugify)
    let lower = s.to_lowercase();
    for suffix in &[" (deprecated)", " (alpha)"] {
        if lower.ends_with(suffix) {
            s.truncate(s.len() - suffix.len());
            break;
        }
    }

    // If it already has spaces, it's a human-readable name — return as-is
    if s.contains(' ') {
        return s;
    }

    // Insert spaces before uppercase letters that follow lowercase (PascalCase split)
    let mut result = String::with_capacity(s.len() + 4);
    let mut prev_lower = false;
    for ch in s.chars() {
        if ch.is_uppercase() && prev_lower {
            result.push(' ');
        }
        result.push(ch);
        prev_lower = ch.is_lowercase();
    }

    result
}

/// Convert a tag name to a CLI-friendly slug.
/// Strips status qualifiers like (Deprecated)/(Alpha), splits PascalCase, lowercases.
pub fn slugify(tag_name: &str) -> String {
    let mut s = tag_name.to_string();

    // Strip trailing status qualifiers (case-insensitive)
    let lower = s.to_lowercase();
    for suffix in &[" (deprecated)", " (alpha)"] {
        if lower.ends_with(suffix) {
            s.truncate(s.len() - suffix.len());
            break;
        }
    }

    // Split PascalCase: insert hyphen before uppercase that follows lowercase
    let mut result = String::with_capacity(s.len() + 4);
    let mut prev_lower = false;
    for ch in s.chars() {
        if ch.is_uppercase() && prev_lower {
            result.push('-');
        }
        result.push(ch);
        prev_lower = ch.is_lowercase();
    }

    // Replace spaces with hyphens, lowercase, collapse multiple hyphens
    let result = result.replace(' ', "-").to_lowercase();
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_hyphen = false;
    for ch in result.chars() {
        if ch == '-' {
            if !prev_hyphen {
                collapsed.push('-');
            }
            prev_hyphen = true;
        } else {
            collapsed.push(ch);
            prev_hyphen = false;
        }
    }
    // Trim leading/trailing hyphens
    collapsed.trim_matches('-').to_string()
}

/// Returns true if the tag name contains "(Deprecated)" (case-insensitive).
pub fn is_deprecated_tag(tag_name: &str) -> bool {
    tag_name.to_lowercase().contains("(deprecated)")
}

/// Returns true if the tag name contains "(Alpha)" (case-insensitive).
pub fn is_alpha_tag(tag_name: &str) -> bool {
    tag_name.to_lowercase().contains("(alpha)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_spaces() {
        assert_eq!(slugify("Access Policies"), "access-policies");
    }

    #[test]
    fn test_slugify_pascal() {
        assert_eq!(slugify("DiscoveryIntegration"), "discovery-integration");
    }

    #[test]
    fn test_slugify_deprecated_stripped() {
        assert_eq!(slugify("Old Pets (Deprecated)"), "old-pets");
    }

    #[test]
    fn test_slugify_alpha_stripped() {
        assert_eq!(slugify("New Feature (Alpha)"), "new-feature");
    }

    #[test]
    fn test_slugify_experimental_alpha() {
        assert_eq!(slugify("Experimental (Alpha)"), "experimental");
    }

    #[test]
    fn test_deprecated_by_name() {
        assert!(is_deprecated_tag("Old Pets (Deprecated)"));
        assert!(is_deprecated_tag("Legacy API (deprecated)"));
        assert!(!is_deprecated_tag("Old Pets"));
    }

    #[test]
    fn test_alpha_by_name() {
        assert!(is_alpha_tag("Beta Feature (Alpha)"));
        assert!(is_alpha_tag("New Stuff (alpha)"));
        assert!(!is_alpha_tag("Beta Feature"));
    }

    #[test]
    fn test_not_deprecated() {
        assert!(!is_deprecated_tag("Access Policies"));
        assert!(!is_alpha_tag("Access Policies"));
    }

    #[test]
    fn test_humanize_pascal_case() {
        assert_eq!(
            humanize_tag_name("DiscoveryIntegration"),
            "Discovery Integration"
        );
        assert_eq!(
            humanize_tag_name("CredentialProvider"),
            "Credential Provider"
        );
        assert_eq!(
            humanize_tag_name("PascalCaseResource"),
            "Pascal Case Resource"
        );
    }

    #[test]
    fn test_humanize_already_spaced() {
        assert_eq!(
            humanize_tag_name("Credential Provider v2"),
            "Credential Provider v2"
        );
        assert_eq!(humanize_tag_name("Access Policies"), "Access Policies");
    }

    #[test]
    fn test_humanize_strips_status_qualifiers() {
        assert_eq!(humanize_tag_name("OldPets (Deprecated)"), "Old Pets");
        assert_eq!(humanize_tag_name("NewFeature (Alpha)"), "New Feature");
    }

    #[test]
    fn test_humanize_simple() {
        assert_eq!(humanize_tag_name("Pets"), "Pets");
    }
}
