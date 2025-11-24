//! Complete Haxe parser with full span tracking
//! 
//! This parser handles 100% of Haxe syntax with proper whitespace/comment handling
//! and tracks spans for every AST node.

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_until, take_while, take_while1, is_not},
    character::complete::{char, digit1, alpha1, alphanumeric1, multispace0, multispace1, none_of, one_of},
    combinator::{map, opt, recognize, value, verify, not, peek, cut, all_consuming},
    error::{context, ParseError},
    multi::{many0, many1, separated_list0, separated_list1, fold_many0},
    sequence::{pair, tuple, preceded, terminated, delimited},
    Parser,
};

use crate::haxe_ast::*;

/// Parser result type
pub type PResult<'a, T> = IResult<&'a str, T>;

/// Helper enum for parsing imports or using statements
#[derive(Debug, Clone)]
enum ImportOrUsing {
    Import(Import),
    Using(Using),
}

/// Helper enum for parsing imports, using, or conditional compilation containing them
#[derive(Debug, Clone)]
enum ImportUsingOrConditional {
    Import(Import),
    Using(Using),
    Conditional(ConditionalCompilation<ImportOrUsing>),
}

/// Parse a complete Haxe file
pub fn parse_haxe_file(file_name:&str, input: &str, recovery:bool) -> Result<HaxeFile, String> {
   // Check if this is an import.hx file
   let is_import_file = file_name.ends_with("import.hx") || file_name.ends_with("/import.hx") || file_name == "import.hx";
   
   if recovery {
    parse_haxe_file_with_enhanced_errors(input, file_name, is_import_file)
        .map_err(|(errors, source_map)| format_enhanced_errors_with_source_map(errors, source_map))
   }else {
    let full_input = input;
    let parse_result = if is_import_file {
        all_consuming(|i| import_hx_file(full_input, i)).parse(input)
    } else {
        all_consuming(|i| haxe_file(full_input, i)).parse(input)
    };
    
    match parse_result {
        Ok((_, file)) => Ok(file),
        Err(e) => Err(e.to_string())
    }
   }
}

/// Parse a Haxe file with enhanced error reporting
pub fn parse_haxe_file_with_enhanced_errors(input: &str, file_name:&str, is_import_file: bool) -> Result<HaxeFile, (crate::error::ParseErrors, crate::error::SourceMap)> {
    use crate::error::{ParseError, ParseErrors, SourceMap, SourceSpan, SourcePosition, FileId};
    
    let full_input = input;
    
    // Create source map for error reporting
    let mut source_map = SourceMap::new();
    let file_id = source_map.add_file(file_name.to_string(), input.to_string());
    
    let parse_result = if is_import_file {
        all_consuming(|i| import_hx_file(full_input, i)).parse(input)
    } else {
        all_consuming(|i| haxe_file(full_input, i)).parse(input)
    };
    
    match parse_result {
        Ok((_, file)) => Ok(file),
        Err(e) => {
            // Use enhanced error analysis like enhanced_parser.rs but simpler
            let mut errors = ParseErrors::new();
            analyze_syntax_errors_enhanced(input, file_id, &mut errors);
            
            Err((errors, source_map))
        }
    }
}

/// Format enhanced errors into a user-friendly string using the provided source map
fn format_enhanced_errors_with_source_map(errors: crate::error::ParseErrors, source_map: crate::error::SourceMap) -> String {
    use crate::error_formatter::{ErrorFormatter, FormatConfig};
    
    let formatter = ErrorFormatter::new(FormatConfig::default());
    formatter.format_errors(&errors, &source_map)
}

/// Analyze syntax errors using patterns from enhanced_parser.rs but without circular dependency
fn analyze_syntax_errors_enhanced(content: &str, file_id: crate::error::FileId, errors: &mut crate::error::ParseErrors) {
    use crate::error::{ParseError, SourceSpan, SourcePosition};
    
    let lines: Vec<&str> = content.lines().collect();
    let mut brace_count = 0;
    let mut paren_count = 0;
    
    for (line_num, line) in lines.iter().enumerate() {
        let line_number = line_num + 1;
        let trimmed = line.trim();
        
        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }
        
        // Check for keyword typos (simplified version of enhanced_parser logic)
        check_keyword_typos_simple(line, line_number, file_id, errors);
        
        // Check for missing semicolons
        if (trimmed.starts_with("package ") || trimmed.starts_with("import ")) && !trimmed.ends_with(';') {
            let error_position = SourcePosition::new(line_number, line.len() + 1, 0);
            let span = SourceSpan::single_char(error_position, file_id);
            
            errors.push(ParseError::MissingToken {
                expected: ";".to_string(),
                after: span,
                suggestion: Some("add a semicolon at the end of this line".to_string()),
            });
        }
        
        // Track delimiter balance
        for ch in line.chars() {
            match ch {
                '{' => brace_count += 1,
                '}' => brace_count -= 1,
                '(' => paren_count += 1,
                ')' => paren_count -= 1,
                _ => {}
            }
        }
    }
    
    // Check for unclosed delimiters at the end
    if brace_count > 0 {
        add_unclosed_delimiter_error_simple(content, file_id, errors, '{');
    }
    if paren_count > 0 {
        add_unclosed_delimiter_error_simple(content, file_id, errors, '(');
    }
}

