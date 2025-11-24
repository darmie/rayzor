use parser::parse_haxe_file_with_diagnostics;

#[test]
fn test_uint_else_branch() {
    // Simplified version of UInt.hx structure
    let source = r#"
#if flash
// Flash implementation
abstract UInt to Int from Int {}
#else
// Generic implementation - should be kept
abstract UInt(Int) from Int to Int {
    private static #if !js inline #end function gt(a:UInt, b:UInt):Bool {
        return true;
    }
}
#end
"#;

    let result = parse_haxe_file_with_diagnostics("UInt.hx", source);

    match &result {
        Ok(parse_result) => {
            println!("Parse successful!");
            if parse_result.diagnostics.has_errors() {
                println!("Errors:");
                for error in parse_result.diagnostics.errors() {
                    println!("  - {}", error.message);
                }
                panic!("Should not have errors");
            }

            // Check the abstract was parsed
            assert_eq!(parse_result.file.declarations.len(), 1, "Should have one declaration");
        }
        Err(e) => {
            panic!("Parse failed: {}", e);
        }
    }
}

#[test]
fn test_uint_complex_condition() {
    // Test the complex condition from UInt.hx
    let source = r#"
#if ((flash || cs) && !doc_gen)
// Flash/CS implementation
abstract UInt to Int from Int {}
#else
// Generic implementation
abstract UInt(Int) from Int to Int {}
#end
"#;

    let result = parse_haxe_file_with_diagnostics("UInt.hx", source);

    match &result {
        Ok(parse_result) => {
            println!("Parse successful!");
            if parse_result.diagnostics.has_errors() {
                for error in parse_result.diagnostics.errors() {
                    println!("  - {}", error.message);
                }
                panic!("Should not have errors");
            }
        }
        Err(e) => {
            panic!("Parse failed: {}", e);
        }
    }
}
