#[cfg(test)]
mod diagnostic_showcase_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_override_diagnostics_with_source_snippets() {
        // Use line numbers for better demonstration
        let haxe_code = r#"// Line 1: Animal class definition
class Animal {
    public function new() {}

    public function makeSound():String {
        return "generic sound";
    }

    public function eat(food:String):Bool {
        return true;
    }
}

// Line 14: Dog class with override issues
class Dog extends Animal {
    public function new() {
        super();
    }

    // Line 20: Missing override modifier
    public function makeSound():String {
        return "Woof!";
    }

    // Line 25: Correct override
    override public function eat(food:String):Bool {
        return food == "dog food";
    }
}

// Line 31: Cat class with invalid override
class Cat extends Animal {
    public function new() {
        super();
    }

    // Line 37: Invalid override - no such method in parent
    override public function purr():String {
        return "Purrr...";
    }
}"#;

        let result = compile_haxe_source(haxe_code);

        println!("\n╔═══════════════════════════════════════════════════════════════════════╗");
        println!("║                 Override Validation Diagnostics                        ║");
        println!("╚═══════════════════════════════════════════════════════════════════════╝\n");

        for (i, error) in result.errors.iter().enumerate() {
            println!("━━━ Diagnostic {} ━━━", i + 1);
            println!("{}", error.message);
            println!();
        }

        println!("Summary: {} diagnostic(s) found\n", result.errors.len());

        // Verify the diagnostics are working
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.message.contains("makeSound") && e.message.contains("override")),
            "Should detect missing override on makeSound method"
        );

        assert!(
            result
                .errors
                .iter()
                .any(|e| e.message.contains("purr") && e.message.contains("no parent method")),
            "Should detect invalid override on purr method"
        );
    }

    #[test]
    fn test_signature_mismatch_diagnostics() {
        let haxe_code = r#"class Shape {
    public function new() {}

    public function draw(x:Int, y:Int):Void {
        // Draw at position
    }

    public function rotate(angle:Float):Void {
        // Rotate by angle
    }
}

class Rectangle extends Shape {
    public function new() {
        super();
    }

    // Wrong parameter types
    override public function draw(x:Float, y:Float):Void {
        // Implementation
    }

    // Wrong return type
    override public function rotate(angle:Float):Bool {
        return true;
    }
}"#;

        let result = compile_haxe_source(haxe_code);

        println!("\n╔═══════════════════════════════════════════════════════════════════════╗");
        println!("║               Signature Mismatch Diagnostics                           ║");
        println!("╚═══════════════════════════════════════════════════════════════════════╝\n");

        for error in &result.errors {
            if error.message.contains("signature") || error.message.contains("incompatible") {
                println!("{}\n", error.message);
            }
        }

        // Verify signature checking works
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.message.contains("signature")),
            "Should detect signature mismatches"
        );
    }

    #[test]
    fn test_interface_implementation_diagnostics() {
        let haxe_code = r#"interface IAnimal {
    function getName():String;
    function getAge():Int;
    function makeSound():String;
}

class Dog implements IAnimal {
    var name:String;

    public function new(name:String) {
        this.name = name;
    }

    // Correct implementation
    public function getName():String {
        return name;
    }

    // Wrong return type
    public function getAge():String {  // Should be Int
        return "5 years";
    }

    // Missing makeSound() method
}"#;

        let result = compile_haxe_source(haxe_code);

        println!("\n╔═══════════════════════════════════════════════════════════════════════╗");
        println!("║             Interface Implementation Diagnostics                       ║");
        println!("╚═══════════════════════════════════════════════════════════════════════╝\n");

        for error in &result.errors {
            println!("{}\n", error.message);
        }

        // Should have at least one interface error
        assert!(
            !result.errors.is_empty(),
            "Should have interface implementation errors"
        );
    }
}