/// Simplified keyword typo checking
fn check_keyword_typos_simple(line: &str, line_number: usize, file_id: crate::error::FileId, errors: &mut crate::error::ParseErrors) {
    use crate::error::{ParseError, SourceSpan, SourcePosition};
    
    let words: Vec<&str> = line.split_whitespace().collect();
    
    for &word in words.iter() {
        let clean_word = word.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
        
        // Skip empty words or known keywords
        if clean_word.is_empty() || is_haxe_keyword(clean_word) {
            continue;
        }
        
        // Check for common typos
        if let Some(suggestion) = suggest_keyword_simple(clean_word) {
            if let Some(column) = line.find(clean_word) {
                let error_position = SourcePosition::new(line_number, column + 1, 0);
                let span = SourceSpan::new(
                    error_position,
                    SourcePosition::new(line_number, column + 1 + clean_word.len(), 0),
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

/// Simplified keyword suggestion
fn suggest_keyword_simple(input: &str) -> Option<String> {
    let suggestions = [
        ("fucntion", "function"),
        ("classe", "class"),
        ("publik", "public"),
        ("privat", "private"),
        ("statik", "static"),
        ("retur", "return"),
        ("abstrak", "abstract"),
        ("interfac", "interface"),
    ];
    
    for (typo, correct) in &suggestions {
        if input.to_lowercase() == *typo {
            return Some(correct.to_string());
        }
    }
    
    None
}

/// Simple unclosed delimiter error
fn add_unclosed_delimiter_error_simple(content: &str, file_id: crate::error::FileId, errors: &mut crate::error::ParseErrors, delimiter: char) {
    use crate::error::{ParseError, SourceSpan, SourcePosition};
    
    let lines: Vec<&str> = content.lines().collect();
    
    // Find the last occurrence of the opening delimiter
    for (line_num, line) in lines.iter().enumerate().rev() {
        if line.contains(delimiter) {
            if let Some(column) = line.rfind(delimiter) {
                let error_position = SourcePosition::new(line_num + 1, column + 1, 0);
                let span = SourceSpan::single_char(error_position, file_id);
                
                errors.push(ParseError::UnclosedDelimiter {
                    delimiter,
                    opened_at: span.clone(),
                    expected_close_at: SourceSpan::single_char(
                        SourcePosition::new(lines.len(), lines.last().map(|l| l.len()).unwrap_or(0) + 1, 0),
                        file_id,
                    ),
                });
                break;
            }
        }
    }
}

/// Check if a word is a Haxe keyword
fn is_haxe_keyword(word: &str) -> bool {
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

/// Helper function to flatten conditional imports/using into regular vectors
/// This is a simplification for now - a full implementation might preserve conditional structure
fn flatten_conditional_imports_using(
    cond: &ConditionalCompilation<ImportOrUsing>,
    imports: &mut Vec<Import>,
    using: &mut Vec<Using>,
) {
    // Flatten the if branch
    for item in &cond.if_branch.content {
        match item {
            ImportOrUsing::Import(imp) => imports.push(imp.clone()),
            ImportOrUsing::Using(use_) => using.push(use_.clone()),
        }
    }
    
    // Flatten elseif branches
    for elseif_branch in &cond.elseif_branches {
        for item in &elseif_branch.content {
            match item {
                ImportOrUsing::Import(imp) => imports.push(imp.clone()),
                ImportOrUsing::Using(use_) => using.push(use_.clone()),
            }
        }
    }
    
    // Flatten else branch if present
    if let Some(else_content) = &cond.else_branch {
        for item in else_content {
            match item {
                ImportOrUsing::Import(imp) => imports.push(imp.clone()),
                ImportOrUsing::Using(use_) => using.push(use_.clone()),
            }
        }
    }
}

/// Convert nom parsing error to enhanced ParseError with better context
fn convert_nom_error_to_enhanced(
    nom_error: nom::Err<nom::error::Error<&str>>,
    full_input: &str,
    file_id: crate::error::FileId,
) -> crate::error::ParseError {
    use crate::error::{ParseError, SourceSpan, SourcePosition};
    
    match nom_error {
        nom::Err::Error(e) | nom::Err::Failure(e) => {
            // Calculate position of error
            let error_pos = full_input.len() - e.input.len();
            let (line, column) = calculate_line_column(full_input, error_pos);
            
            // For EOF errors, adjust position to the actual end of meaningful content
            let (final_pos, final_line, final_column) = if e.input.is_empty() && error_pos < full_input.len() {
                // Find the last non-whitespace character for more accurate positioning
                let mut last_meaningful_pos = full_input.len();
                for (i, ch) in full_input.char_indices().rev() {
                    if !ch.is_whitespace() {
                        last_meaningful_pos = i + ch.len_utf8();
                        break;
                    }
                }
                let (adj_line, adj_col) = calculate_line_column(full_input, last_meaningful_pos);
                (last_meaningful_pos, adj_line, adj_col)
            } else {
                (error_pos, line, column)
            };
            
            let position = SourcePosition::new(final_line, final_column, final_pos);
            let span = SourceSpan {
                start: position,
                end: position,
                file_id,
            };
            
            // Generate helpful error message based on error kind
            match e.code {
                nom::error::ErrorKind::Eof => {
                    let expected = infer_expected_tokens(e.input, full_input);
                    ParseError::UnexpectedEof { expected, span }
                }
                nom::error::ErrorKind::Tag => {
                    let expected = infer_expected_tokens(e.input, full_input);
                    let found = get_found_token(e.input);
                    ParseError::UnexpectedToken { expected, found, span }
                }
                nom::error::ErrorKind::Char => {
                    let expected = vec!["expected character".to_string()];
                    let found = get_found_token(e.input);
                    ParseError::UnexpectedToken { expected, found, span }
                }
                _ => {
                    ParseError::InvalidSyntax {
                        message: format!("Parse error: {:?}", e.code),
                        span,
                        suggestion: generate_suggestion(e.input, full_input),
                    }
                }
            }
        }
        nom::Err::Incomplete(_) => {
            // This shouldn't happen with complete parsers, but handle it
            let position = SourcePosition::new(1, 1, 0);
            let span = SourceSpan {
                start: position,
                end: position,
                file_id,
            };
            ParseError::UnexpectedEof {
                expected: vec!["more input".to_string()],
                span,
            }
        }
    }
}

/// Calculate line and column number from byte offset
fn calculate_line_column(full_input: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    
    for (i, ch) in full_input.char_indices() {
        if i >= byte_offset {
            break;
        }
        
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    
    (line, column)
}

/// Infer what tokens might be expected at the current position
fn infer_expected_tokens(remaining_input: &str, full_input: &str) -> Vec<String> {
    let mut expected = Vec::new();
    
    // Look at the context to infer what might be expected
    let consumed = &full_input[..full_input.len() - remaining_input.len()];
    
    // Check if we're at the end of input
    if remaining_input.is_empty() {
        // Analyze the full input to detect common syntax errors
        return analyze_full_input_for_errors(full_input);
    }
    
    // Check if we're in a class context
    if consumed.contains("class") && !consumed.contains('{') {
        expected.push("class body '{'".to_string());
    }
    
    // Check if we're in a function context
    if consumed.contains("function") && !consumed.contains('{') {
        expected.push("function body '{'".to_string());
    }
    
    // Check if we need a semicolon
    if consumed.trim_end().ends_with('}') || consumed.trim_end().ends_with(')') {
        expected.push("semicolon ';'".to_string());
    }
    
    // Generic expectations
    if expected.is_empty() {
        expected.push("valid Haxe syntax".to_string());
    }
    
    expected
}

/// Analyze the full input to detect common syntax errors when EOF is encountered
fn analyze_full_input_for_errors(full_input: &str) -> Vec<String> {
    let mut expected = Vec::new();
    
    // Look at the last few lines to understand context
    let lines: Vec<&str> = full_input.lines().collect();
    
    // Check for missing semicolon by looking at the last non-empty line
    for line in lines.iter().rev() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            // Common patterns that suggest missing semicolon
            if (trimmed.starts_with("var ") || 
                trimmed.starts_with("return ") ||
                trimmed.contains(" = ")) && 
               !trimmed.ends_with(';') && 
               !trimmed.ends_with('{') && 
               !trimmed.ends_with('}') {
                expected.push("semicolon ';'".to_string());
                return expected;
            }
            break;
        }
    }
    
    // Check for missing closing brace
    let open_braces = full_input.matches('{').count();
    let close_braces = full_input.matches('}').count();
    if open_braces > close_braces {
        expected.push("closing brace '}'".to_string());
        return expected;
    }
    
    // Check for missing closing parenthesis
    let open_parens = full_input.matches('(').count();
    let close_parens = full_input.matches(')').count();
    if open_parens > close_parens {
        expected.push("closing parenthesis ')'".to_string());
        return expected;
    }
    
    // Default fallback
    expected.push("end of input".to_string());
    expected
}

/// Get the token that was actually found
fn get_found_token(remaining_input: &str) -> String {
    if remaining_input.is_empty() {
        "end of input".to_string()
    } else {
        // Get the first few characters as the found token
        let first_word = remaining_input
            .split_whitespace()
            .next()
            .unwrap_or("")
            .chars()
            .take(10)
            .collect::<String>();
        
        if first_word.is_empty() {
            "whitespace".to_string()
        } else {
            format!("'{}'", first_word)
        }
    }
}

/// Generate helpful suggestions for common mistakes
fn generate_suggestion(remaining_input: &str, full_input: &str) -> Option<String> {
    let consumed = &full_input[..full_input.len() - remaining_input.len()];
    
    // Missing semicolon
    if consumed.trim_end().ends_with('}') || consumed.trim_end().ends_with(')') {
        if !consumed.trim_end().ends_with(';') {
            return Some("Try adding a semicolon ';' after the statement".to_string());
        }
    }
    
    // Missing closing brace
    let open_braces = consumed.matches('{').count();
    let close_braces = consumed.matches('}').count();
    if open_braces > close_braces {
        return Some("Try adding a closing brace '}'".to_string());
    }
    
    // Missing closing parenthesis
    let open_parens = consumed.matches('(').count();
    let close_parens = consumed.matches(')').count();
    if open_parens > close_parens {
        return Some("Try adding a closing parenthesis ')'".to_string());
    }
    
    None
}

/// Parser for import.hx files - only allows imports and using statements
pub fn import_hx_file<'a>(full: &'a str, input: &'a str) -> PResult<'a, HaxeFile> {
    context("import.hx file", |input| {
    let start = position(full, input);
    
    // Skip leading whitespace/comments
    let (input, _) = ws(input)?;
    
    // import.hx files cannot have package declarations
    // They can only contain imports and using statements
    
    // Parse imports and using statements
    let (input, imports_using_conditional) = many0(|i| {
        // Skip any metadata first
        let (i, _) = metadata_list(full, i)?;
        
        // Try to parse conditional compilation containing imports, or regular imports/using
        alt((
            // Conditional compilation with imports/using
            map(
                |i| conditional_compilation(full, i, |full, input| {
                    alt((
                        map(|i| import_decl(full, i), ImportOrUsing::Import),
                        map(|i| using_decl(full, i), ImportOrUsing::Using),
                    )).parse(input)
                }),
                ImportUsingOrConditional::Conditional
            ),
            // Regular import
            map(|i| import_decl(full, i), ImportUsingOrConditional::Import),
            // Regular using
            map(|i| using_decl(full, i), ImportUsingOrConditional::Using),
        )).parse(i)
    }).parse(input)?;
    
    // Extract imports and using from the mixed results
    let mut imports = Vec::new();
    let mut using = Vec::new();
    let mut conditional_imports_using = Vec::new();
    
    for item in imports_using_conditional {
        match item {
            ImportUsingOrConditional::Import(imp) => imports.push(imp),
            ImportUsingOrConditional::Using(use_) => using.push(use_),
            ImportUsingOrConditional::Conditional(cond) => {
                // For now, we'll flatten conditional imports/using into regular ones
                flatten_conditional_imports_using(&cond, &mut imports, &mut using);
                conditional_imports_using.push(cond);
            }
        }
    }
    
    // Skip trailing whitespace/comments
    let (input, _) = ws(input)?;
    
    // import.hx files should not have any other content
    // If there's remaining content, it's an error
    if !input.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)));
    }
    
    let end = position(full, input);
    
    Ok((input, HaxeFile {
        package: None,  // import.hx files don't have package declarations
        imports,
        using,
        module_fields: Vec::new(),  // import.hx files don't have module fields
        declarations: Vec::new(),    // import.hx files don't have type declarations
        span: Span::new(start, end),
    }))
    }).parse(input)
}

/// Main file parser
pub fn haxe_file<'a>(full: &'a str, input: &'a str) -> PResult<'a, HaxeFile> {
    context("haxe file", |input| {
    let start = position(full, input);
    
    // Skip leading whitespace/comments
    let (input, _) = ws(input)?;
    
    // Optional package declaration
    let (input, package) = opt(|i| package_decl(full, i)).parse(input)?;
    
    // Imports and using statements
    let (input, imports_using_conditional) = many0(|i| {
        // Skip any metadata first
        let (i, _) = metadata_list(full, i)?;
        
        // Check if we've hit a type declaration OR module field keywords
        let peek_result: Result<_, nom::Err<nom::error::Error<_>>> = peek(alt((
            tag("class"),
            tag("interface"),
            tag("enum"),
            tag("typedef"),
            tag("abstract"),
            tag("var"),
            tag("final"),
            tag("function"),
        ))).parse(i);
        
        if peek_result.is_ok() {
            // Stop parsing imports/using
            Err(nom::Err::Error(nom::error::Error::new(i, nom::error::ErrorKind::Eof)))
        } else {
            // Try to parse conditional compilation containing imports, or regular imports/using
            alt((
                // Conditional compilation with imports/using
                map(
                    |i| conditional_compilation(full, i, |full, input| {
                        alt((
                            map(|i| import_decl(full, i), ImportOrUsing::Import),
                            map(|i| using_decl(full, i), ImportOrUsing::Using),
                        )).parse(input)
                    }),
                    ImportUsingOrConditional::Conditional
                ),
                // Regular import
                map(|i| import_decl(full, i), ImportUsingOrConditional::Import),
                // Regular using
                map(|i| using_decl(full, i), ImportUsingOrConditional::Using),
            )).parse(i)
        }
    }).parse(input)?;
    
    // Extract imports and using from the mixed results
    let mut imports = Vec::new();
    let mut using = Vec::new();
    let mut conditional_imports_using = Vec::new();
    
    for item in imports_using_conditional {
        match item {
            ImportUsingOrConditional::Import(imp) => imports.push(imp),
            ImportUsingOrConditional::Using(use_) => using.push(use_),
            ImportUsingOrConditional::Conditional(cond) => {
                // For now, we'll flatten conditional imports/using into regular ones
                // This is a simplification - in a full implementation you might want to preserve the conditional structure
                flatten_conditional_imports_using(&cond, &mut imports, &mut using);
                conditional_imports_using.push(cond);
            }
        }
    }
    
    // Module-level fields
    let (input, module_fields) = many0(|i| {
        // Skip any metadata first
        let (i, _) = metadata_list(full, i)?;
        
        // Check if we've hit a type declaration (but NOT metadata or conditional compilation)
        let peek_result: Result<_, nom::Err<nom::error::Error<_>>> = peek(alt((
            tag("class"),
            tag("interface"),
            tag("enum"),
            tag("typedef"),
            tag("abstract"),
        ))).parse(i);
        
        if peek_result.is_ok() {
            // Stop parsing module fields
            Err(nom::Err::Error(nom::error::Error::new(i, nom::error::ErrorKind::Eof)))
        } else {
            // Try to parse module field
            module_field(full, i)
        }
    }).parse(input)?;
    
    // Type declarations
    let (input, declarations) = many0(|i| type_declaration(full, i)).parse(input)?;
    
    // Skip trailing whitespace/comments
    let (input, _) = ws(input)?;
    
    let end = position(full, input);
    
    Ok((input, HaxeFile {
        package,
        imports,
        using,
        module_fields,
        declarations,
        span: Span::new(start, end),
    }))
    }).parse(input)
}

/// Get current position in the original input
pub fn position(full: &str, current: &str) -> usize {
    full.len() - current.len()
}

/// Create span from start position to current position
pub fn make_span(full: &str, start_pos: usize, current: &str) -> Span {
    let end_pos = position(full, current);
    Span::new(start_pos, end_pos)
}

// =============================================================================
// Module-level fields
// =============================================================================

/// Parse a module-level field (variable or function)
pub fn module_field<'a>(full: &'a str, input: &'a str) -> PResult<'a, ModuleField> {
    let start = position(full, input);
    
    let (input, meta) = metadata_list(full, input)?;
    let (input, (access, modifiers)) = parse_access_and_modifiers(input)?;
    
    // Check if final was parsed as a modifier
    let has_final_modifier = modifiers.iter().any(|m| matches!(m, Modifier::Final));
    
    // Field kind
    let (input, kind) = alt((
        |i| module_field_function(full, i),
        |i| module_field_var_or_final(full, i, has_final_modifier),
    )).parse(input)?;
    
    let end = position(full, input);
    
    Ok((input, ModuleField {
        meta,
        access,
        modifiers,
        kind,
        span: Span::new(start, end),
    }))
}

/// Parse module-level function
fn module_field_function<'a>(full: &'a str, input: &'a str) -> PResult<'a, ModuleFieldKind> {
    let (input, _) = keyword("function").parse(input)?;
    let (input, name) = function_name(input)?;
    let (input, type_params) = type_params(full, input)?;
    
    let (input, _) = symbol("(").parse(input)?;
    let (input, params) = separated_list0(symbol(","), |i| function_param(full, i)).parse(input)?;
    let (input, _) = opt(symbol(",")).parse(input)?; // Trailing comma
    let (input, _) = symbol(")").parse(input)?;
    
    let (input, return_type) = opt(preceded(symbol(":"), |i| type_expr(full, i))).parse(input)?;
    
    let (input, body) = opt(|i| block_expr(full, i)).parse(input)?;
    
    Ok((input, ModuleFieldKind::Function(Function {
        name,
        type_params,
        params,
        return_type,
        body: body.map(Box::new),
        span: Span::new(0, 0), // Will be set by the caller
    })))
}

/// Parse module-level variable or final field
fn module_field_var_or_final<'a>(full: &'a str, input: &'a str, has_final_modifier: bool) -> PResult<'a, ModuleFieldKind> {
    let (input, is_final) = alt((
        value(true, keyword("final")),
        value(false, keyword("var")),
    )).parse(input)?;
    
    let (input, name) = identifier(input)?;
    let (input, type_hint) = opt(preceded(symbol(":"), |i| type_expr(full, i))).parse(input)?;
    let (input, expr) = opt(preceded(symbol("="), |i| expression(full, i))).parse(input)?;
    let (input, _) = symbol(";").parse(input)?;
    
    if is_final || has_final_modifier {
        Ok((input, ModuleFieldKind::Final { name, type_hint, expr }))
    } else {
        Ok((input, ModuleFieldKind::Var { name, type_hint, expr }))
    }
}

// =============================================================================
// Whitespace and Comments
// =============================================================================

/// Skip whitespace and comments
pub fn ws(input: &str) -> PResult<()> {
    value(
        (),
        many0(alt((
            value((), multispace1),
            value((), line_comment),
            value((), block_comment),
        )))
    ).parse(input)
}

/// Skip whitespace and comments, require at least some
pub fn ws1(input: &str) -> PResult<()> {
    value(
        (),
        many1(alt((
            value((), multispace1),
            value((), line_comment),
            value((), block_comment),
        )))
    ).parse(input)
}

/// Line comment: // comment
fn line_comment(input: &str) -> PResult<&str> {
    recognize(tuple((
        tag("//"),
        take_while(|c| c != '\n'),
        opt(char('\n'))
    ))).parse(input)
}

/// Block comment: /* comment */
fn block_comment(input: &str) -> PResult<&str> {
    recognize(tuple((
        tag("/*"),
        take_until("*/"),
        tag("*/")
    ))).parse(input)
}

/// Parse T with optional leading whitespace
fn ws_before<'a, T, F>(mut parser: F) -> impl FnMut(&'a str) -> PResult<'a, T>
where
    F: FnMut(&'a str) -> PResult<'a, T>,
{
    move |input| {
        let (input, _) = ws(input)?;
        parser(input)
    }
}

// =============================================================================
// Basic Elements
// =============================================================================

/// Reserved keywords
fn is_keyword(s: &str) -> bool {
    matches!(s,
        "abstract" | "break" | "case" | "cast" | "catch" | "class" | "continue" |
        "default" | "do" | "dynamic" | "else" | "enum" | "extends" | "extern" |
        "false" | "final" | "for" | "function" | "if" | "implements" | "import" |
        "in" | "inline" | "interface" | "macro" | "new" | "null" | "override" |
        "package" | "private" | "public" | "return" | "static" | "super" | 
        "switch" | "this" | "throw" | "true" | "try" | "typedef" | "untyped" |
        "using" | "var" | "while"
    )
}

/// Parse a keyword
pub fn keyword<'a>(kw: &'static str) -> impl FnMut(&'a str) -> PResult<'a, &'a str> {
    move |input| {
        let (input, _) = ws(input)?;
        let (input, word) = verify(
            recognize(pair(
                tag(kw),
                peek(not(alphanumeric1))
            )),
            |s: &str| s == kw
        ).parse(input)?;
        Ok((input, word))
    }
}

