//! Position-aware parser that tracks source locations and provides enhanced error reporting
//! 
//! This module wraps the existing nom-based parser with:
//! - Source position tracking for all tokens and AST nodes
//! - Enhanced error types with context and suggestions
//! - Error recovery capabilities
//! - Integration with the error formatting system

use crate::error::{
    ParseError, ParseErrors, SourceMap, SourceSpan, SourcePosition, FileId
};
use crate::error_formatter::ErrorFormatter;
use nom::{
    Parser as NomParser,
    error::{ErrorKind, ParseError as NomParseError},
    character::complete::multispace0,
};
use std::fmt;

/// A spanned AST node with position information
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: SourceSpan,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: SourceSpan) -> Self {
        Self { node, span }
    }
    
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            node: f(self.node),
            span: self.span,
        }
    }
    
    pub fn as_ref(&self) -> Spanned<&T> {
        Spanned {
            node: &self.node,
            span: self.span.clone(),
        }
    }
}

impl<T: fmt::Display> fmt::Display for Spanned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.node)
    }
}

/// Enhanced parser state with position tracking
pub struct PositionParser<'a> {
    input: &'a str,
    original_input: &'a str,
    current_position: SourcePosition,
    file_id: FileId,
    source_map: &'a SourceMap,
    errors: Vec<ParseError>,
    in_recovery: bool,
}

impl<'a> PositionParser<'a> {
    pub fn new(input: &'a str, file_id: FileId, source_map: &'a SourceMap) -> Self {
        Self {
            input,
            original_input: input,
            current_position: SourcePosition::start(),
            file_id,
            source_map,
            errors: Vec::new(),
            in_recovery: false,
        }
    }
    
