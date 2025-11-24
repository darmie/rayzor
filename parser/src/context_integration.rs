//! Integration between nom context errors and enhanced diagnostics
//!
//! This module provides utilities to convert nom's context-based error messages
//! into rich diagnostic information with suggestions and help text.

use diagnostics::{
    Diagnostic, DiagnosticSeverity, SourceSpan, SourcePosition, FileId,
    DiagnosticBuilder
};
use crate::enhanced_context::HaxeDiagnostics;
use nom::{IResult, Parser, error::ParseError as NomParseError};

/// Converts nom context strings into enhanced diagnostics
pub struct ContextErrorCollector {
    pub diagnostics: Vec<Diagnostic>,
    pub file_id: FileId,
}

impl ContextErrorCollector {
    pub fn new(file_id: FileId) -> Self {
        Self {
            diagnostics: Vec::new(),
            file_id,
        }
    }
    
    /// Convert a nom context error into an enhanced diagnostic
    pub fn convert_context_error(&mut self, context: &str, span: SourceSpan) -> Diagnostic {
        let diagnostic = match context {
            // Semicolon-related errors
            ctx if ctx.contains("expected ';' after") => {
                let after_what = ctx
                    .strip_prefix("expected ';' after ")
                    .unwrap_or("statement");
                HaxeDiagnostics::missing_semicolon(span, after_what)
            }
            
            // Function-related errors  
            "expected 'function' keyword" => {
                DiagnosticBuilder::error(
                    "expected function keyword".to_string(),
                    span.clone(),
                )
                .code("E0005")
                .label(span.clone(), "expected 'function' here")
                .suggestion("add 'function' keyword", span, "function".to_string())
                .help("function declarations must start with the 'function' keyword")
                .build()
            }
            
            "expected function name" => {
                DiagnosticBuilder::error(
                    "missing function name".to_string(),
                    span.clone(),
                )
                .code("E0006")
                .label(span.clone(), "function name required here")
                .help("provide a valid identifier for the function name")
                .build()
            }
            
            // Parameter list errors
            "expected '(' to start parameter list" => {
                HaxeDiagnostics::missing_closing_delimiter(
                    span.clone(),
                    span.clone(), 
                    '('
                )
            }
            
            "expected ')' to close parameter list" => {
                HaxeDiagnostics::missing_closing_delimiter(
                    span.clone(),
                    span.clone(),
                    ')'
                )
            }
            
            // Class-related errors
            "expected 'class' keyword" => {
                DiagnosticBuilder::error(
                    "expected class keyword".to_string(),
                    span.clone(),
                )
                .code("E0007")
                .label(span.clone(), "expected 'class' here")
                .suggestion("add 'class' keyword", span, "class".to_string())
                .help("type declarations must specify the declaration type")
                .build()
            }
            
            "expected class name" => {
                DiagnosticBuilder::error(
                    "missing class name".to_string(),
                    span.clone(),
                )
                .code("E0008")
                .label(span.clone(), "class name required here")
                .help("provide a valid identifier for the class name")
                .build()
            }
            
            "expected '{' to start class body" => {
                HaxeDiagnostics::missing_closing_delimiter(
                    span.clone(),
                    span.clone(),
                    '{'
                )
            }
            
            "expected '}' to close class body" => {
                HaxeDiagnostics::missing_closing_delimiter(
                    span.clone(), 
                    span.clone(),
                    '}'
                )
            }
            
            // Import/using errors
            "expected 'import' keyword" => {
                DiagnosticBuilder::error(
                    "expected import keyword".to_string(),
                    span.clone(),
                )
                .code("E0009")
                .label(span.clone(), "expected 'import' here")
                .suggestion("add 'import' keyword", span, "import".to_string())
                .build()
            }
            
            "expected 'using' keyword" => {
                DiagnosticBuilder::error(
                    "expected using keyword".to_string(),
                    span.clone(),
                )
                .code("E0010")
                .label(span.clone(), "expected 'using' here")
                .suggestion("add 'using' keyword", span, "using".to_string())
                .build()
            }
            
            // Package errors
            "expected 'package' keyword" => {
                DiagnosticBuilder::error(
                    "expected package keyword".to_string(),
                    span.clone(),
                )
                .code("E0011")
                .label(span.clone(), "expected 'package' here")
                .suggestion("add 'package' keyword", span, "package".to_string())
                .build()
            }
            
            // Block errors
            "expected '{' to start block" => {
                HaxeDiagnostics::missing_closing_delimiter(
                    span.clone(),
                    span.clone(),
                    '{'
                )
            }
            
            "expected '}' to close block" => {
                HaxeDiagnostics::missing_closing_delimiter(
                    span.clone(),
                    span.clone(), 
                    '}'
                )
            }
            
            // Switch statement errors
            "expected '(' after 'switch'" => {
                HaxeDiagnostics::missing_closing_delimiter(
                    span.clone(),
                    span.clone(),
                    '('
                )
            }
            
            "expected ')' after switch expression" => {
                HaxeDiagnostics::missing_closing_delimiter(
                    span.clone(),
                    span.clone(),
                    ')'
                )
            }
            
            "expected '{' to start switch body" => {
                HaxeDiagnostics::missing_closing_delimiter(
                    span.clone(),
                    span.clone(),
                    '{'
                )
            }
            
            "expected '}' to close switch body" => {
                HaxeDiagnostics::missing_closing_delimiter(
                    span.clone(),
                    span.clone(),
                    '}'
                )
            }
            
            // Variable declaration errors
            "expected variable name" => {
                DiagnosticBuilder::error(
                    "missing variable name".to_string(),
                    span.clone(),
                )
                .code("E0012")
                .label(span.clone(), "variable name required here")
                .help("provide a valid identifier for the variable name")
                .build()
            }
            
            "expected 'var' keyword" => {
                DiagnosticBuilder::error(
                    "expected var keyword".to_string(),
                    span.clone(),
                )
                .code("E0013")
                .label(span.clone(), "expected 'var' here")
                .suggestion("add 'var' keyword", span, "var".to_string())
                .build()
            }
            
            // Type annotation errors
            ctx if ctx.contains("expected ':' before") => {
                let what = ctx
                    .strip_prefix("expected ':' before ")
                    .unwrap_or("type annotation");
                DiagnosticBuilder::error(
                    format!("missing ':' before {}", what),
                    span.clone(),
                )
                .code("E0014")
                .label(span.clone(), "expected ':' here")
                .suggestion("add ':'", span, ":".to_string())
                .help(format!("type annotations must be preceded by ':'"))
                .build()
            }
            
            // Expression errors
            "expected expression" => {
                DiagnosticBuilder::error(
                    "expected expression".to_string(),
                    span.clone(),
                )
                .code("E0015")
                .label(span.clone(), "expression required here")
                .help("provide a valid expression")
                .build()
            }
            
            // Try-catch errors
            "expected 'try' keyword" => {
                DiagnosticBuilder::error(
                    "expected try keyword".to_string(),
                    span.clone(),
                )
                .code("E0016")
                .label(span.clone(), "expected 'try' here")
                .suggestion("add 'try' keyword", span, "try".to_string())
                .build()
            }
            
            "expected 'catch' keyword" => {
                DiagnosticBuilder::error(
                    "expected catch keyword".to_string(),
                    span.clone(),
                )
                .code("E0017")
                .label(span.clone(), "expected 'catch' here")
                .suggestion("add 'catch' keyword", span, "catch".to_string())
                .build()
            }
            
            // Generic fallback for context errors
            _ => {
                DiagnosticBuilder::error(
                    context.to_string(),
                    span.clone(),
                )
                .code("E0099")
                .label(span.clone(), "parse error")
                .build()
            }
        };
        
        diagnostic
    }
    