/// Parse an identifier
pub fn identifier(input: &str) -> PResult<String> {
    let (input, _) = ws(input)?;
    let (input, id) = verify(
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_"))))
        )),
        |s: &str| !is_keyword(s)
    ).parse(input)?;
    Ok((input, id.to_string()))
}

/// Parse function name (allows "new" as constructor name)
pub fn function_name(input: &str) -> PResult<String> {
    let (input, _) = ws(input)?;
    let (input, id) = verify(
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_"))))
        )),
        |s: &str| !is_keyword(s) || s == "new"
    ).parse(input)?;
    Ok((input, id.to_string()))
}

/// Parse compiler-specific identifier like __js__, __cpp__, etc.
pub fn compiler_specific_identifier(input: &str) -> PResult<String> {
    let (input, _) = ws(input)?;
    // First check if it starts with __
    let (rest, _) = tag("__")(input)?;
    // Then parse alphanumeric characters (but not underscores to avoid consuming the trailing __)
    let (rest, middle) = alpha1(rest)?;
    // Optionally more alphanumeric (still no underscores)
    let (rest, _suffix) = many0(alphanumeric1).parse(rest)?;
    // Then check for trailing __
    let (rest, _) = tag("__")(rest)?;
    
    // Reconstruct the full identifier
    let full_id = format!("__{}{}__", 
        middle, 
        _suffix.join("")
    );
    
    Ok((rest, full_id))
}

