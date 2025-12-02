// Dump TAST for phi node bug investigation

use compiler::compilation::{CompilationUnit, CompilationConfig};

fn main() -> Result<(), String> {
    let code = r#"
class Main {
    static function main() {
        var acquired = true;
        if (acquired) {
            var acquired2 = false;
            trace(acquired2);
        } else {
            trace("failed");
        }
        trace("done");
    }
}
"#;

    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    unit.load_stdlib()
        .map_err(|e| format!("Failed to load stdlib: {}", e))?;

    unit.add_file(code, "test_phi_bug.hx")
        .map_err(|e| format!("Failed to add file: {}", e))?;

    let typed_files = unit.lower_to_tast()
        .map_err(|errors| format!("TAST errors: {:?}", errors))?;

    if typed_files.is_empty() {
        return Err("No typed files generated".to_string());
    }

    println!("=== TYPED AST DUMP ===\n");
    println!("{:#?}", typed_files[0]);

    Ok(())
}
