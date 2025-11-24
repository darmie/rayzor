/// Test on-demand stdlib loading without runtime dependencies
///
/// This test validates that:
/// 1. Root stdlib files load at startup (24 files)
/// 2. Package-level stdlib types load on-demand when referenced
/// 3. All stdlib symbols resolve correctly during TAST lowering
/// 4. No parsing errors occur

use compiler::compilation::{CompilationUnit, CompilationConfig};

fn main() {
    println!("=== On-Demand Stdlib Loading Test ===\n");

    let mut passed = 0;
    let mut failed = 0;

    // Test 1: Array with haxe.iterators.ArrayIterator (on-demand)
    println!("\n{}", "=".repeat(70));
    println!("TEST 1: Array with iterator (should load ArrayIterator on-demand)");
    println!("{}", "=".repeat(70));

    let test1 = r#"
class Main {
    static function main() {
        var arr = new Array<Int>();
        arr.push(1);
        arr.push(2);
        arr.push(3);

        // This should trigger loading of haxe.iterators.ArrayIterator
        for (item in arr) {
            var x = item;
        }
    }
}
"#;

    match run_test("test_array_iterator", test1) {
        Ok(_) => {
            println!("✅ TEST 1 PASSED");
            passed += 1;
        }
        Err(e) => {
            println!("❌ TEST 1 FAILED: {}", e);
            failed += 1;
        }
    }

    // Test 2: StringMap (should load haxe.ds.StringMap on-demand)
    println!("\n{}", "=".repeat(70));
    println!("TEST 2: StringMap (should load haxe.ds.StringMap on-demand)");
    println!("{}", "=".repeat(70));

    let test2 = r#"
import haxe.ds.StringMap;

class Main {
    static function main() {
        var map = new StringMap<Int>();
        map.set("one", 1);
        map.set("two", 2);
        var value = map.get("one");
    }
}
"#;

    match run_test("test_stringmap", test2) {
        Ok(_) => {
            println!("✅ TEST 2 PASSED");
            passed += 1;
        }
        Err(e) => {
            println!("❌ TEST 2 FAILED: {}", e);
            failed += 1;
        }
    }

    // Test 3: Math operations (Math.hx is in root, should already be loaded)
    println!("\n{}", "=".repeat(70));
    println!("TEST 3: Math operations (Math.hx in root stdlib)");
    println!("{}", "=".repeat(70));

    let test3 = r#"
class Main {
    static function main() {
        var x = Math.sqrt(16.0);
        var y = Math.floor(3.7);
        var z = Math.ceil(2.1);
        var max = Math.max(10, 20);
    }
}
"#;

    match run_test("test_math", test3) {
        Ok(_) => {
            println!("✅ TEST 3 PASSED");
            passed += 1;
        }
        Err(e) => {
            println!("❌ TEST 3 FAILED: {}", e);
            failed += 1;
        }
    }

    // Test 4: String operations (String.hx is in root)
    println!("\n{}", "=".repeat(70));
    println!("TEST 4: String operations (String.hx in root stdlib)");
    println!("{}", "=".repeat(70));

    let test4 = r#"
class Main {
    static function main() {
        var s = "Hello, World!";
        var upper = s.toUpperCase();
        var lower = s.toLowerCase();
        var len = s.length;
        var sub = s.substring(0, 5);
    }
}
"#;

    match run_test("test_string", test4) {
        Ok(_) => {
            println!("✅ TEST 4 PASSED");
            passed += 1;
        }
        Err(e) => {
            println!("❌ TEST 4 FAILED: {}", e);
            failed += 1;
        }
    }

    // Test 5: Lambda/Function types (should use typedefs from StdTypes.hx)
    println!("\n{}", "=".repeat(70));
    println!("TEST 5: Lambda with Iterator typedef (StdTypes.hx typedefs)");
    println!("{}", "=".repeat(70));

    let test5 = r#"
class Main {
    static function main() {
        var numbers = [1, 2, 3, 4, 5];
        var doubled = numbers.map(function(x) return x * 2);

        // Filter uses lambda with iterator
        var evens = numbers.filter(function(x) return x % 2 == 0);
    }
}
"#;

    match run_test("test_lambda_iterator", test5) {
        Ok(_) => {
            println!("✅ TEST 5 PASSED");
            passed += 1;
        }
        Err(e) => {
            println!("❌ TEST 5 FAILED: {}", e);
            failed += 1;
        }
    }

    // Summary
    println!("\n{}", "=".repeat(70));
    println!("SUMMARY");
    println!("{}", "=".repeat(70));
    println!("Total: {}", passed + failed);
    println!("Passed: {}", passed);
    println!("Failed: {}", failed);

    if failed == 0 {
        println!("\n🎉 All on-demand stdlib tests passed!");
        std::process::exit(0);
    } else {
        println!("\n⚠️  {} test(s) failed", failed);
        std::process::exit(1);
    }
}

fn run_test(name: &str, source: &str) -> Result<(), String> {
    // Create compilation unit with stdlib
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib (only root files - 24 files)
    println!("Loading stdlib...");
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    // Add the test file
    let filename = format!("{}.hx", name);
    unit.add_file(source, &filename)
        .map_err(|e| format!("Failed to add file: {}", e))?;

    // Compile to TAST (this is where on-demand loading should happen)
    println!("Compiling to TAST (on-demand loading will occur here)...");
    let typed_files = unit.lower_to_tast()
        .map_err(|errors| {
            let error_msgs: Vec<String> = errors.iter()
                .map(|e| e.message.clone())
                .collect();
            format!("TAST lowering failed:\n  {}", error_msgs.join("\n  "))
        })?;

    println!("  ✓ TAST lowering succeeded ({} files)", typed_files.len());

    // Get MIR to verify full pipeline works
    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    println!("  ✓ MIR lowering succeeded ({} modules)", mir_modules.len());

    Ok(())
}