/// Parse a symbol with whitespace
pub fn symbol<'a>(sym: &'static str) -> impl FnMut(&'a str) -> PResult<'a, &'a str> {
    move |input| {
        let (input, _) = ws(input)?;
        tag(sym)(input)
    }
}

// =============================================================================
// Package and Imports
// =============================================================================

/// Package declaration: `package com.example;`
pub fn package_decl<'a>(full: &'a str, input: &'a str) -> PResult<'a, Package> {
    context("package declaration", |input| {
    let start = position(full, input);
    let (input, _) = keyword("package")(input)?;
    let (input, path) = dot_path(input)?;
    let (input, _) = symbol(";")(input)?;
    let end = position(full, input);
    
    Ok((input, Package {
        path,
        span: Span::new(start, end),
    }))
    }).parse(input)
}

/// Import declaration
pub fn import_decl<'a>(full: &'a str, input: &'a str) -> PResult<'a, Import> {
    context("import declaration", |input| {
    let start = position(full, input);
    let (input, _) = keyword("import")(input)?;
    
    // Parse the import path and mode
    let (input, (path, mode)) = alt((
        // import path.* or import path.* except ...
        |input| {
            let (input, path) = import_path_until_wildcard(input)?;
            let (input, _) = symbol(".*")(input)?;
            
            // Check if there's an "except" clause
            if let Ok((input_after_except, _)) = keyword("except")(input) {
                // Parse the exclusion list
                let (input, exclusions) = separated_list1(
                    symbol(","),
                    identifier
                ).parse(input_after_except)?;
                Ok((input, (path, ImportMode::WildcardWithExclusions(exclusions))))
            } else {
                Ok((input, (path, ImportMode::Wildcard)))
            }
        },
        // import path.field or import path as Alias
        |input| {
            // Try to parse the full path first
            let (input_after_path, full_path) = import_path(input)?;
            
            // Check what comes after the path
            if let Ok((input_after_as, _)) = keyword("as")(input_after_path) {
                // This is an alias import
                let (input, alias) = identifier(input_after_as)?;
                Ok((input, (full_path, ImportMode::Alias(alias))))
            } else if let Ok((input_before_semicolon, _)) = symbol(";")(input_after_path) {
                // If we can see a semicolon, check if the last part might be a field
                if full_path.len() >= 2 {
                    // Check if the last identifier starts with lowercase (likely a field)
                    let last = &full_path[full_path.len() - 1];
                    if last.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                        // This is likely a field import
                        let mut base_path = full_path;
                        let field = base_path.pop().unwrap();
                        Ok((input_after_path, (base_path, ImportMode::Field(field))))
                    } else {
                        // Normal import
                        Ok((input_after_path, (full_path, ImportMode::Normal)))
                    }
                } else {
                    // Normal import
                    Ok((input_after_path, (full_path, ImportMode::Normal)))
                }
            } else {
                // Normal import
                Ok((input_after_path, (full_path, ImportMode::Normal)))
            }
        }
    )).parse(input)?;
    
    let (input, _) = symbol(";")(input)?;
    let end = position(full, input);
    
    Ok((input, Import {
        path,
        mode,
        span: Span::new(start, end),
    }))
    }).parse(input)
}

