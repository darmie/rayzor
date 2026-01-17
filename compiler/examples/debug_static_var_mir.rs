//! Debug MIR output for static var + loop bound bug
//!
//! Compare MIR between:
//! - WORKING: `var n = 5; for (i in 0...n)`
//! - FAILING: `static var SIZE = 5; var n = SIZE; for (i in 0...n)`

use compiler::compilation::{CompilationConfig, CompilationUnit};

fn dump_mir(name: &str, source: &str) {
    println!("\n{}", "=".repeat(60));
    println!("=== {} ===", name);
    println!("{}\n", "=".repeat(60));

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(source, &format!("{}.hx", name)).expect("parse");
    unit.lower_to_tast().expect("tast");

    let mir_modules = unit.get_mir_modules();

    for module in &mir_modules {
        if module.name == "test" {
            println!("Module: {}", module.name);
            println!("Functions: {}", module.functions.len());
            println!("Extern Functions: {}", module.extern_functions.len());

            // Show extern functions
            if !module.extern_functions.is_empty() {
                println!("\n--- Extern Functions ---");
                for (id, func) in &module.extern_functions {
                    println!("  {:?}: {}", id, func.name);
                }
            }

            // Show main function in detail
            for (func_id, func) in &module.functions {
                if func.name == "main" || func.name.ends_with("_main") {
                    println!("\n--- Function: {} ({:?}) ---", func.name, func_id);
                    println!("Params: {:?}", func.signature.parameters);
                    println!("Return: {:?}", func.signature.return_type);

                    for (block_id, block) in &func.cfg.blocks {
                        println!("\n  Block {:?}:", block_id);
                        for (i, instr) in block.instructions.iter().enumerate() {
                            println!("    [{:3}] {:?}", i, instr);
                        }
                        println!("    Terminator: {:?}", block.terminator);
                    }
                }
            }

            // Show globals if any
            if !module.globals.is_empty() {
                println!("\n--- Globals ---");
                for (id, global) in &module.globals {
                    println!("  {:?}: {} : {:?}", id, global.name, global.ty);
                }
            }
        }
    }
}

fn main() {
    println!("Static Var + Loop Bound MIR Comparison");
    println!("======================================\n");

    // WORKING CASE: Literal to local, used in loop
    dump_mir("working_literal", r#"
package test;
class Main {
    public static function main() {
        var n = 5;
        var sum = 0;
        for (i in 0...n) {
            sum = sum + i;
        }
        trace(sum);
    }
}
"#);

    // FAILING CASE: Static var to local, used in loop
    dump_mir("failing_static", r#"
package test;
class Main {
    static var SIZE = 5;
    public static function main() {
        var n = SIZE;
        var sum = 0;
        for (i in 0...n) {
            sum = sum + i;
        }
        trace(sum);
    }
}
"#);

    // Also check: static var read without loop (this works)
    dump_mir("working_static_no_loop", r#"
package test;
class Main {
    static var SIZE = 5;
    public static function main() {
        var n = SIZE;
        trace(n);
    }
}
"#);

    println!("\n\n======================================");
    println!("Compare the MIR output above to find the difference");
    println!("that causes SIGILL when static var value is used in loop bound.");
}
