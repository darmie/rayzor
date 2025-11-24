//! Human-friendly error formatting for enhanced developer experience
//! 
//! This module provides sophisticated error formatting that includes:
//! - Source code highlighting with error spans
//! - Multi-line error displays with context
//! - Color coding for different error severities  
//! - Helpful suggestions and "did you mean?" features

use crate::error::{ParseError, ParseErrors, SourceMap, SourceSpan, ErrorSeverity};
use std::fmt;

/// ANSI color codes for terminal output
pub struct Colors {
    pub reset: &'static str,
    pub bold: &'static str,
    pub red: &'static str,
    pub yellow: &'static str,
    pub blue: &'static str,
    pub cyan: &'static str,
    pub white: &'static str,
    pub dim: &'static str,
}

impl Colors {
    pub const ENABLED: Colors = Colors {
        reset: "\x1b[0m",
        bold: "\x1b[1m",
        red: "\x1b[31m",
        yellow: "\x1b[33m", 
        blue: "\x1b[34m",
        cyan: "\x1b[36m",
        white: "\x1b[37m",
        dim: "\x1b[2m",
    };
    
    pub const DISABLED: Colors = Colors {
        reset: "",
        bold: "",
        red: "",
        yellow: "",
        blue: "",
        cyan: "",
        white: "",
        dim: "",
    };
    
    pub fn for_severity(&self, severity: ErrorSeverity) -> &'static str {
        match severity {
            ErrorSeverity::Error => self.red,
            ErrorSeverity::Warning => self.yellow,
            ErrorSeverity::Info => self.blue,
            ErrorSeverity::Hint => self.cyan,
        }
    }
}

/// Configuration for error formatting
#[derive(Debug, Clone)]
pub struct FormatConfig {
    pub use_colors: bool,
    pub show_line_numbers: bool,
    pub context_lines: usize,
    pub max_line_length: usize,
    pub tab_width: usize,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            use_colors: true,
            show_line_numbers: true,
            context_lines: 2,
            max_line_length: 120,
            tab_width: 4,
        }
    }
}

/// Formats parse errors with rich, Rust-like output
pub struct ErrorFormatter {
    config: FormatConfig,
    colors: Colors,
}

impl ErrorFormatter {
    pub fn new(config: FormatConfig) -> Self {
        let colors = if config.use_colors {
            Colors::ENABLED
        } else {
            Colors::DISABLED
        };
        
        Self { config, colors }
    }
    
    pub fn with_colors() -> Self {
        Self::new(FormatConfig {
            use_colors: true,
            ..Default::default()
        })
    }
    
    pub fn without_colors() -> Self {
        Self::new(FormatConfig {
            use_colors: false,
            ..Default::default()
        })
    }
    
    /// Format a single error with source highlighting
    pub fn format_error(&self, error: &ParseError, source_map: &SourceMap) -> String {
        let mut output = String::new();
        
        // Error header with severity and message
        self.write_error_header(&mut output, error);
        
        // Source location and file info
        self.write_source_location(&mut output, error.span(), source_map);
        
        // Source code with highlighting
        self.write_source_snippet(&mut output, error.span(), source_map);
        
        // Error-specific details and suggestions
        self.write_error_details(&mut output, error, source_map);
        
        output
    }
    
    /// Format multiple errors together
    pub fn format_errors(&self, errors: &ParseErrors, source_map: &SourceMap) -> String {
        let mut output = String::new();
        
        for (i, error) in errors.errors.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            output.push_str(&self.format_error(error, source_map));
        }
        
        // Summary line
        if errors.len() > 1 {
            output.push('\n');
            self.write_error_summary(&mut output, errors);
        }
        