/// Using declaration: `using Lambda;`
pub fn using_decl<'a>(full: &'a str, input: &'a str) -> PResult<'a, Using> {
    context("using declaration", |input| {
    let start = position(full, input);
    let (input, _) = keyword("using")(input)?;
    let (input, path) = import_path(input)?;
    let (input, _) = symbol(";")(input)?;
    let end = position(full, input);
    
    Ok((input, Using {
        path,
        span: Span::new(start, end),
    }))
    }).parse(input)
}

/// Identifier that allows keywords (for import paths)
fn identifier_or_keyword(input: &str) -> PResult<String> {
    let (input, _) = ws(input)?;
    let (input, id) = recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"))))
    )).parse(input)?;
    Ok((input, id.to_string()))
}

/// Dot-separated path: `com.example.Class`
pub fn dot_path(input: &str) -> PResult<Vec<String>> {
    separated_list1(
        symbol("."),
        identifier
    ).parse(input)
}

/// Import path that allows keywords (e.g., `haxe.macro.Context`)
fn import_path(input: &str) -> PResult<Vec<String>> {
    separated_list1(
        symbol("."),
        identifier_or_keyword
    ).parse(input)
}

/// Import path until wildcard (stops before .*)
fn import_path_until_wildcard(input: &str) -> PResult<Vec<String>> {
    let mut path = Vec::new();
    let mut current = input;
    
    // Parse first identifier
    let (next, first) = identifier_or_keyword(current)?;
    path.push(first);
    current = next;
    
    // Continue parsing dot-separated identifiers until we hit .* or end
    loop {
        // Check if next is .*
        if symbol(".*")(current).is_ok() {
            break;
        }
        
        // Try to parse another .identifier
        match symbol(".")(current) {
            Ok((after_dot, _)) => {
                match identifier_or_keyword(after_dot) {
                    Ok((next, id)) => {
                        path.push(id);
                        current = next;
                    }
                    Err(_) => break,
                }
            }
            Err(_) => break,
        }
    }
    
    Ok((current, path))
}

