use parser::parse_haxe_file;

fn main() {
    let enum_code = r#"
enum Color {
    Red;
    Green;
    Blue;
    RGB(r:Int, g:Int, b:Int);
}

class ColorTest {
    static function colorName(color:Color):String {
        return switch (color) {
            case Red: "red";
            case Green: "green";
            case Blue: "blue";
            case RGB(r, g, b): 'rgb($r, $g, $b)';
        };
    }
}
"#;

    match parse_haxe_file("test.hx", enum_code, true) {
        Ok(ast) => {
            println!("Parse successful!");
            println!("Package: {:?}", ast.package);
            println!("Declarations: {} found", ast.declarations.len());
            for (i, t) in ast.declarations.iter().enumerate() {
                match t {
                    parser::haxe_ast::TypeDeclaration::Enum(e) => {
                        println!(
                            "Type {}: Enum {} with {} constructors",
                            i,
                            e.name,
                            e.constructors.len()
                        );
                    }
                    parser::haxe_ast::TypeDeclaration::Class(c) => {
                        println!(
                            "Type {}: Class {} with {} fields",
                            i,
                            c.name,
                            c.fields.len()
                        );
                    }
                    _ => println!("Type {}: Other", i),
                }
            }
        }
        Err(e) => {
            println!("Parse error: {}", e);
        }
    }
}
