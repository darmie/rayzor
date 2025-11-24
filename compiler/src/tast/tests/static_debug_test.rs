#[cfg(test)]
mod static_debug_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_simple_static_access() {
        let haxe_code = r#"
class TestClass {
    public static var staticVar:Int = 10;
    public var instanceVar:Int = 20;
}

class Main {
    public function new() {}
    
    public function test():Void {
        // This should work - static access
        var x = TestClass.staticVar;
        
        // This should fail - instance member via static access
        var y = TestClass.instanceVar;
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Static Access Debug Test ===");
        println!("Total errors: {}", result.errors.len());
        for error in &result.errors {
            println!("Error: {}", error.message);
            println!("Location: {:?}", error.location);
        }
    }
}