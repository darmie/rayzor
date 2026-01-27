//! Debug the unexpected brace error

use parser::parse_incrementally_enhanced;

fn main() {
    let test_code = r#"
class Test {
    function test() {
        var x = 1;
        var y = 2
        return x + y;
    }
}
"#;

    let result = parse_incrementally_enhanced("test.hx", test_code);

    println!("Diagnostics count: {}", result.diagnostics.len());

    for (i, diag) in result.diagnostics.diagnostics.iter().enumerate() {
        println!("\nDiagnostic {}:", i + 1);
        println!("  Code: {:?}", diag.code);
        println!("  Message: {}", diag.message);
        println!("  Line: {}", diag.span.start.line);
        println!("  Column: {}", diag.span.start.column);
    }
}
