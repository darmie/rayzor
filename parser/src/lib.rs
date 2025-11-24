use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{
        alpha1, alphanumeric1, char, digit1, hex_digit1, multispace0, multispace1, none_of, one_of,
    },
    combinator::{all_consuming, map, opt, recognize, value, verify},
    error::{context, ParseError as NomParseError},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult, Parser,
};



// New complete Haxe AST and parser
pub mod haxe_ast;
pub mod haxe_parser;
pub mod haxe_parser_types;
pub mod haxe_parser_decls;
pub mod haxe_parser_expr;
pub mod haxe_parser_expr2;
pub mod haxe_parser_expr3;
pub mod incremental_parser;

// Import our enhanced error handling system
pub mod error;
pub mod error_formatter;
pub mod position_parser;
pub mod enhanced_parser;



pub use error::{
    ParseError, ParseErrors, SourceMap, SourceSpan, SourcePosition, FileId, 
    ParseResult as EnhancedParseResult, ParseResultMulti, ErrorSeverity
};
pub use error_formatter::{ErrorFormatter, FormatConfig, ErrorHelpers};
pub use position_parser::{PositionParser, Spanned, ParseResultExt};

// Export new Haxe parser
pub use haxe_ast::*;
pub use haxe_parser::parse_haxe_file;

#[cfg(test)]
mod test_dollar_simple;

#[cfg(test)]
mod test_macro_quick;


// Legacy nom result type for compatibility
pub type NomParseResult<'a, T> = IResult<&'a str, T, nom::error::Error<&'a str>>;

// Internal result type uses nom for the actual parsing
type ParseResult<'a, T> = IResult<&'a str, T, nom::error::Error<&'a str>>;

// use ast::{Span, HaxeFile, PackageDecl, ImportDecl, Declaration};
// pub use ast::*;


// Removed duplicate type definitions to avoid conflicts with ast.rs

// ============================================================================