    /// Get the current input remaining to be parsed
    pub fn input(&self) -> &'a str {
        self.input
    }
    
    /// Get the current source position
    pub fn position(&self) -> SourcePosition {
        self.current_position
    }
    
    /// Get all accumulated errors
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }
    
    /// Check if there were any parse errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
    
    /// Add an error to the error list
    pub fn add_error(&mut self, error: ParseError) {
        self.errors.push(error);
    }
    
    /// Create a span from start position to current position
    pub fn span_from(&self, start: SourcePosition) -> SourceSpan {
        SourceSpan::new(start, self.current_position, self.file_id)
    }
    
    /// Create a span for a single character at current position
    pub fn current_span(&self) -> SourceSpan {
        SourceSpan::single_char(self.current_position, self.file_id)
    }
    
    /// Advance the parser by consuming input and updating position
    pub fn advance(&mut self, consumed: &str) {
        let consumed_bytes = consumed.len();
        self.input = &self.input[consumed_bytes..];
        
        // Update position based on consumed text
        for ch in consumed.chars() {
            if ch == '\n' {
                self.current_position.line += 1;
                self.current_position.column = 1;
            } else {
                self.current_position.column += 1;
            }
            self.current_position.byte_offset += ch.len_utf8();
        }
    }
    
    /// Parse with a nom parser and track position
    pub fn parse_with<T>(&mut self, mut parser: impl NomParser<&'a str, Output = T, Error = nom::error::Error<&'a str>>) -> Result<T, ParseError> {
        let start_position = self.current_position;
        
        match parser.parse(self.input) {
            Ok((remaining, result)) => {
                let consumed_len = self.input.len() - remaining.len();
                let consumed = &self.input[..consumed_len];
                self.advance(consumed);
                Ok(result)
            }
            Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
                let span = SourceSpan::single_char(start_position, self.file_id);
                Err(ParseError::NomError {
                    kind: e.code,
                    span,
                    context: None,
                })
            }
            Err(nom::Err::Incomplete(_)) => {
                let span = SourceSpan::single_char(start_position, self.file_id);
                Err(ParseError::UnexpectedEof {
                    expected: vec!["more input".to_string()],
                    span,
                })
            }
        }
    }
    
    /// Parse and wrap result with span information
    pub fn parse_spanned<T>(&mut self, parser: impl NomParser<&'a str, Output = T, Error = nom::error::Error<&'a str>>) -> Result<Spanned<T>, ParseError> {
        let start_position = self.current_position;
        let result = self.parse_with(parser)?;
        let span = self.span_from(start_position);
        Ok(Spanned::new(result, span))
    }
    
    /// Expect a specific token and provide helpful error if not found
    pub fn expect_token(&mut self, expected: &str) -> Result<Spanned<String>, ParseError> {
        let start_position = self.current_position;
        
        if self.input.starts_with(expected) {
            let token = expected.to_string();
            self.advance(expected);
            let span = self.span_from(start_position);
            Ok(Spanned::new(token, span))
        } else {
            let span = self.current_span();
            let found = self.input.chars().next()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "end of file".to_string());
            
            Err(ParseError::UnexpectedToken {
                expected: vec![expected.to_string()],
                found,
                span,
            })
        }
    }
    
    /// Try to parse an identifier with keyword checking
    pub fn parse_identifier(&mut self) -> Result<Spanned<String>, ParseError> {
        let start_position = self.current_position;
        
        // Skip whitespace first
        self.skip_whitespace();
        
        let mut identifier = String::new();
        let mut chars = self.input.chars().peekable();
        
        // First character must be alphabetic or underscore
        if let Some(&first_char) = chars.peek() {
            if first_char.is_alphabetic() || first_char == '_' {
                identifier.push(first_char);
                chars.next();
            } else {
                let span = self.current_span();
                return Err(ParseError::InvalidIdentifier {
                    name: identifier,
                    span,
                    reason: "identifier must start with a letter or underscore".to_string(),
                    suggestion: None,
                });
            }
        } else {
            let span = self.current_span();
            return Err(ParseError::UnexpectedEof {
                expected: vec!["identifier".to_string()],
                span,
            });
        }
        
        // Subsequent characters can be alphanumeric or underscore
        while let Some(&ch) = chars.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                identifier.push(ch);
                chars.next();
            } else {
                break;
            }
        }
        
        // Check if it's a reserved keyword
        if self.is_keyword(&identifier) {
            let span = SourceSpan::new(start_position, self.current_position, self.file_id);
            let suggestion = crate::error_formatter::ErrorHelpers::suggest_keyword(&identifier);
            return Err(ParseError::InvalidIdentifier {
                name: identifier,
                span,
                reason: "this is a reserved keyword".to_string(),
                suggestion,
            });
        }
        
        self.advance(&identifier);
        let span = self.span_from(start_position);
        Ok(Spanned::new(identifier, span))
    }
    
    /// Skip whitespace and comments
    pub fn skip_whitespace(&mut self) {
        let start_input = self.input;
        
        // Use nom's multispace0 to skip whitespace
        if let Ok((remaining, _)) = multispace0::<&str, nom::error::Error<&str>>(self.input) {
            let consumed_len = start_input.len() - remaining.len();
            if consumed_len > 0 {
                let consumed = &start_input[..consumed_len];
                self.advance(consumed);
            }
        }
        
        // Skip comments
        self.skip_comments();
    }
    
    /// Skip line and block comments
    fn skip_comments(&mut self) {
        loop {
            let initial_input = self.input;
            
            // Skip line comments
            if self.input.starts_with("//") {
                if let Some(newline_pos) = self.input.find('\n') {
                    self.advance(&self.input[..newline_pos + 1]);
                } else {
                    // Comment extends to end of file
                    self.advance(self.input);
                }
                continue;
            }
            
            // Skip block comments
            if self.input.starts_with("/*") {
                if let Some(end_pos) = self.input.find("*/") {
                    self.advance(&self.input[..end_pos + 2]);
                } else {
                    // Unclosed block comment
                    let span = self.current_span();
                    self.add_error(ParseError::UnclosedDelimiter {
                        delimiter: '*',
                        opened_at: span.clone(),
                        expected_close_at: span,
                    });
                    self.advance(self.input); // Consume rest of input
                }
                continue;
            }
            
            // Skip whitespace after comments
            if let Ok((remaining, _)) = multispace0::<&str, nom::error::Error<&str>>(self.input) {
                let consumed_len = self.input.len() - remaining.len();
                if consumed_len > 0 {
                    let consumed = &self.input[..consumed_len];
                    self.advance(consumed);
                    continue;
                }
            }
            
            // No more comments or whitespace found
            if self.input == initial_input {
                break;
            }
        }
    }
    
    /// Check if a string is a reserved keyword
    fn is_keyword(&self, word: &str) -> bool {
        matches!(
            word,
            "abstract" | "break" | "case" | "cast" | "catch" | "class" | "continue" | "default"
                | "do" | "dynamic" | "else" | "enum" | "extends" | "extern" | "false" | "final"
                | "for" | "function" | "if" | "implements" | "import" | "in" | "inline" | "interface"
                | "macro" | "new" | "null" | "override" | "package" | "private" | "public"
                | "return" | "static" | "super" | "switch" | "this" | "throw" | "true" | "try"
                | "typedef" | "untyped" | "using" | "var" | "while"
        )
    }
    
    /// Attempt error recovery by finding synchronization points
    pub fn recover_to_sync_point(&mut self) -> bool {
        if self.in_recovery {
            return false; // Avoid infinite recovery loops
        }
        
        self.in_recovery = true;
        let mut recovered = false;
        
        // Look for synchronization tokens: semicolon, closing braces, keywords
        while !self.input.is_empty() {
            if self.input.starts_with(';') || 
               self.input.starts_with('}') ||
               self.input.starts_with("class") ||
               self.input.starts_with("function") ||
               self.input.starts_with("var") ||
               self.input.starts_with("if") ||
               self.input.starts_with("while") ||
               self.input.starts_with("for") {
                recovered = true;
                break;
            }
            
            // Advance one character and try again
            let next_char = self.input.chars().next().unwrap();
            self.advance(&next_char.to_string());
        }
        
        self.in_recovery = false;
        recovered
    }
    
    /// Parse with error recovery - continues parsing even after errors
    pub fn parse_with_recovery<T>(&mut self, parser: impl Fn(&mut Self) -> Result<T, ParseError>) -> Option<T> {
        match parser(self) {
            Ok(result) => Some(result),
            Err(error) => {
                self.add_error(error);
                
                // Attempt recovery
                if self.recover_to_sync_point() {
                    // Try parsing again after recovery
                    match parser(self) {
                        Ok(result) => Some(result),
                        Err(error) => {
                            self.add_error(error);
                            None
                        }
                    }
                } else {
                    None
                }
            }
        }
    }
    
    /// Check if we're at the end of input
    pub fn is_at_end(&self) -> bool {
        self.input.is_empty()
    }
    
    /// Peek at the next character without consuming it
    pub fn peek_char(&self) -> Option<char> {
        self.input.chars().next()
    }
    
    /// Format all accumulated errors
    pub fn format_errors(&self) -> String {
        let errors = ParseErrors {
            errors: self.errors.clone(),
        };
        let formatter = ErrorFormatter::default();
        formatter.format_errors(&errors, self.source_map)
    }
}


