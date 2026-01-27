//! Diagnostics library for rich error reporting
//!
//! This library provides Rust-style diagnostics with:
//! - Multiple severity levels (Error, Warning, Info, Hint)
//! - Source code snippets with highlighting
//! - Suggestions with applicability levels
//! - Multi-file source map support
//! - Colored terminal output

use std::fmt;

// Re-export source mapping types from the source_map crate
pub use source_map::{FileId, SourceFile, SourceMap, SourcePosition, SourceSpan};

/// Severity level for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticSeverity::Error => write!(f, "error"),
            DiagnosticSeverity::Warning => write!(f, "warning"),
            DiagnosticSeverity::Info => write!(f, "info"),
            DiagnosticSeverity::Hint => write!(f, "hint"),
        }
    }
}

/// Style for diagnostic labels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStyle {
    Primary,
    Secondary,
}

/// A label that points to a span of code
#[derive(Debug, Clone)]
pub struct Label {
    pub span: SourceSpan,
    pub message: String,
    pub style: LabelStyle,
}

impl Label {
    pub fn primary(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            style: LabelStyle::Primary,
        }
    }

    pub fn secondary(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            style: LabelStyle::Secondary,
        }
    }
}

/// Applicability level for suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    MachineApplicable,
    HasPlaceholders,
    MaybeIncorrect,
    Unspecified,
}

/// A suggestion for fixing an issue
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub message: String,
    pub span: SourceSpan,
    pub replacement: String,
    pub applicability: Applicability,
}

/// A diagnostic message with severity, labels, and suggestions
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: Option<String>,
    pub message: String,
    pub span: SourceSpan,
    pub labels: Vec<Label>,
    pub suggestions: Vec<Suggestion>,
    pub notes: Vec<String>,
    pub help: Vec<String>,
}

/// Collection of diagnostics
#[derive(Debug, Clone, Default)]
pub struct Diagnostics {
    pub diagnostics: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn extend(&mut self, other: Diagnostics) {
        self.diagnostics.extend(other.diagnostics);
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
    }

    pub fn infos(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Info)
    }

    pub fn hints(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Hint)
    }
}

/// Builder for creating diagnostics
pub struct DiagnosticBuilder {
    severity: DiagnosticSeverity,
    code: Option<String>,
    message: String,
    span: SourceSpan,
    labels: Vec<Label>,
    suggestions: Vec<Suggestion>,
    notes: Vec<String>,
    help: Vec<String>,
}

impl DiagnosticBuilder {
    pub fn error(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: None,
            message: message.into(),
            span,
            labels: vec![],
            suggestions: vec![],
            notes: vec![],
            help: vec![],
        }
    }

    pub fn warning(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code: None,
            message: message.into(),
            span,
            labels: vec![],
            suggestions: vec![],
            notes: vec![],
            help: vec![],
        }
    }

    pub fn info(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            severity: DiagnosticSeverity::Info,
            code: None,
            message: message.into(),
            span,
            labels: vec![],
            suggestions: vec![],
            notes: vec![],
            help: vec![],
        }
    }

    pub fn hint(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            severity: DiagnosticSeverity::Hint,
            code: None,
            message: message.into(),
            span,
            labels: vec![],
            suggestions: vec![],
            notes: vec![],
            help: vec![],
        }
    }

    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn label(mut self, span: SourceSpan, message: impl Into<String>) -> Self {
        self.labels.push(Label::primary(span, message));
        self
    }

    pub fn secondary_label(mut self, span: SourceSpan, message: impl Into<String>) -> Self {
        self.labels.push(Label::secondary(span, message));
        self
    }

    pub fn suggestion(
        mut self,
        message: impl Into<String>,
        span: SourceSpan,
        replacement: impl Into<String>,
    ) -> Self {
        self.suggestions.push(Suggestion {
            message: message.into(),
            span,
            replacement: replacement.into(),
            applicability: Applicability::MachineApplicable,
        });
        self
    }

    pub fn suggestion_with_applicability(
        mut self,
        message: impl Into<String>,
        span: SourceSpan,
        replacement: impl Into<String>,
        applicability: Applicability,
    ) -> Self {
        self.suggestions.push(Suggestion {
            message: message.into(),
            span,
            replacement: replacement.into(),
            applicability,
        });
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn help(mut self, help_msg: impl Into<String>) -> Self {
        self.help.push(help_msg.into());
        self
    }

    pub fn build(self) -> Diagnostic {
        Diagnostic {
            severity: self.severity,
            code: self.code,
            message: self.message,
            span: self.span,
            labels: self.labels,
            suggestions: self.suggestions,
            notes: self.notes,
            help: self.help,
        }
    }
}

