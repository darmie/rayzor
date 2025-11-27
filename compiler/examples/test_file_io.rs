//! Test File I/O functions

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::sync::Arc;

fn main() {
    println!("=== File I/O Test ===\n");

    // Test 1: FileSystem.exists
    println!("Test 1: FileSystem.exists()");
    let source1 = r#"
class Main {
    static function main() {
        // Check if current directory exists (should always be true)
        var exists = FileSystem.exists(".");
        trace(exists);  // Should print true

        // Check if nonexistent file exists
        var notExists = FileSystem.exists("/nonexistent_file_12345");
        trace(notExists);  // Should print false
    }
}
"#;
    run_test(source1, "filesystem_exists");

    // Test 2: FileSystem.isDirectory
    println!("\nTest 2: FileSystem.isDirectory()");
    let source2 = r#"
class Main {
    static function main() {
        // Current directory should be a directory
        var isDir = FileSystem.isDirectory(".");
        trace(isDir);  // Should print true
    }
}
"#;
    run_test(source2, "filesystem_is_directory");

    // Test 3: File.saveContent and File.getContent
    println!("\nTest 3: File.saveContent/getContent()");
    let source3 = r#"
class Main {
    static function main() {
        var testFile = "/tmp/rayzor_test_file.txt";
        var content = "Hello from Rayzor!";

        // Save content to file
        File.saveContent(testFile, content);
        trace("saved");

        // Read it back
        var readContent = File.getContent(testFile);
        trace(readContent);

        // Clean up
        FileSystem.deleteFile(testFile);
        trace("deleted");
    }
}
"#;
    run_test(source3, "file_save_get_content");

    // Test 4: FileSystem.createDirectory and deleteDirectory
    println!("\nTest 4: FileSystem.createDirectory/deleteDirectory()");
    let source4 = r#"
class Main {
    static function main() {
        var testDir = "/tmp/rayzor_test_dir";

        // Create directory
        FileSystem.createDirectory(testDir);

        // Verify it exists and is a directory
        var exists = FileSystem.exists(testDir);
        var isDir = FileSystem.isDirectory(testDir);
        trace(exists);  // true
        trace(isDir);   // true

        // Delete it
        FileSystem.deleteDirectory(testDir);

        // Verify it's gone
        var stillExists = FileSystem.exists(testDir);
        trace(stillExists);  // false
    }
}
"#;
    run_test(source4, "filesystem_directory");

    // Test 5: FileSystem.rename (File.copy)
    println!("\nTest 5: File.copy()");
    let source5 = r#"
class Main {
    static function main() {
        var src = "/tmp/rayzor_copy_src.txt";
        var dst = "/tmp/rayzor_copy_dst.txt";

        // Create source file
        File.saveContent(src, "Copy test content");

        // Copy it
        File.copy(src, dst);

        // Verify both exist
        trace(FileSystem.exists(src));  // true
        trace(FileSystem.exists(dst));  // true

        // Read destination content
        var content = File.getContent(dst);
        trace(content);

        // Clean up
        FileSystem.deleteFile(src);
        FileSystem.deleteFile(dst);
    }
}
"#;
    run_test(source5, "file_copy");
}

fn run_test(source: &str, name: &str) {
    match compile_and_run(source, name) {
        Ok(()) => {
            println!("✅ {} PASSED", name);
        }
        Err(e) => {
            println!("❌ {} FAILED: {}", name, e);
        }
    }
}

fn compile_and_run(source: &str, name: &str) -> Result<(), String> {
    let mut unit = CompilationUnit::new(CompilationConfig::default());
    unit.load_stdlib()?;
    unit.add_file(source, &format!("{}.hx", name))?;

    let _typed_files = unit.lower_to_tast().map_err(|errors| {
        format!("TAST lowering failed: {:?}", errors)
    })?;

    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules generated".to_string());
    }

    let mut backend = compile_to_native(&mir_modules)?;
    execute_main(&mut backend, &mir_modules)?;

    Ok(())
}

fn compile_to_native(modules: &[Arc<IrModule>]) -> Result<CraneliftBackend, String> {
    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

    for module in modules {
        backend.compile_module(module)?;
    }

    Ok(backend)
}

fn execute_main(backend: &mut CraneliftBackend, modules: &[Arc<IrModule>]) -> Result<(), String> {
    for module in modules.iter().rev() {
        if backend.call_main(module).is_ok() {
            return Ok(());
        }
    }
    Err("Failed to execute main".to_string())
}