/// Extension trait for Result to add position-aware error handling
pub trait ParseResultExt<T> {
    fn with_context(self, context: &str, span: SourceSpan) -> Result<T, ParseError>;
    fn or_missing_token(self, expected: &str, span: SourceSpan) -> Result<T, ParseError>;
}

impl<T> ParseResultExt<T> for Result<T, ParseError> {
    fn with_context(self, context: &str, span: SourceSpan) -> Result<T, ParseError> {
        self.map_err(|mut error| {
            if let ParseError::NomError { context: ref mut ctx, .. } = error {
                *ctx = Some(context.to_string());
            }
            error
        })
    }
    
    fn or_missing_token(self, expected: &str, span: SourceSpan) -> Result<T, ParseError> {
        self.or_else(|_| {
            Err(ParseError::MissingToken {
                expected: expected.to_string(),
                after: span,
                suggestion: Some(format!("try adding `{}` here", expected)),
            })
        })
    }
}

/// Convenient macro for parsing with position tracking
#[macro_export]
macro_rules! parse_spanned {
    ($parser:expr, $nom_parser:expr) => {{
        let start_pos = $parser.position();
        match $parser.parse_with($nom_parser) {
            Ok(result) => {
                let span = $parser.span_from(start_pos);
                Ok(Spanned::new(result, span))
            }
            Err(error) => Err(error),
        }
    }};
}

