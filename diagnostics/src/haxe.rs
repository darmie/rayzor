//! Haxe-specific diagnostic builders
//!
//! This module provides helper functions for creating common Haxe diagnostics

use crate::{Diagnostic, DiagnosticBuilder, SourceSpan};

/// Provides common Haxe diagnostic builders
pub struct HaxeDiagnostics;

impl HaxeDiagnostics {
    /// Missing semicolon diagnostic
    pub fn missing_semicolon(span: SourceSpan, after_what: &str) -> Diagnostic {
        DiagnosticBuilder::error(
            format!("missing ';' at the end of {}", after_what),
            span.clone(),
        )
        .code("E0002")
        .label(span.clone(), "expected ';' here")
        .suggestion("add semicolon", span, ";".to_string())
        .help(format!("{} must end with a semicolon", after_what))
        .build()
    }

    /// Missing closing delimiter
    pub fn missing_closing_delimiter(
        span: SourceSpan,
        opening_span: SourceSpan,
        delimiter: char,
    ) -> Diagnostic {
        let closing = match delimiter {
            '(' => ')',
            '{' => '}',
            '[' => ']',
            _ => delimiter,
        };

        DiagnosticBuilder::error(format!("missing closing '{}'", closing), span.clone())
            .code("E0003")
            .label(span.clone(), format!("expected '{}' here", closing))
            .secondary_label(opening_span, format!("opening '{}' here", delimiter))
            .suggestion(format!("add '{}'", closing), span, closing.to_string())
            .help("delimiters must be properly matched".to_string())
            .build()
    }

    /// Unexpected token
    pub fn unexpected_token(span: SourceSpan, found: &str, expected: &[String]) -> Diagnostic {
        let expected_str = if expected.len() == 1 {
            expected[0].clone()
        } else if expected.len() == 2 {
            format!("{} or {}", expected[0], expected[1])
        } else {
            let last = expected.last().unwrap();
            let others = expected[..expected.len() - 1].join(", ");
            format!("{}, or {}", others, last)
        };

        DiagnosticBuilder::error(format!("unexpected token '{}'", found), span.clone())
            .code("E0001")
            .label(span, format!("expected {}", expected_str))
            .help("check the syntax of your code")
            .build()
    }

    /// Invalid identifier
    pub fn invalid_identifier(span: SourceSpan, found: &str, reason: &str) -> Diagnostic {
        let mut builder =
            DiagnosticBuilder::error(format!("invalid identifier '{}'", found), span.clone())
                .code("E0004")
                .label(span.clone(), reason);

        // Add suggestions for common typos
        if let Some(suggestion) = suggest_identifier(found) {
            builder = builder.suggestion(
                format!("did you mean '{}'?", suggestion),
                span,
                suggestion.to_string(),
            );
        }

        builder.build()
    }

    /// Missing type annotation
    pub fn missing_type_annotation(span: SourceSpan, context: &str) -> Diagnostic {
        DiagnosticBuilder::warning(
            format!("missing type annotation for {}", context),
            span.clone(),
        )
        .code("W0001")
        .label(span, "consider adding a type annotation")
        .help("explicit type annotations improve code readability and catch errors early")
        .note("Haxe can infer types, but explicit annotations are recommended")
        .build()
    }

    /// Unused import
    pub fn unused_import(span: SourceSpan, import_path: &str) -> Diagnostic {
        DiagnosticBuilder::warning(format!("unused import '{}'", import_path), span.clone())
            .code("W0002")
            .label(span.clone(), "import is not used")
            .suggestion("remove this import", span, "".to_string())
            .build()
    }

    /// Missing import for type
    pub fn missing_import(
        span: SourceSpan,
        type_name: &str,
        suggested_import: Option<&str>,
    ) -> Diagnostic {
        let mut builder = DiagnosticBuilder::warning(
            format!("type '{}' is not imported", type_name),
            span.clone(),
        )
        .code("W0003")
        .label(span, format!("'{}' used here", type_name));

        if let Some(import) = suggested_import {
            builder = builder.help(format!("add 'import {};' to the file", import));
        }

        builder.build()
    }

    /// Deprecated feature
    pub fn deprecated_feature(
        span: SourceSpan,
        feature: &str,
        alternative: Option<&str>,
    ) -> Diagnostic {
        let mut builder =
            DiagnosticBuilder::warning(format!("use of deprecated {}", feature), span.clone())
                .code("W0004")
                .label(span, "deprecated");

        if let Some(alt) = alternative {
            builder = builder.help(format!("use {} instead", alt));
        }

        builder.build()
    }

