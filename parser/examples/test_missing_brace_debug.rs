//! Debug why missing braces test is not working

use parser::parse_incrementally_enhanced;

fn main() {
    let test_code = r#"
class Test {
    function test() 
        return "hello";
    }
"#;

    let result = parse_incrementally_enhanced("test.hx", test_code);
    
    println!("Has errors: {}", result.has_errors());
    println!("Error count: {}", result.diagnostics.len());
    println!("Parsed elements: {}", result.parsed_elements.len());
    
    if result.has_errors() {
        println!("\nDiagnostics:");
        println!("{}", result.format_diagnostics(true));
    } else {
        println!("\nNo errors detected!");
        println!("Parsed elements: {:?}", result.parsed_elements);
    }
}