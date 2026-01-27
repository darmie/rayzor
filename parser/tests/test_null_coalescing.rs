//! Test null coalescing operator

extern crate parser;
use parser::parse_haxe_file;

#[test]
fn test_null_coalescing_simple() {
    let input = r#"
class Test {
    public function testSimple() {
        var x:String = null;
        var y = x ?? "default";
    }
}
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => println!("✓ Simple null coalescing works"),
        Err(e) => panic!("Failed to parse simple null coalescing: {}", e),
    }
}

#[test]
fn test_null_coalescing_chain() {
    let input = r#"
class Test {
    public function testChain() {
        var result = getValue() ?? fallback() ?? "ultimate default";
    }
}
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => println!("✓ Chained null coalescing works"),
        Err(e) => panic!("Failed to parse chained null coalescing: {}", e),
    }
}

#[test]
fn test_null_coalescing_in_expression() {
    let input = r#"
class Test {
    public function fromArray(arr:Array<Float>):Test {
        return new Test(arr[0] ?? 0, 1);
    }
}
"#;

    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => println!("✓ Null coalescing in expressions works"),
        Err(e) => {
            eprintln!("Failed to parse null coalescing in expression: {}", e);
            eprintln!("Input was:\n{}", input);
            panic!("Parse failed");
        }
    }
}
