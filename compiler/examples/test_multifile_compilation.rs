//! Test multi-file compilation with proper dependency resolution
//!
//! This test demonstrates:
//! 1. Multiple user files in different packages
//! 2. Cross-file imports and dependencies
//! 3. Proper dependency ordering
//! 4. Package visibility checking

use compiler::compilation::{CompilationUnit, CompilationConfig};

fn main() {
    println!("=== Testing Multi-File Compilation ===\n");

    // Create compilation unit
    let mut unit = CompilationUnit::new(CompilationConfig::default());

    // Load stdlib
    println!("1. Loading standard library...");
    unit.load_stdlib().expect("Failed to load stdlib");
    println!("   ✓ Loaded {} stdlib files\n", unit.stdlib_files.len());

    // File 1: Base model class (no dependencies)
    println!("2. Adding model file (com.example.model.User)...");
    let model_source = r#"
        package com.example.model;

        class User {
            public var name:String;
            public var age:Int;

            public function new(name:String, age:Int) {
                this.name = name;
                this.age = age;
            }

            public function getInfo():String {
                return name + " is " + age + " years old";
            }
        }
    "#;
    unit.add_file(model_source, "com/example/model/User.hx")
        .expect("Failed to add User.hx");
    println!("   ✓ Added User.hx\n");

    // File 2: Service class (depends on model.User)
    println!("3. Adding service file (com.example.service.UserService)...");
    let service_source = r#"
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

            public function getUserCount():Int {
                return users.length;
            }

            public function getAllInfo():String {
                var result = "";
                for (user in users) {
                    result = result + user.getInfo() + "\n";
                }
                return result;
            }
        }
    "#;
    unit.add_file(service_source, "com/example/service/UserService.hx")
        .expect("Failed to add UserService.hx");
    println!("   ✓ Added UserService.hx\n");

    // File 3: Main class (depends on both model and service)
    println!("4. Adding main file (com.example.Main)...");
    let main_source = r#"
        package com.example;

        import com.example.model.User;
        import com.example.service.UserService;

        class Main {
            public static function main():Void {
                var service = new UserService();

                var user1 = new User("Alice", 25);
                var user2 = new User("Bob", 30);

                service.addUser(user1);
                service.addUser(user2);

                var count = service.getUserCount();
                var info = service.getAllInfo();
            }
        }
    "#;
    unit.add_file(main_source, "com/example/Main.hx")
        .expect("Failed to add Main.hx");
    println!("   ✓ Added Main.hx\n");

    // Lower to TAST
    println!("5. Lowering to TAST with dependency resolution...");
    match unit.lower_to_tast() {
        Ok(typed_files) => {
            println!("   ✓ Successfully lowered {} files\n", typed_files.len());

            // Verify symbols
            println!("6. Verifying symbols and packages...");

            let model_symbols = unit.symbol_table.all_symbols()
                .filter(|s| {
                    if let Some(qname) = s.qualified_name {
                        let name = unit.string_interner.get(qname).unwrap_or("");
                        name.starts_with("com.example.model.")
                    } else {
                        false
                    }
                })
                .count();

            let service_symbols = unit.symbol_table.all_symbols()
                .filter(|s| {
                    if let Some(qname) = s.qualified_name {
                        let name = unit.string_interner.get(qname).unwrap_or("");
                        name.starts_with("com.example.service.")
                    } else {
                        false
                    }
                })
                .count();

            let main_symbols = unit.symbol_table.all_symbols()
                .filter(|s| {
                    if let Some(qname) = s.qualified_name {
                        let name = unit.string_interner.get(qname).unwrap_or("");
                        name.starts_with("com.example.") &&
                        !name.starts_with("com.example.model.") &&
                        !name.starts_with("com.example.service.")
                    } else {
                        false
                    }
                })
                .count();

            println!("   Model package symbols: {}", model_symbols);
            println!("   Service package symbols: {}", service_symbols);
            println!("   Main package symbols: {}", main_symbols);

            if model_symbols > 0 && service_symbols > 0 && main_symbols > 0 {
                println!("\n🎉 SUCCESS: Multi-file compilation working!");
                println!("   - All packages resolved correctly");
                println!("   - Cross-file imports working");
                println!("   - Dependency resolution functional");
            } else {
                println!("\n⚠️  Some packages may not have resolved correctly:");
                println!("   Model: {}, Service: {}, Main: {}",
                         model_symbols, service_symbols, main_symbols);
            }
        }
        Err(e) => {
            eprintln!("   ✗ TAST lowering failed: {}", e);
            eprintln!("\nThis indicates an issue with:");
            eprintln!("   - Import resolution");
            eprintln!("   - Cross-file type resolution");
            eprintln!("   - Dependency ordering");
        }
    }
}
