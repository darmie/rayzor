//! Debug failing syntax from TAST validation tests

#[test]
fn test_variadic_params() {
    let content = r#"class Test {
    static function variadic(required:String, ...rest:String):Dynamic {
        return rest;
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Variadic params parsed"),
        Err(e) => println!("❌ Variadic params failed: {:?}", e),
    }
}

#[test]
fn test_dollar_expressions() {
    let content = r#"import haxe.macro.Expr;
import haxe.macro.ExprTools;

macro function assert(expr:Expr):Expr {
    return macro {
        if (!$expr) throw "Assertion failed: " + $v{ExprTools.toString(expr)};
    };
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Dollar expressions parsed"),
        Err(e) => println!("❌ Dollar expressions failed: {:?}", e),
    }
}

#[test]
fn test_inline_xml() {
    let content = r#"class Test {
    static function xml() {
        var xml = <div class="test">
            <span>{obj.name}</span>
        </div>;
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Inline XML parsed"),
        Err(e) => println!("❌ Inline XML failed: {:?}", e),
    }
}

#[test]
fn test_safe_navigation() {
    let content = r#"class Test {
    static function safeNav() {
        var nullable:Null<Container<String>> = null;
        var len = nullable?.length;
        var first = nullable?.[0] ?? "default";
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Safe navigation parsed"),
        Err(e) => println!("❌ Safe navigation failed: {:?}", e),
    }
}

#[test]
fn test_null_coalescing() {
    let content = r#"class Test {
    static function nullCoalesce() {
        var nullable:Null<String> = null;
        var value = nullable ?? "default";
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Null coalescing parsed"),
        Err(e) => println!("❌ Null coalescing failed: {:?}", e),
    }
}

#[test]
fn test_metadata_on_expressions() {
    let content = r#"class Test {
    static function metaExpr() {
        @:keep var important = true;
        @:inline var fast = 42;
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Metadata on expressions parsed"),
        Err(e) => println!("❌ Metadata on expressions failed: {:?}", e),
    }
}

#[test]
fn test_do_while() {
    let content = r#"class Test {
    static function doWhileTest() {
        var i = 0;
        do {
            i++;
        } while (i < 10);
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Do-while parsed"),
        Err(e) => println!("❌ Do-while failed: {:?}", e),
    }
}

#[test]
fn test_key_value_iteration() {
    let content = r#"class Test {
    static function keyValueIter() {
        var map = ["a" => 1, "b" => 2];
        for (key => value in map) {
            trace('$key: $value');
        }
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Key-value iteration parsed"),
        Err(e) => println!("❌ Key-value iteration failed: {:?}", e),
    }
}

#[test]
fn test_macro_reification() {
    let content = r#"class Test {
    static function macroReify() {
        var expr = macro $v{42} + $v{8};
        var block = macro {
            trace("hello");
            return 42;
        };
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Macro reification parsed"),
        Err(e) => println!("❌ Macro reification failed: {:?}", e),
    }
}

#[test]
fn test_pattern_matching_guards() {
    let content = r#"class Test {
    static function patternGuards() {
        var obj = {name: "test", value: 42};
        var result = switch (obj) {
            case {name: "test", value: v} if (v > 40): "match";
            case {name: n}: 'name is $n';
            case _: "no match";
        }
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Pattern matching guards parsed"),
        Err(e) => println!("❌ Pattern matching guards failed: {:?}", e),
    }
}

#[test]
fn test_compiler_specific() {
    let content = r#"class Test {
    static function compilerSpecific() {
        untyped {
            __js__("console.log('Native JS')");
        }
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Compiler-specific code parsed"),
        Err(e) => println!("❌ Compiler-specific code failed: {:?}", e),
    }
}

#[test]
fn test_array_comprehension_with_filter() {
    let content = r#"class Test {
    static function arrayComp() {
        var squares = [for (i in 0...10) i * i];
        var filtered = [for (x in squares) if (x > 10) x];
        var nested = [for (i in 0...3) for (j in 0...3) i + j];
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Array comprehension with filter parsed"),
        Err(e) => println!("❌ Array comprehension with filter failed: {:?}", e),
    }
}

#[test]
fn test_map_comprehension_with_filter() {
    let content = r#"class Test {
    static function mapComp() {
        var map = [for (i in 0...5) i => i * i];
        var filtered = [for (k => v in map) if (v > 5) k => v * 2];
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Map comprehension with filter parsed"),
        Err(e) => println!("❌ Map comprehension with filter failed: {:?}", e),
    }
}

#[test]
fn test_object_with_computed_properties() {
    let content = r#"class Test {
    static function computedProps() {
        var key = "dynamic";
        var obj = {
            name: "test",
            [key]: 42,
            "literal-key": true
        };
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Object with computed properties parsed"),
        Err(e) => println!("❌ Object with computed properties failed: {:?}", e),
    }
}

#[test]
fn test_module_level_vars() {
    let content = r#"private var moduleVar:String = "module";
public final moduleConst:Int = 42;

class Test {
    static function main() {}
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Module-level vars parsed"),
        Err(e) => println!("❌ Module-level vars failed: {:?}", e),
    }
}

#[test]
fn test_macro_function() {
    let content = r#"macro function buildType():Array<Field> {
    var fields = Context.getBuildFields();
    fields.push({
        name: "generated",
        pos: Context.currentPos(),
        kind: FVar(macro:String, macro "generated"),
        access: [APublic]
    });
    return fields;
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Macro function parsed"),
        Err(e) => println!("❌ Macro function failed: {:?}", e),
    }
}

#[test]
fn test_conditional_compilation() {
    let content = r#"class Test {
    static function conditional() {
        #if debug
        trace("Debug mode");
        #elseif release
        trace("Release mode");
        #else
        trace("Unknown mode");
        #end
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Conditional compilation parsed"),
        Err(e) => println!("❌ Conditional compilation failed: {:?}", e),
    }
}

#[test]
fn test_regex_literal() {
    let content = r#"class Test {
    static function regexTest() {
        var regex = ~/[a-z]+/i;
        var regex2 = ~/\d{3}-\d{4}/g;
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Regex literal parsed"),
        Err(e) => println!("❌ Regex literal failed: {:?}", e),
    }
}

#[test]
fn test_abstract_with_conversions() {
    let content = r#"@:forward
abstract SafeInt(Int) from Int to Int {
    inline public function new(i:Int) {
        this = i;
    }
    
    @:from static public function fromString(s:String):SafeInt {
        return new SafeInt(Std.parseInt(s));
    }
    
    @:to public function toFloat():Float {
        return this;
    }
    
    @:op(A + B) static public function add(a:SafeInt, b:SafeInt):SafeInt {
        return a.toInt() + b.toInt();
    }
    
    public function toInt():Int return this;
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Abstract with conversions parsed"),
        Err(e) => println!("❌ Abstract with conversions failed: {:?}", e),
    }
}

#[test]
fn test_inline_functions() {
    let content = r#"class Test {
    inline static function helper(x:Int):Int return x * 2;
    
    static function test() {
        inline function local(y:Int):Int return y * 3;
        var result = helper(local(5));
    }
}"#;

    match parser::parse_haxe_file("test.hx", content, false) {
        Ok(_) => println!("✅ Inline functions parsed"),
        Err(e) => println!("❌ Inline functions failed: {:?}", e),
    }
}
