// Comprehensive Test Suite for Phi Type Unification
// Tests various scenarios including edge cases and complex type hierarchies

#[cfg(test)]
mod phi_type_unification_tests {
    use super::*;

    use crate::semantic_graph::cfg::{BasicBlock, ControlFlowGraph};

    use crate::semantic_graph::dfg::{DataFlowNode, DataFlowNodeKind, PhiIncoming};
    use crate::semantic_graph::phi_type::DfgBuilderPhiTypeUnification;
    use crate::semantic_graph::{dfg_builder::*, GraphConstructionError, GraphConstructionOptions};
    use crate::tast::collections::new_id_set;
    use crate::tast::type_checker::TypeChecker;
    use crate::tast::{core::*, SourceLocation};
    use crate::tast::{BlockId, DataFlowNodeId, SsaVariableId, SymbolId, TypeId};
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Helper to create a test DFG with phi nodes
    struct TestDfgBuilder {
        dfg_builder: DfgBuilder,
        type_table: RefCell<TypeTable>,
        node_counter: u32,
    }

    impl TestDfgBuilder {
        fn new() -> Self {
            Self {
                dfg_builder: DfgBuilder::new(GraphConstructionOptions::default()),
                type_table: RefCell::new(TypeTable::new()),
                node_counter: 100, // Start from 100 to avoid conflicts
            }
        }

        /// Create a value node with specific type
        fn create_value_node(&mut self, value_type: TypeId, block: BlockId) -> DataFlowNodeId {
            let node_id = DataFlowNodeId::from_raw(self.node_counter);
            self.node_counter += 1;

            let node = DataFlowNode {
                id: node_id,
                kind: DataFlowNodeKind::Constant {
                    value: crate::semantic_graph::dfg::ConstantValue::Int(42),
                },
                value_type,
                source_location: SourceLocation::unknown(),
                operands: vec![],
                uses: new_id_set(),
                defines: Some(SsaVariableId::from_raw(self.node_counter)),
                basic_block: block,
                metadata: Default::default(),
            };

            self.dfg_builder.dfg.add_node(node);
            node_id
        }

        /// Create a phi node with given operands
        fn create_phi_node(
            &mut self,
            operands: Vec<(DataFlowNodeId, BlockId)>,
            block: BlockId,
        ) -> DataFlowNodeId {
            let node_id = DataFlowNodeId::from_raw(self.node_counter);
            self.node_counter += 1;

            let incoming: Vec<PhiIncoming> = operands
                .into_iter()
                .map(|(value, predecessor)| PhiIncoming { value, predecessor })
                .collect();

            let node = DataFlowNode {
                id: node_id,
                kind: DataFlowNodeKind::Phi {
                    incoming: incoming.clone(),
                },
                value_type: TypeId::invalid(), // Will be resolved
                source_location: SourceLocation::unknown(),
                operands: incoming.iter().map(|inc| inc.value).collect(),
                uses: new_id_set(),
                defines: Some(SsaVariableId::from_raw(self.node_counter)),
                basic_block: block,
                metadata: Default::default(),
            };

            self.dfg_builder.dfg.add_node(node);
            node_id
        }
    }

    #[test]
    fn test_simple_phi_unification_identical_types() {
        let mut test_builder = TestDfgBuilder::new();

        let int_type = test_builder.type_table.borrow().int_type();

        // Create two int value nodes
        let val1 = test_builder.create_value_node(int_type, BlockId::from_raw(1));
        let val2 = test_builder.create_value_node(int_type, BlockId::from_raw(2));

        // Create phi node
        let phi_id = test_builder.create_phi_node(
            vec![(val1, BlockId::from_raw(1)), (val2, BlockId::from_raw(2))],
            BlockId::from_raw(3),
        );

        // Setup type checker
        let symbol_table = crate::tast::SymbolTable::new();
        let scope_tree = crate::tast::ScopeTree::new(crate::tast::ScopeId::first());
        let string_interner = crate::tast::StringInterner::new();
        let type_checker = &mut TypeChecker::new(
            &test_builder.type_table,
            &symbol_table,
            &scope_tree,
            &string_interner,
        );

        // Get phi operands
        let phi_node = test_builder.dfg_builder.dfg.nodes.get(&phi_id).unwrap();
        if let DataFlowNodeKind::Phi { incoming } = &phi_node.kind {
            let unified_type = test_builder
                .dfg_builder
                .resolve_and_validate_phi_operand_types(incoming, type_checker)
                .unwrap();

            assert_eq!(unified_type, int_type);
        }
    }

