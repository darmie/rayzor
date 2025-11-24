//! Expression parsing for Haxe (continuation)
//!
//! This module contains the remaining expression parsers

use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::char,
    combinator::{map, opt, value, recognize},
    error::context,
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{pair, tuple, preceded, terminated, delimited},
    Parser,
};

use crate::haxe_ast::*;
use crate::haxe_parser::{ws, symbol, keyword, identifier, PResult, position, compiler_specific_identifier};
use crate::haxe_parser_types::type_expr;
use crate::haxe_parser_expr::expression;

// Simple expressions

pub fn identifier_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, id) = identifier(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Ident(id),
        span: Span::new(start, end),
    }))
}

/// Parse keyword as identifier (for contexts where keywords can be used as identifiers)
fn keyword_as_identifier(input: &str) -> PResult<String> {
    let (input, _) = ws(input)?;
    alt((
        map(keyword("macro"), |_| "macro".to_string()),
        // Add other keywords as needed
    )).parse(input)
}

/// Parse macro expression: `macro expr`
pub fn macro_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("macro").parse(input)?;
    // Parse at unary expression level to get proper precedence
    let (input, expr) = crate::haxe_parser_expr::unary_expr(full, input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Macro(Box::new(expr)),
        span: Span::new(start, end),
    }))
}

/// Parse dollar expression: either reification `$expr` or dollar identifier `$type`, `$v{...}`, etc.
pub fn reify_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = ws(input)?;
    let (input, _) = char('$')(input)?;
    
    // Try to parse as dollar identifier first
    if let Ok((rest, dollar_ident)) = dollar_identifier(full, input) {
        return Ok((rest, dollar_ident));
    }
    
    // Otherwise, parse as macro reification
    let (input, expr) = expression(full, input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Reify(Box::new(expr)),
        span: Span::new(start, end),
    }))
}

/// Parse dollar identifier: `$type`, `$v{...}`, `$i{...}`, etc.
fn dollar_identifier<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    
    // Parse the identifier name - only allow specific known dollar identifiers
    let (input, name) = alt((
        // Special compiler intrinsic identifiers
        map(tag("type"), |_| "type".to_string()),
        // Macro reification identifiers
        map(tag("v"), |_| "v".to_string()),
        map(tag("i"), |_| "i".to_string()),
        map(tag("a"), |_| "a".to_string()),
        map(tag("b"), |_| "b".to_string()),
        map(tag("p"), |_| "p".to_string()),
        map(tag("e"), |_| "e".to_string()),
    )).parse(input)?;
    
    // Check if there's an argument in braces
    let (input, arg) = if let Ok((rest, _)) = symbol("{").parse(input) {
        let (rest, expr) = expression(full, rest)?;
        let (rest, _) = symbol("}")(rest)?;
        (rest, Some(Box::new(expr)))
    } else {
        (input, None)
    };
    
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::DollarIdent { name, arg },
        span: Span::new(start, end),
    }))
}

pub fn this_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("this").parse(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::This,
        span: Span::new(start, end),
    }))
}

pub fn super_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("super").parse(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Super,
        span: Span::new(start, end),
    }))
}

pub fn null_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("null").parse(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Null,
        span: Span::new(start, end),
    }))
}

/// Parse compiler-specific code block: `__js__("console.log('hello')")`
pub fn compiler_specific_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, target) = compiler_specific_identifier(input)?;
    
    // Parse the code argument (single string argument expected)
    let (input, _) = symbol("(").parse(input)?;
    let (input, code) = expression(full, input)?;
    let (input, _) = symbol(")").parse(input)?;
    
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::CompilerSpecific {
            target,
            code: Box::new(code),
        },
        span: Span::new(start, end),
    }))
}

// Constructor and cast expressions

