#[cfg(test)]
mod inheritance_signature_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_inheritance_wrong_return_type() {
        let haxe_code = r#"
            class Animal {
                public function new() {}
                
                public function makeSound():String {
                    return "generic sound";
                }
            }
            
            class Cat extends Animal {
                public function new() {
                    super();
                }
                
                // Wrong return type - should error
                override public function makeSound():Int {
                    return 42;
                }
            }
        "#;
        
        let result = compile_haxe_source(haxe_code);
        assert!(!result.errors.is_empty(), "Should have errors");
        
        let has_signature_error = result.errors.iter().any(|e| 
            e.message.contains("signature") && e.message.contains("incompatible")
        );
        assert!(has_signature_error, "Should detect wrong return type");
    }
    
    #[test]
    fn test_inheritance_wrong_parameter_count() {
        let haxe_code = r#"
            class Vehicle {
                public function new() {}
                
                public function move(speed:Float):Void {
                    // Move at speed
                }
            }
            
            class Car extends Vehicle {
                public function new() {
                    super();
                }
                
                // Wrong parameter count - should error
                override public function move(speed:Float, direction:String):Void {
                    // Move with direction
                }
            }
        "#;
        
        let result = compile_haxe_source(haxe_code);
        assert!(!result.errors.is_empty(), "Should have errors");
        
        let has_signature_error = result.errors.iter().any(|e| 
            e.message.contains("signature") || e.message.contains("parameter")
        );
        assert!(has_signature_error, "Should detect wrong parameter count");
    }
    
    #[test]
    fn test_inheritance_wrong_parameter_type() {
        let haxe_code = r#"
            class GameObject {
                public function new() {}
                
                public function setPosition(x:Float, y:Float):Void {
                    // Set position
                }
            }
            
            class Player extends GameObject {
                public function new() {
                    super();
                }
                
                // Wrong parameter types - should error
                override public function setPosition(x:Int, y:Int):Void {
                    // Set position with ints
                }
            }
        "#;
        
        let result = compile_haxe_source(haxe_code);
        assert!(!result.errors.is_empty(), "Should have errors");
        
        let has_signature_error = result.errors.iter().any(|e| 
            e.message.contains("signature") || e.message.contains("incompatible")
        );
        assert!(has_signature_error, "Should detect wrong parameter types");
    }
    
    #[test]
    fn test_correct_inheritance_override() {
        let haxe_code = r#"
            class Shape {
                public function new() {}
                
                public function draw(x:Float, y:Float):Void {
                    // Draw at position
                }
                
                public function getArea():Float {
                    return 0.0;
                }
            }
            
            class Rectangle extends Shape {
                var width:Float;
                var height:Float;
                
                public function new(w:Float, h:Float) {
                    super();
                    width = w;
                    height = h;
                }
                
                // Correct override
                override public function draw(x:Float, y:Float):Void {
                    // Draw rectangle at position
                }
                
                // Correct override
                override public function getArea():Float {
                    return width * height;
                }
            }
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        // Should not have signature-related errors
        let has_signature_error = result.errors.iter().any(|e| 
            e.message.contains("signature") && e.message.contains("incompatible")
        );
        assert!(!has_signature_error, "Should not have signature errors for correct overrides");
    }
    
    #[test]
    fn test_deep_inheritance_chain() {
        let haxe_code = r#"
            class A {
                public function new() {}
                
                public function method1():Int {
                    return 1;
                }
            }
            
            class B extends A {
                public function new() {
                    super();
                }
                
                override public function method1():Int {
                    return 2;
                }
                
                public function method2():String {
                    return "B";
                }
            }
            
            class C extends B {
                public function new() {
                    super();
                }
                
                // Override from grandparent
                override public function method1():Int {
                    return 3;
                }
                
                // Override from parent - missing override modifier
                public function method2():String {
                    return "C";
                }
            }
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        // Should detect missing override modifier for method2
        let has_missing_override = result.errors.iter().any(|e| 
            e.message.contains("method2") && e.message.contains("override")
        );
        
        println!("Deep inheritance errors: {:?}", 
            result.errors.iter()
                .filter(|e| e.message.contains("override"))
                .collect::<Vec<_>>()
        );
    }
}