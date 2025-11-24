use parser::parse_haxe_file;

#[test]
fn test_multitype_abstract_simple() {
    let input = r#"
@:multiType 
abstract A<T>(T) {
    public function new();
}
"#;
    
    println!("Parsing: {}", input);
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: {:?}", ast);
            
            // Check if the abstract has @:multiType metadata
            if let Some(decl) = ast.declarations.first() {
                if let parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl) = decl {
                    let has_multitype = abstract_decl.meta.iter().any(|meta| meta.name == "multiType");
                    println!("Has @:multiType metadata: {}", has_multitype);
                    assert!(has_multitype, "Should have @:multiType metadata");
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
fn test_multitype_abstract_with_params() {
    let input = r#"
@:multiType(K) 
abstract MyMap<K, V>(Map<K, V>) {
    public function new();
    
    @:to static inline function toStringMap<K:String, V>(t:Map<K, V>):StringMap<V> {
        return cast t;
    }
}
"#;
    
    println!("Parsing: {}", input);
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: {:?}", ast);
            
            // Check if the abstract has @:multiType metadata with parameter
            if let Some(decl) = ast.declarations.first() {
                if let parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl) = decl {
                    let multitype_meta = abstract_decl.meta.iter()
                        .find(|meta| meta.name == "multiType");
                    
                    assert!(multitype_meta.is_some(), "Should have @:multiType metadata");
                    let multitype_meta = multitype_meta.unwrap();
                    
                    println!("@:multiType params: {:?}", multitype_meta.params);
                    assert!(!multitype_meta.params.is_empty(), "Should have parameter K");
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
fn test_multitype_abstract_complex() {
    let input = r#"
interface IA<T> { }

class StringA implements IA<String> {
    public function new() {}
}

@:multiType 
abstract A<T>(IA<T>) {
    public function new();
    
    @:to static inline function toStringA(t:IA<String>):StringA {
        return new StringA();
    }
}
"#;
    
    println!("Parsing: {}", input);
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: parsed {} declarations", ast.declarations.len());
            
            // Find the abstract declaration
            let abstract_decl = ast.declarations.iter()
                .find_map(|decl| {
                    if let parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl) = decl {
                        Some(abstract_decl)
                    } else {
                        None
                    }
                });
            
            if let Some(abstract_decl) = abstract_decl {
                let has_multitype = abstract_decl.meta.iter()
                    .any(|meta| meta.name == "multiType");
                
                println!("Has @:multiType metadata: {}", has_multitype);
                assert!(has_multitype, "Should have @:multiType metadata");
                
                // Check that it has the expected fields
                println!("Fields count: {}", abstract_decl.fields.len());
                assert!(abstract_decl.fields.len() >= 2, "Should have at least 2 fields (new, @:to function)");
            } else {
                panic!("Should have found abstract declaration");
            }
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should have parsed");
        }
    }
}