pub fn new_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("new").parse(input)?;
    
    // Parse type path
    let (input, path_parts) = separated_list1(symbol("."), identifier).parse(input)?;
    
    // Split into package and name
    let (package, name) = if path_parts.len() == 1 {
        (vec![], path_parts[0].clone())
    } else {
        let mut parts = path_parts;
        let name = parts.pop().unwrap();
        (parts, name)
    };
    
    let type_path = TypePath { package, name, sub: None };
    
    // Type parameters
    let (input, params) = opt(delimited(
        symbol("<"),
        separated_list1(symbol(","), |i| type_expr(full, i)),
        symbol(">")
    )).parse(input)?;
    
    // Arguments
    let (input, _) = symbol("(").parse(input)?;
    let (input, args) = separated_list0(symbol(","), |i| expression(full, i)).parse(input)?;
    let (input, _) = symbol(")").parse(input)?;
    
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::New {
            type_path,
            params: params.unwrap_or_default(),
            args,
        },
        span: Span::new(start, end),
    }))
}

pub fn cast_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("cast").parse(input)?;
    
    alt((
        // cast(expr, Type)
        map(
            delimited(
                symbol("("),
                pair(
                    |i| expression(full, i),
                    opt(preceded(symbol(","), |i| type_expr(full, i)))
                ),
                symbol(")")
            ),
            move |(expr, type_hint)| {
                let end = position(full, input);
                Expr {
                    kind: ExprKind::Cast {
                        expr: Box::new(expr),
                        type_hint,
                    },
                    span: Span::new(start, end),
                }
            }
        ),
        // cast expr
        map(
            |i| expression(full, i),
            move |expr| {
                let end = position(full, input);
                Expr {
                    kind: ExprKind::Cast {
                        expr: Box::new(expr),
                        type_hint: None,
                    },
                    span: Span::new(start, end),
                }
            }
        ),
    )).parse(input)
}

/// Parse untyped expression: `untyped expr`
pub fn untyped_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("untyped").parse(input)?;
    let (input, expr) = expression(full, input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Untyped(Box::new(expr)),
        span: Span::new(start, end),
    }))
}

// Collection literals

pub fn array_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = symbol("[").parse(input)?;
    
    // Check for comprehensions
    if let Ok((_, _)) = keyword("for").parse(input) {
        return array_comprehension(full, start, input);
    }
    
    // Check for map literal (has =>)
    let is_map = {
        let mut check_input = input;
        let mut depth = 0;
        let mut found_arrow = false;
        
        while !check_input.is_empty() && !found_arrow {
            if check_input.starts_with("=>") {
                found_arrow = true;
            } else if check_input.starts_with('[') {
                depth += 1;
            } else if check_input.starts_with(']') {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            check_input = &check_input[1..];
        }
        
        found_arrow
    };
    
    if is_map {
        map_literal(full, start, input)
    } else {
        array_literal(full, start, input)
    }
}

fn array_literal<'a>(full: &'a str, start: usize, input: &'a str) -> PResult<'a, Expr> {
    let (input, elements) = separated_list0(symbol(","), |i| expression(full, i)).parse(input)?;
    let (input, _) = opt(symbol(",")).parse(input)?; // Trailing comma
    let (input, _) = symbol("]").parse(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Array(elements),
        span: Span::new(start, end),
    }))
}

fn map_literal<'a>(full: &'a str, start: usize, input: &'a str) -> PResult<'a, Expr> {
    let (input, pairs) = separated_list0(
        symbol(","),
        map(
            tuple((
                |i| expression(full, i),
                symbol("=>"),
                |i| expression(full, i)
            )),
            |(k, _, v)| (k, v)
        )
    ).parse(input)?;
    
    let (input, _) = opt(symbol(",")).parse(input)?;
    let (input, _) = symbol("]").parse(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Map(pairs),
        span: Span::new(start, end),
    }))
}

fn array_comprehension<'a>(full: &'a str, start: usize, input: &'a str) -> PResult<'a, Expr> {
    let (input, for_parts) = many1(|i|comprehension_for(full, i)).parse(input)?;
    
    // Check for map comprehension (has =>)
    let check_result: IResult<_, _> = tuple((
        |i| expression(full, i),
        symbol("=>"),
        |i| expression(full, i)
    )).parse(input);
    
    let (input, kind) = if let Ok((rest, (key, _, value))) = check_result {
        (rest, ExprKind::MapComprehension {
            for_parts,
            key: Box::new(key),
            value: Box::new(value),
        })
    } else {
        let (rest, expr) = expression(full, input)?;
        (rest, ExprKind::ArrayComprehension {
            for_parts,
            expr: Box::new(expr),
        })
    };
    
    let (input, _) = symbol("]").parse(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind,
        span: Span::new(start, end),
    }))
}