    #[test]
    fn test_phi_unification_numeric_types() {
        let mut test_builder = TestDfgBuilder::new();
        let int_type = test_builder.type_table.borrow().int_type();
        let float_type = test_builder.type_table.borrow().float_type();

        // Create int and float value nodes
        let int_val = test_builder.create_value_node(int_type, BlockId::from_raw(1));
        let float_val = test_builder.create_value_node(float_type, BlockId::from_raw(2));

        // Create phi node
        let phi_id = test_builder.create_phi_node(
            vec![
                (int_val, BlockId::from_raw(1)),
                (float_val, BlockId::from_raw(2)),
            ],
            BlockId::from_raw(3),
        );

        // Setup type checker
        let symbol_table = crate::tast::SymbolTable::new();
        let scope_tree = crate::tast::ScopeTree::new(crate::tast::ScopeId::first());
        let string_interner = crate::tast::StringInterner::new();
        let mut type_checker = TypeChecker::new(
            &test_builder.type_table,
            &symbol_table,
            &scope_tree,
            &string_interner,
        );

        // Get phi operands and unify
        let phi_node = test_builder.dfg_builder.dfg.nodes.get(&phi_id).unwrap();
        if let DataFlowNodeKind::Phi { incoming } = &phi_node.kind {
            let unified_type = test_builder
                .dfg_builder
                .resolve_and_validate_phi_operand_types(incoming, &mut type_checker)
                .unwrap();

            // Should widen to float
            assert_eq!(unified_type, float_type);
        }
    }

    #[test]
    fn test_phi_unification_with_optional() {
        let mut test_builder = TestDfgBuilder::new();
        let int_type = test_builder.type_table.borrow().int_type();
        let optional_int = test_builder
            .type_table
            .borrow_mut()
            .create_optional_type(int_type);

        // Create int and optional<int> value nodes
        let int_val = test_builder.create_value_node(int_type, BlockId::from_raw(1));
        let opt_val = test_builder.create_value_node(optional_int, BlockId::from_raw(2));

        // Create phi node
        let phi_id = test_builder.create_phi_node(
            vec![
                (int_val, BlockId::from_raw(1)),
                (opt_val, BlockId::from_raw(2)),
            ],
            BlockId::from_raw(3),
        );

        // Setup type checker
        let symbol_table = crate::tast::SymbolTable::new();
        let scope_tree = crate::tast::ScopeTree::new(crate::tast::ScopeId::first());
        let string_interner = crate::tast::StringInterner::new();
        let mut type_checker = TypeChecker::new(
            &test_builder.type_table,
            &symbol_table,
            &scope_tree,
            &string_interner,
        );

        // Get phi operands and unify
        let phi_node = test_builder.dfg_builder.dfg.nodes.get(&phi_id).unwrap();
        if let DataFlowNodeKind::Phi { incoming } = &phi_node.kind {
            let unified_type = test_builder
                .dfg_builder
                .resolve_and_validate_phi_operand_types(incoming, &mut type_checker)
                .unwrap();

            // Should widen to optional<int>
            assert_eq!(unified_type, optional_int);
        }
    }

