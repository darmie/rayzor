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
        return new Point(x + p.x, y + p.y);
    }
}

class TestPoint {
    public static function main() {
        var p1 = new Point(1.0, 2.0);
        var p2 = new Point(3.0, 4.0);
        var p3 = p1.add(p2);
        trace(p3.x);
    }
}
    "#;

    println!("Compiling...");
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(&source, "test.hx").expect("parse");
    unit.lower_to_tast().expect("tast");
    let mir_modules = unit.get_mir_modules();

    // Find and print the Point.add function MIR
    for module in &mir_modules {
        for (_id, func) in &module.functions {
            if func.name.contains("Point") && func.name.contains("add") && !func.name.contains("main") {
                println!("\n=== MIR for {} ===", func.name);
                println!("Parameters: {:?}", func.signature.parameters);
                println!("Return type: {:?}", func.signature.return_type);
                println!("Uses sret: {}", func.signature.uses_sret);
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
