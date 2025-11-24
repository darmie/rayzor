//! Enhanced parser with position tracking and rich error reporting
//! 
//! This module provides the main entry point for parsing Haxe code with
//! comprehensive error handling, position tracking, and Rust-like error formatting.

use crate::{
    error::{SourceMap, FileId, ParseErrors, ParseError, SourceSpan, SourcePosition},
    error_formatter::ErrorFormatter,
    haxe_ast::HaxeFile
};
use std::path::Path;

/// Enhanced parser result with rich error information
#[derive(Debug)]
pub struct EnhancedParseResult {
    /// The parsed AST (if successful)
    pub ast: Option<HaxeFile>,
    /// All errors encountered during parsing
    pub errors: ParseErrors,
    /// The source map containing file information
    pub source_map: SourceMap,
    /// File ID for the parsed file
    pub file_id: FileId,
}

impl EnhancedParseResult {
    /// Check if parsing was successful (no errors)
    pub fn is_success(&self) -> bool {
        self.ast.is_some() && !self.errors.has_errors()
    }
    
    /// Get the parsed AST, panicking if there were errors
    pub fn unwrap(self) -> HaxeFile {
        if !self.is_success() {
            panic!("Cannot unwrap failed parse result. Use format_errors() to see details.");
        }
        self.ast.expect("AST should be available when parsing is successful")
    }
    
    /// Get the parsed AST or return errors
    pub fn into_result(self) -> Result<HaxeFile, ParseErrors> {
        if self.is_success() {
            Ok(self.ast.unwrap())
        } else {
            Err(self.errors)
        }
    }
    
    /// Format all errors with rich, colorized output
    pub fn format_errors(&self) -> String {
        self.format_errors_with_config(Default::default())
    }
    
    /// Format errors with custom formatting configuration
    pub fn format_errors_with_config(&self, config: crate::error_formatter::FormatConfig) -> String {
        let formatter = ErrorFormatter::new(config);
        formatter.format_errors(&self.errors, &self.source_map)
    }
    
    /// Format errors without colors (for file output)
    pub fn format_errors_plain(&self) -> String {
        self.format_errors_with_config(crate::error_formatter::FormatConfig {
            use_colors: false,
            ..Default::default()
        })
    }
}

/// Enhanced Haxe parser with comprehensive error handling
pub struct HaxeParser {
    source_map: SourceMap,
}

impl HaxeParser {
    /// Create a new enhanced parser
    pub fn new() -> Self {
        Self {
            source_map: SourceMap::new(),
        }
    }
    
    /// Parse Haxe source code from a string
    pub fn parse_string(&mut self, content: &str, filename: Option<&str>) -> EnhancedParseResult {
        let file_name = filename.unwrap_or("<input>").to_string();
        let file_id = self.source_map.add_file(file_name, content.to_string());
        
        self.parse_with_file_id(content, file_id)
    }
    
    /// Parse Haxe source code from a file
    pub fn parse_file<P: AsRef<Path>>(&mut self, path: P) -> Result<EnhancedParseResult, std::io::Error> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        let file_name = path.to_string_lossy().to_string();
        
