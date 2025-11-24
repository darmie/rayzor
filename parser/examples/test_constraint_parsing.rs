use parser::{parse_haxe_file, haxe_ast::{TypeDeclaration, Type}};

fn main() {
    // Test how constraints are parsed with & operator
    let code = r#"
        class Test<T:String & Int> {
            public function new() {}
        }
    "#;
    
    println!("Testing constraint parsing with '&' operator");
    match parse_haxe_file("test.hx", code, false) {
        Ok(ast) => {
            println!("✓ Parse successful!");
            if let Some(TypeDeclaration::Class(class)) = ast.declarations.first() {
                println!("  Class: {}", class.name);
                println!("  Type params: {}", class.type_params.len());
                
                for (i, param) in class.type_params.iter().enumerate() {
                    println!("\n  Parameter {}: {}", i, param.name);
                    println!("    Constraints: {} total", param.constraints.len());
                    
                    for (j, constraint) in param.constraints.iter().enumerate() {
                        println!("    Constraint {}: {:?}", j, describe_type(constraint));
                    }
                }
            }
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }
}

fn describe_type(t: &Type) -> String {
    match t {
        Type::Path { path, params, .. } => {
            if params.is_empty() {
                format!("Path({})", path.name)
            } else {
                format!("Path({}<...>)", path.name)
            }
        },
        Type::Intersection { left, right, .. } => {
            format!("Intersection({} & {})", describe_type(left), describe_type(right))
        },
        _ => format!("{:?}", t)
    }
}