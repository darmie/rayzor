use compiler::compilation::{CompilationConfig, CompilationUnit};

#[test]
fn test_haxe_macro_type_parses() {
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    match unit.load_stdlib() {
        Ok(_) => {
            // Check if haxe/macro/Type.hx was loaded successfully
            let found = unit
                .stdlib_files
                .iter()
                .any(|f| f.filename.contains("haxe/macro/Type.hx"));
            assert!(found, "haxe/macro/Type.hx should be loaded from stdlib");
        }
        Err(e) => panic!("Failed to load stdlib: {}", e),
    }
}

#[test]
fn test_haxe_macro_tools_parses() {
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    match unit.load_stdlib() {
        Ok(_) => {
            let found = unit
                .stdlib_files
                .iter()
                .any(|f| f.filename.contains("haxe/macro/Tools.hx"));
            assert!(found, "haxe/macro/Tools.hx should be loaded from stdlib");
        }
        Err(e) => panic!("Failed to load stdlib: {}", e),
    }
}

#[test]
fn test_haxe_extern_eithertype_parses() {
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    match unit.load_stdlib() {
        Ok(_) => {
            let found = unit
                .stdlib_files
                .iter()
                .any(|f| f.filename.contains("haxe/extern/EitherType.hx"));
            assert!(
                found,
                "haxe/extern/EitherType.hx should be loaded from stdlib"
            );
        }
        Err(e) => panic!("Failed to load stdlib: {}", e),
    }
}

#[test]
fn test_all_haxe_macro_files_parse() {
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    match unit.load_stdlib() {
        Ok(_) => {
            let haxe_macro_files: Vec<_> = unit
                .stdlib_files
                .iter()
                .filter(|f| f.filename.contains("haxe/macro/"))
                .collect();

            println!("Found {} haxe.macro files", haxe_macro_files.len());
            for file in &haxe_macro_files {
                println!("  - {}", file.filename);
            }

            assert!(
                haxe_macro_files.len() >= 3,
                "Should have at least 3 haxe.macro files (Type, Tools, MacroType)"
            );
        }
        Err(e) => panic!("Failed to load stdlib: {}", e),
    }
}

#[test]
fn test_all_haxe_extern_files_parse() {
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    match unit.load_stdlib() {
        Ok(_) => {
            let haxe_extern_files: Vec<_> = unit
                .stdlib_files
                .iter()
                .filter(|f| f.filename.contains("haxe/extern/"))
                .collect();

            println!("Found {} haxe.extern files", haxe_extern_files.len());
            for file in &haxe_extern_files {
                println!("  - {}", file.filename);
            }

            assert!(
                haxe_extern_files.len() >= 3,
                "Should have at least 3 haxe.extern files (EitherType, AsVar, Rest)"
            );
        }
        Err(e) => panic!("Failed to load stdlib: {}", e),
    }
}