        Ok(self.parse_string(&content, Some(&file_name)))
    }
    
    /// Internal parsing with error recovery
    fn parse_with_file_id(&mut self, content: &str, file_id: FileId) -> EnhancedParseResult {
        // Try the standard nom parser first (using the raw nom parser to avoid circular dependency)
        match crate::haxe_parser::haxe_file(content, content) {
            Ok((remaining, ast)) => {
                // Check if we consumed all input
                let trimmed_remaining = remaining.trim();
                if trimmed_remaining.is_empty() {
                    // Parsing succeeded! Return success with no errors
                    EnhancedParseResult {
                        ast: Some(HaxeFile {
                            package: ast.package,
                            imports: ast.imports,
                            using: ast.using,
                            module_fields: ast.module_fields,
                            declarations: ast.declarations,
                            span: ast.span,
                        }),
                        errors: ParseErrors::new(),
                        source_map: self.source_map.clone(),
                        file_id,
                    }
                } else {
                    // Partial parsing - still do error analysis
                    let mut errors = ParseErrors::new();
                    self.analyze_syntax_errors(content, file_id, &mut errors);
                    
                    EnhancedParseResult {
                        ast: None,
                        errors,
                        source_map: self.source_map.clone(),
                        file_id,
                    }
                }
            }
            Err(_nom_error) => {
                // Standard parser failed, do enhanced error analysis
                let mut errors = ParseErrors::new();
                self.analyze_syntax_errors(content, file_id, &mut errors);
                
                EnhancedParseResult {
                    ast: None,
                    errors,
                    source_map: self.source_map.clone(),
                    file_id,
                }
            }
        }
    }
    
    /// Analyze content for specific syntax errors
    fn analyze_syntax_errors(&self, content: &str, file_id: FileId, errors: &mut ParseErrors) {
        let lines: Vec<&str> = content.lines().collect();
        let mut brace_count = 0;
        let mut paren_count = 0;
        let mut bracket_count = 0;
        
        for (line_num, line) in lines.iter().enumerate() {
            let line_number = line_num + 1;
            let trimmed = line.trim();
            
            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }
            
            // Check for keyword typos
            self.check_keyword_typos(line, line_number, file_id, errors);
            
            // Check for missing semicolons
            self.check_missing_semicolons(line, line_number, file_id, errors);
            
            // Check for reserved keywords used as identifiers
            self.check_invalid_identifiers(line, line_number, file_id, errors);
            
            // Check for Haxe-specific patterns
            self.check_haxe_patterns(line, line_number, file_id, errors);
            
            // Track delimiter balance
            for ch in line.chars() {
                match ch {
                    '{' => brace_count += 1,
                    '}' => brace_count -= 1,
                    '(' => paren_count += 1,
                    ')' => paren_count -= 1,
                    '[' => bracket_count += 1,
                    ']' => bracket_count -= 1,
                    _ => {}
                }
            }
        }
        
        // Check for unclosed delimiters at the end
        if brace_count > 0 {
            self.add_unclosed_delimiter_error(content, file_id, errors, '{', brace_count);
        }
        if paren_count > 0 {
            self.add_unclosed_delimiter_error(content, file_id, errors, '(', paren_count);
        }
        if bracket_count > 0 {
            self.add_unclosed_delimiter_error(content, file_id, errors, '[', bracket_count);
        }
    }
    
    /// Check for keyword typos in a line
    fn check_keyword_typos(&self, line: &str, line_number: usize, file_id: FileId, errors: &mut ParseErrors) {
        let words: Vec<&str> = line.split_whitespace().collect();
        
        for (i, &word) in words.iter().enumerate() {
            // Clean word of punctuation for checking
            let clean_word = word.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
            
            // Skip known keywords and empty words
            if clean_word.is_empty() || self.is_keyword(clean_word) {
                continue;
            }
            
            // Check if this could be a typo of a keyword
            if let Some(suggestion) = crate::error_formatter::ErrorHelpers::suggest_keyword(clean_word) {
                // For standalone words that look like typos, always suggest
                let should_suggest = self.looks_like_keyword_position(clean_word, i, &words) ||
                    // Always catch common typos regardless of position
                    matches!(clean_word, "fucntion" | "classe" | "publik" | "privat" | "statik" | "retur" | "abstrak");
                
                if should_suggest {
                    if let Some(column) = line.find(clean_word) {
                        let error_position = SourcePosition {
                            line: line_number,
                            column: column + 1,
                            byte_offset: 0,
                        };
                        let span = SourceSpan::new(
                            error_position,
                            SourcePosition {
                                line: line_number,
                                column: column + 1 + clean_word.len(),
                                byte_offset: 0,
                            },
                            file_id,
                        );
                        
                        errors.push(ParseError::InvalidIdentifier {
                            name: clean_word.to_string(),
                            span,
                            reason: "unknown keyword, did you mean something else?".to_string(),
                            suggestion: Some(suggestion),
                        });
                    }
                }
            }
        }
    }
    
    /// Check if a word appears to be in a keyword position
    fn looks_like_keyword_position(&self, word: &str, position: usize, words: &[&str]) -> bool {
        // First word in line is often a keyword
        if position == 0 {
            return true;
        }
        
        // After certain keywords
        if position > 0 {
            let prev_word = words[position - 1];
            if matches!(prev_word, "public" | "private" | "static" | "override" | "inline" | "abstract") {
                return true;
            }
        }
        
        // Common keyword typos that should always be caught
        if matches!(word, "fucntion" | "classe" | "publik" | "privat" | "statik" | "retur" | "abstrak" | "interfac") {
            return true;
        }
        
        false
    }
    
    /// Check for missing semicolons
    fn check_missing_semicolons(&self, line: &str, line_number: usize, file_id: FileId, errors: &mut ParseErrors) {
        let trimmed = line.trim();
        
        if (trimmed.starts_with("package ") || trimmed.starts_with("import ")) && !trimmed.ends_with(';') {
            let error_position = SourcePosition {
                line: line_number,
                column: line.len() + 1,
                byte_offset: 0,
            };
            let span = SourceSpan::single_char(error_position, file_id);
            
            errors.push(ParseError::MissingToken {
                expected: ";".to_string(),
                after: span,
                suggestion: Some("add a semicolon at the end of this line".to_string()),
            });
        }
    }
    
    /// Check for invalid identifier usage
    fn check_invalid_identifiers(&self, line: &str, line_number: usize, file_id: FileId, errors: &mut ParseErrors) {
        // Skip comment lines entirely
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
            return;
        }
        
        let words: Vec<&str> = line.split_whitespace().collect();
        
        for (i, &word) in words.iter().enumerate() {
            // Clean the word of punctuation
            let clean_word = word.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
            
            // Check for reserved keywords used as variable names
            // Only after "var" declarations, but not flagging "var" or "function" themselves
            if word == "var" && i + 1 < words.len() {
                let next_word = words[i + 1].trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if self.is_keyword(next_word) {
                    if let Some(column) = line.find(next_word) {
                        let error_position = SourcePosition {
                            line: line_number,
                            column: column + 1,
                            byte_offset: 0,
                        };
                        let span = SourceSpan::new(
                            error_position,
                            SourcePosition {
                                line: line_number,
                                column: column + 1 + next_word.len(),
                                byte_offset: 0,
                            },
                            file_id,
                        );
                        
                        errors.push(ParseError::InvalidIdentifier {
                            name: next_word.to_string(),
                            span,
                            reason: "reserved keyword cannot be used as identifier".to_string(),
                            suggestion: Some(format!("try using a different name like '{}_value' or 'my_{}'", next_word, next_word)),
                        });
                    }
                }
            }
            
            // Check for invalid identifier patterns (starting with numbers)
            // But only for standalone words that look like they should be identifiers
            if clean_word.chars().next().map_or(false, |c| c.is_ascii_digit()) &&
               clean_word.len() > 1 && // Skip single digit numbers
               clean_word.chars().any(|c| c.is_alphabetic()) // Must contain letters to be an invalid identifier
            {
                if let Some(column) = line.find(clean_word) {
                    let error_position = SourcePosition {
                        line: line_number,
                        column: column + 1,
                        byte_offset: 0,
                    };
                    let span = SourceSpan::new(
                        error_position,
                        SourcePosition {
                            line: line_number,
                            column: column + 1 + clean_word.len(),
                            byte_offset: 0,
                        },
                        file_id,
                    );
                    
                    errors.push(ParseError::InvalidIdentifier {
                        name: clean_word.to_string(),
                        span,
                        reason: "identifier cannot start with a number".to_string(),
                        suggestion: Some(format!("try renaming to 'value_{}'", clean_word)),
                    });
                }
            }
            
            // Check for invalid characters in identifiers (like hyphens)
            // But exclude comments and only check actual identifier contexts
            if clean_word.contains('-') && 
               clean_word.chars().any(|c| c.is_alphabetic()) &&
               !clean_word.starts_with("//") &&
               !clean_word.contains("*") // Skip comment markers
            {
                if let Some(column) = line.find(clean_word) {
                    let error_position = SourcePosition {
                        line: line_number,
                        column: column + 1,
                        byte_offset: 0,
                    };
                    let span = SourceSpan::new(
                        error_position,
                        SourcePosition {
                            line: line_number,
                            column: column + 1 + clean_word.len(),
                            byte_offset: 0,
                        },
                        file_id,
                    );
                    
                    let suggestion = clean_word.replace('-', "_");
                    errors.push(ParseError::InvalidIdentifier {
                        name: clean_word.to_string(),
                        span,
                        reason: "identifier cannot contain hyphens".to_string(),
                        suggestion: Some(format!("try using '{}' instead", suggestion)),
                    });
                }
            }
        }
    }
    
    /// Add unclosed delimiter error
    fn add_unclosed_delimiter_error(&self, content: &str, file_id: FileId, errors: &mut ParseErrors, delimiter: char, _count: i32) {
        // Find the last occurrence of the opening delimiter
        let lines: Vec<&str> = content.lines().collect();
        
        for (line_num, line) in lines.iter().enumerate().rev() {
            if line.contains(delimiter) {
                if let Some(column) = line.rfind(delimiter) {
                    let error_position = SourcePosition {
                        line: line_num + 1,
                        column: column + 1,
                        byte_offset: 0,
                    };
                    let span = SourceSpan::single_char(error_position, file_id);
                    
                    errors.push(ParseError::UnclosedDelimiter {
                        delimiter,
                        opened_at: span.clone(),
                        expected_close_at: SourceSpan::single_char(
                            SourcePosition {
                                line: lines.len(),
                                column: lines.last().map(|l| l.len()).unwrap_or(0) + 1,
                                byte_offset: 0,
                            },
                            file_id,
                        ),
                    });
                    break;
                }
            }
        }
    }
    
    /// Check for Haxe-specific patterns and common mistakes
    fn check_haxe_patterns(&self, line: &str, line_number: usize, file_id: FileId, errors: &mut ParseErrors) {
        let trimmed = line.trim();
        
        // Check for const usage (should be final in Haxe)
        if trimmed.starts_with("const ") {
            if let Some(column) = line.find("const") {
                let span = SourceSpan::new(
                    SourcePosition { line: line_number, column: column + 1, byte_offset: 0 },
                    SourcePosition { line: line_number, column: column + 6, byte_offset: 0 },
                    file_id,
                );
                errors.push(ParseError::InvalidSyntax {
                    message: "use 'final' instead of 'const' in Haxe".to_string(),
                    span,
                    suggestion: Some("replace 'const' with 'final'".to_string()),
                });
            }
        }
        
        // Check for elseif (should be else if)
        if trimmed.contains("elseif") {
            if let Some(column) = line.find("elseif") {
                let span = SourceSpan::new(
                    SourcePosition { line: line_number, column: column + 1, byte_offset: 0 },
                    SourcePosition { line: line_number, column: column + 7, byte_offset: 0 },
                    file_id,
                );
                errors.push(ParseError::InvalidSyntax {
                    message: "use 'else if' instead of 'elseif'".to_string(),
                    span,
                    suggestion: Some("replace 'elseif' with 'else if'".to_string()),
                });
            }
        }
        
        // Check for C-style for loops
        if trimmed.contains("for (var ") || trimmed.contains("for (let ") {
            if let Some(column) = line.find("for (") {
                let span = SourceSpan::new(
                    SourcePosition { line: line_number, column: column + 1, byte_offset: 0 },
                    SourcePosition { line: line_number, column: column + 10, byte_offset: 0 },
                    file_id,
                );
                errors.push(ParseError::InvalidSyntax {
                    message: "C-style for loops are not supported in Haxe".to_string(),
                    span,
                    suggestion: Some("use 'for (i in 0...10)' or 'for (item in array)' syntax".to_string()),
                });
            }
        }
        
        // Check for missing parentheses in control structures
        let control_keywords = ["if", "while", "switch", "for"];
        for keyword in &control_keywords {
            if let Some(pos) = trimmed.find(keyword) {
                // Check if it's at the start of a word
                if pos == 0 || !trimmed.chars().nth(pos.saturating_sub(1)).unwrap_or(' ').is_alphanumeric() {
                    let after_keyword = pos + keyword.len();
                    if let Some(ch) = trimmed.chars().nth(after_keyword) {
                        if ch != '(' && ch.is_whitespace() {
                            // Check if there's a non-parenthesis after whitespace
                            let rest = &trimmed[after_keyword..].trim_start();
                            if !rest.is_empty() && !rest.starts_with('(') {
                                let column = line.find(keyword).unwrap_or(0);
                                let span = SourceSpan::new(
                                    SourcePosition { line: line_number, column: column + 1, byte_offset: 0 },
                                    SourcePosition { line: line_number, column: column + keyword.len() + 1, byte_offset: 0 },
                                    file_id,
                                );
                                errors.push(ParseError::MissingToken {
                                    expected: "(".to_string(),
                                    after: span,
                                    suggestion: Some(format!("{} conditions must be enclosed in parentheses", keyword)),
                                });
                            }
                        }
                    }
                }
            }
        }
        
        // Check for arrow function syntax errors
        if trimmed.contains("=>") && !trimmed.contains("->") {
            if let Some(column) = line.find("=>") {
                let span = SourceSpan::new(
                    SourcePosition { line: line_number, column: column + 1, byte_offset: 0 },
                    SourcePosition { line: line_number, column: column + 3, byte_offset: 0 },
                    file_id,
                );
                errors.push(ParseError::InvalidSyntax {
                    message: "use '->' instead of '=>' for arrow functions in Haxe".to_string(),
                    span,
                    suggestion: Some("replace '=>' with '->'".to_string()),
                });
            }
        }
        
        // Check for invalid type annotations
        if trimmed.contains(": Array<>") {
            if let Some(column) = line.find(": Array<>") {
                let span = SourceSpan::new(
                    SourcePosition { line: line_number, column: column + 1, byte_offset: 0 },
                    SourcePosition { line: line_number, column: column + 10, byte_offset: 0 },
                    file_id,
                );
                errors.push(ParseError::InvalidType {
                    type_text: "Array<>".to_string(),
                    span,
                    reason: "Array type requires a type parameter, e.g., Array<Int>".to_string(),
                });
            }
        }
        
        // Check for duplicate semicolons
        if trimmed.ends_with(";;") {
            if let Some(column) = line.rfind(";;") {
                let span = SourceSpan::new(
                    SourcePosition { line: line_number, column: column + 2, byte_offset: 0 },
                    SourcePosition { line: line_number, column: column + 3, byte_offset: 0 },
                    file_id,
                );
                errors.push(ParseError::InvalidSyntax {
                    message: "duplicate semicolon".to_string(),
                    span,
                    suggestion: Some("remove the extra semicolon".to_string()),
                });
            }
        }
        
        // Check for Map type with wrong number of parameters
        if let Some(pos) = trimmed.find("Map<") {
            let rest = &trimmed[pos+4..];
            if let Some(end) = rest.find('>') {
                let type_params = &rest[..end];
                let param_count = type_params.split(',').count();
                if param_count == 1 && !type_params.trim().is_empty() {
                    let column = line.find("Map<").unwrap_or(0);
                    let span = SourceSpan::new(
                        SourcePosition { line: line_number, column: column + 1, byte_offset: 0 },
                        SourcePosition { line: line_number, column: column + 4 + end + 1, byte_offset: 0 },
                        file_id,
                    );
                    errors.push(ParseError::InvalidType {
                        type_text: format!("Map<{}>", type_params),
                        span,
                        reason: "Map type requires two type parameters: key and value types".to_string(),
                    });
                }
            }
        }
        
        // Check for varargs syntax (should use Array)
        if trimmed.contains("...") && trimmed.contains("function") {
            if let Some(column) = line.find("...") {
                let span = SourceSpan::new(
                    SourcePosition { line: line_number, column: column + 1, byte_offset: 0 },
                    SourcePosition { line: line_number, column: column + 4, byte_offset: 0 },
                    file_id,
                );
                errors.push(ParseError::InvalidSyntax {
                    message: "varargs syntax '...' is not supported in Haxe".to_string(),
                    span,
                    suggestion: Some("use 'args:Array<Dynamic>' for variable arguments".to_string()),
                });
            }
        }
        
        // Check for object literal syntax errors (= instead of :)
        if trimmed.contains("{") && trimmed.contains("=") && !trimmed.contains("=>") && !trimmed.contains("<=") && !trimmed.contains(">=") && !trimmed.contains("==") && !trimmed.contains("!=") {
            // Simple heuristic: if line has { and = but not assignment operators
            if let Some(eq_pos) = line.find(" = ") {
                // Check if it looks like object literal context
                let before_eq = &line[..eq_pos];
                let after_eq = &line[eq_pos+3..];
                if !before_eq.contains("var") && !before_eq.contains("final") && before_eq.trim().ends_with(|c: char| c.is_alphanumeric() || c == '_') {
                    let span = SourceSpan::new(
                        SourcePosition { line: line_number, column: eq_pos + 2, byte_offset: 0 },
                        SourcePosition { line: line_number, column: eq_pos + 3, byte_offset: 0 },
                        file_id,
                    );
                    errors.push(ParseError::InvalidSyntax {
                        message: "use ':' instead of '=' in object literals".to_string(),
                        span,
                        suggestion: Some("object properties should use colon syntax: { x: 10, y: 20 }".to_string()),
                    });
                }
            }
        }
        
        // Check for wrong string interpolation quotes
        if trimmed.contains("\"") && trimmed.contains("$") {
            // Check if $ is inside double quotes
            let mut in_string = false;
            let mut string_start = 0;
            for (i, ch) in line.chars().enumerate() {
                if ch == '"' && (i == 0 || line.chars().nth(i-1) != Some('\\')) {
                    if !in_string {
                        in_string = true;
                        string_start = i;
                    } else {
                        // Check if string contains $
                        let string_content = &line[string_start..i+1];
                        if string_content.contains("$") {
                            let span = SourceSpan::new(
                                SourcePosition { line: line_number, column: string_start + 1, byte_offset: 0 },
                                SourcePosition { line: line_number, column: i + 2, byte_offset: 0 },
                                file_id,
                            );
                            errors.push(ParseError::InvalidSyntax {
                                message: "string interpolation requires single quotes".to_string(),
                                span,
                                suggestion: Some("use single quotes for string interpolation: 'Hello $name'".to_string()),
                            });
                        }
                        in_string = false;
                    }
                }
            }
        }
        
        // Check for missing function body
        if trimmed.contains("function") && trimmed.ends_with(";") && !trimmed.contains("=") {
            // Likely a function declaration without body
            let span = SourceSpan::new(
                SourcePosition { line: line_number, column: line.len(), byte_offset: 0 },
                SourcePosition { line: line_number, column: line.len() + 1, byte_offset: 0 },
                file_id,
            );
            errors.push(ParseError::InvalidSyntax {
                message: "function declaration requires a body".to_string(),
                span,
                suggestion: Some("add function body with curly braces: { }".to_string()),
            });
        }
    }
    
    /// Check if a word is a Haxe keyword
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
}

