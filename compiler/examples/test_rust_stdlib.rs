//! Test calling Rust stdlib functions from JIT-compiled code

extern crate rayzor_runtime;

use rayzor_runtime::{HaxeVec, HaxeString};

fn main() {
    println!("🚀 Testing Rust Stdlib Integration");
    println!();

    // Test 1: Direct Rust function calls
    println!("📋 Test 1: Direct Rust function calls");
    unsafe {
        let mut vec = rayzor_runtime::vec::haxe_vec_new();
        println!("  ✓ Created Vec: ptr={:p}, len={}, cap={}", vec.ptr, vec.len, vec.cap);

        rayzor_runtime::vec::haxe_vec_push(&mut vec, 42);
        rayzor_runtime::vec::haxe_vec_push(&mut vec, 100);
        rayzor_runtime::vec::haxe_vec_push(&mut vec, 200);

        let len = rayzor_runtime::vec::haxe_vec_len(&vec);
        println!("  ✓ Pushed 3 values, len={}", len);

        let val0 = rayzor_runtime::vec::haxe_vec_get(&vec, 0);
        let val1 = rayzor_runtime::vec::haxe_vec_get(&vec, 1);
        let val2 = rayzor_runtime::vec::haxe_vec_get(&vec, 2);

        println!("  ✓ Values: [{}, {}, {}]", val0, val1, val2);
        assert_eq!(val0, 42);
        assert_eq!(val1, 100);
        assert_eq!(val2, 200);

        rayzor_runtime::vec::haxe_vec_free(&mut vec);
        println!("  ✓ Freed Vec");
    }

    println!();

    // Test 2: String operations
    println!("📋 Test 2: String operations");
    unsafe {
        let s1 = rayzor_runtime::string::haxe_string_from_bytes(b"Hello, ".as_ptr(), 7);
        let s2 = rayzor_runtime::string::haxe_string_from_bytes(b"World!".as_ptr(), 6);

        println!("  ✓ Created two strings");

        let s3 = rayzor_runtime::string::haxe_string_concat(&s1, &s2);
        let len = rayzor_runtime::string::haxe_string_len(&s3);

        println!("  ✓ Concatenated: len={}", len);
        assert_eq!(len, 13);

        // Print the string
        let slice = std::slice::from_raw_parts(s3.ptr, s3.len);
        let str_val = std::str::from_utf8(slice).unwrap();
        println!("  ✓ Result: '{}'", str_val);
        assert_eq!(str_val, "Hello, World!");

        rayzor_runtime::string::haxe_string_free(&mut s1.clone());
        rayzor_runtime::string::haxe_string_free(&mut s2.clone());
        rayzor_runtime::string::haxe_string_free(&mut s3.clone());
        println!("  ✓ Freed strings");
    }

    println!();
    println!("✅ All tests passed!");
}
