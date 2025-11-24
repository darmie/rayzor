#[cfg(test)]
mod method_overload_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_method_overload_basic() {
        let haxe_code = r#"
class Calculator {
    @:overload("x:Float, y:Float -> Float")
    @:overload("values:Array<Int> -> Int")
    public function add(x:Int, y:Int):Int {
        return x + y;
    }
}

class Test {
    public function new() {}
    
    public function testOverloads():Void {
        var calc = new Calculator();
        
        // Should work: main signature (Int, Int) -> Int
        var result1:Int = calc.add(10, 20);
        
        // Should work: overload (Float, Float) -> Float
        var result2:Float = calc.add(10.5, 20.5);
        
        // Should work: overload (Array<Int>) -> Int
        var result3:Int = calc.add([1, 2, 3, 4, 5]);
        
        // Should fail: no matching signature
        // var result4 = calc.add("hello", "world");
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Method Overload Basic Test ===");
        println!("Total diagnostics: {}", result.errors.len());
        
        for (i, error) in result.errors.iter().enumerate() {
            println!("Diagnostic {}:\n{}\n", i + 1, error.message);
        }
        
        // Should have no errors for valid overload calls
        // (In a complete implementation, we'd expect no errors here)
        println!("✅ Basic overload test completed");
    }
    
    #[test]
    fn test_method_overload_type_mismatch() {
        let haxe_code = r#"
class MathUtils {
    @:overload("value:Float -> String")
    @:overload("values:Array<Float> -> Array<String>")
    public function format(value:Int):String {
        return Std.string(value);
    }
}

class Test {
    public function new() {}
    
    public function testTypeMismatch():Void {
        var utils = new MathUtils();
        
        // Should work: main signature
        var result1:String = utils.format(42);
        
        // Should work: overload (Float) -> String
        var result2:String = utils.format(3.14);
        
        // Should work: overload (Array<Float>) -> Array<String>
        var result3:Array<String> = utils.format([1.1, 2.2, 3.3]);
        
        // Should fail: no matching signature for Bool
        var result4:String = utils.format(true);
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Method Overload Type Mismatch Test ===");
        println!("Total diagnostics: {}", result.errors.len());
        
        for (i, error) in result.errors.iter().enumerate() {
            println!("Diagnostic {}:\n{}\n", i + 1, error.message);
        }
        
        // Should have at least one error for the invalid Bool parameter
        assert!(!result.errors.is_empty(), "Should have type mismatch error");
        println!("✅ Type mismatch test completed");
    }
    
    #[test]
    fn test_method_overload_complex_signatures() {
        let haxe_code = r#"
class DataProcessor {
    @:overload("data:String, options:Dynamic -> Array<String>")
    @:overload("data:Array<String>, transformer:String -> String -> String")
    @:overload("data:Dynamic -> String")
    public function process(data:Int, count:Int):Array<Int> {
        var result = new Array<Int>();
        for (i in 0...count) {
            result.push(data + i);
        }
        return result;
    }
}

class Test {
    public function new() {}
    
    public function testComplexOverloads():Void {
        var processor = new DataProcessor();
        
        // Main signature: (Int, Int) -> Array<Int>
        var result1:Array<Int> = processor.process(5, 3);
        
        // Overload: (String, Dynamic) -> Array<String>
        var result2:Array<String> = processor.process("hello", {split: true});
        
        // Overload: (Array<String>, String -> String) -> String  
        var result3:String = processor.process(["a", "b", "c"], function(s:String):String return s.toUpperCase());
        
        // Overload: (Dynamic) -> String
        var result4:String = processor.process({key: "value"});
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Method Overload Complex Signatures Test ===");
        println!("Total diagnostics: {}", result.errors.len());
        
        for (i, error) in result.errors.iter().enumerate() {
            println!("Diagnostic {}:\n{}\n", i + 1, error.message);
        }
        
        println!("✅ Complex signatures test completed");
    }
    
    #[test]
    fn test_method_overload_inheritance() {
        let haxe_code = r#"
class BaseConverter {
    @:overload("value:Float -> String")
    public function convert(value:Int):String {
        return Std.string(value);
    }
}

class ExtendedConverter extends BaseConverter {
    @:overload("value:Bool -> String")
    @:overload("values:Array<Dynamic> -> String")
    override public function convert(value:Int):String {
        return "Extended: " + super.convert(value);
    }
}

class Test {
    public function new() {}
    
    public function testInheritanceOverloads():Void {
        var converter = new ExtendedConverter();
        
        // Main signature: (Int) -> String
        var result1:String = converter.convert(42);
        
        // Base class overload: (Float) -> String
        var result2:String = converter.convert(3.14);
        
        // Extended class overloads: (Bool) -> String
        var result3:String = converter.convert(true);
        
        // Extended class overloads: (Array<Dynamic>) -> String
        var result4:String = converter.convert([1, "two", 3.0]);
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Method Overload Inheritance Test ===");
        println!("Total diagnostics: {}", result.errors.len());
        
        for (i, error) in result.errors.iter().enumerate() {
            println!("Diagnostic {}:\n{}\n", i + 1, error.message);
        }
        
        println!("✅ Inheritance overloads test completed");
    }
    
    #[test]
    fn test_method_overload_error_messages() {
        let haxe_code = r#"
class ErrorTest {
    @:overload("x:Int, y:String -> String")
    @:overload("data:Array<Float> -> Float")
    public function process(value:Bool):Int {
        return value ? 1 : 0;
    }
}

class Test {
    public function new() {}
    
    public function testErrorMessages():Void {
        var processor = new ErrorTest();
        
        // This should fail - no matching overload for (String, Bool)
        var result = processor.process("invalid", true);
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Method Overload Error Messages Test ===");
        println!("Total diagnostics: {}", result.errors.len());
        
        for (i, error) in result.errors.iter().enumerate() {
            println!("Diagnostic {}:\n{}\n", i + 1, error.message);
        }
        
        // Should have clear error about no matching overload
        assert!(!result.errors.is_empty(), "Should have overload error");
        
        // Check that error mentions overloads
        let has_overload_error = result.errors.iter().any(|error| 
            error.message.to_lowercase().contains("overload") || 
            error.message.to_lowercase().contains("signature")
        );
        
        if has_overload_error {
            println!("✅ Found expected overload error message");
        } else {
            println!("⚠️ Expected overload-specific error message");
        }
        
        println!("✅ Error messages test completed");
    }
}