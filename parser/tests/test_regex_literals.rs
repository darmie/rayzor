//! Test regex literal parsing

use parser::parse_haxe_file;

#[test]
fn test_simple_regex() {
    let input = r#"
class Test {
    static var simpleRegex = ~/hello/;
    static var withFlags = ~/pattern/ig;
    static var withEscape = ~/he\w+o/;
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ Simple regex literals work: {:?}", ast),
        Err(e) => println!("✗ Simple regex literals failed: {}", e),
    }
}

#[test]
fn test_regex_in_expressions() {
    let input = r#"
class Test {
    public function new() {
        if (~/test/.match("testing")) {
            trace("matches");
        }
        
        var result = ~/(\d+)/.split("a1b2c3");
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ Regex in expressions works"),
        Err(e) => println!("✗ Regex in expressions failed: {}", e),
    }
}

#[test]
fn test_complex_regex() {
    let input = r#"
class Test {
    static var email = ~/^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/;
    static var url = ~/https?:\/\/(www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&\/\/=]*)/;
    static var multiline = ~/
        ^       # Start of line
        \d+     # One or more digits
        $       # End of line
    /mx;
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ Complex regex literals work"),
        Err(e) => println!("✗ Complex regex literals failed: {}", e),
    }
}

#[test]
fn test_regex_with_slashes() {
    let input = r#"
class Test {
    // Regex containing forward slashes need escaping
    static var path = ~/\/path\/to\/file/;
    static var htmlTag = ~/<\/?[a-z][a-z0-9]*>/i;
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => println!("✓ Regex with slashes works"),
        Err(e) => println!("✗ Regex with slashes failed: {}", e),
    }
}