fn comprehension_for<'a>(full: &'a str, input: &'a str) -> PResult<'a, ComprehensionFor> {
    let start = position(full, input);
    let (input, _) = keyword("for").parse(input)?;
    let (input, _) = symbol("(").parse(input)?;
    
    // Try to parse key => value pattern first
    let (input, var, key_var) = match parse_key_value_pattern(input) {
        Ok((rest, (key, value))) => (rest, value, Some(key)),
        Err(_) => {
            // Fall back to simple variable
            let (rest, var) = identifier(input)?;
            (rest, var, None)
        }
    };
    
    let (input, _) = keyword("in").parse(input)?;
    let (input, iter) = expression(full, input)?;
    let (input, _) = symbol(")").parse(input)?;
    let end = position(full, input);
    
    Ok((input, ComprehensionFor {
        var,
        key_var,
        iter,
        span: Span::new(start, end),
    }))
}

pub fn object_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = symbol("{").parse(input)?;
    let (input, fields) = separated_list0(symbol(","), |i| object_field(full, i)).parse(input)?;
    let (input, _) = opt(symbol(",")).parse(input)?;
    let (input, _) = symbol("}").parse(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Object(fields),
        span: Span::new(start, end),
    }))
}

fn object_field<'a>(full: &'a str, input: &'a str) -> PResult<'a, ObjectField> {
    let start = position(full, input);
    let (input, name) = identifier(input)?;
    let (input, _) = symbol(":").parse(input)?;
    let (input, expr) = expression(full, input)?;
    let end = position(full, input);
    
    Ok((input, ObjectField {
        name,
        expr,
        span: Span::new(start, end),
    }))
}

// Control flow expressions

pub fn block_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = context("expected '{' to start block", symbol("{")).parse(input)?;
    let (input, elements) = context("expected block contents", many0(|i|block_element(full, i))).parse(input)?;
    let (input, _) = context("expected '}' to close block", symbol("}")).parse(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Block(elements),
        span: Span::new(start, end),
    }))
}

fn block_element<'a>(full: &'a str, input: &'a str) -> PResult<'a, BlockElement> {
    alt((
        // Conditional compilation inside block
        |i| {
            let peek_result: Result<_, nom::Err<nom::error::Error<_>>> = nom::combinator::peek(tag("#if")).parse(i);
            if peek_result.is_ok() {
                map(
                    |i| crate::haxe_parser::conditional_compilation(full, i, block_element),
                    BlockElement::Conditional
                ).parse(i)
            } else {
                Err(nom::Err::Error(nom::error::Error::new(i, nom::error::ErrorKind::Tag)))
            }
        },
        // Import/using inside block
        map(|i| crate::haxe_parser::import_decl(full, i), BlockElement::Import),
        map(|i| crate::haxe_parser::using_decl(full, i), BlockElement::Using),
        // Expression - semicolon is optional for control flow statements
        map(
            |i| {
                let (input, expr) = expression(full, i)?;
                // Check if this is a control flow statement that doesn't need semicolon
                let needs_semicolon = match &expr.kind {
                    ExprKind::If { .. } | ExprKind::Switch { .. } | ExprKind::For { .. } | 
                    ExprKind::While { .. } | ExprKind::DoWhile { .. } | ExprKind::Try { .. } |
                    ExprKind::Block(_) => false,
                    _ => true,
                };
                
                if needs_semicolon {
                    // Provide specific context based on the expression type
                    let error_msg = match &expr.kind {
                        ExprKind::Var { .. } => "expected ';' after variable declaration",
                        ExprKind::Final { .. } => "expected ';' after final variable declaration", 
                        ExprKind::Assign { .. } => "expected ';' after assignment",
                        ExprKind::Call { .. } => "expected ';' after function call",
                        ExprKind::Return { .. } => "expected ';' after return statement",
                        ExprKind::Break { .. } => "expected ';' after break statement",
                        ExprKind::Continue { .. } => "expected ';' after continue statement",
                        ExprKind::Throw { .. } => "expected ';' after throw statement",
                        _ => "expected ';' after statement",
                    };
                    let (input, _) = context(error_msg, symbol(";")).parse(input)?;
                    Ok((input, expr))
                } else {
                    // Optional semicolon for control flow statements
                    let (input, _) = opt(symbol(";")).parse(input)?;
                    Ok((input, expr))
                }
            },
            BlockElement::Expr
        ),
    )).parse(input)
}

