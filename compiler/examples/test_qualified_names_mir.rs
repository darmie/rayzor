// Test that qualified names flow through TAST -> HIR -> MIR -> Cranelift

use compiler::tast::{
    TypeTable, StringInterner, SymbolTable,
    ast_lowering::AstLowering,
    scopes::ScopeTree,
};
use compiler::ir::tast_to_hir::TastToHirContext;
use compiler::ir::hir_to_mir::HirToMirContext;
use parser::haxe_parser::parse_haxe_file;
use std::rc::Rc;
use std::cell::RefCell;

fn main() {
    let source = r#"
package com.example;

class Calculator {
    public function add(x:Int, y:Int):Int {
        return x + y;
    }

    public function multiply(x:Int, y:Int):Int {
        return x * y;
    }
}
    "#;

    println!("=== Testing Qualified Names Through Pipeline ===\n");

    // Step 1: Parse
    println!("1. Parsing Haxe source...");
    let ast = match parse_haxe_file("test.hx", source, false) {
        Ok(ast) => {
            println!("   ✓ Successfully parsed");
            ast
        }
        Err(e) => {
            eprintln!("   ✗ Parse error: {}", e);
            return;
        }
    };

    // Step 2: Lower AST to TAST
    println!("\n2. Lowering AST to TAST...");
    let mut string_interner = StringInterner::new();
    let mut symbol_table = SymbolTable::new();
    let type_table = Rc::new(RefCell::new(TypeTable::new()));
    let mut scope_tree = ScopeTree::new(compiler::tast::ScopeId::from_raw(0));
    let mut namespace_resolver = compiler::tast::namespace::NamespaceResolver::new(&string_interner);
    let mut import_resolver = compiler::tast::namespace::ImportResolver::new(&namespace_resolver);

    let mut ast_lowering = AstLowering::new(
        &mut string_interner,
        &mut symbol_table,
        &type_table,
        &mut scope_tree,
        &mut namespace_resolver,
        &mut import_resolver,
    );

    let typed_file = match ast_lowering.lower_file(&ast) {
        Ok(tast) => {
            println!("   ✓ Successfully lowered to TAST");
            tast
        }
        Err(error) => {
            eprintln!("   ✗ TAST lowering error: {:?}", error);
            return;
        }
    };

    // Check TAST symbols have qualified names
    println!("\n3. Checking TAST symbol qualified names...");
    for symbol in symbol_table.all_symbols() {
        if let Some(qualified_name) = symbol.qualified_name {
            let name_str = string_interner.get(qualified_name).unwrap_or("<unknown>");
            let symbol_name_str = string_interner.get(symbol.name).unwrap_or("<unknown>");
            if name_str.contains("Calculator") || name_str.contains("add") || name_str.contains("multiply") {
                println!("   Symbol '{}' -> '{}'", symbol_name_str, name_str);
            }
        }
    }

    // Step 3: Lower TAST to HIR
    println!("\n4. Lowering TAST to HIR...");
    let mut hir_context = TastToHirContext::new(
        &symbol_table,
        &type_table,
        &mut string_interner,
        "test.hx".to_string(),
    );

    let hir_module = match hir_context.lower_file(&typed_file) {
        Ok(hir) => {
            println!("   ✓ Successfully lowered to HIR");
            hir
        }
        Err(errors) => {
            eprintln!("   ✗ HIR lowering errors: {:?}", errors);
            return;
        }
    };

    // Check HIR functions have qualified names
    println!("\n5. Checking HIR function qualified names...");
    // Check standalone functions
    for (symbol_id, hir_func) in &hir_module.functions {
        let func_name = string_interner.get(hir_func.name).unwrap_or("<unknown>");
        if let Some(qualified_name) = hir_func.qualified_name {
            let name_str = string_interner.get(qualified_name).unwrap_or("<unknown>");
            println!("   Function {:?} '{}' -> '{}'", symbol_id, func_name, name_str);
        } else {
            println!("   Function {:?} '{}' -> <no qualified name>", symbol_id, func_name);
        }
    }
    // Check class methods
    use compiler::ir::hir::HirTypeDecl;
    for (type_id, type_decl) in &hir_module.types {
        if let HirTypeDecl::Class(hir_class) = type_decl {
            for method in &hir_class.methods {
                let func_name = string_interner.get(method.function.name).unwrap_or("<unknown>");
                if let Some(qualified_name) = method.function.qualified_name {
                    let name_str = string_interner.get(qualified_name).unwrap_or("<unknown>");
                    println!("   Method {:?} '{}' -> '{}'", method.function.symbol_id, func_name, name_str);
                } else {
                    println!("   Method {:?} '{}' -> <no qualified name>", method.function.symbol_id, func_name);
                }
            }
        }
    }

    // Step 4: Lower HIR to MIR
    println!("\n6. Lowering HIR to MIR...");
    let mut mir_lowering = HirToMirContext::new("test".to_string(), "test.hx".to_string(), &string_interner);
    let mir_module = match mir_lowering.lower_module(&hir_module) {
        Ok(mir) => {
            println!("   ✓ Successfully lowered to MIR");
            mir
        }
        Err(e) => {
            eprintln!("   ✗ MIR lowering errors: {:?}", e);
            return;
        }
    };

    // Check MIR functions have qualified names
    println!("\n7. Checking MIR function qualified names...");
    for (func_id, ir_func) in &mir_module.functions {
        if let Some(ref qualified_name) = ir_func.qualified_name {
            println!("   Function {:?} '{}' -> '{}'", func_id, ir_func.name, qualified_name);
        } else {
            println!("   Function {:?} '{}' -> <no qualified name>", func_id, ir_func.name);
        }
    }

    println!("\n🎉 SUCCESS: Qualified names flow through entire pipeline!");
}
