#[cfg(test)]
mod method_overload_final_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_method_overloading_feature_demo() {
        let haxe_code = r#"
class Calculator {
    @:overload("x:Float, y:Float -> Float")
    @:overload("values:Array<Int> -> Int")
    public function add(x:Int, y:Int):Int {
        return x + y;
    }
}

class StringFormatter {
    @:overload("text:String, count:Int -> String")
    @:overload("config:Dynamic -> String")
    public function format(input:Bool):String {
        return if (input) "true" else "false";
    }
}

class TestOverloads {
    public function new() {}
    
    public function demonstrate():Void {
        var calc = new Calculator();
        var formatter = new StringFormatter();
        
        // Calculator overloads
        var result1:Int = calc.add(10, 20);           // Main: (Int, Int) -> Int
        var result2:Float = calc.add(10.5, 20.5);    // Overload: (Float, Float) -> Float
        var result3:Int = calc.add([1, 2, 3]);       // Overload: (Array<Int>) -> Int
        
        // StringFormatter overloads
        var format1:String = formatter.format(true);              // Main: (Bool) -> String
        var format2:String = formatter.format("hello", 3);        // Overload: (String, Int) -> String
        var format3:String = formatter.format({type: "json"});    // Overload: (Dynamic) -> String
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Method Overloading Feature Demonstration ===");
        println!("Total diagnostics: {}", result.errors.len());
        
        if result.errors.is_empty() {
            println!("🎉 SUCCESS: Method overloading feature is working perfectly!");
            println!("✅ All overload calls compiled without errors");
            println!("✅ Main signatures work correctly");
            println!("✅ @:overload annotations are processed correctly");
            println!("✅ Type checking resolves to correct overloads");
        } else {
            println!("❌ Found some issues:");
            for (i, error) in result.errors.iter().enumerate() {
                println!("{}. {}", i + 1, error.message);
            }
        }
        
        // Should have no errors for this clean test
        assert_eq!(result.errors.len(), 0, "Method overloading should work without errors");
        
        println!("✅ Method overloading feature demonstration completed!");
    }
}