pub fn if_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("if").parse(input)?;
    let (input, _) = symbol("(").parse(input)?;
    let (input, cond) = expression(full, input)?;
    let (input, _) = symbol(")").parse(input)?;
    let (input, then_branch) = expression(full, input)?;
    
    let (input, else_branch) = opt(preceded(
        keyword("else"),
        |i| expression(full, i)
    )).parse(input)?;
    
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::If {
            cond: Box::new(cond),
            then_branch: Box::new(then_branch),
            else_branch: else_branch.map(Box::new),
        },
        span: Span::new(start, end),
    }))
}

pub fn switch_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("switch").parse(input)?;
    let (input, _) = context("expected '(' after 'switch'", symbol("(")).parse(input)?;
    let (input, expr) = context("expected expression inside switch parentheses", |i| expression(full, i)).parse(input)?;
    let (input, _) = context("expected ')' after switch expression", symbol(")")).parse(input)?;
    let (input, _) = context("expected '{' to start switch body", symbol("{")).parse(input)?;
    
    let (input, cases) = context("expected switch cases", many0(|i| case(full, i))).parse(input)?;
    
    let (input, default) = opt(preceded(
        keyword("default"),
        preceded(
            context("expected ':' after 'default'", symbol(":")), 
            context("expected default case body", |i| parse_case_body(full, i))
        )
    )).parse(input)?;
    
    let (input, _) = context("expected '}' to close switch body", symbol("}")).parse(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::Switch {
            expr: Box::new(expr),
            cases,
            default: default.map(Box::new),
        },
        span: Span::new(start, end),
    }))
}

fn case<'a>(full: &'a str, input: &'a str) -> PResult<'a, Case> {
    let start = position(full, input);
    let (input, _) = keyword("case").parse(input)?;
    let (input, patterns) = context("expected pattern(s) after 'case'", 
        separated_list1(symbol("|"), |i| pattern(full, i))
    ).parse(input)?;
    
    let (input, guard) = opt(preceded(
        keyword("if"),
        delimited(
            context("expected '(' after 'if' in case guard", symbol("(")),
            context("expected guard expression", |i| expression(full, i)),
            context("expected ')' after guard expression", symbol(")"))
        )
    )).parse(input)?;
    
    let (input, _) = context("expected ':' after case pattern", symbol(":")).parse(input)?;
    
    // Parse case body as a sequence of statements until next case/default/}
    let (input, body) = context("expected case body", |i| parse_case_body(full, i)).parse(input)?;
    let end = position(full, input);
    
    Ok((input, Case {
        patterns,
        guard,
        body,
        span: Span::new(start, end),
    }))
}

fn parse_case_body<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let mut statements = Vec::new();
    let mut current_input = input;
    
    // Parse statements until we hit a case, default, or closing brace
    loop {
        // Skip whitespace
        let (input, _) = ws(current_input)?;
        current_input = input;
        
        // Check for terminating conditions
        if current_input.is_empty() ||
           current_input.starts_with("}") {
            break;
        }
        
        // Check for case/default keywords
        let trimmed = current_input.trim_start();
        if trimmed.starts_with("case ") ||
           trimmed.starts_with("default:") || 
           trimmed.starts_with("default ") {
            break;
        }
        
        // Parse next statement
        let (input, expr) = expression(full, current_input)?;
        statements.push(BlockElement::Expr(expr));
        
        // Check if this is a control flow statement that doesn't need semicolon
        let needs_semicolon = match &statements.last().unwrap() {
            BlockElement::Expr(expr) => match &expr.kind {
                ExprKind::If { .. } | ExprKind::Switch { .. } | ExprKind::For { .. } | 
                ExprKind::While { .. } | ExprKind::DoWhile { .. } | ExprKind::Try { .. } |
                ExprKind::Block(_) => false,
                _ => true,
            },
            _ => true,
        };
        
        if needs_semicolon {
            let (input, _) = symbol(";")(input)?;
            current_input = input;
        } else {
            // Optional semicolon for control flow statements
            let (input, _) = opt(symbol(";")).parse(input)?;
            current_input = input;
        }
    }
    
    let end = position(full, current_input);
    
    // If we have no statements, return an empty block
    if statements.is_empty() {
        Ok((current_input, Expr {
            kind: ExprKind::Block(vec![]),
            span: Span::new(start, end),
        }))
    } else if statements.len() == 1 {
        // Single statement - return it directly
        let first_stmt = statements.into_iter().next().unwrap();
        if let BlockElement::Expr(expr) = first_stmt {
            Ok((current_input, expr))
        } else {
            Ok((current_input, Expr {
                kind: ExprKind::Block(vec![first_stmt]),
                span: Span::new(start, end),
            }))
        }
    } else {
        // Multiple statements - wrap in a block
        Ok((current_input, Expr {
            kind: ExprKind::Block(statements),
            span: Span::new(start, end),
        }))
    }
}