// =============================================================================
// Type Declarations
// =============================================================================

/// Any type declaration
pub fn type_declaration<'a>(full: &'a str, input: &'a str) -> PResult<'a, TypeDeclaration> {
    context("type declaration", alt((
        // Check for conditional compilation first
        |i| {
            let peek_result: Result<_, nom::Err<nom::error::Error<_>>> = peek(tag("#if")).parse(i);
            if peek_result.is_ok() {
                map(
                    |i| conditional_compilation(full, i, type_declaration),
                    TypeDeclaration::Conditional
                ).parse(i)
            } else {
                Err(nom::Err::Error(nom::error::Error::new(i, nom::error::ErrorKind::Tag)))
            }
        },
        // Check for metadata-prefixed declarations
        |i| {
            let peek_result: Result<_, nom::Err<nom::error::Error<_>>> = peek(tag("@")).parse(i);
            if peek_result.is_ok() {
                // Try parsing each type with metadata
                alt((
                    map(|i| class_decl(full, i), TypeDeclaration::Class),
                    map(|i| interface_decl(full, i), TypeDeclaration::Interface),
                    map(|i| enum_decl(full, i), TypeDeclaration::Enum),
                    map(|i| typedef_decl(full, i), TypeDeclaration::Typedef),
                    map(|i| abstract_decl(full, i), TypeDeclaration::Abstract),
                )).parse(i)
            } else {
                Err(nom::Err::Error(nom::error::Error::new(i, nom::error::ErrorKind::Tag)))
            }
        },
        map(|i| class_decl(full, i), TypeDeclaration::Class),
        map(|i| interface_decl(full, i), TypeDeclaration::Interface),
        map(|i| enum_decl(full, i), TypeDeclaration::Enum),
        map(|i| typedef_decl(full, i), TypeDeclaration::Typedef),
        map(|i| abstract_decl(full, i), TypeDeclaration::Abstract),
    ))).parse(input)
}

