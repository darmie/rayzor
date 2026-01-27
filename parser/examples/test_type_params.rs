use parser::parse_haxe_file;

fn main() {
    // Test 1: Simple generic class
    let code1 = r#"
        class SimpleGeneric<T> {
            public function new() {}
        }
    "#;

    println!("Test 1: Simple generic");
    match parse_haxe_file("test.hx", code1, true) {
        Ok(ast) => {
            println!("✓ Parse successful!");
            println!("  Declarations: {} found", ast.declarations.len());
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }

    // Test 2: Generic with single colon constraint
    let code2 = r#"
        class GenericClass<T:Comparable<T>> {
            public function new() {}
        }
    "#;

    println!("\nTest 2: Generic with single ':' constraint");
    match parse_haxe_file("test.hx", code2, true) {
        Ok(ast) => {
            println!("✓ Parse successful!");
            println!("  Declarations: {} found", ast.declarations.len());
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }

    // Test 3: Generic with multiple constraints using &
    let code3 = r#"
        class GenericClass<T:Iterable<String> & Measurable> {
            public function new() {}
        }
    "#;

    println!("\nTest 3: Generic with multiple constraints using '&'");
    match parse_haxe_file("test.hx", code3, true) {
        Ok(ast) => {
            println!("✓ Parse successful!");
            println!("  Declarations: {} found", ast.declarations.len());
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }

    // Test 4: Two type params, second with constraint
    let code4 = r#"
        class GenericClass<T, U:Comparable<U>> {
            public function new() {}
        }
    "#;

    println!("\nTest 4: Two type params, second with constraint");
    match parse_haxe_file("test.hx", code4, true) {
        Ok(ast) => {
            println!("✓ Parse successful!");
            println!("  Declarations: {} found", ast.declarations.len());
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }

    // Test 5: Intersection without type param name (incorrect syntax)
    let code5 = r#"
        class IntersectionClass<T, U & Comparable<U>> {
            public function new() {}
        }
    "#;

    println!("\nTest 5: Intersection without type param name (incorrect syntax)");
    match parse_haxe_file("test.hx", code5, true) {
        Ok(ast) => {
            println!("✓ Parse successful!");
            println!("  Declarations: {} found", ast.declarations.len());
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }

    // Test 6: Debug AST output for successful case
    let code6 = r#"
        class Test<T:String & Int> {}
    "#;

    println!("\nTest 6: Debug AST for T:String & Int");
    match parse_haxe_file("test.hx", code6, false) {
        Ok(ast) => {
            println!("✓ Parse successful!");
            if let Some(parser::haxe_ast::TypeDeclaration::Class(class)) = ast.declarations.first()
            {
                println!("  Class name: {}", class.name);
                println!("  Type params: {} found", class.type_params.len());
                for (i, param) in class.type_params.iter().enumerate() {
                    println!(
                        "    Param {}: name='{}', constraints={}",
                        i,
                        param.name,
                        param.constraints.len()
                    );
                }
            }
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }
}
