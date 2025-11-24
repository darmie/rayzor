/// Test what AST the pipeline sees during lowering

use parser::parse_haxe_file_with_diagnostics;

fn main() {
    let source = r#"
        @:safety(strict=true)
        class Main {
            static function main() {}
        }
    "#;

    // Use the same parsing function as the pipeline
    match parse_haxe_file_with_diagnostics("test.hx", source) {
        Ok(result) => {
            let haxe_file = result.file;
            println!("Parsed file:");
            println!("  Declarations: {}", haxe_file.declarations.len());

            for decl in &haxe_file.declarations {
                match decl {
                    parser::TypeDeclaration::Class(class) => {
                        println!("\nClass: {}", class.name);
                        println!("  Metadata count: {}", class.meta.len());

                        for meta in &class.meta {
                            println!("\n  Metadata '{}':", meta.name);
                            println!("    Params count: {}", meta.params.len());

                            for (i, param) in meta.params.iter().enumerate() {
                                println!("    Param {}: {:?}", i, param.kind);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Err(error_string) => {
            println!("Parse error: {}", error_string);
        }
    }
}
