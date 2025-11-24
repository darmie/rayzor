use compiler::pipeline::compile_haxe_source;

fn main() {
    println!("=== Testing @:derive([Send, Sync]) Parsing ===\n");

    let source = r#"
@:derive([Send])
class SendOnly {
    public var x: Int;
    public function new() { x = 0; }
}

@:derive([Sync])
class SyncOnly {
    public var y: Int;
    public function new() { y = 0; }
}

@:derive([Send, Sync])
class Both {
    public var z: Int;
    public function new() { z = 0; }
}

@:derive([Clone, Copy, Send, Sync, Hash])
class AllTraits {
    public var value: Int;
    public function new(v: Int) { value = v; }
}

class Main {
    static function main() {
        var s = new SendOnly();
        var sy = new SyncOnly();
        var b = new Both();
        var a = new AllTraits(42);

        trace(s.x);
    }
}
"#;

    println!("Compiling Haxe code with Send/Sync traits...\n");
    let result = compile_haxe_source(source);

    if result.errors.is_empty() {
        println!("✓ Compilation successful!");
        println!("✓ Send/Sync traits parsed correctly");

        // Check if we have the classes
        if !result.typed_files.is_empty() {
            let typed_file = &result.typed_files[0];
            println!("\nClasses found:");
            for class in &typed_file.classes {
                let name = typed_file.string_interner.borrow().get(class.name)
                    .unwrap_or("<unknown>").to_string();

                print!("  - {}: derives [", name);
                let traits: Vec<String> = class.derived_traits.iter()
                    .map(|t| t.as_str().to_string())
                    .collect();
                print!("{}", traits.join(", "));
                println!("]");
            }
        }
    } else {
        println!("✗ Compilation failed with {} error(s):", result.errors.len());
        for error in &result.errors {
            println!("  {}", error.message);
        }
    }

    println!("\n=== Test Complete ===");
}
