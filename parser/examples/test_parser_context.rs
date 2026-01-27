//! Test to show parser works correctly with valid code

use parser::parse_haxe_file;

fn main() {
    // Test 1: Valid code parses correctly
    let valid_code = r#"
class Test {
    function test() {
        var x = 1;
        var y = 2;
        return x + y;
    }
}
"#;

    match parse_haxe_file("test.hx", valid_code, false) {
        Ok(file) => {
            println!("✅ Valid code parsed successfully!");
            println!("Classes: {}", file.declarations.len());
        }
        Err(e) => {
            println!("❌ Unexpected error: {}", e);
        }
    }

    // Test 2: Single missing semicolon
    let missing_semi = r#"
class Test {
    function test() {
        var x = 1;
        var y = 2
        return x + y;
    }
}
"#;

    match parse_haxe_file("test.hx", missing_semi, false) {
        Ok(file) => {
            println!("\n✅ Code with missing semicolon parsed with recovery!");
            println!("Classes: {}", file.declarations.len());
        }
        Err(e) => {
            println!("\n❌ Error (as expected): {}", e);
        }
    }
}