        output
    }
    
    fn write_error_header(&self, output: &mut String, error: &ParseError) {
        let severity = error.severity();
        let color = self.colors.for_severity(severity);
        
        output.push_str(&format!(
            "{}{}{}: {}{}",
            color,
            self.colors.bold,
            severity,
            self.colors.reset,
            self.error_message(error)
        ));
        output.push('\n');
    }
    
    fn write_source_location(&self, output: &mut String, span: &SourceSpan, source_map: &SourceMap) {
        if let Some(file) = source_map.get_file(span.file_id) {
            output.push_str(&format!(
                "{}  --> {}{}:{}:{}{}",
                self.colors.blue,
                self.colors.reset,
                file.name,
                span.start.line,
                span.start.column,
                self.colors.reset
            ));
            output.push('\n');
        }
    }
    
    fn write_source_snippet(&self, output: &mut String, span: &SourceSpan, source_map: &SourceMap) {
        let file = match source_map.get_file(span.file_id) {
            Some(file) => file,
            None => return,
        };
        
        let start_line = span.start.line.saturating_sub(self.config.context_lines).max(1);
        let end_line = (span.end.line + self.config.context_lines).min(file.line_starts.len());
        
        // Calculate line number width for alignment
        let line_num_width = end_line.to_string().len();
        
        output.push_str(&format!("{}   |{}\n", self.colors.blue, self.colors.reset));
        
        for line_num in start_line..=end_line {
            if let Some(line_text) = source_map.get_line(span.file_id, line_num) {
                // Expand tabs for consistent display
                let expanded_line = self.expand_tabs(line_text);
                
                if line_num >= span.start.line && line_num <= span.end.line {
                    // This line contains part of the error span
                    self.write_error_line(output, line_num, &expanded_line, span, line_num_width);
                } else {
                    // Context line
                    self.write_context_line(output, line_num, &expanded_line, line_num_width);
                }
            }
        }
        
        output.push_str(&format!("{}   |{}\n", self.colors.blue, self.colors.reset));
    }
    
    fn write_error_line(&self, output: &mut String, line_num: usize, line_text: &str, span: &SourceSpan, width: usize) {
        // Line number and content
        output.push_str(&format!(
            "{}{:width$} |{} {}",
            self.colors.blue,
            line_num,
            self.colors.reset,
            line_text,
            width = width
        ));
        output.push('\n');
        
        // Error highlighting line
        let start_col = if line_num == span.start.line {
            span.start.column.saturating_sub(1)
        } else {
            0
        };
        
        let end_col = if line_num == span.end.line {
            span.end.column.saturating_sub(1)
        } else {
            line_text.len()
        };
        
        if start_col < end_col {
            let highlight_len = end_col - start_col;
            let highlight_char = if highlight_len == 1 { '^' } else { '~' };
            
            output.push_str(&format!(
                "{}{:width$} |{} {:pad$}{}{}{}",
                self.colors.blue,
                "",
                self.colors.reset,
                "",
                self.colors.red,
                highlight_char.to_string().repeat(highlight_len),
                self.colors.reset,
                width = width,
                pad = start_col
            ));
            output.push('\n');
        }
    }
    
    fn write_context_line(&self, output: &mut String, line_num: usize, line_text: &str, width: usize) {
        output.push_str(&format!(
            "{}{:width$} |{} {}",
            self.colors.blue,
            line_num,
            self.colors.reset,
            line_text,
            width = width
        ));
        output.push('\n');
    }
    
    fn write_error_details(&self, output: &mut String, error: &ParseError, _source_map: &SourceMap) {
        match error {
            ParseError::UnexpectedToken { expected, found, .. } => {
                if !expected.is_empty() {
                    output.push_str(&format!(
                        "{}   = help:{} expected {}",
                        self.colors.cyan,
                        self.colors.reset,
                        self.format_expected_list(expected)
                    ));
                    output.push('\n');
                }
            }
            
            ParseError::UnclosedDelimiter { delimiter, .. } => {
                let closing_delimiter = self.get_closing_delimiter(*delimiter);
                let delimiter_name = self.get_delimiter_name(*delimiter);
                output.push_str(&format!(
                    "{}   = help:{} add a closing {} `{}` to match this opening {}",
                    self.colors.cyan,
                    self.colors.reset,
                    delimiter_name,
                    closing_delimiter,
                    delimiter_name
                ));
                output.push('\n');
            }
            
            ParseError::MissingToken { suggestion: Some(suggestion), .. } => {
                output.push_str(&format!(
                    "{}   = help:{} {}",
                    self.colors.cyan,
                    self.colors.reset,
                    suggestion
                ));
                output.push('\n');
            }
            
            ParseError::InvalidIdentifier { suggestion: Some(suggestion), .. } => {
                output.push_str(&format!(
                    "{}   = help:{} did you mean `{}`?",
                    self.colors.cyan,
                    self.colors.reset,
                    suggestion
                ));
                output.push('\n');
            }
            
            ParseError::InvalidPattern { expected_patterns, .. } if !expected_patterns.is_empty() => {
                output.push_str(&format!(
                    "{}   = help:{} valid patterns are: {}",
                    self.colors.cyan,
                    self.colors.reset,
                    expected_patterns.join(", ")
                ));
                output.push('\n');
            }
            
            _ => {}
        }
    }
    
    fn write_error_summary(&self, output: &mut String, errors: &ParseErrors) {
        let error_count = errors.errors().count();
        let warning_count = errors.warnings().count();
        
        let mut parts = Vec::new();
        
        if error_count > 0 {
            parts.push(format!(
                "{}{} error{}{}",
                self.colors.red,
                error_count,
                if error_count == 1 { "" } else { "s" },
                self.colors.reset
            ));
        }
        
        if warning_count > 0 {
            parts.push(format!(
                "{}{} warning{}{}",
                self.colors.yellow,
                warning_count,
                if warning_count == 1 { "" } else { "s" },
                self.colors.reset
            ));
        }
        
        if !parts.is_empty() {
            output.push_str(&format!("aborting due to {}", parts.join(" and ")));
            output.push('\n');
        }
    }
    
    fn error_message(&self, error: &ParseError) -> String {
        match error {
            ParseError::UnexpectedToken { expected, found, .. } => {
                if expected.is_empty() {
                    format!("unexpected token `{}`", found)
                } else if expected.len() == 1 {
                    format!("expected `{}`, found `{}`", expected[0], found)
                } else {
                    format!("unexpected token `{}`", found)
                }
            }
            
            ParseError::UnclosedDelimiter { delimiter, opened_at, .. } => {
                let closing_delimiter = self.get_closing_delimiter(*delimiter);
                let delimiter_name = self.get_delimiter_name(*delimiter);
                format!("missing closing {} `{}` to match `{}` opened at {}:{}", 
                    delimiter_name,
                    closing_delimiter, 
                    delimiter,
                    opened_at.start.line,
                    opened_at.start.column
                )
            }
            
            ParseError::InvalidSyntax { message, .. } => {
                message.clone()
            }
            
            ParseError::MissingToken { expected, .. } => {
                format!("expected `{}`", expected)
            }
            
            ParseError::InvalidPattern { pattern, .. } => {
                format!("invalid pattern `{}`", pattern)
            }
            
            ParseError::UnexpectedEof { expected, .. } => {
                if expected.len() == 1 {
                    format!("unexpected end of file, expected `{}`", expected[0])
                } else {
                    "unexpected end of file".to_string()
                }
            }
            
            ParseError::InvalidNumber { value, reason, .. } => {
                format!("invalid number `{}`: {}", value, reason)
            }
            
            ParseError::InvalidString { reason, .. } => {
                format!("invalid string literal: {}", reason)
            }
            
            ParseError::InvalidIdentifier { name, reason, .. } => {
                format!("invalid identifier `{}`: {}", name, reason)
            }
            
            ParseError::InvalidType { type_text, reason, .. } => {
                format!("invalid type `{}`: {}", type_text, reason)
            }
            
            ParseError::NomError { kind, context, .. } => {
                if let Some(ctx) = context {
                    format!("parse error in {}: {:?}", ctx, kind)
                } else {
                    format!("parse error: {:?}", kind)
                }
            }
        }
    }
    
    fn format_expected_list(&self, expected: &[String]) -> String {
        match expected.len() {
            0 => "nothing".to_string(),
            1 => format!("`{}`", expected[0]),
            2 => format!("`{}` or `{}`", expected[0], expected[1]),
            _ => {
                let (last, rest) = expected.split_last().unwrap();
                format!("{}, or `{}`", 
                    rest.iter().map(|s| format!("`{}`", s)).collect::<Vec<_>>().join(", "),
                    last
                )
            }
        }
    }
    
    fn expand_tabs(&self, line: &str) -> String {
        let mut result = String::new();
        let mut col = 0;
        
        for ch in line.chars() {
            if ch == '\t' {
                let spaces = self.config.tab_width - (col % self.config.tab_width);
                result.extend(std::iter::repeat(' ').take(spaces));
                col += spaces;
            } else {
                result.push(ch);
                col += 1;
            }
        }
        
        result
    }
    
    /// Map opening delimiter to its closing counterpart
    fn get_closing_delimiter(&self, opening: char) -> char {
        match opening {
            '{' => '}',
            '(' => ')',
            '[' => ']',
            '<' => '>',
            _ => opening, // Fallback to the same character
        }
    }
    
    /// Get a human-readable name for the delimiter
    fn get_delimiter_name(&self, delimiter: char) -> &'static str {
        match delimiter {
            '{' => "brace",
            '(' => "parenthesis", 
            '[' => "bracket",
            '<' => "angle bracket",
            _ => "delimiter",
        }
    }
}

