use parser::parse_haxe_file;

#[test]
fn test_forward_no_params() {
    let input = r#"
@:forward
abstract StringWrapper(String) {
    public function new(s:String) this = s;
}
"#;

    let ast = parse_haxe_file("test.hx", input, false).expect("Should parse successfully");

    if let Some(parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl)) =
        ast.declarations.first()
    {
        let forward_meta = abstract_decl.meta.iter().find(|m| m.name == "forward");
        assert!(forward_meta.is_some(), "Should have @:forward metadata");
        assert!(
            forward_meta.unwrap().params.is_empty(),
            "Should have no parameters"
        );
    } else {
        panic!("Expected abstract declaration");
    }
}

#[test]
fn test_forward_single_field() {
    let input = r#"
@:forward(length)
abstract IntWrapper(Int) {
    public function new(i:Int) this = i;
}
"#;

    let ast = parse_haxe_file("test.hx", input, false).expect("Should parse successfully");

    if let Some(parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl)) =
        ast.declarations.first()
    {
        let forward_meta = abstract_decl.meta.iter().find(|m| m.name == "forward");
        assert!(forward_meta.is_some(), "Should have @:forward metadata");

        let forward_meta = forward_meta.unwrap();
        assert_eq!(forward_meta.params.len(), 1, "Should have 1 parameter");

        if let parser::haxe_ast::ExprKind::Ident(name) = &forward_meta.params[0].kind {
            assert_eq!(name, "length", "Parameter should be 'length'");
        } else {
            panic!("Expected identifier parameter");
        }
    } else {
        panic!("Expected abstract declaration");
    }
}

#[test]
fn test_forward_multiple_fields() {
    let input = r#"
@:forward(length, charAt, substr, indexOf)
abstract StringWrapper(String) {
    public function new(s:String) this = s;
    
    public function toUpperCase():String {
        return this.toUpperCase();
    }
}
"#;

    let ast = parse_haxe_file("test.hx", input, false).expect("Should parse successfully");

    if let Some(parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl)) =
        ast.declarations.first()
    {
        let forward_meta = abstract_decl.meta.iter().find(|m| m.name == "forward");
        assert!(forward_meta.is_some(), "Should have @:forward metadata");

        let forward_meta = forward_meta.unwrap();
        assert_eq!(forward_meta.params.len(), 4, "Should have 4 parameters");

        let expected_fields = ["length", "charAt", "substr", "indexOf"];
        for (i, expected) in expected_fields.iter().enumerate() {
            if let parser::haxe_ast::ExprKind::Ident(name) = &forward_meta.params[i].kind {
                assert_eq!(name, expected, "Parameter {} should be '{}'", i, expected);
            } else {
                panic!("Expected identifier parameter at index {}", i);
            }
        }
    } else {
        panic!("Expected abstract declaration");
    }
}

#[test]
fn test_forward_with_mixed_params() {
    let input = r#"
@:forward(length, "charAt", 42)
abstract MixedWrapper(String) {
    public function new(s:String) this = s;
}
"#;

    let ast = parse_haxe_file("test.hx", input, false).expect("Should parse successfully");

    if let Some(parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl)) =
        ast.declarations.first()
    {
        let forward_meta = abstract_decl.meta.iter().find(|m| m.name == "forward");
        assert!(forward_meta.is_some(), "Should have @:forward metadata");

        let forward_meta = forward_meta.unwrap();
        assert_eq!(forward_meta.params.len(), 3, "Should have 3 parameters");

        // Check first param (identifier)
        if let parser::haxe_ast::ExprKind::Ident(name) = &forward_meta.params[0].kind {
            assert_eq!(name, "length", "First parameter should be 'length'");
        } else {
            panic!("Expected identifier for first parameter");
        }

        // Check second param (string literal)
        if let parser::haxe_ast::ExprKind::String(s) = &forward_meta.params[1].kind {
            assert_eq!(s, "charAt", "Second parameter should be 'charAt'");
        } else {
            panic!("Expected string literal for second parameter");
        }

        // Check third param (integer literal)
        if let parser::haxe_ast::ExprKind::Int(i) = &forward_meta.params[2].kind {
            assert_eq!(*i, 42, "Third parameter should be 42");
        } else {
            panic!("Expected integer literal for third parameter");
        }
    } else {
        panic!("Expected abstract declaration");
    }
}

