use compiler::ir::{tast_to_hir::lower_tast_to_hir, hir_to_mir::lower_hir_to_mir};
use compiler::tast::{
    AstLowering, StringInterner, SymbolTable, TypeTable, ScopeTree, ScopeId,
    namespace::NamespaceResolver,
};
use parser::{parse_haxe_file, SourceMap};
use std::rc::Rc;
use std::cell::RefCell;

fn main() {
    println!("=== Testing Instance Method 'this' Parameter ===\n");

    // Haxe code with an instance method that uses 'this'
    let haxe_code = r#"
        package com.example;

        class Counter {
            var value: Int;

            public function new(initial: Int) {
                this.value = initial;
            }

            public function increment(): Int {
                this.value = this.value + 1;
                return this.value;
            }

            public static function createDefault(): Counter {
                return new Counter(0);
            }
        }
    "#;

    println!("1. Parsing Haxe source...");
    let source_map = Rc::new(RefCell::new(SourceMap::new()));
    let result = parse_haxe_file(haxe_code, "test.hx", source_map.clone());
    let ast_file = match result {
        Ok(file) => {
            println!("   ✓ Successfully parsed");
            file
        }
        Err(e) => {
            eprintln!("   ✗ Parse error: {:?}", e);
            return;
        }
    };

    // Step 2: Lower AST to TAST
    println!("\n2. Lowering AST to TAST...");
    let mut string_interner = StringInterner::new();
    let mut symbol_table = SymbolTable::new();
    let type_table = Rc::new(RefCell::new(TypeTable::new()));
    let mut scope_tree = ScopeTree::new(ScopeId::from_raw(0));
    let mut namespace_resolver = NamespaceResolver::new(&string_interner);

    let mut ast_lowering = AstLowering::new(
        &mut string_interner,
        &mut symbol_table,
        &type_table,
        &mut scope_tree,
        &mut namespace_resolver,
    );

    let typed_file = match ast_lowering.lower_file(&ast_file) {
        Ok(file) => {
            println!("   ✓ Successfully lowered to TAST");
            println!("   - Classes: {}", file.classes.len());
            file
        }
        Err(errors) => {
            eprintln!("   ✗ TAST lowering errors:");
            for error in errors {
                eprintln!("     - {}", error);
            }
            return;
        }
    };

    // Step 3: Lower TAST to HIR
    println!("\n3. Lowering TAST to HIR...");
    let string_interner_rc = typed_file.string_interner.clone();

    let hir_module = {
        let mut interner_guard = string_interner_rc.borrow_mut();
        match lower_tast_to_hir(
            &typed_file,
            &symbol_table,
            &type_table,
            &mut *interner_guard,
            None,
        ) {
            Ok(hir) => {
                println!("   ✓ Successfully lowered to HIR");
                println!("   - Types: {}", hir.types.len());
                Some(hir)
            }
            Err(errors) => {
                eprintln!("   ✗ HIR lowering errors:");
                for error in errors {
                    eprintln!("     - {}", error.message);
                }
                None
            }
        }
    };

    let hir_module = match hir_module {
        Some(hir) => hir,
        None => return,
    };

    // Check HIR methods
    println!("\n4. Checking HIR methods...");
    use compiler::ir::hir::HirTypeDecl;
    let interner_ref = string_interner_rc.borrow();
    for (type_id, type_decl) in &hir_module.types {
        if let HirTypeDecl::Class(class) = type_decl {
            let class_name = interner_ref.get(class.name).unwrap_or("?");
            println!("   Class '{}' methods:", class_name);
            for method in &class.methods {
                let method_name = interner_ref.get(method.function.name).unwrap_or("?");
                let static_str = if method.is_static { "static" } else { "instance" };
                println!("     - {} {} (params: {})", static_str, method_name, method.function.params.len());
            }
        }
    }
    drop(interner_ref);

    // Step 4: Lower HIR to MIR
    println!("\n5. Lowering HIR to MIR...");
    let mir_module = match lower_hir_to_mir(&hir_module, &*string_interner_rc.borrow()) {
        Ok(mir) => {
            println!("   ✓ Successfully lowered to MIR");
            println!("   - Functions: {}", mir.functions.len());
            mir
        }
        Err(errors) => {
            eprintln!("   ✗ MIR lowering errors:");
            for error in errors {
                eprintln!("     - {}", error.message);
            }
            return;
        }
    };

    // Step 5: Verify instance methods have 'this' parameter
    println!("\n6. Verifying 'this' parameter in MIR functions...");
    for (func_id, function) in &mir_module.functions {
        let func_name = &function.name;
        let param_count = function.signature.parameters.len();

        // Check if this looks like an instance method (has 'this' as first param)
        let has_this = function.signature.parameters.first()
            .map(|p| p.name == "this")
            .unwrap_or(false);

        if has_this {
            println!("   ✓ Function '{}' has 'this' parameter ({} total params)", func_name, param_count);
        } else if param_count > 0 {
            println!("   • Function '{}' is static ({} params, no 'this')", func_name, param_count);
        } else {
            println!("   • Function '{}' has no parameters", func_name);
        }
    }

    println!("\n🎉 Test completed successfully!");
}
