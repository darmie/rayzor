use parser::parse_haxe_file_with_diagnostics;

#[test]
fn test_math_hx_parses() {
    let source = r#"
extern class Math {
    #if !eval
    private static function __init__():Void
        untyped {
            #if flash
            NaN = __global__["Number"].NaN;
            NEGATIVE_INFINITY = __global__["Number"].NEGATIVE_INFINITY;
            POSITIVE_INFINITY = __global__["Number"].POSITIVE_INFINITY;
            #else
            Math.NaN = Number["NaN"];
            Math.NEGATIVE_INFINITY = Number["NEGATIVE_INFINITY"];
            Math.POSITIVE_INFINITY = Number["POSITIVE_INFINITY"];
            #end
            Math.isFinite = function(i) {
                return #if flash __global__["isFinite"](i); #else false; #end
            };
            Math.isNaN = function(i) {
                return #if flash __global__["isNaN"](i); #else false; #end
            };
        }
    #end
}
"#;

    let result = parse_haxe_file_with_diagnostics("Math.hx", source);

    match &result {
        Ok(parse_result) => {
            println!("Parse successful!");
            if parse_result.diagnostics.has_errors() {
                println!("But has errors:");
                for error in parse_result.diagnostics.errors() {
                    println!("  - {:?}: {}", error.severity, error.message);
                }
            }
            assert!(
                !parse_result.diagnostics.has_errors(),
                "Should not have errors"
            );
        }
        Err(e) => {
            println!("Parse failed: {}", e);
            panic!("Math.hx should parse");
        }
    }
}

#[test]
fn test_inline_conditional_simple() {
    let source = r#"
class Test {
    public function test() {
        var x = #if flash 1; #else 2; #end
    }
}
"#;

    let result = parse_haxe_file_with_diagnostics("Test.hx", source);

    match &result {
        Ok(parse_result) => {
            if parse_result.diagnostics.has_errors() {
                println!("Errors:");
                for error in parse_result.diagnostics.errors() {
                    println!("  - {}", error.message);
                }
            }
            assert!(!parse_result.diagnostics.has_errors());
        }
        Err(e) => panic!("Should parse: {}", e),
    }
}
