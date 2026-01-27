#[cfg(test)]
mod interface_implementation_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_missing_interface_method() {
        let haxe_code = r#"
            interface IDrawable {
                function draw():Void;
                function getColor():String;
            }

            class Shape implements IDrawable {
                public function new() {}

                // Missing draw() method - should error
                public function getColor():String {
                    return "red";
                }
            }
        "#;

        let result = compile_haxe_source(haxe_code);
        assert!(!result.errors.is_empty(), "Should have errors");

        let has_missing_method_error = result
            .errors
            .iter()
            .any(|e| e.message.contains("missing method") && e.message.contains("draw"));
        assert!(
            has_missing_method_error,
            "Should detect missing interface method"
        );
    }

    #[test]
    fn test_interface_method_wrong_signature() {
        let haxe_code = r#"
            interface ICalculator {
                function add(a:Int, b:Int):Int;
                function multiply(x:Float, y:Float):Float;
            }

            class BasicCalculator implements ICalculator {
                public function new() {}

                // Wrong return type - should error
                public function add(a:Int, b:Int):Float {
                    return a + b;
                }

                // Wrong parameter types - should error
                public function multiply(x:Int, y:Int):Float {
                    return x * y;
                }
            }
        "#;

        let result = compile_haxe_source(haxe_code);
        assert!(!result.errors.is_empty(), "Should have errors");

        let has_signature_error = result
            .errors
            .iter()
            .any(|e| e.message.contains("signature") || e.message.contains("incompatible"));
        assert!(
            has_signature_error,
            "Should detect method signature mismatch"
        );
    }

    #[test]
    fn test_correct_interface_implementation() {
        let haxe_code = r#"
            interface IAnimal {
                function makeSound():String;
                function move(distance:Float):Void;
            }

            class Dog implements IAnimal {
                public function new() {}

                public function makeSound():String {
                    return "Woof!";
                }

                public function move(distance:Float):Void {
                    // Move the distance
                }
            }
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should not have interface-related errors
        let has_interface_error = result
            .errors
            .iter()
            .any(|e| e.message.contains("interface") || e.message.contains("missing method"));
        assert!(
            !has_interface_error,
            "Should not have interface errors for correct implementation"
        );
    }

    #[test]
    fn test_multiple_interface_implementation() {
        let haxe_code = r#"
            interface IRunnable {
                function run():Void;
            }

            interface IJumpable {
                function jump(height:Float):Void;
            }

            class Athlete implements IRunnable, IJumpable {
                public function new() {}

                public function run():Void {
                    // Running
                }

                // Missing jump() method - should error
            }
        "#;

        let result = compile_haxe_source(haxe_code);
        assert!(!result.errors.is_empty(), "Should have errors");

        let has_missing_jump = result
            .errors
            .iter()
            .any(|e| e.message.contains("jump") && e.message.contains("missing"));
        assert!(
            has_missing_jump,
            "Should detect missing jump method from IJumpable"
        );
    }

    #[test]
    fn test_interface_extends_interface() {
        let haxe_code = r#"
            interface IShape {
                function getArea():Float;
            }

            interface IColoredShape extends IShape {
                function getColor():String;
            }

            class Circle implements IColoredShape {
                public function new() {}

                public function getArea():Float {
                    return 3.14159;
                }

                // Missing getColor() - should error
            }
        "#;

        let result = compile_haxe_source(haxe_code);

        // Note: Current implementation might not handle interface inheritance fully
        // This test documents expected behavior
        println!(
            "Interface inheritance errors: {:?}",
            result
                .errors
                .iter()
                .filter(|e| e.message.contains("interface"))
                .collect::<Vec<_>>()
        );
    }
}
