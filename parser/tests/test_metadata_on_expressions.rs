//! Test metadata on expressions functionality

use parser::parse_haxe_file;

fn test_expression_parsing(code: &str, description: &str) {
    match parse_haxe_file("test.hx", code, false) {
        Ok(_) => println!("✓ {}", description),
        Err(e) => panic!("✗ {}: {}", description, e),
    }
}

#[test]
fn test_simple_metadata_on_expressions() {
    let code = r#"
class Test {
    function test() {
        @:pure 1 + 2;
        @:inline getValue();
        @:noopt x * y;
    }
}
"#;
    test_expression_parsing(code, "Simple metadata on expressions");
}

#[test]
fn test_metadata_with_parameters() {
    let code = r#"
class Test {
    function test() {
        @:native("custom_func") someFunction();
        @:deprecated("Use newMethod instead") oldMethod();
        @:jsRequire("module", "name") require();
    }
}
"#;
    test_expression_parsing(code, "Metadata with parameters");
}

#[test]
fn test_multiple_metadata_on_same_expression() {
    let code = r#"
class Test {
    function test() {
        @:inline @:pure getValue();
        @:noopt @:final @:inline x * y;
        @:keep @:native("exported") someFunction();
    }
}
"#;
    test_expression_parsing(code, "Multiple metadata on same expression");
}

#[test]
fn test_metadata_on_complex_expressions() {
    let code = r#"
class Test {
    function test() {
        var fn = @:inline function(x) return x * 2;
        var arr = @:pure [1, 2, 3, 4];
        var obj = @:structInit { x: 10, y: 20 };
        var cond = @:pure if (flag) value else defaultValue;
    }
}
"#;
    test_expression_parsing(code, "Metadata on complex expressions");
}

#[test]
fn test_metadata_with_parentheses() {
    let code = r#"
class Test {
    function test() {
        var result = @:pure (1 + 2 * 3);
        var fn = @:inline (function(x) return x * 2);
        var casted = @:unsafe (cast value);
    }
}
"#;
    test_expression_parsing(code, "Metadata with parentheses (spaced)");
}

#[test]
fn test_nested_expressions_with_metadata() {
    let code = r#"
class Test {
    function test() {
        @:inline (@:pure getValue()) + (@:cached getOtherValue());
        @:guard (@:pure condition) ? @:fast truePath() : @:slow falsePath();
    }
}
"#;
    test_expression_parsing(code, "Nested expressions with metadata");
}

#[test]
fn test_both_metadata_formats() {
    let code = r#"
class Test {
    function test() {
        @inline getValue();
        @:inline getOtherValue();
        @author("John") @:deprecated("Old method") oldFunction();
    }
}
"#;
    test_expression_parsing(code, "Both @ and @: formats");
}

#[test]
fn test_metadata_in_variable_assignments() {
    let code = r#"
class Test {
    function test() {
        var result = @:pure (1 + 2);
        var fn = @:inline getValue();
        var expr = @:noopt x * y;
    }
}
"#;
    test_expression_parsing(code, "Metadata in variable assignments");
}