impl Default for ErrorFormatter {
    fn default() -> Self {
        Self::new(FormatConfig::default())
    }
}

/// Helper for creating common error messages with suggestions
pub struct ErrorHelpers;

impl ErrorHelpers {
    /// Suggest corrections for common typos in keywords
    pub fn suggest_keyword(input: &str) -> Option<String> {
        let suggestions = [
            ("fucntion", "function"),
            ("classe", "class"),
            ("publik", "public"),
            ("privat", "private"),
            ("statik", "static"),
            ("overrid", "override"),
            ("abstrak", "abstract"),
            ("interfac", "interface"),
            ("extend", "extends"),
            ("implemen", "implements"),
            ("packg", "package"),
            ("imprt", "import"),
            ("varr", "var"),
            ("finall", "final"),
            ("retur", "return"),
            ("swittch", "switch"),
            ("whil", "while"),
            ("forr", "for"),
            ("iff", "if"),
            ("els", "else"),
            ("tru", "true"),
            ("fals", "false"),
            ("nul", "null"),
        ];
        
        // First check exact matches (case insensitive)
        for (typo, correct) in &suggestions {
            if input.to_lowercase() == *typo {
                return Some(correct.to_string());
            }
        }
        
        // Then check edit distance for close matches
        Self::closest_match(input, &[
            "function", "class", "public", "private", "static", "override",
            "abstract", "interface", "extends", "implements", "package", 
            "import", "var", "final", "return", "switch", "while", "for",
            "if", "else", "true", "false", "null", "this", "super"
        ])
    }
    
