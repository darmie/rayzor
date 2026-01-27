use parser::parse_haxe_file;

#[test]
fn test_generic_metadata_comprehensive() {
    let input = r#"
// Test comprehensive @:generic metadata support
@:generic
class GenericClass<T> {
    public var value: T;
    
    public function new(value: T) {
        this.value = value;
    }
    
    public function get(): T {
        return this.value;
    }
}

@:generic
interface GenericInterface<T, U> {
    function convert(input: T): U;
}

@:generic
abstract GenericAbstract<T>(T) {
    public inline function new(value: T) {
        this = value;
    }
}

@:generic
@:final
class FinalGenericClass<T> {
    public final value: T;
    
    public function new(value: T) {
        this.value = value;
    }
}

@:generic
enum GenericEnum<T> {
    Value(value: T);
    Empty;
}

@:generic
typedef GenericTypedef<T> = {
    value: T,
    process: T -> T
}
"#;

    println!("Parsing comprehensive @:generic metadata test...");

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: {:?}", ast);

            // Check we have 6 declarations
            assert_eq!(ast.declarations.len(), 6);

            // Check each declaration has @:generic metadata
            let type_names = [
                "GenericClass",
                "GenericInterface",
                "GenericAbstract",
                "FinalGenericClass",
                "GenericEnum",
                "GenericTypedef",
            ];

            for (i, decl) in ast.declarations.iter().enumerate() {
                let has_generic = match decl {
                    parser::haxe_ast::TypeDeclaration::Class(class) => {
                        class.meta.iter().any(|meta| meta.name == "generic")
                    }
                    parser::haxe_ast::TypeDeclaration::Interface(interface) => {
                        interface.meta.iter().any(|meta| meta.name == "generic")
                    }
                    parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl) => {
                        abstract_decl.meta.iter().any(|meta| meta.name == "generic")
                    }
                    parser::haxe_ast::TypeDeclaration::Enum(enum_decl) => {
                        enum_decl.meta.iter().any(|meta| meta.name == "generic")
                    }
                    parser::haxe_ast::TypeDeclaration::Typedef(typedef_decl) => {
                        typedef_decl.meta.iter().any(|meta| meta.name == "generic")
                    }
                    _ => false,
                };

                println!(
                    "Declaration {}: {} has @:generic metadata: {}",
                    i, type_names[i], has_generic
                );
                assert!(
                    has_generic,
                    "Declaration {} should have @:generic metadata",
                    type_names[i]
                );
            }

            // Check that FinalGenericClass also has @:final metadata
            if let parser::haxe_ast::TypeDeclaration::Class(class) = &ast.declarations[3] {
                let has_final = class.meta.iter().any(|meta| meta.name == "final");
                println!("FinalGenericClass has @:final metadata: {}", has_final);
                assert!(has_final, "FinalGenericClass should have @:final metadata");
            }
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should have parsed successfully");
        }
    }
}

#[test]
fn test_generic_metadata_edge_cases() {
    let input = r#"
// Test edge cases for @:generic metadata
@:generic
class A<T> {}

@:generic class B<T> {} // Inline declaration

@:generic
@:native("NativeGeneric")
@:build(macro Builder.build())
class C<T> {}

@:generic
interface D<T> extends E<T> {}

@:generic
abstract F<T>(T) from T to T {}
"#;

    println!("Parsing @:generic metadata edge cases...");

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: {:?}", ast);

            // Check all declarations have @:generic metadata
            for (i, decl) in ast.declarations.iter().enumerate() {
                let has_generic = match decl {
                    parser::haxe_ast::TypeDeclaration::Class(class) => {
                        class.meta.iter().any(|meta| meta.name == "generic")
                    }
                    parser::haxe_ast::TypeDeclaration::Interface(interface) => {
                        interface.meta.iter().any(|meta| meta.name == "generic")
                    }
                    parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl) => {
                        abstract_decl.meta.iter().any(|meta| meta.name == "generic")
                    }
                    _ => false,
                };

                println!("Declaration {}: has @:generic metadata: {}", i, has_generic);
                assert!(
                    has_generic,
                    "Declaration {} should have @:generic metadata",
                    i
                );
            }
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should have parsed successfully");
        }
    }
}

#[test]
fn test_generic_metadata_complex_type_params() {
    let input = r#"
@:generic
class ComplexGeneric<T:Iterable<U>, U:Comparable<U>, V:(T, U) -> V> {
    public function process(items: T): V {
        return null;
    }
}

@:generic
interface GenericWithConstraints<T:{name:String, age:Int}> {
    function validate(item: T): Bool;
}
"#;

    println!("Parsing @:generic metadata with complex type parameters...");

    match parse_haxe_file("test.hx", input, false) {
        Ok(ast) => {
            println!("Success: {:?}", ast);

            // Check both declarations have @:generic metadata
            for (i, decl) in ast.declarations.iter().enumerate() {
                let has_generic = match decl {
                    parser::haxe_ast::TypeDeclaration::Class(class) => {
                        class.meta.iter().any(|meta| meta.name == "generic")
                    }
                    parser::haxe_ast::TypeDeclaration::Interface(interface) => {
                        interface.meta.iter().any(|meta| meta.name == "generic")
                    }
                    _ => false,
                };

                println!("Declaration {}: has @:generic metadata: {}", i, has_generic);
                assert!(
                    has_generic,
                    "Declaration {} should have @:generic metadata",
                    i
                );
            }
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should have parsed successfully");
        }
    }
}