/// Macro for expecting specific tokens with good error messages
#[macro_export]
macro_rules! expect_token {
    ($parser:expr, $token:literal) => {
        $parser.expect_token($token)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::character::complete::{alpha1, digit1};

    // Helper function for tests - returns just the essentials for testing
    fn create_test_setup(input: &str) -> (FileId, SourceMap) {
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file("test.hx".to_string(), input.to_string());
        (file_id, source_map)
    }
    
    fn create_test_parser<'a>(input: &'a str, file_id: FileId, source_map: &'a SourceMap) -> PositionParser<'a> {
        PositionParser::new(input, file_id, source_map)
    }

    #[test]
    fn test_position_tracking() {
        let input = "hello\nworld";
        let (file_id, source_map) = create_test_setup(input);
        let mut parser = create_test_parser(input, file_id, &source_map);
        
        // Parse "hello"
        let result = parser.parse_with(alpha1).unwrap();
        assert_eq!(result, "hello");
        assert_eq!(parser.position().line, 1);
        assert_eq!(parser.position().column, 6);
        
        // Skip newline
        parser.advance("\n");
        assert_eq!(parser.position().line, 2);
        assert_eq!(parser.position().column, 1);
        
        // Parse "world"
        let result = parser.parse_with(alpha1).unwrap();
        assert_eq!(result, "world");
        assert_eq!(parser.position().line, 2);
        assert_eq!(parser.position().column, 6);
    }

    #[test]
    fn test_spanned_parsing() {
        let input = "123";
        let (file_id, source_map) = create_test_setup(input);
        let mut parser = create_test_parser(input, file_id, &source_map);
        
        let spanned_result = parser.parse_spanned(digit1).unwrap();
        assert_eq!(spanned_result.node, "123");
        assert_eq!(spanned_result.span.start.line, 1);
        assert_eq!(spanned_result.span.start.column, 1);
        assert_eq!(spanned_result.span.end.column, 4);
    }

    #[test]
    fn test_identifier_parsing() {
        let input = "myVariable";
        let (file_id, source_map) = create_test_setup(input);
        let mut parser = create_test_parser(input, file_id, &source_map);
        
        let identifier = parser.parse_identifier().unwrap();
        assert_eq!(identifier.node, "myVariable");
    }

    #[test]
    fn test_keyword_detection() {
        let input = "class";
        let (file_id, source_map) = create_test_setup(input);
        let mut parser = create_test_parser(input, file_id, &source_map);
        
        let result = parser.parse_identifier();
        assert!(result.is_err());
        
        if let Err(ParseError::InvalidIdentifier { reason, .. }) = result {
            assert!(reason.contains("reserved keyword"));
        } else {
            panic!("Expected InvalidIdentifier error");
        }
    }

    #[test]
    fn test_whitespace_skipping() {
        let input = "   \t\n  hello";
        let (file_id, source_map) = create_test_setup(input);
        let mut parser = create_test_parser(input, file_id, &source_map);
        
        parser.skip_whitespace();
        assert_eq!(parser.input(), "hello");
    }

    #[test]
    fn test_comment_skipping() {
        let input = "// comment\nhello";
        let (file_id, source_map) = create_test_setup(input);
        let mut parser = create_test_parser(input, file_id, &source_map);
        
        parser.skip_whitespace();
        assert_eq!(parser.input(), "hello");
    }

    #[test]
    fn test_block_comment_skipping() {
        let input = "/* block comment */hello";
        let (file_id, source_map) = create_test_setup(input);
        let mut parser = create_test_parser(input, file_id, &source_map);
        
        parser.skip_whitespace();
        assert_eq!(parser.input(), "hello");
    }

    #[test]
    fn test_error_recovery() {
        let input = "invalid syntax ; var x = 5;";
        let (file_id, source_map) = create_test_setup(input);
        let mut parser = create_test_parser(input, file_id, &source_map);
        
        // Simulate a parse error
        parser.add_error(ParseError::InvalidSyntax {
            message: "test error".to_string(),
            span: parser.current_span(),
            suggestion: None,
        });
        
        // Try recovery
        let recovered = parser.recover_to_sync_point();
        assert!(recovered);
        assert!(parser.input().starts_with(';'));
    }
}
