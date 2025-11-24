use parser::parse_haxe_file;

fn print_expr(expr: &parser::Expr, indent: usize) {
    let spaces = " ".repeat(indent);
    match &expr.kind {
        parser::ExprKind::New { type_path, params, args } => {
            println!("{}New {{", spaces);
            println!("{}  type_path: {:?}", spaces, type_path);
            println!("{}  params: {} items", spaces, params.len());
            println!("{}  args: {} items", spaces, args.len());
            println!("{}  span: {:?}", spaces, expr.span);
            println!("{}}}", spaces);
        }
        parser::ExprKind::Call { expr: func, args } => {
            println!("{}Call {{", spaces);
            println!("{}  args: {} items", spaces, args.len());
            println!("{}  span: {:?}", spaces, expr.span);
            println!("{}  function:", spaces);
            print_expr(func, indent + 4);
            println!("{}}}", spaces);
        }
        parser::ExprKind::Var { name, expr, .. } => {
            println!("{}Var '{}' =", spaces, name);
            if let Some(init_expr) = expr {
                print_expr(init_expr, indent + 2);
            }
        }
        parser::ExprKind::Ident(name) => {
            println!("{}Ident: {}", spaces, name);
        }
        parser::ExprKind::Block(elements) => {
            println!("{}Block with {} elements", spaces, elements.len());
            for elem in elements {
                if let parser::BlockElement::Expr(e) = elem {
                    print_expr(e, indent + 2);
                }
            }
        }
        _ => {
            println!("{}{:?}", spaces, expr.kind);
        }
    }
}

fn main() {
    let test_code = r#"
class Test {
    public function test():Void {
        var x = new Test();
        var y = new List<String>();
    }
}
"#;

    match parse_haxe_file("test.hx", test_code, false) {
        Ok(ast) => {
            println!("Parse successful!");
            for decl in &ast.declarations {
                match decl {
                    parser::TypeDeclaration::Class(class) => {
                        for class_field in &class.fields {
                            if let parser::ClassFieldKind::Function(func) = &class_field.kind {
                                if let Some(body) = &func.body {
                                    println!("\nFunction '{}' body:", func.name);
                                    print_expr(body, 2);
                                }
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