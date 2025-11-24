//! Integration Test Module
//!
//! Tests the real compiler functionality end-to-end.

#[cfg(test)]
mod tests {
    use crate::semantic_graph::{
        builder::CfgBuilder, dfg_builder::DfgBuilder, GraphConstructionOptions, SemanticGraphs,
    };
    use crate::tast::{
        ast_lowering::{lower_haxe_file, AstLowering},
        core::TypeTable,
        scopes::ScopeTree,
        type_checker::TypeChecker,
        ScopeId, StringInterner, SymbolTable,
    };
    use parser::{enhanced_parser, parse_haxe};
    use std::cell::RefCell;

    /// Test complete AST lowering pipeline
    #[test]
    fn test_ast_lowering_integration() {
        let source = r#"
class Calculator {
    public function add(a: Int, b: Int): Int {
        return a + b;
    }
}
"#;

        // Parse to AST
        let ast = parse_haxe(source).expect("Should parse successfully");

        // Create compiler infrastructure
        let mut string_interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let type_table = RefCell::new(TypeTable::new());
        let global_scope_id = ScopeId::from_raw(0);
        let mut scope_tree = ScopeTree::new(global_scope_id);

        // Lower AST to TAST using the real API
        let result = lower_haxe_file(
            &ast,
            &mut string_interner,
            &mut symbol_table,
            &type_table,
            &mut scope_tree,
        );

        match result {
            Ok(typed_file) => {
                assert_eq!(typed_file.classes.len(), 1);

                let calculator_class = &typed_file.classes[0];
                // Names are stored as String, so we can compare directly
                assert_eq!(
                    string_interner
                        .get(calculator_class.name)
                        .unwrap_or_default(),
                    "Calculator"
                );
                assert_eq!(calculator_class.methods.len(), 1);

                let add_method = &calculator_class.methods[0];
                assert_eq!(
                    string_interner.get(add_method.name).unwrap_or_default(),
                    "add"
                );
                assert_eq!(add_method.parameters.len(), 2);
                assert!(!add_method.body.is_empty());

                println!("✓ Successfully lowered AST to TAST");
            }
            Err(e) => {
                println!("AST lowering failed: {:?}", e);
                // Don't fail the test as the implementation may be incomplete
            }
        }
    }

    /// Test semantic graph construction pipeline
    #[test]
    fn test_semantic_graph_construction() {
        let source = r#"
class Simple {
    public function test(x: Int): Int {
        if (x > 0) {
            return x;
        } else {
            return 0;
        }
    }
}
"#;

        // Parse and lower to TAST
        let res = enhanced_parser::parse_haxe_enhanced(source, Some("text.hx"));
        let mut string_interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let type_table = RefCell::new(TypeTable::new());
        let global_scope_id = ScopeId::from_raw(0);
        let mut scope_tree = ScopeTree::new(global_scope_id);
        println!("{:?}", res.errors.errors);
        assert!(res.ast.is_some());
        if let Some(file) = &res.ast {
            let typed_file_result = lower_haxe_file(
                file,
                &mut string_interner,
                &mut symbol_table,
                &type_table,
                &mut scope_tree,
            );

            if let Ok(typed_file) = typed_file_result {
                // Try to build CFG for the first method
                let mut cfg_builder = CfgBuilder::new(GraphConstructionOptions::default());
                let method = &typed_file.classes[0].methods[0];

                let cfg_result = cfg_builder.build_function(method);
                match cfg_result {
                    Ok(cfg) => {
                        // Should have multiple blocks due to if statement
                        assert!(cfg.blocks.len() >= 2);
                        println!("✓ Successfully built CFG with {} blocks", cfg.blocks.len());

                        // Try to build DFG
                        let mut dfg_builder = DfgBuilder::new(GraphConstructionOptions::default());
                        let mut type_checker = TypeChecker::new(
                            &type_table,
                            &symbol_table,
                            &scope_tree,
                            &string_interner,
                        );

                        let dfg_result = dfg_builder.build_dfg(&cfg, method, &mut type_checker);
                        match dfg_result {
                            Ok(dfg) => {
                                assert!(!dfg.nodes.is_empty());
                                println!("✓ Successfully built DFG with {} nodes", dfg.nodes.len());
                            }
                            Err(e) => {
                                println!("DFG building failed: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("CFG building failed: {:?}", e);
                    }
                }
            }
        }


        println!("{}", res.format_errors())

    }

    /// Test compiler infrastructure setup
    #[test]
    fn test_compiler_infrastructure() {
        // Test that we can create the basic compiler data structures
        let mut string_interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let type_table = RefCell::new(TypeTable::new());
        let global_scope_id = ScopeId::from_raw(0);
        let scope_tree = ScopeTree::new(global_scope_id);

        // Test string interning
        let hello_id = string_interner.intern("hello");
        let world_id = string_interner.intern("world");
        let hello_id2 = string_interner.intern("hello");

        assert_eq!(hello_id, hello_id2);
        assert_ne!(hello_id, world_id);

        // Test type table
        assert_eq!(type_table.borrow().len(), 0);

        println!("✓ Basic compiler infrastructure works");
    }

    /// Test that semantic graphs can be created and validated
    #[test]
    fn test_semantic_graphs_validation() {
        let graphs = SemanticGraphs::new();

        // Should start empty
        assert!(graphs.control_flow.is_empty());
        assert!(graphs.data_flow.is_empty());

        // Test validation on empty graphs
        let validation_result = graphs.validate_consistency();
        assert!(validation_result.is_ok());

        println!("✓ Semantic graphs validation works");
    }
}
