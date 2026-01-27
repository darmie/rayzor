#[cfg(test)]
mod human_friendly_diagnostics_tests {
    use crate::pipeline::compile_haxe_source;

    /// Strip ANSI color codes from diagnostic messages for cleaner display
    fn strip_ansi_codes(s: &str) -> String {
        let re = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap_or_else(|_| {
            // Fallback if regex fails - just remove common patterns
            regex::Regex::new(r"\[(?:31|32|33|34|35|36|96|0)m").unwrap()
        });
        re.replace_all(s, "").to_string()
    }

    #[test]
    fn test_missing_override_human_friendly() {
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
    var name:String;

    public function new(name:String) {
        super();
        this.name = name;
    }

    // Missing override modifier - this is a common mistake
    public function makeSound():String {
        return "Woof! I'm " + name;
    }

    // Also missing override modifier
    public function move(distance:Float):Void {
        trace(name + " is running " + distance + " meters");
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║          Human-Friendly Override Validation Diagnostics         ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");

        // Filter and display override-related errors
        let override_errors: Vec<_> = result.errors.iter()
            .filter(|e| e.message.contains("override"))
            .collect();

        println!("Found {} override-related errors:\n", override_errors.len());

        for (i, error) in override_errors.iter().enumerate() {
            println!("─── Error {} ───", i + 1);

            // Clean the message for display
            let clean_message = strip_ansi_codes(&error.message);

            // Split the message into logical parts
            let lines: Vec<&str> = clean_message.lines().collect();

            // Extract the main error message
            if let Some(error_line) = lines.iter().find(|l| l.contains("error[E")) {
                let parts: Vec<&str> = error_line.split(": ").collect();
                if parts.len() >= 2 {
                    println!("❌ {}", parts[1]);
                }
            }

            // Show the location
            if let Some(location_line) = lines.iter().find(|l| l.contains("-->")) {
                println!("📍 Location: {}", location_line.trim());
            }

            // Show the help message
            if let Some(help_line) = lines.iter().find(|l| l.contains("help")) {
                let help_text = help_line.split("help").nth(1).unwrap_or("").trim();
                println!("💡 Suggestion: {}", help_text);
            }

            // Show the note if present
            if let Some(note_line) = lines.iter().find(|l| l.contains("note")) {
                let note_text = note_line.split("note").nth(1).unwrap_or("").trim();
                println!("📝 Note: {}", note_text);
            }

            println!();
        }

        // Verify we found the expected errors
        assert_eq!(override_errors.len(), 2, "Should find 2 missing override errors");
    }

    #[test]
    fn test_invalid_override_human_friendly() {
        let haxe_code = r#"
class Vehicle {
    public function new() {}

    public function start():Bool {
        return true;
    }
}

class Bicycle extends Vehicle {
    public function new() {
        super();
    }

    // This is wrong - bicycles don't have engines!
    override public function startEngine():Bool {
        return false;
    }

    // Another mistake - no such method in parent
    override public function pedal():Void {
        // Pedaling implementation
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║           Invalid Override Method Diagnostics                   ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");

        for error in &result.errors {
            if error.message.contains("no parent method") {
                let clean_message = strip_ansi_codes(&error.message);
                let lines: Vec<&str> = clean_message.lines().collect();

                // Extract method name from error
                if let Some(error_line) = lines.iter().find(|l| l.contains("error[E")) {
                    if let Some(method_match) = error_line.split("'").nth(1) {
                        println!("⚠️  Method '{}' is marked as override but:", method_match);
                        println!("   - No method with this name exists in parent class");
                        println!("   - Did you mean to override a different method?");
                        println!("   - Or should this be a new method without 'override'?");
                        println!();
                    }
                }
            }
        }
    }

    #[test]
    fn test_signature_mismatch_human_friendly() {
        let haxe_code = r#"
class Shape {
    public function new() {}

    public function calculateArea(precision:Int):Float {
        return 0.0;
    }

    public function draw(x:Int, y:Int, color:String):Void {
        // Draw shape
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

    // Wrong parameter type - common mistake
    override public function calculateArea(precision:Float):Float {
        return width * height;
    }

    // Missing parameter - another common mistake
    override public function draw(x:Int, y:Int):Void {
        // Forgot the color parameter!
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║         Method Signature Mismatch Diagnostics                   ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");

        for error in &result.errors {
            let clean_message = strip_ansi_codes(&error.message);

            if clean_message.contains("incompatible signature") {
                println!("🔍 Signature Mismatch Detected!");

                // Extract the parent and override signatures
                let lines: Vec<&str> = clean_message.lines().collect();

                for line in &lines {
                    if line.contains("Parent:") {
                        println!("   {}", line.trim());
                    } else if line.contains("Override:") {
                        println!("   {}", line.trim());
                    }
                }

                // Extract method name
                if let Some(method_line) = lines.iter().find(|l| l.contains("Overridden method")) {
                    if let Some(method_name) = method_line.split("'").nth(1) {
                        println!("\n   ⚡ Method '{}' has different signature than parent", method_name);
                        println!("   📋 Check parameter types and count match exactly");
                    }
                }

                println!();
            }
        }
    }

    #[test]
    fn test_comprehensive_error_summary() {
        let haxe_code = r#"
// Base classes
class Vehicle {
    public function new() {}
    public function start():Bool { return true; }
    public function accelerate(speed:Float):Void {}
}

interface IIdentifiable {
    function getId():String;
    function setId(id:String):Void;
}

// Problem class with multiple issues
class Car extends Vehicle implements IIdentifiable {
    var id:String;

    public function new() {
        super();
        id = "CAR-001";
    }

    // Missing override modifier
    public function start():Bool {
        return super.start();
    }

    // Wrong signature with override
    override public function accelerate(speed:Int):Void {
        // Using Int instead of Float
    }

    // Invalid override - no such method in parent
    override public function honk():Void {
        trace("Beep beep!");
    }

    // Missing interface method: getId()

    // Wrong signature for interface method
    public function setId(id:Int):Void {  // Should be String, not Int
        // Wrong parameter type
    }
}
        "#;

        let result = compile_haxe_source(haxe_code);

        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║                  Error Summary Report                           ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");

        // Categorize errors
        let mut missing_override = 0;
        let mut invalid_override = 0;
        let mut signature_mismatch = 0;
        let mut interface_errors = 0;
        let mut other_errors = 0;

        for error in &result.errors {
            let msg = strip_ansi_codes(&error.message);
            if msg.contains("missing the 'override' modifier") {
                missing_override += 1;
            } else if msg.contains("no parent method") {
                invalid_override += 1;
            } else if msg.contains("incompatible signature") {
                signature_mismatch += 1;
            } else if msg.contains("interface") || msg.contains("not implemented") {
                interface_errors += 1;
            } else {
                other_errors += 1;
            }
        }

        println!("📊 Error Categories:");
        println!("├─ Missing Override Modifier: {}", missing_override);
        println!("├─ Invalid Override Usage: {}", invalid_override);
        println!("├─ Signature Mismatches: {}", signature_mismatch);
        println!("├─ Interface Implementation: {}", interface_errors);
        println!("└─ Other Errors: {}", other_errors);
        println!("\n📈 Total Errors: {}", result.errors.len());

        println!("\n🔧 Quick Fix Guide:");
        if missing_override > 0 {
            println!("• Add 'override' keyword to methods that override parent methods");
        }
        if invalid_override > 0 {
            println!("• Remove 'override' from methods that don't exist in parent class");
        }
        if signature_mismatch > 0 {
            println!("• Ensure overridden methods match parent signatures exactly");
        }
        if interface_errors > 0 {
            println!("• Implement all required interface methods with correct signatures");
        }

        // Verify we have a mix of errors
        assert!(result.errors.len() >= 4, "Should have multiple types of errors");
    }
}

// Add regex as a dev dependency if not already present
#[cfg(test)]
extern crate regex;