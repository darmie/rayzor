#[cfg(test)]
mod static_method_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_static_method_access() {
        let haxe_code = r#"
class TestClass {
    public static function staticMethod():String {
        return "static";
    }
    
    public function instanceMethod():String {
        return "instance";
    }
}

class Main {
    public function new() {}
    
    public function test():Void {
        var obj = new TestClass();
        
        // This should error - calling static method through instance
        obj.staticMethod();
        
        // This should error - calling instance method through class
        TestClass.instanceMethod();
    }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Static Method Access Test ===");
        println!("Total errors: {}", result.errors.len());
        for error in &result.errors {
            println!("Error: {}", error.message);
        }
    }
}