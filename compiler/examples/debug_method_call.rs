use compiler::compilation::{CompilationConfig, CompilationUnit};

fn main() {
    let source = r#"
package test;

class Point {
    public var x:Float;
    public var y:Float;

    public function new(x:Float, y:Float) {
        this.x = x;
        this.y = y;
    }

    public function add(p:Point):Point {
        var rx = x + p.x;
        var ry = y + p.y;
        return new Point(rx, ry);
    }
}

class TestInstanceMethod {
    public static function main() {
        var p1 = new Point(1.0, 2.0);
        var p2 = new Point(3.0, 4.0);
        var p3 = p1.add(p2);
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(source, "test.hx").expect("parse");
    unit.lower_to_tast().expect("tast");
    let mir_modules = unit.get_mir_modules();

    // Find the func_id for "add" and print it
    println!("\n=== Functions with 'add' in name ===");
    for module in &mir_modules {
        for (func_id, func) in &module.functions {
            if func.name.contains("add") || func.name.contains("Add") {
                println!("  {:?}: {} (qualified: {:?})", func_id, func.name, func.qualified_name);
            }
        }
    }

    // Find main and print all instructions
    println!("\n=== Main function (qualified name containing 'TestInstance') ===");
    for module in &mir_modules {
        for (func_id, func) in &module.functions {
            if func.name == "main" {
                println!("  Found main {:?} (qualified: {:?})", func_id, func.qualified_name);

                // Print all instructions
                for (block_id, block) in &func.cfg.blocks {
                    println!("    Block {:?}:", block_id);
                    for instr in &block.instructions {
                        // Check if it's a CallDirect and show the target
                        match instr {
                            compiler::ir::IrInstruction::CallDirect { func_id, args, .. } => {
                                let called = module.functions.get(func_id)
                                    .map(|f| format!("{} (qualified: {:?})", f.name, f.qualified_name))
                                    .or_else(|| module.extern_functions.get(func_id).map(|f| format!("EXTERN:{}", f.name)))
                                    .unwrap_or_else(|| format!("UNKNOWN {:?}", func_id));
                                println!("      CallDirect {:?} -> {} ({} args)", func_id, called, args.len());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
