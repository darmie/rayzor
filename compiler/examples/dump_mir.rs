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
        trace(p3.x);
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib().expect("stdlib");
    unit.add_file(source, "test.hx").expect("parse");
    unit.lower_to_tast().expect("tast");
    let mir_modules = unit.get_mir_modules();

    // Dump ALL functions to see what IrFunctionId(3) is
    for module in &mir_modules {
        println!("\n=== Module {} ===", module.name);
        for (func_id, func) in &module.functions {
            println!("  {:?}: {} (qualified: {:?})", func_id, func.name, func.qualified_name);
        }
    }

    println!("\n\n=== Detailed dump for main and add ===");
    for module in &mir_modules {
        for (func_id, func) in &module.functions {
            if func.name == "main" || func.name == "add" || func.name == "new" {
                println!("\n=== {} (func_id: {:?}) ===", func.name, func_id);
                println!("  Params: {:?}", func.signature.parameters);
                println!("  Return: {:?}", func.signature.return_type);
                println!("  uses_sret: {}", func.signature.uses_sret);
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
