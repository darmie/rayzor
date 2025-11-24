//! Incremental parsing module for Haxe files
//! 
//! This module provides utilities for parsing Haxe files incrementally,
//! allowing for better error recovery and partial parsing results.

use crate::haxe_ast::*;
use crate::haxe_parser::{
    type_declaration, import_decl, using_decl, 
    package_decl, ws
};

/// Result of incremental parsing
#[derive(Debug)]
pub struct IncrementalParseResult {
    /// Successfully parsed elements
    pub parsed_elements: Vec<ParsedElement>,
    /// Errors encountered during parsing
    pub errors: Vec<ParseError>,
    /// Whether the entire file was successfully parsed
    pub complete: bool,
}

/// A successfully parsed element
#[derive(Debug)]
pub enum ParsedElement {
    Package(Package),
    Import(Import),
    Using(Using),
    TypeDeclaration(TypeDeclaration),
    ConditionalBlock(String), // Simplified for now
}

/// Parse error with location information
#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub remaining_input: String,
}

/// Parse a Haxe file incrementally
pub fn parse_incrementally(_file_name: &str, input: &str) -> IncrementalParseResult {
    let mut parsed_elements = Vec::new();
    let mut errors = Vec::new();
    let mut current_input = input;
    let full_input = input;
    
    // Helper to calculate line/column
    let calculate_position = |remaining: &str| -> (usize, usize) {
        let consumed = full_input.len() - remaining.len();
        let consumed_str = &full_input[..consumed];
        let line = consumed_str.lines().count();
        let column = consumed_str.lines().last().map(|l| l.len() + 1).unwrap_or(1);
        (line, column)
    };
    
    // Skip leading whitespace
    if let Ok((input, _)) = ws(current_input) {
        current_input = input;
    }
    
    // Try to parse package declaration
    match package_decl(full_input, current_input) {
        Ok((remaining, pkg)) => {
            parsed_elements.push(ParsedElement::Package(pkg));
            current_input = remaining;
        }
        Err(_) => {
            // Package is optional, continue
        }
    }
    
    // Parse imports and using declarations
    loop {
        // Skip whitespace
        if let Ok((input, _)) = ws(current_input) {
            current_input = input;
        }
        
        if current_input.is_empty() {
            break;
        }
        
        // Check if we've reached type declarations
        if current_input.starts_with("class") || 
           current_input.starts_with("interface") ||
           current_input.starts_with("enum") ||
           current_input.starts_with("typedef") ||
           current_input.starts_with("abstract") ||
           current_input.starts_with('@') {
            break;
        }
        
        // Try parsing import
        match import_decl(full_input, current_input) {
            Ok((remaining, import)) => {
                parsed_elements.push(ParsedElement::Import(import));
                current_input = remaining;
                continue;
            }
            Err(_) => {}
        }
        
        // Try parsing using
        match using_decl(full_input, current_input) {
            Ok((remaining, using)) => {
                parsed_elements.push(ParsedElement::Using(using));
                current_input = remaining;
                continue;
            }
            Err(_) => {}
        }
        
        // Try parsing conditional compilation
        if current_input.starts_with("#if") {
            // For now, skip the entire conditional block
            if let Some(end_pos) = current_input.find("#end") {
                let block = &current_input[..end_pos + 4];
                parsed_elements.push(ParsedElement::ConditionalBlock(block.to_string()));
                current_input = &current_input[end_pos + 4..];
                continue;
            }
        }
        
        // If we can't parse anything, skip to next line
        if let Some(newline_pos) = current_input.find('\n') {
            let (line, column) = calculate_position(current_input);
            errors.push(ParseError {
                line,
                column,
                message: format!("Unable to parse line: {}", current_input.lines().next().unwrap_or("")),
                remaining_input: current_input[..newline_pos.min(100)].to_string(),
            });
            current_input = &current_input[newline_pos + 1..];
        } else {
            break;
        }
    }
    
    // Parse type declarations
    loop {
        // Skip whitespace
        if let Ok((input, _)) = ws(current_input) {
            current_input = input;
        }
        
        if current_input.is_empty() {
            break;
        }
        
        match type_declaration(full_input, current_input) {
            Ok((remaining, type_decl)) => {
                parsed_elements.push(ParsedElement::TypeDeclaration(type_decl));
                current_input = remaining;
            }
            Err(e) => {
                let (line, column) = calculate_position(current_input);
                errors.push(ParseError {
                    line,
                    column,
                    message: format!("Failed to parse type declaration: {:?}", e),
                    remaining_input: current_input[..current_input.len().min(200)].to_string(),
                });
                
                // Try to recover by finding the next type declaration keyword
                let keywords = ["class", "interface", "enum", "typedef", "abstract"];
                let mut found = false;
                
                for keyword in &keywords {
                    if let Some(pos) = current_input.find(keyword) {
                        // Make sure it's a keyword (not part of another word)
                        if pos > 0 {
                            let before = current_input.chars().nth(pos - 1);
                            if before.map_or(false, |c| c.is_alphanumeric() || c == '_') {
                                continue;
                            }
                        }
                        current_input = &current_input[pos..];
                        found = true;
                        break;
                    }
                }
                
                if !found {
                    break;
                }
            }
        }
    }
    
    // Check if we consumed all input
    let complete = current_input.trim().is_empty();
    
    IncrementalParseResult {
        parsed_elements,
        errors,
        complete,
    }
}

/// Parse a specific section of a Haxe file
pub fn parse_section(section: &str, full_context: &str) -> Result<ParsedElement, String> {
    let trimmed = section.trim();
    
    if trimmed.starts_with("package") {
        package_decl(full_context, trimmed)
            .map(|(_, pkg)| ParsedElement::Package(pkg))
            .map_err(|e| format!("Failed to parse package: {:?}", e))
    } else if trimmed.starts_with("import") {
        import_decl(full_context, trimmed)
            .map(|(_, imp)| ParsedElement::Import(imp))
            .map_err(|e| format!("Failed to parse import: {:?}", e))
    } else if trimmed.starts_with("using") {
        using_decl(full_context, trimmed)
            .map(|(_, use_)| ParsedElement::Using(use_))
            .map_err(|e| format!("Failed to parse using: {:?}", e))
    } else if trimmed.starts_with("class") || 
              trimmed.starts_with("interface") ||
              trimmed.starts_with("enum") ||
              trimmed.starts_with("typedef") ||
              trimmed.starts_with("abstract") ||
              trimmed.starts_with('@') {
        type_declaration(full_context, trimmed)
            .map(|(_, td)| ParsedElement::TypeDeclaration(td))
            .map_err(|e| format!("Failed to parse type declaration: {:?}", e))
    } else {
        Err("Unknown element type".to_string())
    }
}