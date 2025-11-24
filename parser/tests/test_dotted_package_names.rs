use parser::parse_haxe_file_with_diagnostics;

#[test]
fn test_package_haxe_macro() {
    let source = r#"
package haxe.macro;

class Type {
    public function new() {}
}
"#;

    let result = parse_haxe_file_with_diagnostics("Type.hx", source);

    match &result {
        Ok(parse_result) => {
            println!("Parse successful!");
            println!("Errors: {}", parse_result.diagnostics.has_errors());
            for error in parse_result.diagnostics.errors() {
                println!("  - {:?}: {}", error.severity, error.message);
            }

            // Check package was parsed correctly
            let package = &parse_result.file.package;
            assert!(package.is_some(), "Package should be parsed");
            let pkg = package.as_ref().unwrap();
            assert_eq!(pkg.path, vec!["haxe", "macro"], "Package path should be haxe.macro");
        }
        Err(e) => {
            println!("Parse failed: {}", e);
        }
    }

    assert!(result.is_ok(), "Should parse package haxe.macro");
}

#[test]
fn test_package_haxe_extern() {
    let source = r#"
package haxe.extern;

typedef EitherType<A,B> = Dynamic;
"#;

    let result = parse_haxe_file_with_diagnostics("EitherType.hx", source);

    match &result {
        Ok(parse_result) => {
            let package = &parse_result.file.package;
            assert!(package.is_some(), "Package should be parsed");
            let pkg = package.as_ref().unwrap();
            assert_eq!(pkg.path, vec!["haxe", "extern"], "Package path should be haxe.extern");
        }
        Err(e) => {
            println!("Parse failed: {}", e);
        }
    }

    assert!(result.is_ok(), "Should parse package haxe.extern");
}

#[test]
fn test_package_with_keyword_segments() {
    let source = r#"
package test.macro.extern.class;

class Foo {}
"#;

    let result = parse_haxe_file_with_diagnostics("Foo.hx", source);

    match &result {
        Ok(parse_result) => {
            let package = &parse_result.file.package;
            assert!(package.is_some(), "Package should be parsed");
            let pkg = package.as_ref().unwrap();
            // Note: 'class' is a keyword but should be allowed in package path
            assert_eq!(pkg.path, vec!["test", "macro", "extern", "class"],
                      "Package path should allow keywords");
        }
        Err(e) => {
            println!("Parse failed: {}", e);
        }
    }

    assert!(result.is_ok(), "Should parse package with multiple keyword segments");
}