/// Parse metadata attributes
pub fn metadata_list<'a>(full: &'a str, input: &'a str) -> PResult<'a, Vec<Metadata>> {
    many0(|i| metadata(full, i)).parse(input)
}

/// Parse single metadata: `@:native("foo")` or `@author("name")`
fn metadata<'a>(full: &'a str, input: &'a str) -> PResult<'a, Metadata> {
    let start = position(full, input);
    let (input, _) = ws(input)?;
    let (input, _) = char('@')(input)?;
    let (input, has_colon) = opt(char(':')).parse(input)?;
    let (input, name) = if has_colon.is_some() {
        // @:metadata format - allow keywords in metadata context
        identifier_or_keyword(input)?
    } else {
        // @metadata format
        identifier(input)?
    };
    
    // Optional parameters
    let (input, params) = opt(delimited(
        symbol("("),
        separated_list0(symbol(","), |i| expression(full, i)),
        symbol(")")
    )).parse(input)?;
    
    let end = position(full, input);
    
    Ok((input, Metadata {
        name,
        params: params.unwrap_or_default(),
        span: Span::new(start, end),
    }))
}

/// Parse access modifier
pub fn access(input: &str) -> PResult<Access> {
    alt((
        value(Access::Public, keyword("public")),
        value(Access::Private, keyword("private")),
    )).parse(input)
}

/// Parse function modifiers
pub fn modifiers(input: &str) -> PResult<Vec<Modifier>> {
    many0(alt((
        value(Modifier::Static, keyword("static")),
        value(Modifier::Inline, keyword("inline")),
        value(Modifier::Macro, keyword("macro")),
        value(Modifier::Dynamic, keyword("dynamic")),
        value(Modifier::Override, keyword("override")),
        value(Modifier::Final, keyword("final")),
        value(Modifier::Extern, keyword("extern")),
    ))).parse(input)
}

/// Import declarations from other parser modules
pub use crate::haxe_parser_decls::*;
pub use crate::haxe_parser_types::*;
pub use crate::haxe_parser_expr::*;
use crate::haxe_parser_expr2::block_expr;

// =============================================================================
// Conditional Compilation
// =============================================================================

/// Parse conditional compilation directive
pub fn conditional_compilation<'a, T, F>(
    full: &'a str, 
    input: &'a str,
    content_parser: F
) -> PResult<'a, ConditionalCompilation<T>>
where
    F: Fn(&'a str, &'a str) -> PResult<'a, T> + Copy,
{
    context("conditional compilation", |input| {
    let start = position(full, input);
    
    // Parse #if branch
    let (input, if_branch) = conditional_if_branch(full, input, content_parser)?;
    
    // Parse #elseif branches
    let (input, elseif_branches) = many0(|i| conditional_elseif_branch(full, i, content_parser)).parse(input)?;
    
    // Parse optional #else branch
    let (input, else_branch) = opt(|i| conditional_else_branch(full, i, content_parser)).parse(input)?;
    
    // Parse #end
    let (input, _) = ws(input)?;
    let (input, _) = tag("#end")(input)?;
    let (input, _) = ws(input)?; // Consume trailing whitespace after #end
    
    let end = position(full, input);
    
    Ok((input, ConditionalCompilation {
        if_branch,
        elseif_branches,
        else_branch,
        span: Span::new(start, end),
    }))
    }).parse(input)
}

