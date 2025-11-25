/// Comprehensive test for String class methods
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() -> Result<(), String> {
    println!("=== Testing String Class Methods ===\n");

    let haxe_source = r#"
package test;

class Main {
    static function main() {
        // Test 1: String length property
        var s:String = "hello";
        trace(s.length);  // Should print 5

        // Test 2: toUpperCase
        trace(s.toUpperCase());  // Should print HELLO

        // Test 3: toLowerCase
        var upper:String = "WORLD";
        trace(upper.toLowerCase());  // Should print world

        // Test 4: charAt
        trace(s.charAt(0));  // Should print h
        trace(s.charAt(4));  // Should print o

        // Test 5: charCodeAt
        trace(s.charCodeAt(0));  // Should print 104 (ASCII for 'h')
        trace(s.charCodeAt(1));  // Should print 101 (ASCII for 'e')

        // Test 6: indexOf
        var text:String = "hello world";
        trace(text.indexOf("o", 0));    // Should print 4
        trace(text.indexOf("world", 0)); // Should print 6
        trace(text.indexOf("xyz", 0));   // Should print -1

        // Test 7: lastIndexOf
        trace(text.lastIndexOf("o", 100)); // Should print 7
        trace(text.lastIndexOf("l", 100)); // Should print 9

        // Test 8: substr
        trace(text.substr(0, 5));  // Should print hello
        trace(text.substr(6, 5));  // Should print world

        // Test 9: substring
        trace(text.substring(0, 5));  // Should print hello
        trace(text.substring(6, 11)); // Should print world

        // Test 10: String.fromCharCode (static)
        trace(String.fromCharCode(65));  // Should print A
        trace(String.fromCharCode(90));  // Should print Z
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    println!("Adding test file...");
    unit.add_file(haxe_source, "test_string_class.hx")
        .map_err(|e| format!("Failed to add file: {}", e))?;

    println!("Compiling to TAST...");
    unit.lower_to_tast()
        .map_err(|errors| format!("TAST errors: {:?}", errors))?;

    println!("Getting MIR modules...");
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    println!("MIR modules: {}", mir_modules.len());

    println!("\nCompiling to native code...");
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    println!("Codegen complete!\n");

    println!("=== Expected Output ===");
    println!("5");       // length
    println!("HELLO");   // toUpperCase
    println!("world");   // toLowerCase
    println!("h");       // charAt(0)
    println!("o");       // charAt(4)
    println!("104");     // charCodeAt(0)
    println!("101");     // charCodeAt(1)
    println!("4");       // indexOf("o")
    println!("6");       // indexOf("world")
    println!("-1");      // indexOf("xyz")
    println!("7");       // lastIndexOf("o")
    println!("9");       // lastIndexOf("l")
    println!("hello");   // substr(0, 5)
    println!("world");   // substr(6, 5)
    println!("hello");   // substring(0, 5)
    println!("world");   // substring(6, 11)
    println!("A");       // fromCharCode(65)
    println!("Z");       // fromCharCode(90)
    println!("\n=== Actual Output ===\n");

    for module in mir_modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            println!("\n=== Test Complete ===");
            return Ok(());
        }
    }

    Err("Failed to execute main".to_string())
}
