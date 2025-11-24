#[cfg(test)]
mod diagnostic_simple_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_simple_missing_override() {
        let haxe_code = r#"
class Base {
    public function new() {}
    public function test():String { return "base"; }
}

class Child extends Base {
    public function new() { super(); }
    public function test():String { return "child"; }
}
        "#;
        
        let result = compile_haxe_source(haxe_code);
        
        println!("\n=== Compilation Result ===");
        println!("Errors: {}", result.errors.len());
        println!("Warnings: {}", result.warnings.len());
        
        for (i, error) in result.errors.iter().enumerate() {
            println!("\nError {}:", i + 1);
            println!("Message: {}", error.message);
            println!("Location: {:?}", error.location);
            println!("Category: {:?}", error.category);
        }
        
        // Just check we have errors
        assert!(!result.errors.is_empty(), "Should have at least one error");
    }
}