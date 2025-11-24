use parser::parse_haxe_file;

fn main() {
    let input = r#"
class Test {
    function test() {
        switch (value) {
            case 1:
                handleOne();
        }
    }
}
"#;

    println!("Parsing: {}", input);
    
    match parse_haxe_file(input) {
        Ok(ast) => {
            println!("Success: {:?}", ast);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}