impl Default for HaxeParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to parse a Haxe string with enhanced error reporting
pub fn parse_haxe_enhanced(content: &str, filename: Option<&str>) -> EnhancedParseResult {
    let mut parser = HaxeParser::new();
    parser.parse_string(content, filename)
}

/// Convenience function to parse a Haxe file with enhanced error reporting
pub fn parse_haxe_file_enhanced<P: AsRef<Path>>(path: P) -> Result<EnhancedParseResult, std::io::Error> {
    let mut parser = HaxeParser::new();
    parser.parse_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_parser_success() {
        let input = r#"
            package com.example;
            
            class TestClass {
                public function test(): String {
                    return "Hello World";
                }
            }
        "#;
        
        let result = parse_haxe_enhanced(input, Some("test.hx"));
        assert!(result.is_success());
        
        let ast = result.unwrap();
        assert!(ast.package.is_some());
        assert_eq!(ast.declarations.len(), 1);
    }
    
    #[test]
    fn test_enhanced_parser_error_recovery() {
        let input = r#"
            package com.example;
            
            classe TestClass {  // Error: "classe" instead of "class"
                public function test(): String {
                    return "Hello World";
                }
            }
        "#;
        
        let result = parse_haxe_enhanced(input, Some("test.hx"));
        assert!(!result.is_success());
        assert!(result.errors.len() > 0);
        
        // Test error formatting
        let formatted = result.format_errors();
        assert!(formatted.contains("class") || formatted.contains("error"));
        assert!(formatted.contains("test.hx"));
    }
    
    #[test]
    fn test_enhanced_parser_keyword_suggestion() {
        let input = r#"
            classe TestClass {}  // Typo: "classe" instead of "class"
        "#;
        
        let result = parse_haxe_enhanced(input, Some("test.hx"));
        assert!(!result.is_success());
        
        let formatted = result.format_errors();
        // Should suggest "class" as a correction
        assert!(formatted.contains("class") || formatted.contains("did you mean"));
    }
    
    #[test]
    fn test_enhanced_parser_missing_semicolon() {
        let input = r#"
            package com.example  // Missing semicolon
            
            class TestClass {}
        "#;
        
        let result = parse_haxe_enhanced(input, Some("test.hx"));
        assert!(!result.is_success());
        
        let formatted = result.format_errors();
        assert!(formatted.contains("error"));
    }
    
    #[test]
    fn test_enhanced_parser_unclosed_brace() {
        let input = r#"
            class TestClass {
                public function test() {
                    var x = 5;
                // Missing closing brace
        "#;
        
        let result = parse_haxe_enhanced(input, Some("test.hx"));
        assert!(!result.is_success());
        
        let formatted = result.format_errors();
        assert!(formatted.contains("error"));
    }
    
    #[test]
    fn test_enhanced_parser_invalid_identifier() {
        let input = r#"
            class TestClass {
                var class = "invalid"; // "class" is a reserved keyword
            }
        "#;
        
        let result = parse_haxe_enhanced(input, Some("test.hx"));
        assert!(!result.is_success());
        
        let formatted = result.format_errors();
        assert!(formatted.contains("reserved keyword") || formatted.contains("error"));
    }
}
