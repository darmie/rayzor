#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unreachable_patterns,
    unused_mut,
    unused_assignments,
    unused_parens
)]
#![allow(
    clippy::single_component_path_imports,
    clippy::for_kv_map,
    clippy::explicit_auto_deref
)]
#![allow(
    clippy::println_empty_string,
    clippy::len_zero,
    clippy::useless_vec,
    clippy::field_reassign_with_default
)]
#![allow(
    clippy::needless_borrow,
    clippy::redundant_closure,
    clippy::bool_assert_comparison
)]
#![allow(
    clippy::empty_line_after_doc_comments,
    clippy::useless_format,
    clippy::clone_on_copy
)]
//! Test File I/O (sys.io.File and sys.FileSystem)

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use std::sync::Arc;

fn main() {
    println!("=== File I/O Test ===\n");

    // Test 1: FileSystem.exists
    println!("Test 1: FileSystem.exists");
    let source1 = r#"
import sys.FileSystem;

class Main {
    static function main() {
        // Check if current directory exists (should always be true)
        var exists = FileSystem.exists(".");
        trace(exists);  // true

        // Check non-existent file
        var notExists = FileSystem.exists("/nonexistent_path_12345");
        trace(notExists);  // false
    }
}
"#;
    run_test(source1, "filesystem_exists");

    // Test 2: FileSystem.isDirectory
    println!("\nTest 2: FileSystem.isDirectory");
    let source2 = r#"
import sys.FileSystem;

class Main {
    static function main() {
        // Current directory should be a directory
        var isDir = FileSystem.isDirectory(".");
        trace(isDir);  // true
    }
}
"#;
    run_test(source2, "filesystem_is_directory");

    // Test 3: File.getContent / File.saveContent
    println!("\nTest 3: File.getContent / File.saveContent");
    let source3 = r#"
import sys.io.File;
import sys.FileSystem;

class Main {
    static function main() {
        var testPath = "/tmp/rayzor_test_file.txt";

        // Write content to file
        File.saveContent(testPath, "Hello from Rayzor!");

        // Read it back
        var content = File.getContent(testPath);
        trace(content);  // "Hello from Rayzor!"

        // Clean up
        FileSystem.deleteFile(testPath);
        trace(FileSystem.exists(testPath));  // false (deleted)
    }
}
"#;
    run_test(source3, "file_read_write");

    // Test 4: FileSystem.createDirectory / deleteDirectory
    println!("\nTest 4: FileSystem.createDirectory / deleteDirectory");
    let source4 = r#"
import sys.FileSystem;

class Main {
    static function main() {
        var testDir = "/tmp/rayzor_test_dir";

        // Create directory
        FileSystem.createDirectory(testDir);
        trace(FileSystem.exists(testDir));  // true
        trace(FileSystem.isDirectory(testDir));  // true

        // Delete directory
        FileSystem.deleteDirectory(testDir);
        trace(FileSystem.exists(testDir));  // false
    }
}
"#;
    run_test(source4, "filesystem_directory");

    // Test 5: FileSystem.fullPath / absolutePath
    println!("\nTest 5: FileSystem.fullPath / absolutePath");
    let source5 = r#"
import sys.FileSystem;

class Main {
    static function main() {
        // Get full path of current directory
        var fullPath = FileSystem.fullPath(".");
        trace(fullPath);  // Should be absolute path

        var absPath = FileSystem.absolutePath("./test");
        trace(absPath);  // Should be absolute path ending in /test
    }
}
"#;
    run_test(source5, "filesystem_paths");

    // Test 6: File.copy
    println!("\nTest 6: File.copy");
    let source6 = r#"
import sys.io.File;
import sys.FileSystem;

class Main {
    static function main() {
        var srcPath = "/tmp/rayzor_copy_src.txt";
        var dstPath = "/tmp/rayzor_copy_dst.txt";

        // Create source file
        File.saveContent(srcPath, "Copy test content");

        // Copy it
        File.copy(srcPath, dstPath);

        // Verify copy
        trace(FileSystem.exists(dstPath));  // true
        var content = File.getContent(dstPath);
        trace(content);  // "Copy test content"

        // Clean up
        FileSystem.deleteFile(srcPath);
        FileSystem.deleteFile(dstPath);
    }
}
"#;
    run_test(source6, "file_copy");

    // Test 7: FileSystem.rename
    println!("\nTest 7: FileSystem.rename");
    let source7 = r#"
import sys.io.File;
import sys.FileSystem;

class Main {
    static function main() {
        var oldPath = "/tmp/rayzor_rename_old.txt";
        var newPath = "/tmp/rayzor_rename_new.txt";

        // Create file
        File.saveContent(oldPath, "Rename test");
        trace(FileSystem.exists(oldPath));  // true

        // Rename it
        FileSystem.rename(oldPath, newPath);

        // Verify rename
        trace(FileSystem.exists(oldPath));  // false
        trace(FileSystem.exists(newPath));  // true

        // Clean up
        FileSystem.deleteFile(newPath);
    }
}
"#;
    run_test(source7, "filesystem_rename");

    // Test 8: FileSystem.stat with field access
    println!("\nTest 8: FileSystem.stat (with field access)");
    let source8 = r#"
import sys.FileSystem;
import sys.FileStat;
import sys.io.File;

class Main {
    static function main() {
        var testPath = "/tmp/rayzor_stat_test.txt";

        // Create a test file with known content
        File.saveContent(testPath, "Hello World!");

        // Get file stats and access fields
        var stat = FileSystem.stat(testPath);
        trace(stat.size);  // Should be 12 (length of "Hello World!")

        // Clean up
        FileSystem.deleteFile(testPath);
    }
}
"#;
    run_test(source8, "filesystem_stat");

    // Test 9: FileSystem.readDirectory - Basic call test
    // NOTE: Accessing .length on returned Array<String> needs proper type propagation
    println!("\nTest 9: FileSystem.readDirectory (basic)");
    let source9 = r#"
import sys.FileSystem;
import sys.io.File;

class Main {
    static function main() {
        var testDir = "/tmp/rayzor_readdir_test";

        // Create test directory and files
        FileSystem.createDirectory(testDir);
        File.saveContent(testDir + "/file1.txt", "content1");
        File.saveContent(testDir + "/file2.txt", "content2");

        // Read directory - just verify it returns something
        var entries = FileSystem.readDirectory(testDir);
        trace(entries != null);  // true

        // Clean up
        FileSystem.deleteFile(testDir + "/file1.txt");
        FileSystem.deleteFile(testDir + "/file2.txt");
        FileSystem.deleteDirectory(testDir);
    }
}
"#;
    run_test(source9, "filesystem_read_directory");
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

    let _typed_files = unit
        .lower_to_tast()
        .map_err(|errors| format!("TAST lowering failed: {:?}", errors))?;

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