    /// Find the closest match using simple edit distance
    fn closest_match(input: &str, candidates: &[&str]) -> Option<String> {
        let mut best_match = None;
        let mut best_distance = input.len() / 2; // Only suggest if reasonably close
        
        for candidate in candidates {
            let distance = Self::edit_distance(input, candidate);
            if distance < best_distance {
                best_distance = distance;
                best_match = Some(candidate.to_string());
            }
        }
        
        best_match
    }
    
    /// Simple Levenshtein distance calculation
    fn edit_distance(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let a_len = a_chars.len();
        let b_len = b_chars.len();
        
        let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];
        
        for i in 0..=a_len {
            matrix[i][0] = i;
        }
        for j in 0..=b_len {
            matrix[0][j] = j;
        }
        
        for i in 1..=a_len {
            for j in 1..=b_len {
                let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }
        
        matrix[a_len][b_len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{SourceMap, FileId, SourcePosition, SourceSpan};

    #[test]
    fn test_error_formatter_creation() {
        let formatter = ErrorFormatter::with_colors();
        assert!(formatter.config.use_colors);
        
        let formatter = ErrorFormatter::without_colors();
        assert!(!formatter.config.use_colors);
    }

    #[test]
    fn test_keyword_suggestions() {
        assert_eq!(ErrorHelpers::suggest_keyword("fucntion"), Some("function".to_string()));
        assert_eq!(ErrorHelpers::suggest_keyword("classe"), Some("class".to_string()));
        assert_eq!(ErrorHelpers::suggest_keyword("xyz123"), None);
    }

    #[test]
    fn test_edit_distance() {
        assert_eq!(ErrorHelpers::edit_distance("cat", "cat"), 0);
        assert_eq!(ErrorHelpers::edit_distance("cat", "bat"), 1);
        assert_eq!(ErrorHelpers::edit_distance("", "abc"), 3);
        assert_eq!(ErrorHelpers::edit_distance("abc", ""), 3);
    }

    #[test]
    fn test_tab_expansion() {
        let formatter = ErrorFormatter::default();
        // "hello" = 5 chars, so tab should add 3 spaces to reach next tab stop (8)
        assert_eq!(formatter.expand_tabs("hello\tworld"), "hello   world");
        assert_eq!(formatter.expand_tabs("\t\t"), "        ");
    }

    #[test]
    fn test_format_expected_list() {
        let formatter = ErrorFormatter::default();
        assert_eq!(formatter.format_expected_list(&[]), "nothing");
        assert_eq!(formatter.format_expected_list(&["x".to_string()]), "`x`");
        assert_eq!(formatter.format_expected_list(&["x".to_string(), "y".to_string()]), "`x` or `y`");
        assert_eq!(
            formatter.format_expected_list(&["x".to_string(), "y".to_string(), "z".to_string()]), 
            "`x`, `y`, or `z`"
        );
    }
}
