#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unreachable_patterns,
    unused_mut,
    unused_assignments,
    unused_parens
)]
#![allow(
    clippy::single_component_path_imports,
    clippy::for_kv_map,
    clippy::explicit_auto_deref
)]
#![allow(
    clippy::println_empty_string,
    clippy::len_zero,
    clippy::useless_vec,
    clippy::field_reassign_with_default
)]
#![allow(
    clippy::needless_borrow,
    clippy::redundant_closure,
    clippy::bool_assert_comparison
)]
#![allow(
    clippy::empty_line_after_doc_comments,
    clippy::useless_format,
    clippy::clone_on_copy
)]
#![allow(clippy::single_match)]
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