/// Formatter for displaying diagnostics
pub struct ErrorFormatter {
    use_colors: bool,
}

impl ErrorFormatter {
    pub fn new() -> Self {
        Self { use_colors: false }
    }

    pub fn with_colors() -> Self {
        Self { use_colors: true }
    }

    pub fn format_diagnostics(&self, diagnostics: &Diagnostics, source_map: &SourceMap) -> String {
        let mut output = String::new();

        for (i, diagnostic) in diagnostics.diagnostics.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            output.push_str(&self.format_diagnostic(diagnostic, source_map));
        }

        output
    }

    pub fn format_diagnostic(&self, diagnostic: &Diagnostic, source_map: &SourceMap) -> String {
        let mut output = String::new();

        // Add blank line before error for better separation
        output.push('\n');

        // Header
        if self.use_colors {
            let color = match diagnostic.severity {
                DiagnosticSeverity::Error => "\x1b[31m",
                DiagnosticSeverity::Warning => "\x1b[33m",
                DiagnosticSeverity::Info => "\x1b[36m",
                DiagnosticSeverity::Hint => "\x1b[32m",
            };
            output.push_str(color);
            output.push_str(&format!("{}", diagnostic.severity));

            if let Some(code) = &diagnostic.code {
                output.push_str(&format!("[{}]", code));
            }

            // Use white/bright color for the message to make it stand out
            output.push_str("\x1b[0m: \x1b[1;97m");
            output.push_str(&diagnostic.message);
            output.push_str("\x1b[0m\n");
        } else {
            output.push_str(&format!("{}", diagnostic.severity));

            if let Some(code) = &diagnostic.code {
                output.push_str(&format!("[{}]", code));
            }

            output.push_str(&format!(": {}\n", diagnostic.message));
        }

        // Source location
        if let Some(file) = source_map.get_file(diagnostic.span.file_id) {
            if self.use_colors {
                output.push_str(&format!(
                    "  \x1b[96m-->\x1b[0m {}:{}:{}\n",
                    file.name, diagnostic.span.start.line, diagnostic.span.start.column
                ));
            } else {
                output.push_str(&format!(
                    "  --> {}:{}:{}\n",
                    file.name, diagnostic.span.start.line, diagnostic.span.start.column
                ));
            }

            // Source snippet
            let line_num = diagnostic.span.start.line;
            let line_num_width = line_num.to_string().len();

            // Blank line
            if self.use_colors {
                output.push_str(&format!(
                    "{:width$} \x1b[96m|\x1b[0m\n",
                    "",
                    width = line_num_width
                ));
            } else {
                output.push_str(&format!("{:width$} |\n", "", width = line_num_width));
            }

            // Source line
            if let Some(line) = source_map.get_line(diagnostic.span.file_id, line_num) {
                if self.use_colors {
                    output.push_str(&format!(
                        "\x1b[96m{}\x1b[0m \x1b[96m|\x1b[0m {}\n",
                        line_num, line
                    ));
                } else {
                    output.push_str(&format!("{} | {}\n", line_num, line));
                }

                // Underline
                let padding = " ".repeat(diagnostic.span.start.column - 1);
                let mut underline_len = if diagnostic.span.start.line == diagnostic.span.end.line {
                    diagnostic.span.end.column - diagnostic.span.start.column
                } else {
                    // For multi-line spans, underline from start column to end of line
                    if diagnostic.span.start.column > 0
                        && line.len() >= diagnostic.span.start.column - 1
                    {
                        line.len() - (diagnostic.span.start.column - 1)
                    } else {
                        1
                    }
                };

                // If underline_len is 0 or 1 (single position), try to detect the token length from source
                if underline_len <= 1 && diagnostic.span.start.column > 0 {
                    let start_col = diagnostic.span.start.column - 1; // Convert to 0-based
                    if start_col < line.len() {
                        let remaining = &line[start_col..];
                        // Find the length of the identifier (alphanumeric + underscore)
                        let detected_len = remaining
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                            .count();
                        if detected_len >= 1 {
                            // Changed from > 1 to >= 1
                            underline_len = detected_len;
                        }
                    }
                }

                let underline = if self.use_colors {
                    format!("\x1b[31m{}\x1b[0m", "^".repeat(underline_len.max(1)))
                } else {
                    "^".repeat(underline_len.max(1))
                };

                if self.use_colors {
                    output.push_str(&format!(
                        "{:width$} \x1b[96m|\x1b[0m {}{}",
                        "",
                        padding,
                        underline,
                        width = line_num_width
                    ));
                } else {
                    output.push_str(&format!(
                        "{:width$} | {}{}",
                        "",
                        padding,
                        underline,
                        width = line_num_width
                    ));
                }

                // Primary label message - underlined and bold for visibility
                if let Some(label) = diagnostic
                    .labels
                    .iter()
                    .find(|l| l.style == LabelStyle::Primary)
                {
                    if self.use_colors {
                        // Bold + underlined red text for the error message
                        output.push_str(&format!(" \x1b[1;4;31m{}\x1b[0m", label.message));
                    } else {
                        output.push_str(&format!(" {}", label.message));
                    }
                }
                output.push('\n');
            }
        }

        // Additional labels
        for label in &diagnostic.labels {
            if label.style == LabelStyle::Secondary
                && let Some(file) = source_map.get_file(label.span.file_id)
            {
                if self.use_colors {
                    output.push_str(&format!(
                        "  \x1b[96m-->\x1b[0m {}:{}:{}: {}\n",
                        file.name, label.span.start.line, label.span.start.column, label.message
                    ));
                } else {
                    output.push_str(&format!(
                        "  --> {}:{}:{}: {}\n",
                        file.name, label.span.start.line, label.span.start.column, label.message
                    ));
                }
            }
        }

        // Suggestions
        for suggestion in &diagnostic.suggestions {
            output.push('\n');
            if self.use_colors {
                output.push_str("\x1b[38;5;208msuggestion\x1b[0m: ");
            } else {
                output.push_str("suggestion: ");
            }
            output.push_str(&suggestion.message);
            output.push('\n');
        }

        // Help messages - indented for better readability with yellow/golden color
        for help_msg in &diagnostic.help {
            if self.use_colors {
                // Green 'help:' label, yellow/golden message text
                output.push_str("     \x1b[32mhelp\x1b[0m: \x1b[33m");
                output.push_str(help_msg);
                output.push_str("\x1b[0m\n");
            } else {
                output.push_str("     help: ");
                output.push_str(help_msg);
                output.push('\n');
            }
        }

        // Notes
        for note in &diagnostic.notes {
            if self.use_colors {
                output.push_str("\x1b[34mnote\x1b[0m: ");
            } else {
                output.push_str("note: ");
            }
            output.push_str(note);
            output.push('\n');
        }

        output
    }
}

