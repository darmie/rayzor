use parser::parse_haxe_file_with_diagnostics;

#[test]
fn test_enum_abstract_basic() {
    let source = r#"
enum abstract XmlType(Int) {
    var Element = 0;
    var PCData = 1;
}
"#;

    let result = parse_haxe_file_with_diagnostics("test.hx", source);

    match &result {
        Ok(parse_result) => {
            println!("Parse successful!");
            println!("Errors: {}", parse_result.diagnostics.has_errors());
            for error in parse_result.diagnostics.errors() {
                println!("  - {:?}: {}", error.severity, error.message);
            }
        }
        Err(e) => {
            println!("Parse failed: {}", e);
        }
    }

    assert!(result.is_ok(), "Should parse enum abstract");
}

#[test]
fn test_cast_with_type_annotation() {
    let source = r#"
class Test {
    public function test() {
        var x = (cast this : Test);
    }
}
"#;

    let result = parse_haxe_file_with_diagnostics("test.hx", source);

    match &result {
        Ok(parse_result) => {
            println!("Parse successful!");
            println!("Errors: {}", parse_result.diagnostics.has_errors());
            for error in parse_result.diagnostics.errors() {
                println!("  - {:?}: {}", error.severity, error.message);
            }
        }
        Err(e) => {
            println!("Parse failed: {}", e);
        }
    }

    assert!(result.is_ok(), "Should parse cast with type annotation");
}

#[test]
fn test_enum_abstract_with_method() {
    let source = r#"
enum abstract XmlType(Int) {
    var Element = 0;
    var PCData = 1;

    public function toString():String {
        return switch (cast this : XmlType) {
            case Element: "Element";
            case PCData: "PCData";
        };
    }
}
"#;

    let result = parse_haxe_file_with_diagnostics("test.hx", source);

    match &result {
        Ok(parse_result) => {
            println!("Parse successful!");
            println!("Errors: {}", parse_result.diagnostics.has_errors());
            for error in parse_result.diagnostics.errors() {
                println!("  - {:?}: {}", error.severity, error.message);
            }
        }
        Err(e) => {
            println!("Parse failed: {}", e);
        }
    }

    assert!(result.is_ok(), "Should parse enum abstract with methods");
}
