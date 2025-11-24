//! Enhanced error handling and reporting for the Haxe parser
//! 
//! This module provides:
//! - Rich error types with context and suggestions
//! - Source position tracking with line/column information
//! - Rust-like error formatting with source highlighting
//! - Error recovery capabilities

use std::fmt;
use std::collections::HashMap;

/// Represents a position in source code
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourcePosition {
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based) 
    pub column: usize,
    /// Byte offset from start of file
    pub byte_offset: usize,
}

impl SourcePosition {
    pub fn new(line: usize, column: usize, byte_offset: usize) -> Self {
        Self {
            line,
            column,
            byte_offset,
        }
    }
    
    pub fn start() -> Self {
        Self::new(1, 1, 0)
    }
}

impl fmt::Display for SourcePosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Represents a span of source code from start to end position
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
    pub file_id: FileId,
}

impl SourceSpan {
    pub fn new(start: SourcePosition, end: SourcePosition, file_id: FileId) -> Self {
        Self { start, end, file_id }
    }
    
    pub fn single_char(pos: SourcePosition, file_id: FileId) -> Self {
        let end = SourcePosition::new(pos.line, pos.column + 1, pos.byte_offset + 1);
        Self::new(pos, end, file_id)
    }
    
    pub fn to(self, other: SourceSpan) -> Self {
        Self {
            start: self.start,
            end: other.end,
            file_id: self.file_id,
        }
    }
}

/// File identifier for tracking multiple source files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub usize);

impl FileId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}

/// Maps file IDs to their source content and metadata
#[derive(Debug, Default, Clone)]
pub struct SourceMap {
    files: HashMap<FileId, SourceFile>,
    next_file_id: usize,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: FileId,
    pub name: String,
    pub content: String,
    pub line_starts: Vec<usize>, // Byte offsets where each line starts
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_file(&mut self, name: String, content: String) -> FileId {
        let file_id = FileId::new(self.next_file_id);
        self.next_file_id += 1;
        
        let line_starts = self.compute_line_starts(&content);
        let file = SourceFile {
            id: file_id,
            name,
            content,
            line_starts,
        };
        
        self.files.insert(file_id, file);
        file_id
    }
    
    pub fn get_file(&self, file_id: FileId) -> Option<&SourceFile> {
        self.files.get(&file_id)
    }
    
    pub fn get_line(&self, file_id: FileId, line_number: usize) -> Option<&str> {
        let file = self.get_file(file_id)?;
        
        if line_number == 0 || line_number > file.line_starts.len() {
            return None;
        }
        
        let line_start = file.line_starts[line_number - 1];
        let line_end = if line_number < file.line_starts.len() {
            file.line_starts[line_number] - 1 // Exclude the newline
        } else {
            file.content.len()
        };
        
        Some(&file.content[line_start..line_end])
    }
    
    pub fn get_source_text(&self, span: &SourceSpan) -> Option<&str> {
        let file = self.get_file(span.file_id)?;
        let start = span.start.byte_offset;
        let end = span.end.byte_offset.min(file.content.len());
        
        if start <= end {
            Some(&file.content[start..end])
        } else {
            None
        }
    }
    
    fn compute_line_starts(&self, content: &str) -> Vec<usize> {
        let mut line_starts = vec![0]; // First line starts at byte 0
        
        for (i, byte) in content.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(i + 1);
            }
        }
        
        line_starts
    }
    
    pub fn byte_to_position(&self, file_id: FileId, byte_offset: usize) -> Option<SourcePosition> {
        let file = self.get_file(file_id)?;
        
        // Binary search to find the line
        let line_index = match file.line_starts.binary_search(&byte_offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        
        let line = line_index + 1;
        let line_start = file.line_starts[line_index];
        let column = byte_offset - line_start + 1;
        
        Some(SourcePosition::new(line, column, byte_offset))
    }
}

/// Comprehensive error types for Haxe parsing
#[derive(Debug, Clone)]
pub enum ParseError {
    /// Unexpected token encountered
    UnexpectedToken {
        expected: Vec<String>,
        found: String,
        span: SourceSpan,
    },
    
    /// Unclosed delimiter (parentheses, brackets, braces)
    UnclosedDelimiter {
        delimiter: char,
        opened_at: SourceSpan,
        expected_close_at: SourceSpan,
    },
    
    /// Invalid syntax pattern
    InvalidSyntax {
        message: String,
        span: SourceSpan,
        suggestion: Option<String>,
    },
    
    /// Missing expected token (semicolon, comma, etc.)
    MissingToken {
        expected: String,
        after: SourceSpan,
        suggestion: Option<String>,
    },
    
