#[cfg(test)]
mod switch_expression_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_switch_expression_basic() {
        let haxe_code = r#"
class SwitchTest {
    static function main() {
        var x = 5;

        // Basic switch expression
        var result = switch (x) {
            case 1: "one";
            case 2: "two";
            case 5: "five";
            default: "other";
        };

        trace(result); // Should be "five"
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should compile without errors
        assert!(
            result.errors.is_empty(),
            "Expected no errors, but got: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_switch_expression_type_consistency() {
        let haxe_code = r#"
class SwitchTest {
    static function main() {
        var x = 5;

        // All branches should return the same type
        var result:String = switch (x) {
            case 1: "one";
            case 2: 2;  // Error: returns Int instead of String
            case 3: "three";
            default: "other";
        };
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should have a type mismatch error
        assert!(!result.errors.is_empty(), "Expected type mismatch error");
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.message.contains("Type mismatch")),
            "Expected type mismatch error, but got: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_switch_expression_void_not_allowed() {
        let haxe_code = r#"
class SwitchTest {
    static function main() {
        var x = 5;

        // Switch expressions cannot have void branches
        var result = switch (x) {
            case 1: "one";
            case 2: trace("two");  // Error: void expression
            default: "other";
        };
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should have an error about void in expression context
        assert!(
            !result.errors.is_empty(),
            "Expected error for void in expression"
        );
    }

    #[test]
    fn test_switch_expression_no_default() {
        let haxe_code = r#"
class SwitchTest {
    static function main() {
        var x = 5;

        // Switch expression without default may not be exhaustive
        var result = switch (x) {
            case 1: "one";
            case 2: "two";
            // No default - compiler should warn or error
        };
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        // Depending on implementation, this might be an error or warning
        // For now, we'll just check if it compiles
        println!("Switch without default - errors: {:?}", result.errors);
    }

    #[test]
    fn test_switch_expression_with_blocks() {
        let haxe_code = r#"
class SwitchTest {
    static function main() {
        var x = 5;

        // Switch expression with block statements
        var result = switch (x) {
            case 1: {
                var temp = "number ";
                temp + "one";
            }
            case 2: {
                var temp = "number ";
                temp + "two";
            }
            default: "other";
        };

        trace(result);
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should compile without errors
        assert!(
            result.errors.is_empty(),
            "Expected no errors, but got: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_switch_expression_nested() {
        let haxe_code = r#"
class SwitchTest {
    static function main() {
        var x = 5;
        var y = 2;

        // Nested switch expressions
        var result = switch (x) {
            case 1: "one";
            case 2: switch (y) {
                case 1: "two-one";
                case 2: "two-two";
                default: "two-other";
            };
            default: "other";
        };

        trace(result);
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should compile without errors
        assert!(
            result.errors.is_empty(),
            "Expected no errors, but got: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_switch_expression_with_pattern_matching() {
        let haxe_code = r#"
enum Color {
    Red;
    Green;
    Blue;
    RGB(r:Int, g:Int, b:Int);
}

class SwitchTest {
    static function main() {
        var color = Color.RGB(255, 0, 0);

        // Switch expression with enum pattern matching
        var name = switch (color) {
            case Red: "red";
            case Green: "green";
            case Blue: "blue";
            case RGB(255, 0, 0): "bright red";
            case RGB(r, g, b): 'rgb($r, $g, $b)';
        };

        trace(name);
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should compile without errors
        assert!(
            result.errors.is_empty(),
            "Expected no errors, but got: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_switch_expression_type_inference() {
        let haxe_code = r#"
class SwitchTest {
    static function main() {
        var x = 5;

        // Type should be inferred from branches
        var result = switch (x) {
            case 1: 100;
            case 2: 200;
            default: 300;
        };

        // result should be inferred as Int
        var y:Int = result; // Should work
        var z:String = result; // Should error
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should have type mismatch on String assignment
        assert!(!result.errors.is_empty(), "Expected type mismatch error");
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.message.contains("Type mismatch")
                    && e.message.contains("String")
                    && e.message.contains("Int")),
            "Expected Int to String type mismatch, but got: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_switch_expression_with_guard() {
        let haxe_code = r#"
class SwitchTest {
    static function main() {
        var x = 5;

        // Switch expression with guards
        var category = switch (x) {
            case n if n < 0: "negative";
            case 0: "zero";
            case n if n > 0 && n <= 10: "small positive";
            case n if n > 10: "large positive";
            default: "unknown"; // Should never reach
        };

        trace(category);
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should compile without errors
        assert!(
            result.errors.is_empty(),
            "Expected no errors, but got: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_switch_expression_fallthrough_not_allowed() {
        let haxe_code = r#"
class SwitchTest {
    static function main() {
        var x = 5;

        // In expression context, each case must have a value
        var result = switch (x) {
            case 1:  // Error: no expression
            case 2: "two";
            default: "other";
        };
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should have an error about missing expression
        assert!(
            !result.errors.is_empty(),
            "Expected error for missing case expression"
        );
    }
}
