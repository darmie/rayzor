/// Test what the raw parser actually produces

use parser::parse_haxe_file_with_diagnostics;

fn main() {
    let source = r#"
        @:safety(strict=true)
        class Main {
            static function main() {}
        }
    "#;

    match parse_haxe_file_with_diagnostics("test.hx", source) {
        Ok(result) => {
            println!("Parse result:");
            println!("  Declarations: {}", result.file.declarations.len());

            for decl in &result.file.declarations {
                match decl {
                    parser::TypeDeclaration::Class(class) => {
                        println!("\nClass: {}", class.name);
                        println!("  Metadata count: {}", class.meta.len());

                        for meta in &class.meta {
                            println!("\n  Metadata '{}':", meta.name);
                            println!("    Params count: {}", meta.params.len());

                            for (i, param) in meta.params.iter().enumerate() {
                                println!("    Param {}: {:#?}", i, param);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            println!("Parse error: {}", e);
        }
    }
}
