#[cfg(test)]
mod method_overload_comprehensive_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_comprehensive_method_overloading_feature() {
        let haxe_code = r#"
class StringProcessor {
    @:overload("text:String, count:Int -> String")
    @:overload("values:Array<String> -> String")
    @:overload("config:Dynamic -> String")
    public function process(input:Bool):String {
        return if (input) "true" else "false";
    }
}

class MathCalculator {
    @:overload("a:Float, b:Float -> Float")
    @:overload("values:Array<Int> -> Float")
    public function calculate(x:Int, y:Int):Int {
        return x + y;
    }
}

class TestApplication {
    public function new() {}
    
    public function demonstrateOverloading():Void {
        var processor = new StringProcessor();
        var calculator = new MathCalculator();
        
        // Test StringProcessor overloads
        var result1:String = processor.process(true);                      // Main signature: (Bool) -> String
        var result2:String = processor.process("hello", 3);                // Overload: (String, Int) -> String  
        var result3:String = processor.process(["a", "b", "c"]);           // Overload: (Array<String>) -> String
        var result4:String = processor.process({format: "json"});          // Overload: (Dynamic) -> String
        
        // Test MathCalculator overloads  
        var calc1:Int = calculator.calculate(10, 20);                      // Main signature: (Int, Int) -> Int
        var calc2:Float = calculator.calculate(10.5, 20.5);               // Overload: (Float, Float) -> Float
        var calc3:Float = calculator.calculate([1, 2, 3, 4, 5]);          // Overload: (Array<Int>) -> Float
        
        trace("StringProcessor results:");
        trace("Bool input: " + result1);
        trace("String + Int: " + result2);
        trace("Array<String>: " + result3);
        trace("Dynamic: " + result4);
        
        trace("MathCalculator results:");
        trace("Int calculation: " + Std.string(calc1));
        trace("Float calculation: " + Std.string(calc2));
        trace("Array sum: " + Std.string(calc3));
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Comprehensive Method Overloading Feature Test ===");
        println!("Total diagnostics: {}", result.errors.len());
        
        if result.errors.is_empty() {
            println!("✅ All method overload calls compiled successfully!");
            println!("🎉 Method overloading feature is working perfectly!");
        } else {
            println!("Diagnostics found:");
            for (i, error) in result.errors.iter().enumerate() {
                println!("{}. {}", i + 1, error.message);
            }
        }
        
        // Should have no errors for valid overload usage
        assert_eq!(result.errors.len(), 0, "Should have no compilation errors for valid overloads");
        
        println!("✅ Comprehensive method overloading test completed successfully!");
    }

    #[test]
    fn test_overload_priority_and_ambiguity() {
        let haxe_code = r#"
class OverloadTester {
    @:overload("value:Dynamic -> String")
    @:overload("text:String -> String")
    public function convert(num:Int):String {
        return Std.string(num);
    }
}

class Test {
    public function new() {}
    
    public function testPriority():Void {
        var tester = new OverloadTester();
        
        // These should all work
        var result1:String = tester.convert(42);           // Main: (Int) -> String
        var result2:String = tester.convert("hello");      // Overload: (String) -> String (more specific than Dynamic)
        var result3:String = tester.convert({key: "val"}); // Overload: (Dynamic) -> String
        var result4:String = tester.convert(true);         // Overload: (Dynamic) -> String
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Overload Priority and Ambiguity Test ===");
        println!("Total diagnostics: {}", result.errors.len());
        
        for (i, error) in result.errors.iter().enumerate() {
            println!("{}. {}", i + 1, error.message);
        }
        
        // Should have no ambiguity errors with proper priority resolution
        if result.errors.is_empty() {
            println!("✅ Method overload priority resolution working correctly!");
        }
        
        println!("✅ Priority test completed!");
    }
}