    #[test]
    fn test_phi_unification_multiple_operands() {
        let mut test_builder = TestDfgBuilder::new();
        let int_type = test_builder.type_table.borrow().int_type();
        let float_type = test_builder.type_table.borrow().float_type();
        let optional_float = test_builder
            .type_table
            .borrow_mut()
            .create_optional_type(float_type);

        // Create nodes with different types
        let int_val = test_builder.create_value_node(int_type, BlockId::from_raw(1));
        let float_val = test_builder.create_value_node(float_type, BlockId::from_raw(2));
        let opt_float_val = test_builder.create_value_node(optional_float, BlockId::from_raw(3));

        // Create phi node with three operands
        let phi_id = test_builder.create_phi_node(
            vec![
                (int_val, BlockId::from_raw(1)),
                (float_val, BlockId::from_raw(2)),
                (opt_float_val, BlockId::from_raw(3)),
            ],
            BlockId::from_raw(4),
        );

        // Setup type checker
        let symbol_table = crate::tast::SymbolTable::new();
        let scope_tree = crate::tast::ScopeTree::new(crate::tast::ScopeId::first());
        let string_interner = crate::tast::StringInterner::new();
        let mut type_checker = TypeChecker::new(
            &test_builder.type_table,
            &symbol_table,
            &scope_tree,
            &string_interner,
        );

        // Get phi operands and unify
        let phi_node = test_builder.dfg_builder.dfg.nodes.get(&phi_id).unwrap();
        if let DataFlowNodeKind::Phi { incoming } = &phi_node.kind {
            let unified_type = test_builder
                .dfg_builder
                .resolve_and_validate_phi_operand_types(incoming, &mut type_checker)
                .unwrap();

            // Should widen to optional<float>
            assert_eq!(unified_type, optional_float);
        }
    }

    #[test]
    fn test_phi_unification_with_dynamic() {
        let mut test_builder = TestDfgBuilder::new();
        let int_type = test_builder.type_table.borrow().int_type();
        let string_type = test_builder.type_table.borrow().string_type();
        let dynamic_type = test_builder.type_table.borrow().dynamic_type();

        // Create nodes with incompatible types and dynamic
        let int_val = test_builder.create_value_node(int_type, BlockId::from_raw(1));
        let string_val = test_builder.create_value_node(string_type, BlockId::from_raw(2));
        let dynamic_val = test_builder.create_value_node(dynamic_type, BlockId::from_raw(3));

        // Create phi node
        let phi_id = test_builder.create_phi_node(
            vec![
                (int_val, BlockId::from_raw(1)),
                (string_val, BlockId::from_raw(2)),
                (dynamic_val, BlockId::from_raw(3)),
            ],
            BlockId::from_raw(4),
        );

        // Setup type checker
        let symbol_table = crate::tast::SymbolTable::new();
        let scope_tree = crate::tast::ScopeTree::new(crate::tast::ScopeId::first());
        let string_interner = crate::tast::StringInterner::new();
        let mut type_checker = TypeChecker::new(
            &test_builder.type_table,
            &symbol_table,
            &scope_tree,
            &string_interner,
        );

        // Get phi operands and unify
        let phi_node = test_builder.dfg_builder.dfg.nodes.get(&phi_id).unwrap();
        if let DataFlowNodeKind::Phi { incoming } = &phi_node.kind {
            let unified_type = test_builder
                .dfg_builder
                .resolve_and_validate_phi_operand_types(
                    incoming,
                  
                    &mut type_checker,
                )
                .unwrap();

            // Should unify to dynamic (supertype of all)
            assert_eq!(unified_type, dynamic_type);
        }
    }