    /// Invalid pattern in switch case or destructuring
    InvalidPattern {
        pattern: String,
        span: SourceSpan,
        expected_patterns: Vec<String>,
    },
    
    /// Unexpected end of file
    UnexpectedEof {
        expected: Vec<String>,
        span: SourceSpan,
    },
    
    /// Invalid numeric literal
    InvalidNumber {
        value: String,
        span: SourceSpan,
        reason: String,
    },
    
    /// Invalid string literal
    InvalidString {
        span: SourceSpan,
        reason: String,
    },
    
    /// Invalid identifier (reserved keyword, invalid characters)
    InvalidIdentifier {
        name: String,
        span: SourceSpan,
        reason: String,
        suggestion: Option<String>,
    },
    
    /// Type annotation errors
    InvalidType {
        type_text: String,
        span: SourceSpan,
        reason: String,
    },
    
    /// Generic nom error converted to our system
    NomError {
        kind: nom::error::ErrorKind,
        span: SourceSpan,
        context: Option<String>,
    },
}

impl ParseError {
    pub fn span(&self) -> &SourceSpan {
        match self {
            ParseError::UnexpectedToken { span, .. } => span,
            ParseError::UnclosedDelimiter { opened_at, .. } => opened_at,
            ParseError::InvalidSyntax { span, .. } => span,
            ParseError::MissingToken { after, .. } => after,
            ParseError::InvalidPattern { span, .. } => span,
            ParseError::UnexpectedEof { span, .. } => span,
            ParseError::InvalidNumber { span, .. } => span,
            ParseError::InvalidString { span, .. } => span,
            ParseError::InvalidIdentifier { span, .. } => span,
            ParseError::InvalidType { span, .. } => span,
            ParseError::NomError { span, .. } => span,
        }
    }
    
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ParseError::UnexpectedToken { .. } => ErrorSeverity::Error,
            ParseError::UnclosedDelimiter { .. } => ErrorSeverity::Error,
            ParseError::InvalidSyntax { .. } => ErrorSeverity::Error,
            ParseError::MissingToken { .. } => ErrorSeverity::Error,
            ParseError::InvalidPattern { .. } => ErrorSeverity::Error,
            ParseError::UnexpectedEof { .. } => ErrorSeverity::Error,
            ParseError::InvalidNumber { .. } => ErrorSeverity::Error,
            ParseError::InvalidString { .. } => ErrorSeverity::Error,
            ParseError::InvalidIdentifier { .. } => ErrorSeverity::Warning,
            ParseError::InvalidType { .. } => ErrorSeverity::Error,
            ParseError::NomError { .. } => ErrorSeverity::Error,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Error => write!(f, "error"),
            ErrorSeverity::Warning => write!(f, "warning"),
            ErrorSeverity::Info => write!(f, "info"),
            ErrorSeverity::Hint => write!(f, "hint"),
        }
    }
}

/// Collection of parse errors with utilities for reporting
#[derive(Debug, Default)]
pub struct ParseErrors {
    pub errors: Vec<ParseError>,
}

impl ParseErrors {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn single(error: ParseError) -> Self {
        Self {
            errors: vec![error],
        }
    }
    
    pub fn push(&mut self, error: ParseError) {
        self.errors.push(error);
    }
    
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
    
    pub fn len(&self) -> usize {
        self.errors.len()
    }
    
    pub fn has_errors(&self) -> bool {
        self.errors.iter().any(|e| e.severity() == ErrorSeverity::Error)
    }
    
    pub fn errors(&self) -> impl Iterator<Item = &ParseError> {
        self.errors.iter().filter(|e| e.severity() == ErrorSeverity::Error)
    }
    
    pub fn warnings(&self) -> impl Iterator<Item = &ParseError> {
        self.errors.iter().filter(|e| e.severity() == ErrorSeverity::Warning)
    }
}

/// Result type for parsing operations
pub type ParseResult<T> = Result<T, ParseError>;

/// Result type that can accumulate multiple errors
pub type ParseResultMulti<T> = Result<T, ParseErrors>;

/// Helper trait for converting nom errors to our error system
pub trait IntoParseError<I> {
    fn into_parse_error(self, input: I, file_id: FileId, source_map: &SourceMap) -> ParseError;
}

