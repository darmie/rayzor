use compiler::pipeline::*;

#[test]
fn test_for_in_loop_type_inference() {
    let source = r#"
class TestForIn {
    static function main() {
        var numbers = [1, 2, 3, 4, 5];
        
        // Simple for-in loop
        for (num in numbers) {
            trace(num * 2); // num should have type Int, not Unknown
        }
        
        // Key-value for-in loop  
        var items = ["a", "b", "c"];
        for (index => value in items) {
            trace(index); // index should be Int
            trace(value); // value should be String
        }
        
        // Test with map
        var map = new Map<String, Int>();
        map.set("one", 1);
        map.set("two", 2);
        
        for (key => val in map) {
            trace(key); // key should be String  
            trace(val); // val should be Int
        }
    }
}
"#;

    // Create a minimal compilation pipeline
    let mut pipeline = HaxeCompilationPipeline::new();

    // Compile the source
    let result = pipeline.compile_file("test_for_in.hx", source);

    if result.errors.is_empty() {
        println!("✅ For-in compilation succeeded!");
        println!("  Typed files: {}", result.typed_files.len());
    } else {
        println!("❌ For-in compilation failed with errors:");
        for error in &result.errors {
            println!("  - {}", error.message);
        }
        panic!("Type checking should succeed for valid for-in loops");
    }
}
