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
// Debug parser to see what's happening
use parser::haxe_parser::{parse_haxe_file, parse_haxe_file_with_debug};

fn main() {
    let simple_source = r#"
class TestClass {
}
"#;

    println!("Testing simple class parsing...");
    println!("Source: {:?}", simple_source);

    match parse_haxe_file_with_debug("simple.hx", simple_source, true, true) {
        Ok(ast) => {
            println!("✓ Parse successful");
            println!("Declarations: {}", ast.declarations.len());
            for decl in &ast.declarations {
                use parser::haxe_ast::TypeDeclaration;
                let decl_type = match decl {
                    TypeDeclaration::Class(c) => format!("Class({})", c.name),
                    TypeDeclaration::Interface(i) => format!("Interface({})", i.name),
                    TypeDeclaration::Enum(e) => format!("Enum({})", e.name),
                    TypeDeclaration::Abstract(a) => format!("Abstract({})", a.name),
                    TypeDeclaration::Typedef(t) => format!("Typedef({})", t.name),
                    TypeDeclaration::Conditional(_) => "Conditional".to_string(),
                };
                println!("  • {}", decl_type);
            }
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }

    println!("\nTesting full source...");
    let full_source = r#"
package test;

class TestClass {
    var x:Int = 10;
    var name:String = "test";
    
    public function test(a:Int):Int {
        var result = a + x;
        
        // Test loop with break/continue
        while (result < 100) {
            if (result > 50) {
                break;
            }
            result = result * 2;
        }
        
        // Test for-in
        var sum = 0;
        for (i in [1, 2, 3]) {
            sum = sum + i;
        }
        
        // Test switch
        switch(result) {
            case 42:
                sum = sum + 1;
            case 100:
                sum = sum + 2;
            default:
                sum = sum + 3;
        }
        
        return result + sum;
    }
    
    function testPatterns(value:Dynamic):String {
        // Test pattern matching
        return switch(value) {
            case 1 | 2 | 3:
                "small";
            case n if n > 100:
                "large";
            case _:
                "medium";
        };
    }
}

enum Color {
    Red;
    Green;
    Blue;
    RGB(r:Int, g:Int, b:Int);
}

interface IDrawable {
    function draw():Void;
}

abstract AbstractInt(Int) from Int to Int {
    public inline function new(i:Int) {
        this = i;
    }
    
    @:op(A + B)
    public inline function add(rhs:AbstractInt):AbstractInt {
        return new AbstractInt(this + rhs.toInt());
    }
    
    public inline function toInt():Int {
        return this;
    }
}
"#;

    match parse_haxe_file_with_debug("full.hx", full_source, true, true) {
        Ok(ast) => {
            println!("✓ Parse successful");
            println!("Declarations: {}", ast.declarations.len());
            for decl in &ast.declarations {
                use parser::haxe_ast::TypeDeclaration;
                let decl_type = match decl {
                    TypeDeclaration::Class(c) => format!("Class({})", c.name),
                    TypeDeclaration::Interface(i) => format!("Interface({})", i.name),
                    TypeDeclaration::Enum(e) => format!("Enum({})", e.name),
                    TypeDeclaration::Abstract(a) => format!("Abstract({})", a.name),
                    TypeDeclaration::Typedef(t) => format!("Typedef({})", t.name),
                    TypeDeclaration::Conditional(_) => "Conditional".to_string(),
                };
                println!("  • {}", decl_type);
            }
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }
}