    /// Type mismatch
    pub fn type_mismatch(span: SourceSpan, expected: &str, found: &str) -> Diagnostic {
        DiagnosticBuilder::error("type mismatch", span.clone())
            .code("E0020")
            .label(span, format!("expected '{}', found '{}'", expected, found))
            .help("ensure the types match or add an explicit cast")
            .build()
    }

    /// Unreachable code
    pub fn unreachable_code(span: SourceSpan) -> Diagnostic {
        DiagnosticBuilder::warning("unreachable code", span.clone())
            .code("W0005")
            .label(span, "this code will never be executed")
            .help("remove the unreachable code or fix the control flow")
            .build()
    }

    /// Naming convention violation
    pub fn naming_convention(
        span: SourceSpan,
        kind: &str,
        name: &str,
        expected_style: &str,
    ) -> Diagnostic {
        DiagnosticBuilder::info(
            format!("{} '{}' does not follow naming convention", kind, name),
            span.clone(),
        )
        .code("I0001")
        .label(span.clone(), format!("should be {}", expected_style))
        .suggestion(
            format!("rename to follow {} convention", expected_style),
            span,
            to_convention(name, expected_style),
        )
        .note(format!("{} names should be {}", kind, expected_style))
        .build()
    }
}

/// Suggest corrections for common identifier typos
fn suggest_identifier(found: &str) -> Option<&'static str> {
    match found {
        "fucntion" | "funtion" | "functoin" => Some("function"),
        "calss" | "clas" => Some("class"),
        "interfcae" | "interfac" => Some("interface"),
        "pacakge" | "packge" => Some("package"),
        "improt" | "imoprt" => Some("import"),
        "usign" | "usnig" => Some("using"),
        "retrun" | "retunr" => Some("return"),
        "swtich" | "swithc" => Some("switch"),
        "defualt" | "defautl" => Some("default"),
        "braek" | "brak" => Some("break"),
        "contiune" | "contnue" => Some("continue"),
        _ => None,
    }
}

/// Convert a name to a naming convention
fn to_convention(name: &str, convention: &str) -> String {
    match convention {
        "camelCase" => to_camel_case(name),
        "PascalCase" => to_pascal_case(name),
        "snake_case" => to_snake_case(name),
        "UPPER_CASE" => name.to_uppercase(),
        _ => name.to_string(),
    }
}

fn to_camel_case(s: &str) -> String {
    let parts: Vec<&str> = s.split('_').collect();
    if parts.is_empty() {
        return String::new();
    }

    let mut result = parts[0].to_lowercase();
    for part in &parts[1..] {
        if !part.is_empty() {
            result.push_str(&capitalize(part));
        }
    }
    result
}

fn to_pascal_case(s: &str) -> String {
    s.split('_').map(capitalize).collect::<Vec<_>>().join("")
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_is_lower = false;

    for ch in s.chars() {
        if ch.is_uppercase() && prev_is_lower {
            result.push('_');
        }
        result.push(ch.to_lowercase().next().unwrap());
        prev_is_lower = ch.is_lowercase();
    }

    result
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiagnosticSeverity, FileId, SourcePosition};

    #[test]
    fn test_missing_semicolon() {
        let span = SourceSpan::new(
            SourcePosition::new(1, 10, 9),
            SourcePosition::new(1, 11, 10),
            FileId::new(0),
        );

        let diagnostic = HaxeDiagnostics::missing_semicolon(span, "variable declaration");

        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.code, Some("E0002".to_string()));
        assert!(diagnostic.message.contains("variable declaration"));
        assert!(!diagnostic.suggestions.is_empty());
    }

    #[test]
    fn test_suggest_identifier() {
        assert_eq!(suggest_identifier("fucntion"), Some("function"));
        assert_eq!(suggest_identifier("calss"), Some("class"));
        assert_eq!(suggest_identifier("retrun"), Some("return"));
        assert_eq!(suggest_identifier("unknown"), None);
    }

    #[test]
    fn test_naming_conventions() {
        assert_eq!(to_camel_case("hello_world"), "helloWorld");
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_snake_case("helloWorld"), "hello_world");
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
    }
}
