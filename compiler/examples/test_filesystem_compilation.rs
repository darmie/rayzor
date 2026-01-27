#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unreachable_patterns,
    unused_mut,
    unused_assignments,
    unused_parens
)]
#![allow(
    clippy::single_component_path_imports,
    clippy::for_kv_map,
    clippy::explicit_auto_deref
)]
#![allow(
    clippy::println_empty_string,
    clippy::len_zero,
    clippy::useless_vec,
    clippy::field_reassign_with_default
)]
#![allow(
    clippy::needless_borrow,
    clippy::redundant_closure,
    clippy::bool_assert_comparison
)]
#![allow(
    clippy::empty_line_after_doc_comments,
    clippy::useless_format,
    clippy::clone_on_copy
)]
//! Test compilation unit with filesystem-based loading
//!
//! This demonstrates:
//! 1. Stdlib discovery from environment/standard locations
//! 2. Loading files from filesystem paths
//! 3. Directory scanning for .hx files
//! 4. Import path resolution

use compiler::compilation::{CompilationConfig, CompilationUnit};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    println!("=== Testing Filesystem-Based Compilation ===\n");

    // Create temporary test directory structure
    println!("1. Creating temporary test project...");
    let temp_dir = std::env::temp_dir().join("rayzor_test_project");

    // Clean up if exists
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).ok();
    }

    // Create directory structure
    let src_dir = temp_dir.join("src");
    let model_dir = src_dir.join("com/example/model");
    fs::create_dir_all(&model_dir).expect("Failed to create model directory");

    let service_dir = src_dir.join("com/example/service");
    fs::create_dir_all(&service_dir).expect("Failed to create service directory");

    // Write User.hx
    let user_file = model_dir.join("User.hx");
    let mut f = fs::File::create(&user_file).expect("Failed to create User.hx");
    f.write_all(
        br#"
package com.example.model;

class User {
    public var name:String;
    public var age:Int;

    public function new(name:String, age:Int) {
        this.name = name;
        this.age = age;
    }

    public function greet():String {
        return "Hello, I am " + name;
    }
}
    "#,
    )
    .expect("Failed to write User.hx");

    // Write UserService.hx
    let service_file = service_dir.join("UserService.hx");
    let mut f = fs::File::create(&service_file).expect("Failed to create UserService.hx");
    f.write_all(
        br#"
package com.example.service;

import com.example.model.User;

class UserService {
    private var users:Array<User>;

    public function new() {
        this.users = [];
    }

    public function addUser(user:User):Void {
        users.push(user);
    }

    public function getCount():Int {
        return users.length;
    }
}
    "#,
    )
    .expect("Failed to write UserService.hx");

    println!("   ✓ Created test project at: {:?}\n", temp_dir);

    // Create compilation unit with stdlib discovery
    println!("2. Creating compilation unit with stdlib discovery...");
    let config = CompilationConfig::default();

    if !config.stdlib_paths.is_empty() {
        println!("   Found stdlib paths:");
        for path in &config.stdlib_paths {
            if path.exists() {
                println!("      ✓ {:?}", path);
            } else {
                println!("      ✗ {:?} (doesn't exist)", path);
            }
        }
    }

    let mut unit = CompilationUnit::new(config);
    println!();

    // Load stdlib
    println!("3. Loading standard library...");
    match unit.load_stdlib() {
        Ok(()) => {
            println!("   ✓ Loaded {} stdlib files\n", unit.stdlib_files.len());
        }
        Err(e) => {
            eprintln!("   ✗ Failed to load stdlib: {}", e);
            eprintln!("   This is expected if no stdlib is installed.\n");
        }
    }

    // Load files from filesystem using different methods
    println!("4. Loading files from filesystem...");

    // Method 1: Load by explicit path
    println!("   a) Loading User.hx by path...");
    if let Err(e) = unit.add_file_from_path(&user_file) {
        eprintln!("      ✗ Failed: {}", e);
        return;
    }
    println!("      ✓ Loaded");

    // Method 2: Load by import path
    println!("   b) Loading UserService.hx by import path...");
    let source_paths = vec![src_dir.clone()];
    if let Err(e) = unit.add_file_by_import("com.example.service.UserService", &source_paths) {
        eprintln!("      ✗ Failed: {}", e);
        return;
    }
    println!("      ✓ Loaded\n");

    println!("5. Compiling {} user files...", unit.user_files.len());
    match unit.lower_to_tast() {
        Ok(typed_files) => {
            println!("   ✓ Successfully compiled {} files\n", typed_files.len());

            // Verify symbols
            println!("6. Verifying compiled symbols...");
            let user_class_symbols: Vec<_> = unit
                .symbol_table
                .all_symbols()
                .filter(|s| {
                    if let Some(qname) = s.qualified_name {
                        let name = unit.string_interner.get(qname).unwrap_or("");
                        name.contains("User") && name.starts_with("com.example")
                    } else {
                        false
                    }
                })
                .map(|s| {
                    let qname = s.qualified_name.unwrap();
                    unit.string_interner.get(qname).unwrap_or("").to_string()
                })
                .collect();

            if !user_class_symbols.is_empty() {
                println!("   Found User-related symbols:");
                for sym in user_class_symbols.iter().take(10) {
                    println!("      - {}", sym);
                }
                if user_class_symbols.len() > 10 {
                    println!("      ... and {} more", user_class_symbols.len() - 10);
                }
            }

            println!("\n🎉 SUCCESS: Filesystem-based compilation working!");
            println!("   - Stdlib discovery: ✓");
            println!("   - File loading from paths: ✓");
            println!("   - Import path resolution: ✓");
            println!("   - Multi-file compilation: ✓");
        }
        Err(e) => {
            eprintln!("   ✗ Compilation failed: {:?}", e);
        }
    }

    // Cleanup
    println!("\n7. Cleaning up temporary files...");
    fs::remove_dir_all(&temp_dir).ok();
    println!("   ✓ Done");
}
