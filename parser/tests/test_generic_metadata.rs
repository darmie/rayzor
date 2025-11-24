use parser::parse_haxe_file;

#[test]
fn test_generic_metadata_simple() {
    let input = r#"
@:generic
class GenericClass<T> {
}
"#;
    
    println!("Parsing: {}", input);
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: {:?}", ast);
            
            // Check if the class has @:generic metadata
            if let Some(decl) = ast.declarations.first() {
                if let parser::haxe_ast::TypeDeclaration::Class(class) = decl {
                    let has_generic = class.meta.iter().any(|meta| meta.name == "generic");
                    println!("Has @:generic metadata: {}", has_generic);
                    assert!(has_generic, "Should have @:generic metadata");
                }
            }
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should have parsed");
        }
    }
}

#[test]
fn test_generic_metadata_with_params() {
    let input = r#"
@:generic
class GenericClass<T, U> {
    function process(item: T): U {
        return cast item;
    }
}
"#;
    
    println!("Parsing: {}", input);
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: {:?}", ast);
            
            // Check if the class has @:generic metadata
            if let Some(decl) = ast.declarations.first() {
                if let parser::haxe_ast::TypeDeclaration::Class(class) = decl {
                    let has_generic = class.meta.iter().any(|meta| meta.name == "generic");
                    println!("Has @:generic metadata: {}", has_generic);
                    assert!(has_generic, "Should have @:generic metadata");
                    
                    // Check type parameters
                    assert_eq!(class.type_params.len(), 2);
                    assert_eq!(class.type_params[0].name, "T");
                    assert_eq!(class.type_params[1].name, "U");
                }
            }
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should have parsed");
        }
    }
}

#[test]
fn test_generic_metadata_on_interface() {
    let input = r#"
@:generic
interface GenericInterface<T> {
    function process(item: T): T;
}
"#;
    
    println!("Parsing: {}", input);
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: {:?}", ast);
            
            // Check if the interface has @:generic metadata
            if let Some(decl) = ast.declarations.first() {
                if let parser::haxe_ast::TypeDeclaration::Interface(interface) = decl {
                    let has_generic = interface.meta.iter().any(|meta| meta.name == "generic");
                    println!("Has @:generic metadata: {}", has_generic);
                    assert!(has_generic, "Should have @:generic metadata");
                }
            }
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should have parsed");
        }
    }
}

#[test]
fn test_generic_metadata_on_abstract() {
    let input = r#"
@:generic
abstract GenericAbstract<T>(T) {
    public function new(value: T) {
        this = value;
    }
}
"#;
    
    println!("Parsing: {}", input);
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: {:?}", ast);
            
            // Check if the abstract has @:generic metadata
            if let Some(decl) = ast.declarations.first() {
                if let parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl) = decl {
                    let has_generic = abstract_decl.meta.iter().any(|meta| meta.name == "generic");
                    println!("Has @:generic metadata: {}", has_generic);
                    assert!(has_generic, "Should have @:generic metadata");
                }
            }
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should have parsed");
        }
    }
}

#[test]
fn test_generic_metadata_combined_with_other_metadata() {
    let input = r#"
@:generic
@:native("NativeGeneric")
class GenericClass<T> {
}
"#;
    
    println!("Parsing: {}", input);
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: {:?}", ast);
            
            // Check if the class has both metadata
            if let Some(decl) = ast.declarations.first() {
                if let parser::haxe_ast::TypeDeclaration::Class(class) = decl {
                    let has_generic = class.meta.iter().any(|meta| meta.name == "generic");
                    let has_native = class.meta.iter().any(|meta| meta.name == "native");
                    println!("Has @:generic metadata: {}", has_generic);
                    println!("Has @:native metadata: {}", has_native);
                    assert!(has_generic, "Should have @:generic metadata");
                    assert!(has_native, "Should have @:native metadata");
                }
            }
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should have parsed");
        }
    }
}