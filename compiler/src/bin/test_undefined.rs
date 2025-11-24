use compiler::pipeline::compile_haxe_source;

fn main() {
    // Test static method accessing instance member
    let code1 = r#"
class StaticContext {
    private static var staticData:Int = 100;
    private var instanceData:String = "data";
    
    public static function staticWork():Void {
        // Valid: static accessing static
        var x = staticData;
        
        // Invalid: static method accessing instance member
        var y = instanceData;  // Should error
    }
}
    "#;
    
    let result1 = compile_haxe_source(code1);
    println!("=== Static method context test ===");
    println!("Total errors: {}", result1.errors.len());
    for error in &result1.errors {
        println!("Error: {}", error.message);
    }
    
    // Test undefined variable
    let code2 = r#"
class Test {
    static function main() {
        trace(undefinedVar);  // Should error
    }
}
    "#;
    
    let result2 = compile_haxe_source(code2);
    println!("\n=== Undefined variable test ===");
    println!("Total errors: {}", result2.errors.len());
    for error in &result2.errors {
        println!("Error: {} at {}:{}:{}", 
            error.message, 
            error.location.file_id, 
            error.location.line, 
            error.location.column
        );
    }
}