    #[test]
    fn test_phi_unification_creates_union() {
        let mut test_builder = TestDfgBuilder::new();
        let int_type = test_builder.type_table.borrow().int_type();
        let string_type = test_builder.type_table.borrow().string_type();

        // Create nodes with incompatible types
        let int_val = test_builder.create_value_node(int_type, BlockId::from_raw(1));
        let string_val = test_builder.create_value_node(string_type, BlockId::from_raw(2));

        // Create phi node
        let phi_id = test_builder.create_phi_node(
            vec![
                (int_val, BlockId::from_raw(1)),
                (string_val, BlockId::from_raw(2)),
            ],
            BlockId::from_raw(3),
        );

        // Setup type checker
        let symbol_table = crate::tast::SymbolTable::new();
        let scope_tree = crate::tast::ScopeTree::new(crate::tast::ScopeId::first());
        let string_interner = crate::tast::StringInterner::new();
        let mut type_checker = TypeChecker::new(
            &test_builder.type_table,
            &symbol_table,
            &scope_tree,
            &string_interner,
        );

        // Get phi operands and unify
        let phi_node = test_builder.dfg_builder.dfg.nodes.get(&phi_id).unwrap();
        if let DataFlowNodeKind::Phi { incoming } = &phi_node.kind {
            let unified_type = test_builder
                .dfg_builder
                .resolve_and_validate_phi_operand_types(
                    incoming,
                    
                    &mut type_checker,
                )
                .unwrap();

            // Should create a union type
            if let Some(unified) = test_builder.type_table.borrow().get(unified_type) {
                match &unified.kind {
                    TypeKind::Union { types } => {
                        assert_eq!(types.len(), 2);
                        assert!(types.contains(&int_type));
                        assert!(types.contains(&string_type));
                    }
                    _ => panic!("Expected union type, got {:?}", unified.kind),
                }
            }
        }
    }

    #[test]
    fn test_phi_validation_catches_incompatible_operands() {
        let mut test_builder = TestDfgBuilder::new();
        let int_type = test_builder.type_table.borrow().int_type();
        let string_type = test_builder.type_table.borrow().string_type();

        // Create nodes with incompatible strict types
        let int_val = test_builder.create_value_node(int_type, BlockId::from_raw(1));
        let string_val = test_builder.create_value_node(string_type, BlockId::from_raw(2));

        // Setup type checker
        let symbol_table = crate::tast::SymbolTable::new();
        let scope_tree = crate::tast::ScopeTree::new(crate::tast::ScopeId::first());
        let string_interner = crate::tast::StringInterner::new();
        let mut type_checker = TypeChecker::new(
            &test_builder.type_table,
            &symbol_table,
            &scope_tree,
            &string_interner,
        );

        // Create phi operands
        let phi_operands = vec![
            PhiIncoming {
                value: int_val,
                predecessor: BlockId::from_raw(1),
            },
            PhiIncoming {
                value: string_val,
                predecessor: BlockId::from_raw(2),
            },
        ];

        // The unification should succeed (creating a union), but we can test
        // that the unified type properly represents both types
        let unified_type = test_builder
            .dfg_builder
            .resolve_and_validate_phi_operand_types(
                &phi_operands,
                &mut type_checker,
            )
            .unwrap();

        // Verify it created a union type
        if let Some(unified) = test_builder.type_table.borrow().get(unified_type) {
            match &unified.kind {
                TypeKind::Union { types } => {
                    assert!(types.contains(&int_type));
                    assert!(types.contains(&string_type));
                }
                _ => panic!("Expected union type for incompatible types"),
            }
        };
    }

    #[test]
    fn test_empty_phi_operands_error() {
        let test_builder = TestDfgBuilder::new();

        // Setup type checker
        let symbol_table = crate::tast::SymbolTable::new();
        let scope_tree = crate::tast::ScopeTree::new(crate::tast::ScopeId::first());
        let string_interner = crate::tast::StringInterner::new();
        let mut type_checker = TypeChecker::new(
            &test_builder.type_table,
            &symbol_table,
            &scope_tree,
            &string_interner,
        );

        // Try to unify empty phi operands
        let result = test_builder.dfg_builder.resolve_and_validate_phi_operand_types(
            &[],
            &mut type_checker,
        );

        assert!(result.is_err());
        if let Err(e) = result {
            match e {
                GraphConstructionError::InternalError { message } => {
                    assert!(message.contains("no operands"));
                }
                _ => panic!("Expected InternalError"),
            }
        }
    }
}