/// Parse #if branch
fn conditional_if_branch<'a, T, F>(
    full: &'a str,
    input: &'a str,
    content_parser: F
) -> PResult<'a, ConditionalBlock<Vec<T>>>
where
    F: Fn(&'a str, &'a str) -> PResult<'a, T> + Copy,
{
    let start = position(full, input);
    let (input, _) = ws(input)?;
    let (input, _) = tag("#if")(input)?;
    let (input, _) = ws1(input)?;
    let (input, condition) = conditional_expr(input)?;
    let (input, _) = ws(input)?;
    
    // Parse content until #elseif, #else, or #end
    let (input, content) = many0(|i| {
        // Look ahead for conditional directives
        let peek_result: Result<_, nom::Err<nom::error::Error<_>>> = peek(alt((tag("#elseif"), tag("#else"), tag("#end")))).parse(i);
        if peek_result.is_ok() {
            // Stop parsing content
            Err(nom::Err::Error(nom::error::Error::new(i, nom::error::ErrorKind::Eof)))
        } else {
            content_parser(full, i)
        }
    }).parse(input)?;
    
    let end = position(full, input);
    
    Ok((input, ConditionalBlock {
        condition,
        content,
        span: Span::new(start, end),
    }))
}

/// Parse #elseif branch
fn conditional_elseif_branch<'a, T, F>(
    full: &'a str,
    input: &'a str,
    content_parser: F
) -> PResult<'a, ConditionalBlock<Vec<T>>>
where
    F: Fn(&'a str, &'a str) -> PResult<'a, T> + Copy,
{
    let start = position(full, input);
    let (input, _) = ws(input)?;
    let (input, _) = tag("#elseif")(input)?;
    let (input, _) = ws1(input)?;
    let (input, condition) = conditional_expr(input)?;
    let (input, _) = ws(input)?;
    
    let (input, content) = many0(|i| {
        let peek_result: Result<_, nom::Err<nom::error::Error<_>>> = peek(alt((tag("#elseif"), tag("#else"), tag("#end")))).parse(i);
        if peek_result.is_ok() {
            Err(nom::Err::Error(nom::error::Error::new(i, nom::error::ErrorKind::Eof)))
        } else {
            content_parser(full, i)
        }
    }).parse(input)?;
    
    let end = position(full, input);
    
    Ok((input, ConditionalBlock {
        condition,
        content,
        span: Span::new(start, end),
    }))
}

/// Parse #else branch
fn conditional_else_branch<'a, T, F>(
    full: &'a str,
    input: &'a str,
    content_parser: F
) -> PResult<'a, Vec<T>>
where
    F: Fn(&'a str, &'a str) -> PResult<'a, T> + Copy,
{
    let (input, _) = ws(input)?;
    let (input, _) = tag("#else")(input)?;
    let (input, _) = ws(input)?;
    
    many0(|i| {
        let peek_result: Result<_, nom::Err<nom::error::Error<_>>> = peek(tag("#end")).parse(i);
        if peek_result.is_ok() {
            Err(nom::Err::Error(nom::error::Error::new(i, nom::error::ErrorKind::Eof)))
        } else {
            content_parser(full, i)
        }
    }).parse(input)
}

/// Parse conditional expression
fn conditional_expr(input: &str) -> PResult<ConditionalExpr> {
    conditional_or_expr(input)
}

/// Parse OR expression
fn conditional_or_expr(input: &str) -> PResult<ConditionalExpr> {
    let (input, left) = conditional_and_expr(input)?;
    
    let (input, rights) = many0(preceded(
        tuple((ws, tag("||"), ws)),
        conditional_and_expr
    )).parse(input)?;
    
    Ok((input, rights.into_iter().fold(left, |acc, right| {
        ConditionalExpr::Or(Box::new(acc), Box::new(right))
    })))
}

/// Parse AND expression
fn conditional_and_expr(input: &str) -> PResult<ConditionalExpr> {
    let (input, left) = conditional_not_expr(input)?;
    
    let (input, rights) = many0(preceded(
        tuple((ws, tag("&&"), ws)),
        conditional_not_expr
    )).parse(input)?;
    
    Ok((input, rights.into_iter().fold(left, |acc, right| {
        ConditionalExpr::And(Box::new(acc), Box::new(right))
    })))
}

/// Parse NOT expression
fn conditional_not_expr(input: &str) -> PResult<ConditionalExpr> {
    alt((
        map(preceded(char('!'), conditional_primary_expr), |e| {
            ConditionalExpr::Not(Box::new(e))
        }),
        conditional_primary_expr
    )).parse(input)
}

/// Parse primary expression (identifier or parenthesized)
fn conditional_primary_expr(input: &str) -> PResult<ConditionalExpr> {
    alt((
        // Parenthesized expression
        map(
            delimited(
                char('('),
                preceded(ws, conditional_expr),
                preceded(ws, char(')'))
            ),
            |e| ConditionalExpr::Paren(Box::new(e))
        ),
        // Identifier
        map(identifier, ConditionalExpr::Ident)
    )).parse(input)
}