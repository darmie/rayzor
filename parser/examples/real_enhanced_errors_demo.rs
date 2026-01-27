//! Demo showing real enhanced error reporting from the parser
//!
//! This example demonstrates how the enhanced error system actually works
//! with real parsing scenarios.

use parser::{
    DiagnosticBuilder, Diagnostics, ErrorFormatter, HaxeDiagnostics, SourceMap, SourcePosition,
    SourceSpan,
};

fn main() {
    println!("🎯 Real Enhanced Error Reporting Demo");
    println!("=====================================\n");

    // Demo 1: Simulate what the parser would generate for missing semicolon
    demo_missing_semicolon();

    // Demo 2: Simulate parser errors for invalid function
    demo_invalid_function();

    // Demo 3: Show how context errors would be converted
    demo_context_conversion();
}

fn demo_missing_semicolon() {
    println!("📋 Demo 1: Missing Semicolon (What Parser Should Generate)");
    println!("----------------------------------------------------------\n");

    let test_code = r#"
class Test {
    var x = 1
    function test() {
        return x;
    }
}
"#;

    // Create source map
    let mut source_map = SourceMap::new();
    let file_id = source_map.add_file("test.hx".to_string(), test_code.to_string());

    // Create diagnostics as the parser should
    let mut diagnostics = Diagnostics::new();

    // Missing semicolon after "var x = 1"
    let span = SourceSpan::new(
        SourcePosition::new(3, 14, 28), // After the "1"
        SourcePosition::new(3, 15, 29),
        file_id,
    );
    diagnostics.push(HaxeDiagnostics::missing_semicolon(
        span,
        "variable declaration",
    ));

    // Format and display
    let formatter = ErrorFormatter::with_colors();
    println!(
        "{}",
        formatter.format_diagnostics(&diagnostics, &source_map)
    );

    let separator = "=".repeat(60);
    println!("{}\n", separator);
}

fn demo_invalid_function() {
    println!("📋 Demo 2: Invalid Function Declaration");
    println!("---------------------------------------\n");

    let test_code = r#"
class Test {
    fucntion test() {
        var x = 1
        return x;
    }
}
"#;

    // Create source map
    let mut source_map = SourceMap::new();
    let file_id = source_map.add_file("test.hx".to_string(), test_code.to_string());

    // Create diagnostics
    let mut diagnostics = Diagnostics::new();

    // Invalid identifier "fucntion"
    let span = SourceSpan::new(
        SourcePosition::new(3, 5, 18),
        SourcePosition::new(3, 13, 26),
        file_id,
    );
    diagnostics.push(HaxeDiagnostics::invalid_identifier(
        span,
        "fucntion",
        "unknown keyword, did you mean something else?",
    ));

    // Missing semicolon after "var x = 1"
    let span2 = SourceSpan::new(
        SourcePosition::new(4, 18, 46),
        SourcePosition::new(4, 19, 47),
        file_id,
    );
    diagnostics.push(HaxeDiagnostics::missing_semicolon(
        span2,
        "variable declaration",
    ));

    // Format and display
    let formatter = ErrorFormatter::with_colors();
    println!(
        "{}",
        formatter.format_diagnostics(&diagnostics, &source_map)
    );

    let separator = "=".repeat(60);
    println!("{}\n", separator);
}

fn demo_context_conversion() {
    println!("📋 Demo 3: Context Error Conversion");
    println!("-----------------------------------\n");

    let test_code = r#"
class Calculator {
    function calculate(a: Float, b: Float)
        return a + b;
    }
}
"#;

    // Create source map
    let mut source_map = SourceMap::new();
    let file_id = source_map.add_file("calculator.hx".to_string(), test_code.to_string());

    // Create diagnostics that would come from context errors
    let mut diagnostics = Diagnostics::new();

    // Missing opening brace for function body
    let span = SourceSpan::new(
        SourcePosition::new(3, 43, 61),
        SourcePosition::new(3, 44, 62),
        file_id,
    );
    diagnostics.push(HaxeDiagnostics::missing_closing_delimiter(
        span.clone(),
        span,
        '{',
    ));

    // Could also add a hint about return type
    let func_span = SourceSpan::new(
        SourcePosition::new(3, 5, 23),
        SourcePosition::new(3, 14, 32),
        file_id,
    );
    diagnostics.push(
        DiagnosticBuilder::hint(
            "consider adding a return type".to_string(),
            func_span.clone(),
        )
        .code("H0001")
        .label(func_span, "function lacks explicit return type")
        .help("add ': Float' after the parameter list")
        .note("explicit return types improve code readability")
        .build(),
    );

    // Format and display
    let formatter = ErrorFormatter::with_colors();
    println!(
        "{}",
        formatter.format_diagnostics(&diagnostics, &source_map)
    );

    println!("\n💡 Note: These diagnostics show what the parser SHOULD generate");
    println!("        when integrated with the enhanced error system.");

    let separator = "=".repeat(60);
    println!("\n{}\n", separator);
}
