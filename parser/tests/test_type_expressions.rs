//! Type expression parsing tests for the new Haxe parser

use parser::parse_haxe_file;

fn test_type_parsing(type_expr: &str) {
    let input = &format!("class Test {{ var field: {}; }}", type_expr);
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Failed to parse type '{}': {}", type_expr, e),
    }
}

#[test]
fn test_basic_types() {
    test_type_parsing("Int");
    test_type_parsing("Float");
    test_type_parsing("String");
    test_type_parsing("Bool");
    test_type_parsing("Dynamic");
    test_type_parsing("Void");
}

#[test]
fn test_path_types() {
    test_type_parsing("haxe.Json");
    test_type_parsing("sys.io.File");
    test_type_parsing("com.example.MyClass");
}

#[test]
fn test_generic_types() {
    test_type_parsing("Array<String>");
    test_type_parsing("Map<String, Int>");
    test_type_parsing("Promise<Result<Data, Error>>");
    test_type_parsing("Container<List<Item>>");
}

#[test]
fn test_function_types() {
    test_type_parsing("Void -> Int");
    test_type_parsing("String -> Void");
    test_type_parsing("Int -> String -> Bool");
    test_type_parsing("(Int, String) -> Float");
    test_type_parsing("Array<String> -> Map<String, Int>");
}

#[test]
fn test_optional_types() {
    test_type_parsing("?String");
    test_type_parsing("?Int");
    test_type_parsing("?Array<String>");
    test_type_parsing("?Void -> Int");
}

#[test]
fn test_anonymous_types() {
    test_type_parsing("{}");
    test_type_parsing("{x: Int, y: Int}");
    test_type_parsing("{name: String, age: Int, active: Bool}");
    test_type_parsing("{?optional: String, required: Int}");
    test_type_parsing("{callback: String -> Void}");
}

#[test]
fn test_parenthesized_types() {
    test_type_parsing("(String)");
    test_type_parsing("(Int -> String)");
    test_type_parsing("((Int -> String) -> Bool)");
}

#[test]
fn test_complex_function_signatures() {
    let input = r#"
class Test {
    function method1(): Void {}
    function method2(x: Int): String { return ""; }
    function method3(x: Int, y: String): Bool { return true; }
    function method4<T>(item: T): Array<T> { return []; }
    function method5<T, U>(transformer: T -> U, items: Array<T>): Array<U> { return []; }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Complex function signatures should parse, got: {}", e),
    }
}

#[test]
fn test_type_parameters() {
    let input = r#"
class Container<T> {
    var value: T;
    function get(): T { return value; }
    function set(value: T): Void {}
}

class Mapper<T, U> {
    function map(value: T, transform: T -> U): U { return transform(value); }
}

class Constrained<T: Comparable<T>> {
    function compare(a: T, b: T): Int { return 0; }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Type parameters should parse, got: {}", e),
    }
}

#[test]
fn test_typedef_types() {
    let input = r#"
typedef Point = {x: Float, y: Float};
typedef Rect = {x: Float, y: Float, width: Float, height: Float};
typedef Callback<T> = T -> Void;
typedef EventHandler = String -> Dynamic -> Void;

class Test {
    var point: Point;
    var rect: Rect;
    var callback: Callback<String>;
    var handler: EventHandler;
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Typedef types should parse, got: {}", e),
    }
}

#[test]
fn test_enum_types() {
    let input = r#"
enum Color {
    Red;
    Green;
    Blue;
    RGB(r: Int, g: Int, b: Int);
}

class Test {
    var color: Color;
    var colors: Array<Color>;
    var colorMap: Map<String, Color>;
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Enum types should parse, got: {}", e),
    }
}

#[test]
fn test_abstract_types() {
    let input = r#"
abstract Vec2(Point) from Point to Point {
    public function new(x: Float, y: Float) {
        this = {x: x, y: y};
    }
}

class Test {
    var vector: Vec2;
    var vectors: Array<Vec2>;
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Abstract types should parse, got: {}", e),
    }
}

#[test]
fn test_nested_generics() {
    test_type_parsing("Array<Array<String>>");
    test_type_parsing("Map<String, Array<Int>>");
    test_type_parsing("Promise<Result<Array<Item>, Error>>");
    test_type_parsing("Either<Success<Data>, Failure<Error>>");
}

#[test]
fn test_function_with_optional_params() {
    let input = r#"
class Test {
    function method(?optional: String, required: Int, ?callback: String -> Void): Void {}
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Function with optional params should parse, got: {}", e),
    }
}

#[test]
fn test_rest_parameters() {
    let input = r#"
class Test {
    function varArgs(first: String, ...rest: Array<Dynamic>): Void {}
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Rest parameters should parse, got: {}", e),
    }
}