impl Default for ErrorFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Result type that includes diagnostics
pub type DiagnosticResult<T> = Result<T, Diagnostics>;

// Haxe-specific diagnostics
pub mod haxe;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_map() {
        let mut source_map = SourceMap::new();
        let file_id =
            source_map.add_file("test.hx".to_string(), "line 1\nline 2\nline 3".to_string());

        assert_eq!(source_map.get_line(file_id, 1), Some("line 1"));
        assert_eq!(source_map.get_line(file_id, 2), Some("line 2"));
        assert_eq!(source_map.get_line(file_id, 3), Some("line 3"));
        assert_eq!(source_map.get_line(file_id, 4), None);
    }

    #[test]
    fn test_diagnostic_builder() {
        let span = SourceSpan::new(
            SourcePosition::new(1, 5, 4),
            SourcePosition::new(1, 6, 5),
            FileId::new(0),
        );

        let diagnostic = DiagnosticBuilder::error("test error", span.clone())
            .code("E0001")
            .label(span, "here")
            .help("try this")
            .note("additional info")
            .build();

        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.code, Some("E0001".to_string()));
        assert_eq!(diagnostic.message, "test error");
        assert_eq!(diagnostic.labels.len(), 1);
        assert_eq!(diagnostic.help.len(), 1);
        assert_eq!(diagnostic.notes.len(), 1);
    }
}
