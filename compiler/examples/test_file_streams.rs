//! Test FileInput/FileOutput stream-based file I/O

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::sync::Arc;

fn main() {
    println!("=== File Stream I/O Test ===\n");

    // Test 1: File.write and File.read with writeByte/readByte
    println!("Test 1: Basic write and read with byte operations");
    let source1 = r#"
import sys.io.File;
import sys.io.FileOutput;
import sys.io.FileInput;
import sys.FileSystem;

class Main {
    static function main() {
        var testPath = "/tmp/rayzor_stream_test.txt";

        // Write bytes using FileOutput
        var output:FileOutput = File.write(testPath, true);
        output.writeByte(72);  // 'H'
        output.writeByte(105); // 'i'
        output.writeByte(33);  // '!'
        output.close();

        // Read bytes using FileInput
        var input:FileInput = File.read(testPath, true);
        var b1 = input.readByte();
        var b2 = input.readByte();
        var b3 = input.readByte();
        input.close();

        trace(b1);  // 72
        trace(b2);  // 105
        trace(b3);  // 33

        // Clean up
        FileSystem.deleteFile(testPath);
    }
}
"#;
    run_test(source1, "byte_operations");

    // Test 2: FileInput.tell
    println!("\nTest 2: FileInput tell");
    let source2 = r#"
import sys.io.File;
import sys.io.FileOutput;
import sys.io.FileInput;
import sys.FileSystem;

class Main {
    static function main() {
        var testPath = "/tmp/rayzor_tell_test.txt";

        // Create a file with known content
        File.saveContent(testPath, "ABCDEFGHIJ");

        // Open for reading
        var input:FileInput = File.read(testPath, true);

        // Check initial position
        trace(input.tell());  // 0

        // Read first byte
        var first = input.readByte();
        trace(first);  // 65 ('A')
        trace(input.tell());  // 1

        // Read more bytes
        input.readByte();  // B
        input.readByte();  // C
        trace(input.tell());  // 3

        input.close();
        FileSystem.deleteFile(testPath);
    }
}
"#;
    run_test(source2, "tell_position");

    // Test 3: FileInput.eof
    println!("\nTest 3: FileInput EOF detection");
    let source3 = r#"
import sys.io.File;
import sys.io.FileOutput;
import sys.io.FileInput;
import sys.FileSystem;

class Main {
    static function main() {
        var testPath = "/tmp/rayzor_eof_test.txt";

        // Create a small file
        File.saveContent(testPath, "AB");

        var input:FileInput = File.read(testPath, true);

        // Not at EOF yet
        trace(input.eof());  // false

        // Read all bytes
        input.readByte();  // 'A'
        input.readByte();  // 'B'

        // Try to read past end
        var pastEnd = input.readByte();  // -1 (EOF)
        trace(pastEnd);
        trace(input.eof());  // true

        input.close();
        FileSystem.deleteFile(testPath);
    }
}
"#;
    run_test(source3, "eof_detection");

    // Test 4: FileOutput.tell
    println!("\nTest 4: FileOutput tell");
    let source4 = r#"
import sys.io.File;
import sys.io.FileOutput;
import sys.io.FileInput;
import sys.FileSystem;

class Main {
    static function main() {
        var testPath = "/tmp/rayzor_output_tell_test.txt";

        var output:FileOutput = File.write(testPath, true);
        trace(output.tell());  // 0

        output.writeByte(65);  // 'A'
        trace(output.tell());  // 1

        output.writeByte(66);  // 'B'
        output.writeByte(67);  // 'C'
        trace(output.tell());  // 3

        output.close();

        // Verify content
        var content = File.getContent(testPath);
        trace(content);  // "ABC"

        FileSystem.deleteFile(testPath);
    }
}
"#;
    run_test(source4, "output_tell");

    // Test 5: File.append
    println!("\nTest 5: File.append");
    let source5 = r#"
import sys.io.File;
import sys.io.FileOutput;
import sys.io.FileInput;
import sys.FileSystem;

class Main {
    static function main() {
        var testPath = "/tmp/rayzor_append_test.txt";

        // Create initial file
        File.saveContent(testPath, "Hello");

        // Append to it
        var output:FileOutput = File.append(testPath, true);
        output.writeByte(32);  // ' '
        output.writeByte(87);  // 'W'
        output.writeByte(111); // 'o'
        output.writeByte(114); // 'r'
        output.writeByte(108); // 'l'
        output.writeByte(100); // 'd'
        output.close();

        // Verify content
        var content = File.getContent(testPath);
        trace(content);  // "Hello World"

        FileSystem.deleteFile(testPath);
    }
}
"#;
    run_test(source5, "append_mode");

    // Test 6: FileOutput.flush
    println!("\nTest 6: FileOutput flush");
    let source6 = r#"
import sys.io.File;
import sys.io.FileOutput;
import sys.io.FileInput;
import sys.FileSystem;

class Main {
    static function main() {
        var testPath = "/tmp/rayzor_flush_test.txt";

        var output:FileOutput = File.write(testPath, true);
        output.writeByte(84);  // 'T'
        output.writeByte(101); // 'e'
        output.writeByte(115); // 's'
        output.writeByte(116); // 't'
        output.flush();  // Ensure data is written to disk

        // We can verify the file exists and has content
        trace(FileSystem.exists(testPath));  // true

        output.close();
        FileSystem.deleteFile(testPath);
    }
}
"#;
    run_test(source6, "flush_operation");
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
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
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