fn pattern<'a>(full: &'a str, input: &'a str) -> PResult<'a, Pattern> {
    alt((
        // Null pattern
        map(keyword("null"), |_| Pattern::Null),
        
        // Type pattern: (var:Type)
        |input| {
            if let Ok((input, _)) = symbol("(").parse(input) {
                if let Ok((rest, var)) = identifier(input) {
                    if let Ok((rest, _)) = symbol(":").parse(rest) {
                        if let Ok((rest, type_hint)) = type_expr(full, rest) {
                            if let Ok((rest, _)) = symbol(")").parse(rest) {
                                return Ok((rest, Pattern::Type { var, type_hint }));
                            }
                        }
                    }
                }
            }
            Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
        },
        
        // Object pattern: {x: 0, y: 0}
        |input| {
            if let Ok((input, _)) = symbol("{").parse(input) {
                let (input, fields) = separated_list0(
                    symbol(","),
                    |i| {
                        let (i, field_name) = identifier(i)?;
                        let (i, _) = symbol(":").parse(i)?;
                        let (i, field_pattern) = pattern(full, i)?;
                        Ok((i, (field_name, field_pattern)))
                    }
                ).parse(input)?;
                let (input, _) = symbol("}").parse(input)?;
                Ok((input, Pattern::Object { fields }))
            } else {
                Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
            }
        },
        
        // Array pattern with possible rest
        |input| {
            if let Ok((input, _)) = symbol("[").parse(input) {
                let mut elements = Vec::new();
                let mut rest = None;
                let mut current_input = input;
                
                loop {
                    // Skip whitespace
                    let (input, _) = ws(current_input)?;
                    current_input = input;
                    
                    // Check for closing bracket
                    if let Ok((input, _)) = symbol("]").parse(current_input) {
                        current_input = input;
                        break;
                    }
                    
                    // Check for rest pattern
                    if let Ok((input, _)) = symbol("...").parse(current_input) {
                        let (input, rest_var) = identifier(input)?;
                        rest = Some(rest_var);
                        current_input = input;
                        
                        // Skip optional comma
                        if let Ok((input, _)) = symbol(",").parse(current_input) {
                            current_input = input;
                        }
                        
                        // Must be at end
                        let (input, _) = symbol("]").parse(current_input)?;
                        current_input = input;
                        break;
                    }
                    
                    // Parse regular pattern
                    let (input, pat) = pattern(full, current_input)?;
                    elements.push(pat);
                    current_input = input;
                    
                    // Check for comma
                    if let Ok((input, _)) = symbol(",").parse(current_input) {
                        current_input = input;
                    } else {
                        // No comma, must be at end
                        let (input, _) = symbol("]").parse(current_input)?;
                        current_input = input;
                        break;
                    }
                }
                
                if rest.is_some() {
                    Ok((current_input, Pattern::ArrayRest { elements, rest }))
                } else {
                    Ok((current_input, Pattern::Array(elements)))
                }
            } else {
                Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
            }
        },
        
        // Try extractor pattern first since it's more specific (has =>)
        |input| {
            // Try to parse a postfix expression (stops before binary operators to avoid consuming ':')
            match crate::haxe_parser_expr::postfix_expr(full, input) {
                Ok((rest, expr)) => {
                    // Check if followed by =>
                    match symbol("=>").parse(rest) {
                        Ok((rest, _)) => {
                            // Parse the value expression (also postfix level)
                            match crate::haxe_parser_expr::postfix_expr(full, rest) {
                                Ok((rest, value)) => {
                                    Ok((rest, Pattern::Extractor {
                                        expr,
                                        value,
                                    }))
                                },
                                Err(_) => {
                                    // Not a valid extractor
                                    Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
                                }
                            }
                        },
                        Err(_) => {
                            // Not an extractor pattern
                            Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
                        }
                    }
                },
                Err(_) => {
                    Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
                }
            }
        },
        
        // Constructor pattern
        |input| {
            let (input, path_parts) = separated_list1(symbol("."), identifier).parse(input)?;
            
            // Check if followed by parentheses (constructor with params)
            if let Ok((rest, _)) = symbol("(").parse(input) {
                let (rest, params) = separated_list0(symbol(","), |i| pattern(full, i)).parse(rest)?;
                let (rest, _) = symbol(")")(rest)?;
                
                let (package, name) = if path_parts.len() == 1 {
                    (vec![], path_parts[0].clone())
                } else {
                    let mut parts = path_parts;
                    let name = parts.pop().unwrap();
                    (parts, name)
                };
                
                Ok((rest, Pattern::Constructor {
                    path: TypePath { package, name, sub: None },
                    params,
                }))
            } else {
                // Just a variable
                Ok((input, Pattern::Var(path_parts.join("."))))
            }
        },
        
        // Try literal expressions as constant patterns
        |input| {
            if let Ok((rest, expr)) = crate::haxe_parser_expr::literal_expr(full, input) {
                Ok((rest, Pattern::Const(expr)))
            } else {
                Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
            }
        },
        
        // Underscore pattern - must be after extractor pattern to avoid consuming "_."
        // Only match standalone underscore, not _.something
        |input| {
            let (input, _) = symbol("_").parse(input)?;
            
            // Check if followed by a dot - if so, this should be an expression
            if let Ok((_, _)) = symbol(".").parse(input) {
                // This is _.something, should be parsed as an expression
                Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
            } else {
                Ok((input, Pattern::Underscore))
            }
        },
    )).parse(input)
}

