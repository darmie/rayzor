#[cfg(test)]
mod override_diagnostics_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_missing_override_diagnostic_output() {
        let haxe_code = r#"
class Animal {
    public function new() {}
    
    public function makeSound():String {
        return "generic animal sound";
    }
    
    public function move(distance:Float):Void {
        trace("Moving " + distance + " meters");
    }
}

class Dog extends Animal {
    public function new() {
        super();
    }
    
    // Missing override modifier - should show helpful diagnostic
    public function makeSound():String {
        return "Woof!";
    }
    
    // Also missing override modifier
    public function move(distance:Float):Void {
        trace("Dog running " + distance + " meters");
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        // Print the actual diagnostic messages
        println!("\n=== Missing Override Modifier Diagnostics ===\n");
        for error in &result.errors {
            if error.message.contains("override") {
                println!("{}", error.message);
                println!("---");
            }
        }
        
        // Verify we have the expected errors
        assert!(!result.errors.is_empty(), "Should have errors");
        
        // Check that we have helpful error messages
        let has_helpful_errors = result.errors.iter().any(|e| 
            e.message.contains("makeSound") && 
            e.message.contains("overrides parent method") &&
            e.message.contains("Add 'override' modifier")
        );
        assert!(has_helpful_errors, "Should have helpful diagnostic messages");
    }
    
    #[test]
    fn test_invalid_override_diagnostic_output() {
        let haxe_code = r#"
class BaseClass {
    public function new() {}
    
    public function existingMethod():Int {
        return 42;
    }
}

class DerivedClass extends BaseClass {
    public function new() {
        super();
    }
    
    // This method doesn't exist in parent - should show helpful diagnostic
    override public function nonExistentMethod():Void {
        trace("This shouldn't override anything");
    }
    
    // Another invalid override
    override public function anotherFakeMethod():String {
        return "nope";
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        // Print the actual diagnostic messages
        println!("\n=== Invalid Override Diagnostics ===\n");
        for error in &result.errors {
            if error.message.contains("override") {
                println!("{}", error.message);
                println!("---");
            }
        }
        
        // Verify we have the expected errors
        assert!(!result.errors.is_empty(), "Should have errors");
        
        // Check for helpful suggestions
        let has_helpful_suggestion = result.errors.iter().any(|e| 
            e.message.contains("no parent method") &&
            e.message.contains("Remove the 'override' modifier")
        );
        assert!(has_helpful_suggestion, "Should suggest removing override modifier");
    }
    
    #[test]
    fn test_signature_mismatch_diagnostic_output() {
        let haxe_code = r#"
interface ICalculator {
    function calculate(a:Float, b:Float):Float;
}

class BasicCalculator implements ICalculator {
    public function new() {}
    
    // Wrong parameter types - should show detailed signature mismatch
    public function calculate(a:Int, b:Int):Float {
        return a + b;
    }
}

class Shape {
    public function new() {}
    
    public function getArea():Float {
        return 0.0;
    }
    
    public function draw(x:Int, y:Int):Void {
        // Draw at position
    }
}

class Circle extends Shape {
    public function new() {
        super();
    }
    
    // Wrong return type - should show parent vs child signatures
    override public function getArea():Int {
        return 314;  // Should be Float
    }
    
    // Wrong parameter types
    override public function draw(x:Float, y:Float):Void {
        // Draw circle
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        // Print all diagnostic messages to see the formatting
        println!("\n=== Signature Mismatch Diagnostics ===\n");
        for error in &result.errors {
            println!("{}", error.message);
            println!("---");
        }
        
        // Check for signature mismatch errors with detailed info
        let has_signature_details = result.errors.iter().any(|e| 
            e.message.contains("Parent:") && 
            e.message.contains("Override:") &&
            e.message.contains("incompatible signature")
        );
        
        println!("\nTotal errors found: {}", result.errors.len());
        println!("Has signature details: {}", has_signature_details);
    }
    
    #[test]
    fn test_comprehensive_diagnostic_formatting() {
        let haxe_code = r#"
class Vehicle {
    public function new() {}
    
    public function start():Bool {
        return true;
    }
    
    public function accelerate(speed:Float):Void {
        // Accelerate to speed
    }
}

class Car extends Vehicle {
    public function new() {
        super();
    }
    
    // Missing override
    public function start():Bool {
        return false;
    }
    
    // Has override but wrong signature
    override public function accelerate(speed:Int):Void {
        // Wrong parameter type
    }
}

class Bicycle extends Vehicle {
    public function new() {
        super();
    }
    
    // Invalid override - no such method in parent
    override public function pedal():Void {
        // Pedaling
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Comprehensive Diagnostic Output ===\n");
        
        // Group errors by type
        let mut missing_override_errors = Vec::new();
        let mut invalid_override_errors = Vec::new();
        let mut signature_mismatch_errors = Vec::new();
        let mut other_errors = Vec::new();
        
        for error in &result.errors {
            if error.message.contains("missing the 'override' modifier") {
                missing_override_errors.push(error);
            } else if error.message.contains("no parent method") {
                invalid_override_errors.push(error);
            } else if error.message.contains("incompatible signature") {
                signature_mismatch_errors.push(error);
            } else {
                other_errors.push(error);
            }
        }
        
        if !missing_override_errors.is_empty() {
            println!("📋 Missing Override Modifiers:");
            for error in &missing_override_errors {
                println!("{}", error.message);
            }
            println!();
        }
        
        if !invalid_override_errors.is_empty() {
            println!("❌ Invalid Override Usage:");
            for error in &invalid_override_errors {
                println!("{}", error.message);
            }
            println!();
        }
        
        if !signature_mismatch_errors.is_empty() {
            println!("🔀 Signature Mismatches:");
            for error in &signature_mismatch_errors {
                println!("{}", error.message);
            }
            println!();
        }
        
        if !other_errors.is_empty() {
            println!("ℹ️  Other Errors:");
            for error in &other_errors {
                // Only show first line to keep output concise
                let first_line = error.message.lines().next().unwrap_or(&error.message);
                println!("  - {}", first_line);
            }
        }
        
        println!("\n📊 Summary:");
        println!("  - Missing override modifiers: {}", missing_override_errors.len());
        println!("  - Invalid overrides: {}", invalid_override_errors.len());
        println!("  - Signature mismatches: {}", signature_mismatch_errors.len());
        println!("  - Other errors: {}", other_errors.len());
        println!("  - Total errors: {}", result.errors.len());
        
        // Verify we have at least one of each expected error type
        assert!(!missing_override_errors.is_empty(), "Should have missing override errors");
        assert!(!invalid_override_errors.is_empty(), "Should have invalid override errors");
        assert!(!signature_mismatch_errors.is_empty(), "Should have signature mismatch errors");
    }
}