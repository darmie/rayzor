//! Debug MIR output for Complex class constructor and methods

use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    let source = r#"
package benchmarks;

class Complex {
    public var re:Float;
    public var im:Float;

    public function new(re:Float, im:Float) {
        this.re = re;
        this.im = im;
    }

    public function add(c:Complex):Complex {
        return new Complex(re + c.re, im + c.im);
    }

    public function mul(c:Complex):Complex {
        return new Complex(re * c.re - im * c.im, re * c.im + im * c.re);
    }
}

class Main {
    public static function main() {
        var c1 = new Complex(1.0, 2.0);
        var c2 = new Complex(3.0, 4.0);
        var c3 = c1.add(c2);
        trace(c3.re);
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(source, "test.hx").expect("parse");
    unit.lower_to_tast().expect("tast");
    let mir_modules = unit.get_mir_modules();

    println!("\n=== MIR Dump for Complex Functions ===\n");

    for module in &mir_modules {
        for (func_id, func) in &module.functions {
            // Only show Complex-related functions and main
            if func.name == "new" || func.name == "add" || func.name == "mul"
               || func.name == "main" || func.name.ends_with("_main") {
                println!("\n=== {} ({:?}) ===", func.name, func_id);
                println!("  qualified: {:?}", func.qualified_name);
                println!("  Params: {:?}", func.signature.parameters);
                println!("  Return: {:?}", func.signature.return_type);
                println!("  uses_sret: {}", func.signature.uses_sret);

                // Print blocks
                for (block_id, block) in &func.cfg.blocks {
                    println!("\n  Block {:?}:", block_id);
                    for (i, instr) in block.instructions.iter().enumerate() {
                        println!("    [{}] {:?}", i, instr);
                    }
                    println!("    Terminator: {:?}", block.terminator);
                }
            }
        }
    }

    // Also show extern functions
    println!("\n=== Extern Functions (ALL) ===");
    for module in &mir_modules {
        println!("  Module '{}' extern_functions ({} total):", module.name, module.extern_functions.len());
        for (id, func) in &module.extern_functions {
            println!("    {:?}: {} (calling_conv: {:?})", id, func.name, func.signature.calling_convention);
        }
    }
}