    /// Add a context error to the diagnostics collection
    pub fn add_context_error(&mut self, context: &str, span: SourceSpan) {
        let diagnostic = self.convert_context_error(context, span);
        self.diagnostics.push(diagnostic);
    }
    
    /// Get all collected diagnostics
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// Enhanced parser combinator that collects context errors
pub fn enhanced_parser<'a, I, O, E, F>(
    mut parser: F,
    _context_msg: &'static str,
    _collector: &'a mut ContextErrorCollector,
) -> impl FnMut(I) -> IResult<I, O, E> + 'a
where
    I: Clone,
    F: Parser<I, Output = O, Error = E> + 'a,
    E: NomParseError<I>,
{
    move |input: I| {
        match parser.parse(input.clone()) {
            Ok(result) => Ok(result),
            Err(err) => {
                // For now, just pass through the error
                // TODO: Integrate proper span calculation and diagnostic collection
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_semicolon_context_conversion() {
        let mut collector = ContextErrorCollector::new(FileId::new(0));
        let span = SourceSpan::new(
            SourcePosition::new(1, 10, 9),
            SourcePosition::new(1, 11, 10),
            FileId::new(0),
        );
        
        let diagnostic = collector.convert_context_error(
            "expected ';' after variable declaration",
            span
        );
        
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.code, Some("E0002".to_string()));
        assert!(diagnostic.message.contains("variable declaration"));
        assert!(!diagnostic.suggestions.is_empty());
    }
    
    #[test]
    fn test_function_keyword_context() {
        let mut collector = ContextErrorCollector::new(FileId::new(0));
        let span = SourceSpan::new(
            SourcePosition::new(2, 5, 15),
            SourcePosition::new(2, 6, 16),
            FileId::new(0),
        );
        
        let diagnostic = collector.convert_context_error(
            "expected 'function' keyword",
            span
        );
        
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.code, Some("E0005".to_string()));
        assert!(diagnostic.suggestions.iter().any(|s| s.replacement == "function"));
    }
    
    #[test]
    fn test_delimiter_context() {
        let mut collector = ContextErrorCollector::new(FileId::new(0));
        let span = SourceSpan::new(
            SourcePosition::new(3, 20, 35),
            SourcePosition::new(3, 21, 36),
            FileId::new(0),
        );
        
        let diagnostic = collector.convert_context_error(
            "expected ')' to close parameter list",
            span
        );
        
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.code, Some("E0003".to_string()));
        assert!(diagnostic.suggestions.iter().any(|s| s.replacement == ")"));
    }
}