impl IntoParseError<&str> for nom::error::Error<&str> {
    fn into_parse_error(self, input: &str, file_id: FileId, source_map: &SourceMap) -> ParseError {
        // Calculate position from remaining input
        let consumed_bytes = input.as_ptr() as usize - input.as_ptr() as usize;
        let position = source_map.byte_to_position(file_id, consumed_bytes)
            .unwrap_or_else(|| SourcePosition::start());
        
        let span = SourceSpan::single_char(position, file_id);
        
        ParseError::NomError {
            kind: self.code,
            span,
            context: None,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken { expected, found, .. } => {
                if expected.is_empty() {
                    write!(f, "unexpected token `{}`", found)
                } else if expected.len() == 1 {
                    write!(f, "expected `{}`, found `{}`", expected[0], found)
                } else {
                    write!(f, "expected one of {}, found `{}`", 
                        expected.iter().map(|s| format!("`{}`", s)).collect::<Vec<_>>().join(", "), 
                        found)
                }
            }
            
            ParseError::UnclosedDelimiter { delimiter, .. } => {
                write!(f, "unclosed delimiter `{}`", delimiter)
            }
            
            ParseError::InvalidSyntax { message, .. } => {
                write!(f, "{}", message)
            }
            
            ParseError::MissingToken { expected, .. } => {
                write!(f, "missing `{}`", expected)
            }
            
            ParseError::InvalidPattern { pattern, .. } => {
                write!(f, "invalid pattern `{}`", pattern)
            }
            
            ParseError::UnexpectedEof { expected, .. } => {
                if expected.len() == 1 {
                    write!(f, "unexpected end of file, expected `{}`", expected[0])
                } else {
                    write!(f, "unexpected end of file")
                }
            }
            
            ParseError::InvalidNumber { value, reason, .. } => {
                write!(f, "invalid number `{}`: {}", value, reason)
            }
            
            ParseError::InvalidString { reason, .. } => {
                write!(f, "invalid string literal: {}", reason)
            }
            
            ParseError::InvalidIdentifier { name, reason, .. } => {
                write!(f, "invalid identifier `{}`: {}", name, reason)
            }
            
            ParseError::InvalidType { type_text, reason, .. } => {
                write!(f, "invalid type `{}`: {}", type_text, reason)
            }
            
            ParseError::NomError { kind, context, .. } => {
                if let Some(ctx) = context {
                    write!(f, "parse error in {}: {:?}", ctx, kind)
                } else {
                    write!(f, "parse error: {:?}", kind)
                }
            }
        }
    }
}

impl From<nom::Err<nom::error::Error<&str>>> for ParseError {
    fn from(err: nom::Err<nom::error::Error<&str>>) -> Self {
        let nom_error = match err {
            nom::Err::Error(e) | nom::Err::Failure(e) => e,
            nom::Err::Incomplete(_) => {
                return ParseError::UnexpectedEof {
                    expected: vec!["more input".to_string()],
                    span: SourceSpan::single_char(SourcePosition::start(), FileId::new(0)),
                };
            }
        };
        
        ParseError::NomError {
            kind: nom_error.code,
            span: SourceSpan::single_char(SourcePosition::start(), FileId::new(0)),
            context: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_position() {
        let pos = SourcePosition::new(10, 5, 100);
        assert_eq!(pos.line, 10);
        assert_eq!(pos.column, 5);
        assert_eq!(pos.byte_offset, 100);
        assert_eq!(pos.to_string(), "10:5");
    }

    #[test]
    fn test_source_map() {
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file("test.hx".to_string(), "line1\nline2\nline3".to_string());
        
        assert_eq!(source_map.get_line(file_id, 1), Some("line1"));
        assert_eq!(source_map.get_line(file_id, 2), Some("line2"));
        assert_eq!(source_map.get_line(file_id, 3), Some("line3"));
        assert_eq!(source_map.get_line(file_id, 4), None);
    }

    #[test]
    fn test_byte_to_position() {
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file("test.hx".to_string(), "hello\nworld\n!".to_string());
        
        // Test various positions
        assert_eq!(source_map.byte_to_position(file_id, 0), Some(SourcePosition::new(1, 1, 0)));
        assert_eq!(source_map.byte_to_position(file_id, 5), Some(SourcePosition::new(1, 6, 5))); // End of "hello"
        assert_eq!(source_map.byte_to_position(file_id, 6), Some(SourcePosition::new(2, 1, 6))); // Start of "world"
        assert_eq!(source_map.byte_to_position(file_id, 12), Some(SourcePosition::new(3, 1, 12))); // "!"
    }

    #[test]
    fn test_parse_errors() {
        let mut errors = ParseErrors::new();
        assert!(errors.is_empty());
        
        let span = SourceSpan::single_char(SourcePosition::start(), FileId::new(0));
        errors.push(ParseError::InvalidSyntax {
            message: "test error".to_string(),
            span,
            suggestion: None,
        });
        
        assert!(!errors.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors.has_errors());
    }
}
