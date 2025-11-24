//! Test stdlib method mapping
//!
//! Demonstrates how Haxe standard library method calls are mapped to runtime functions.

use compiler::stdlib::{StdlibMapping, MethodSignature};

fn main() {
    println!("🗺️  Haxe Standard Library Runtime Mapping\n");
    println!("{}", "=".repeat(70));

    let mapping = StdlibMapping::new();

    // String methods
    println!("\n📦 String Methods");
    println!("{}", "-".repeat(70));
    test_mapping(&mapping, "String", "charAt", false, "haxe_string_char_at");
    test_mapping(&mapping, "String", "indexOf", false, "haxe_string_index_of");
    test_mapping(&mapping, "String", "substring", false, "haxe_string_substring");
    test_mapping(&mapping, "String", "toUpperCase", false, "haxe_string_to_upper_case");
    test_mapping(&mapping, "String", "fromCharCode", true, "haxe_string_from_char_code");

    // Array methods
    println!("\n📦 Array Methods");
    println!("{}", "-".repeat(70));
    test_mapping(&mapping, "Array", "push", false, "haxe_array_push");
    test_mapping(&mapping, "Array", "pop", false, "haxe_array_pop");
    test_mapping(&mapping, "Array", "slice", false, "haxe_array_slice");
    test_mapping(&mapping, "Array", "indexOf", false, "haxe_array_index_of");

    // Math methods
    println!("\n📦 Math Methods");
    println!("{}", "-".repeat(70));
    test_mapping(&mapping, "Math", "sin", true, "haxe_math_sin");
    test_mapping(&mapping, "Math", "cos", true, "haxe_math_cos");
    test_mapping(&mapping, "Math", "sqrt", true, "haxe_math_sqrt");
    test_mapping(&mapping, "Math", "random", true, "haxe_math_random");

    // Sys methods
    println!("\n📦 Sys Methods");
    println!("{}", "-".repeat(70));
    test_mapping(&mapping, "Sys", "print", true, "haxe_string_print");
    test_mapping(&mapping, "Sys", "println", true, "haxe_sys_println");
    test_mapping(&mapping, "Sys", "exit", true, "haxe_sys_exit");
    test_mapping(&mapping, "Sys", "time", true, "haxe_sys_time");

    println!("\n{}", "=".repeat(70));
    println!("\n✅ All stdlib methods successfully mapped to runtime functions!");

    // Show call conventions
    println!("\n📋 Call Convention Examples:\n");
    show_call_convention(&mapping, "String", "charAt", false);
    show_call_convention(&mapping, "String", "substring", false);
    show_call_convention(&mapping, "Array", "push", false);
    show_call_convention(&mapping, "Math", "sin", true);

    println!("\n{}", "=".repeat(70));
}

fn test_mapping(
    mapping: &StdlibMapping,
    class: &'static str,
    method: &'static str,
    is_static: bool,
    expected_runtime: &str,
) {
    let sig = MethodSignature {
        class,
        method,
        is_static,
    };

    match mapping.get(&sig) {
        Some(call) => {
            let kind = if is_static { "static" } else { "instance" };
            println!(
                "  ✓ {}.{:20} ({:8}) -> {}",
                class, method, kind, call.runtime_name
            );
            assert_eq!(
                call.runtime_name, expected_runtime,
                "Runtime function mismatch for {}.{}",
                class, method
            );
        }
        None => {
            panic!("Missing mapping for {}.{}", class, method);
        }
    }
}

fn show_call_convention(
    mapping: &StdlibMapping,
    class: &'static str,
    method: &'static str,
    is_static: bool,
) {
    let sig = MethodSignature {
        class,
        method,
        is_static,
    };

    if let Some(call) = mapping.get(&sig) {
        println!("  {}.{}():", class, method);
        println!("    Runtime: {}", call.runtime_name);
        println!("    Needs out param: {}", call.needs_out_param);
        println!("    Has self param: {}", call.has_self_param);
        println!("    Additional params: {}", call.param_count);
        println!("    Has return value: {}", call.has_return);

        // Show call signature
        let mut params = Vec::new();
        if call.needs_out_param {
            params.push("out: *mut T".to_string());
        }
        if call.has_self_param {
            params.push("self: *const T".to_string());
        }
        for i in 0..call.param_count {
            params.push(format!("arg{}: T", i));
        }

        let ret = if call.has_return {
            " -> T"
        } else if call.needs_out_param {
            " (returns via out)"
        } else {
            ""
        };

        println!("    C signature: {}({}){}", call.runtime_name, params.join(", "), ret);
        println!();
    }
}
