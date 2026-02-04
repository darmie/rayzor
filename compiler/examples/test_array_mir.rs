//! Dump MIR for array push to see what's happening

use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    let source = r#"
package test;

class ArrayTest {
    public static function main() {
        var arr = new Array<Int>();
        arr.push(1);
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(&source, "array_test.hx").expect("parse");
    unit.lower_to_tast().expect("tast");
    let mir_modules = unit.get_mir_modules();

    for module in &mir_modules {
        println!("=== Module ===");
        for (id, func) in &module.functions {
            if func.name.contains("main") || func.name.contains("array") {
                println!("\n--- Function: {} ({:?}) ---", func.name, id);
                println!("  Kind: {:?}", func.kind);
                println!(
                    "  Calling convention: {:?}",
                    func.signature.calling_convention
                );
                println!("  Parameters:");
                for p in &func.signature.parameters {
                    println!("    {:?}: {:?}", p.reg, p.ty);
                }
                println!("  Return type: {:?}", func.signature.return_type);
                println!("  Blocks:");
                for (block_id, block) in &func.cfg.blocks {
                    println!("    Block {:?}:", block_id);
                    for inst in &block.instructions {
                        println!("      {:?}", inst);
                    }
                    println!("      Term: {:?}", block.terminator);
                }
            }
        }
    }
}
