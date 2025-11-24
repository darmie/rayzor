use parser::parse_haxe_file;

#[test]
fn test_untyped_expressions() {
    // Test 1: Simple untyped expression
    let test1 = r#"class Test {
    function test() {
        var result = untyped 42;
        return result;
    }
}"#;
    
    println!("Test 1: simple untyped expression");
    match parse_haxe_file("test.hx", test1, false) {
        Ok(_) => println!("✓ Works"),
        Err(e) => println!("✗ Error: {}", e),
    }
    assert!(parse_haxe_file("test.hx", test1, false).is_ok());
    
    // Test 2: Untyped block expression
    let test2 = r#"class Test {
    function test() {
        var result = untyped {
            some_js_code();
            return 42;
        };
        return result;
    }
}"#;
    
    println!("Test 2: untyped block expression");
    match parse_haxe_file("test.hx", test2, false) {
        Ok(_) => println!("✓ Works"),
        Err(e) => println!("✗ Error: {}", e),
    }
    assert!(parse_haxe_file("test.hx", test2, false).is_ok());
    
    // Test 3: Untyped function call
    let test3 = r#"class Test {
    function test() {
        var result = untyped window.alert("hello");
        return result;
    }
}"#;
    
    println!("Test 3: untyped function call");
    match parse_haxe_file("test.hx", test3, false) {
        Ok(_) => println!("✓ Works"),
        Err(e) => println!("✗ Error: {}", e),
    }
    assert!(parse_haxe_file("test.hx", test3, false).is_ok());
    
    // Test 4: Untyped field access
    let test4 = r#"class Test {
    function test() {
        var result = untyped this.someField;
        return result;
    }
}"#;
    
    println!("Test 4: untyped field access");
    match parse_haxe_file("test.hx", test4, false) {
        Ok(_) => println!("✓ Works"),
        Err(e) => println!("✗ Error: {}", e),
    }
    assert!(parse_haxe_file("test.hx", test4, false).is_ok());
    
    // Test 5: Nested untyped expressions
    let test5 = r#"class Test {
    function test() {
        var result = untyped (untyped someVar + 10);
        return result;
    }
}"#;
    
    println!("Test 5: nested untyped expressions");
    match parse_haxe_file("test.hx", test5, false) {
        Ok(_) => println!("✓ Works"),
        Err(e) => println!("✗ Error: {}", e),
    }
    assert!(parse_haxe_file("test.hx", test5, false).is_ok());
}

#[test]
fn test_untyped_return_statements() {
    // Test 6: Untyped return statement
    let test6 = r#"class Test {
    function test() {
        return untyped jsFunction();
    }
}"#;
    
    println!("Test 6: untyped return statement");
    match parse_haxe_file("test.hx", test6, false) {
        Ok(_) => println!("✓ Works"),
        Err(e) => println!("✗ Error: {}", e),
    }
    assert!(parse_haxe_file("test.hx", test6, false).is_ok());
    
    // Test 7: Untyped with complex expression
    let test7 = r#"class Test {
    function test() {
        var x = untyped obj.method().property[0];
        return x;
    }
}"#;
    
    println!("Test 7: untyped with complex expression");
    match parse_haxe_file("test.hx", test7, false) {
        Ok(_) => println!("✓ Works"),
        Err(e) => println!("✗ Error: {}", e),
    }
    assert!(parse_haxe_file("test.hx", test7, false).is_ok());
}