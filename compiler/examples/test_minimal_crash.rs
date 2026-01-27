/// Minimal reproduction test for heap corruption
/// Run specific subtests to isolate which one causes the crash.
use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;
use std::sync::Arc;

fn compile_and_run(name: &str, source: &str) -> Result<(), String> {
    println!("\n--- Running: {} ---", name);

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().map_err(|e| format!("stdlib: {}", e))?;

    let filename = format!("{}.hx", name);
    unit.add_file(source, &filename).map_err(|e| format!("add_file: {}", e))?;

    let _typed_files = unit.lower_to_tast().map_err(|e| format!("tast: {:?}", e))?;

    let mir_modules = unit.get_mir_modules();
    if mir_modules.is_empty() {
        return Err("No MIR modules".to_string());
    }

    let plugin = rayzor_runtime::plugin_impl::get_plugin();
    let symbols = plugin.runtime_symbols();
    let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

    let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;
    for module in &mir_modules {
        backend.compile_module(module)?;
    }

    for module in mir_modules.iter().rev() {
        if let Ok(()) = backend.call_main(module) {
            println!("  OK: {} executed", name);
            return Ok(());
        }
    }

    Err("No main found".to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let test_name = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    let tests: Vec<(&str, &str)> = vec![
        // Test A: Thread with no-capture closure
        ("thread_nocapture", r#"
package test;
import rayzor.concurrent.Thread;
class Main {
    static function main() {
        var handle = Thread.spawn(() -> { return 42; });
        var result = handle.join();
    }
}
"#),
        // Test B: Thread with captured primitive
        ("thread_capture_int", r#"
package test;
import rayzor.concurrent.Thread;
class Main {
    static function main() {
        var x = 100;
        var handle = Thread.spawn(() -> { return x; });
        var result = handle.join();
    }
}
"#),
        // Test C: Arc only (no thread)
        ("arc_only", r#"
package test;
import rayzor.concurrent.Arc;
class Main {
    static function main() {
        var a = new Arc(42);
        var b = a.clone();
        var v = a.get();
    }
}
"#),
        // Test D: Channel only (no thread, no Arc)
        ("channel_only", r#"
package test;
import rayzor.concurrent.Channel;
class Main {
    static function main() {
        var ch = new Channel(10);
        ch.send(42);
        var v = ch.tryReceive();
    }
}
"#),
        // Test E: Arc + Thread, no channel
        ("arc_thread", r#"
package test;
import rayzor.concurrent.Thread;
import rayzor.concurrent.Arc;
class Main {
    static function main() {
        var shared = new Arc(42);
        var clone = shared.clone();
        var handle = Thread.spawn(() -> {
            var val = clone.get();
            return 1;
        });
        handle.join();
    }
}
"#),
        // Test F: Full Arc+Channel+Thread (original failing test)
        ("full_channel", r#"
package test;
import rayzor.concurrent.Thread;
import rayzor.concurrent.Channel;
import rayzor.concurrent.Arc;
class Main {
    static function main() {
        var channel = new Arc(new Channel(10));
        var threadChannel = channel.clone();
        var sender = Thread.spawn(() -> {
            threadChannel.get().send(42);
            return 1;
        });
        sender.join();
        var v1 = channel.get().tryReceive();
        return;
    }
}
"#),
        // Test G: Thread with class instance (user-defined, AutoDrop)
        ("thread_class", r#"
package test;
import rayzor.concurrent.Thread;
@:derive([Send])
class Data {
    public var x: Int;
    public function new(v: Int) { this.x = v; }
}
class Main {
    static function main() {
        var d = new Data(42);
        var handle = Thread.spawn(() -> {
            return d.x;
        });
        var result = handle.join();
    }
}
"#),
    ];

    for (name, source) in &tests {
        if test_name != "all" && *name != test_name {
            continue;
        }
        match compile_and_run(name, source) {
            Ok(()) => println!("  PASS: {}", name),
            Err(e) => println!("  FAIL: {} - {}", name, e),
        }
    }
    println!("\nDone.");
}