// Loop expressions

pub fn for_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("for").parse(input)?;
    let (input, _) = symbol("(").parse(input)?;
    
    // Try to parse key => value pattern first
    let (input, var, key_var) = match parse_key_value_pattern(input) {
        Ok((rest, (key, value))) => (rest, value, Some(key)),
        Err(_) => {
            // Fall back to simple variable
            let (rest, var) = identifier(input)?;
            (rest, var, None)
        }
    };
    
    let (input, _) = keyword("in").parse(input)?;
    let (input, iter) = expression(full, input)?;
    let (input, _) = symbol(")").parse(input)?;
    let (input, body) = expression(full, input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::For {
            var,
            key_var,
            iter: Box::new(iter),
            body: Box::new(body),
        },
        span: Span::new(start, end),
    }))
}

/// Parse key => value pattern for for loops
fn parse_key_value_pattern(input: &str) -> PResult<(String, String)> {
    let (input, key) = identifier(input)?;
    let (input, _) = symbol("=>").parse(input)?;
    let (input, value) = identifier(input)?;
    Ok((input, (key, value)))
}

pub fn while_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("while").parse(input)?;
    let (input, _) = symbol("(").parse(input)?;
    let (input, cond) = expression(full, input)?;
    let (input, _) = symbol(")").parse(input)?;
    let (input, body) = expression(full, input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::While {
            cond: Box::new(cond),
            body: Box::new(body),
        },
        span: Span::new(start, end),
    }))
}

pub fn do_while_expr<'a>(full: &'a str, input: &'a str) -> PResult<'a, Expr> {
    let start = position(full, input);
    let (input, _) = keyword("do").parse(input)?;
    let (input, body) = expression(full, input)?;
    let (input, _) = keyword("while").parse(input)?;
    let (input, _) = symbol("(").parse(input)?;
    let (input, cond) = expression(full, input)?;
    let (input, _) = symbol(")").parse(input)?;
    let end = position(full, input);
    
    Ok((input, Expr {
        kind: ExprKind::DoWhile {
            body: Box::new(body),
            cond: Box::new(cond),
        },
        span: Span::new(start, end),
    }))
}

// Continue in next part...