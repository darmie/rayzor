use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    let source = r#"
package test;

class Box {
    public var value:Int;

    public function new() {
        this.value = 42;
    }
}

class TestStaticNew {
    public static function makeBox():Box {
        return new Box();
    }
    
    public static function main() {
        var b = makeBox();
        trace(b.value);
    }
}
    "#;

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(&source, "test.hx").expect("parse");
    unit.lower_to_tast().expect("tast");
    let mir_modules = unit.get_mir_modules();

    for module in &mir_modules {
        for (_id, func) in &module.functions {
            if func.name == "makeBox" {
                println!("\n=== MIR for {} ===", func.name);
                println!("Signature:");
                println!("  Parameters: {:?}", func.signature.parameters);
                println!("  Return type: {:?}", func.signature.return_type);
                println!("  Uses sret: {}", func.signature.uses_sret);
                println!("\nBlocks:");
                for (block_id, block) in &func.cfg.blocks {
                    println!("  Block {:?}:", block_id);
                    for instr in &block.instructions {
                        println!("    {:?}", instr);
                    }
                    println!("    Terminator: {:?}", block.terminator);
                }
            }
        }
    }
}