#[test]
fn test_forward_complex_abstract() {
    let input = r#"
@:forward(push, pop, length)
abstract IntArray(Array<Int>) from Array<Int> to Array<Int> {
    public function new() {
        this = [];
    }
    
    @:to
    public function toString():String {
        return this.toString();
    }
    
    @:from
    public static function fromString(s:String):IntArray {
        return new IntArray();
    }
}
"#;

    let ast = parse_haxe_file("test.hx", input, false).expect("Should parse successfully");

    if let Some(parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl)) =
        ast.declarations.first()
    {
        let forward_meta = abstract_decl.meta.iter().find(|m| m.name == "forward");
        assert!(forward_meta.is_some(), "Should have @:forward metadata");

        let forward_meta = forward_meta.unwrap();
        assert_eq!(forward_meta.params.len(), 3, "Should have 3 parameters");

        let expected_fields = ["push", "pop", "length"];
        for (i, expected) in expected_fields.iter().enumerate() {
            if let parser::haxe_ast::ExprKind::Ident(name) = &forward_meta.params[i].kind {
                assert_eq!(name, expected, "Parameter {} should be '{}'", i, expected);
            } else {
                panic!("Expected identifier parameter at index {}", i);
            }
        }

        // Check that we have from/to types
        assert!(!abstract_decl.from.is_empty(), "Should have @:from types");
        assert!(!abstract_decl.to.is_empty(), "Should have @:to types");

        // Check that we have fields with @:to and @:from metadata
        let has_to_meta = abstract_decl
            .fields
            .iter()
            .any(|f| f.meta.iter().any(|m| m.name == "to"));
        let has_from_meta = abstract_decl
            .fields
            .iter()
            .any(|f| f.meta.iter().any(|m| m.name == "from"));

        assert!(has_to_meta, "Should have a field with @:to metadata");
        assert!(has_from_meta, "Should have a field with @:from metadata");
    } else {
        panic!("Expected abstract declaration");
    }
}

#[test]
fn test_multiple_forward_metadata() {
    let input = r#"
@:forward(length)
@:forward(charAt)
@:native("MyString")
abstract MultipleForwardWrapper(String) {
    public function new(s:String) this = s;
}
"#;

    let ast = parse_haxe_file("test.hx", input, false).expect("Should parse successfully");

    if let Some(parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl)) =
        ast.declarations.first()
    {
        let forward_metas: Vec<_> = abstract_decl
            .meta
            .iter()
            .filter(|m| m.name == "forward")
            .collect();
        assert_eq!(forward_metas.len(), 2, "Should have 2 @:forward metadata");

        // Check first @:forward
        assert_eq!(
            forward_metas[0].params.len(),
            1,
            "First @:forward should have 1 parameter"
        );
        if let parser::haxe_ast::ExprKind::Ident(name) = &forward_metas[0].params[0].kind {
            assert_eq!(name, "length");
        } else {
            panic!("Expected identifier for first @:forward parameter");
        }

        // Check second @:forward
        assert_eq!(
            forward_metas[1].params.len(),
            1,
            "Second @:forward should have 1 parameter"
        );
        if let parser::haxe_ast::ExprKind::Ident(name) = &forward_metas[1].params[0].kind {
            assert_eq!(name, "charAt");
        } else {
            panic!("Expected identifier for second @:forward parameter");
        }

        // Check that we also have @:native
        let native_meta = abstract_decl.meta.iter().find(|m| m.name == "native");
        assert!(native_meta.is_some(), "Should have @:native metadata");
    } else {
        panic!("Expected abstract declaration");
    }
}

#[test]
fn test_forward_with_spaces() {
    let input = r#"
@:forward(  length  ,  charAt  ,  substr  )
abstract StringWrapper(String) {
    public function new(s:String) this = s;
}
"#;

    let ast = parse_haxe_file("test.hx", input, false).expect("Should parse successfully");

    if let Some(parser::haxe_ast::TypeDeclaration::Abstract(abstract_decl)) =
        ast.declarations.first()
    {
        let forward_meta = abstract_decl.meta.iter().find(|m| m.name == "forward");
        assert!(forward_meta.is_some(), "Should have @:forward metadata");

        let forward_meta = forward_meta.unwrap();
        assert_eq!(forward_meta.params.len(), 3, "Should have 3 parameters");

        let expected_fields = ["length", "charAt", "substr"];
        for (i, expected) in expected_fields.iter().enumerate() {
            if let parser::haxe_ast::ExprKind::Ident(name) = &forward_meta.params[i].kind {
                assert_eq!(name, expected, "Parameter {} should be '{}'", i, expected);
            } else {
                panic!("Expected identifier parameter at index {}", i);
            }
        }
    } else {
        panic!("Expected abstract declaration");
    }
}
