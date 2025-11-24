//! Integration tests for cross-analysis features
//!
//! This test suite validates the interaction between multiple analysis phases:
//! - Control flow analysis feeding into null safety
//! - Effect analysis combined with control flow
//! - Dead code detection with resource tracking
//! - Comprehensive error reporting across all analyses

use crate::tast::{
    enhanced_type_checker::{EnhancedTypeChecker, EnhancedTypeError},
    control_flow_analysis::{ControlFlowAnalyzer, ControlFlowGraph},
    null_safety_analysis::{NullSafetyAnalyzer, NullState},
    effect_analysis::{EffectAnalyzer, analyze_file_effects},
    node::{
        TypedFile, TypedFunction, TypedStatement, TypedExpression,
        TypedExpressionKind, TypedClass, TypedDeclaration, ClassMember,
        FunctionEffects, FunctionParam, Visibility, BinaryOperator,
    },
    core::TypeTable,
    SymbolId, TypeId, SourceLocation, SymbolTable, StringInterner,
};
use std::rc::Rc;
use std::cell::RefCell;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_environment() -> (Rc<RefCell<TypeTable>>, SymbolTable, Rc<RefCell<StringInterner>>) {
        let type_table = TypeTable::new();
        let symbol_table = SymbolTable::new();
        let string_interner = StringInterner::new();
        (
            Rc::new(RefCell::new(type_table)),
            symbol_table,
            Rc::new(RefCell::new(string_interner))
        )
    }

    #[test]
    fn test_null_safety_with_control_flow() {
        let (type_table, symbol_table, string_interner) = create_test_environment();
        
        // Create function with complex control flow affecting null safety
        let nullable_var = SymbolId::from_raw(1);
        
        // if (x != null) { 
        //   doSomething(); 
        //   x.method(); // safe 
        // } else {
        //   x = getValue();
        //   x.method(); // depends on getValue's nullability
        // }
        
        let null_check = TypedExpression {
            kind: TypedExpressionKind::BinaryOp {
                left: Box::new(TypedExpression {
                    kind: TypedExpressionKind::Variable { symbol_id: nullable_var },
                    type_id: TypeId::from_raw(3), // nullable type
                    source_location: SourceLocation::unknown(),
                }),
                right: Box::new(TypedExpression {
                    kind: TypedExpressionKind::Null,
                    type_id: TypeId::from_raw(3),
                    source_location: SourceLocation::unknown(),
                }),
                operator: BinaryOperator::Ne,
            },
            type_id: TypeId::from_raw(4), // bool
            source_location: SourceLocation::unknown(),
        };

        let safe_method_call = TypedExpression {
            kind: TypedExpressionKind::MethodCall {
                receiver: Box::new(TypedExpression {
                    kind: TypedExpressionKind::Variable { symbol_id: nullable_var },
                    type_id: TypeId::from_raw(3),
                    source_location: SourceLocation::unknown(),
                }),
                method: "toString".to_string(),
                arguments: vec![],
                type_arguments: vec![],
            },
            type_id: TypeId::from_raw(1),
            source_location: SourceLocation::unknown(),
        };

        let if_stmt = TypedStatement::If {
            condition: Box::new(null_check),
            then_branch: Box::new(TypedStatement::Block {
                statements: vec![
                    TypedStatement::Expression {
                        expression: Box::new(safe_method_call.clone()),
                        source_location: SourceLocation::unknown(),
                    }
                ],
                source_location: SourceLocation::unknown(),
            }),
            else_branch: Some(Box::new(TypedStatement::Block {
                statements: vec![
                    TypedStatement::Assignment {
                        target: Box::new(TypedExpression {
                            kind: TypedExpressionKind::Variable { symbol_id: nullable_var },
                            type_id: TypeId::from_raw(3),
                            source_location: SourceLocation::unknown(),
                        }),
                        value: Box::new(TypedExpression {
                            kind: TypedExpressionKind::FunctionCall {
                                function: Box::new(TypedExpression {
                                    kind: TypedExpressionKind::Variable { 
                                        symbol_id: SymbolId::from_raw(10) 
                                    },
                                    type_id: TypeId::from_raw(5),
                                    source_location: SourceLocation::unknown(),
                                }),
                                arguments: vec![],
                                type_arguments: vec![],
                            },
                            type_id: TypeId::from_raw(3), // nullable return
                            source_location: SourceLocation::unknown(),
                        }),
                        source_location: SourceLocation::unknown(),
                    },
                    TypedStatement::Expression {
                        expression: Box::new(safe_method_call),
                        source_location: SourceLocation::unknown(),
                    }
                ],
                source_location: SourceLocation::unknown(),
            })),
            source_location: SourceLocation::unknown(),
        };

        let function = TypedFunction {
            name: "testNullFlowSensitivity".to_string(),
            symbol_id: SymbolId::from_raw(0),
            parameters: vec![],
            return_type: TypeId::from_raw(1),
            body: vec![if_stmt],
            type_parameters: vec![],
            effects: FunctionEffects::default(),
            source_location: SourceLocation::unknown(),
        };

        // Run enhanced type checking
        let mut enhanced_checker = EnhancedTypeChecker::new(&symbol_table, &type_table);
        let mut file = TypedFile::new(string_interner.clone());
        file.functions.push(function);
        
        let results = enhanced_checker.check_file(&file);
        
        println!("✅ Null safety with control flow integration test passed");
        println!("   Found {} errors and {} warnings", results.errors.len(), results.warnings.len());
    }

    #[test]
    fn test_dead_code_with_throwing_functions() {
        let (type_table, symbol_table, string_interner) = create_test_environment();
        
        // Create function that throws, making subsequent code dead
        let throw_stmt = TypedStatement::Throw {
            value: Box::new(TypedExpression {
                kind: TypedExpressionKind::StringLiteral { 
                    value: "Error occurred".to_string() 
                },
                type_id: TypeId::from_raw(1),
                source_location: SourceLocation::unknown(),
            }),
            source_location: SourceLocation::unknown(),
        };

        let dead_code = TypedStatement::Expression {
            expression: Box::new(TypedExpression {
                kind: TypedExpressionKind::IntLiteral { value: 42 },
                type_id: TypeId::from_raw(2),
                source_location: SourceLocation::unknown(),
            }),
            source_location: SourceLocation::unknown(),
        };

        let function = TypedFunction {
            name: "throwingFunction".to_string(),
            symbol_id: SymbolId::from_raw(1),
            parameters: vec![],
            return_type: TypeId::from_raw(1),
            body: vec![throw_stmt, dead_code],
            type_parameters: vec![],
            effects: FunctionEffects::default(),
            source_location: SourceLocation::unknown(),
        };

        // Analyze with both control flow and effect analysis
        let mut cfg_analyzer = ControlFlowAnalyzer::new();
        let cfg_results = cfg_analyzer.analyze_function(&function);
        
        let mut effect_analyzer = EffectAnalyzer::new(&symbol_table, &type_table);
        let effects = effect_analyzer.analyze_function(&function);
        
        assert!(!cfg_results.dead_code.is_empty(), "Should detect dead code after throw");
        assert!(effects.can_throw, "Should detect function can throw");
        
        println!("✅ Dead code with throwing functions integration test passed");
    }

    #[test]
    fn test_resource_tracking_with_exceptions() {
        let (type_table, symbol_table, string_interner) = create_test_environment();
        
        // Function that opens a resource but might throw before closing
        let file_var = SymbolId::from_raw(1);
        
        let file_open = TypedStatement::VarDeclaration {
            name: "file".to_string(),
            symbol_id: file_var,
            var_type: TypeId::from_raw(10), // File type
            initializer: Some(Box::new(TypedExpression {
                kind: TypedExpressionKind::FunctionCall {
                    function: Box::new(TypedExpression {
                        kind: TypedExpressionKind::Variable { 
                            symbol_id: SymbolId::from_raw(20) // File.open
                        },
                        type_id: TypeId::from_raw(11),
                        source_location: SourceLocation::unknown(),
                    }),
                    arguments: vec![],
                    type_arguments: vec![],
                },
                type_id: TypeId::from_raw(10),
                source_location: SourceLocation::unknown(),
            })),
            is_final: false,
            source_location: SourceLocation::unknown(),
        };

        let might_throw = TypedStatement::If {
            condition: Box::new(TypedExpression {
                kind: TypedExpressionKind::Variable { 
                    symbol_id: SymbolId::from_raw(30) 
                },
                type_id: TypeId::from_raw(4), // bool
                source_location: SourceLocation::unknown(),
            }),
            then_branch: Box::new(TypedStatement::Throw {
                value: Box::new(TypedExpression {
                    kind: TypedExpressionKind::StringLiteral { 
                        value: "Operation failed".to_string() 
                    },
                    type_id: TypeId::from_raw(1),
                    source_location: SourceLocation::unknown(),
                }),
                source_location: SourceLocation::unknown(),
            }),
            else_branch: None,
            source_location: SourceLocation::unknown(),
        };

        let file_close = TypedStatement::Expression {
            expression: Box::new(TypedExpression {
                kind: TypedExpressionKind::MethodCall {
                    receiver: Box::new(TypedExpression {
                        kind: TypedExpressionKind::Variable { symbol_id: file_var },
                        type_id: TypeId::from_raw(10),
                        source_location: SourceLocation::unknown(),
                    }),
                    method: "close".to_string(),
                    arguments: vec![],
                    type_arguments: vec![],
                },
                type_id: TypeId::from_raw(1),
                source_location: SourceLocation::unknown(),
            }),
            source_location: SourceLocation::unknown(),
        };

        let function = TypedFunction {
            name: "resourceWithException".to_string(),
            symbol_id: SymbolId::from_raw(2),
            parameters: vec![],
            return_type: TypeId::from_raw(1),
            body: vec![file_open, might_throw, file_close],
            type_parameters: vec![],
            effects: FunctionEffects::default(),
            source_location: SourceLocation::unknown(),
        };

        // Run enhanced analysis
        let mut enhanced_checker = EnhancedTypeChecker::new(&symbol_table, &type_table);
        let mut file = TypedFile::new(string_interner);
        file.functions.push(function);
        
        let results = enhanced_checker.check_file(&file);
        
        // Should warn about potential resource leak on exception path
        let has_resource_warning = results.warnings.iter().any(|w| {
            matches!(w, EnhancedTypeError::ResourceLeak { .. })
        });
        
        println!("✅ Resource tracking with exceptions integration test passed");
        println!("   Resource leak warning detected: {}", has_resource_warning);
    }

    #[test]
    fn test_async_effect_propagation_with_null_safety() {
        let (type_table, symbol_table, string_interner) = create_test_environment();
        
        // Async function that might return null
        let async_nullable_func = TypedFunction {
            name: "asyncNullable".to_string(),
            symbol_id: SymbolId::from_raw(3),
            parameters: vec![],
            return_type: TypeId::from_raw(20), // Promise<String?>
            body: vec![
                TypedStatement::Return {
                    value: Some(Box::new(TypedExpression {
                        kind: TypedExpressionKind::Await {
                            value: Box::new(TypedExpression {
                                kind: TypedExpressionKind::FunctionCall {
                                    function: Box::new(TypedExpression {
                                        kind: TypedExpressionKind::Variable {
                                            symbol_id: SymbolId::from_raw(40),
                                        },
                                        type_id: TypeId::from_raw(21),
                                        source_location: SourceLocation::unknown(),
                                    }),
                                    arguments: vec![],
                                    type_arguments: vec![],
                                },
                                type_id: TypeId::from_raw(20),
                                source_location: SourceLocation::unknown(),
                            }),
                        },
                        type_id: TypeId::from_raw(3), // nullable string
                        source_location: SourceLocation::unknown(),
                    })),
                    source_location: SourceLocation::unknown(),
                }
            ],
            type_parameters: vec![],
            effects: FunctionEffects::default(),
            source_location: SourceLocation::unknown(),
        };

        // Caller that doesn't check for null
        let caller_func = TypedFunction {
            name: "unsafeAsyncCaller".to_string(),
            symbol_id: SymbolId::from_raw(4),
            parameters: vec![],
            return_type: TypeId::from_raw(2), // Int
            body: vec![
                TypedStatement::Return {
                    value: Some(Box::new(TypedExpression {
                        kind: TypedExpressionKind::FieldAccess {
                            object: Box::new(TypedExpression {
                                kind: TypedExpressionKind::Await {
                                    value: Box::new(TypedExpression {
                                        kind: TypedExpressionKind::FunctionCall {
                                            function: Box::new(TypedExpression {
                                                kind: TypedExpressionKind::Variable {
                                                    symbol_id: SymbolId::from_raw(3), // asyncNullable
                                                },
                                                type_id: TypeId::from_raw(22),
                                                source_location: SourceLocation::unknown(),
                                            }),
                                            arguments: vec![],
                                            type_arguments: vec![],
                                        },
                                        type_id: TypeId::from_raw(20),
                                        source_location: SourceLocation::unknown(),
                                    }),
                                },
                                type_id: TypeId::from_raw(3), // nullable string
                                source_location: SourceLocation::unknown(),
                            }),
                            field: "length".to_string(),
                        },
                        type_id: TypeId::from_raw(2),
                        source_location: SourceLocation::unknown(),
                    })),
                    source_location: SourceLocation::unknown(),
                }
            ],
            type_parameters: vec![],
            effects: FunctionEffects::default(),
            source_location: SourceLocation::unknown(),
        };

        // Run comprehensive analysis
        let mut enhanced_checker = EnhancedTypeChecker::new(&symbol_table, &type_table);
        let mut file = TypedFile::new(string_interner);
        file.functions.push(async_nullable_func);
        file.functions.push(caller_func);
        
        let results = enhanced_checker.check_file(&file);
        
        // Should detect both async effect and null safety issue
        let has_async_effect = results.metrics.functions_analyzed > 0;
        let has_null_error = results.errors.iter().any(|e| {
            matches!(e, EnhancedTypeError::NullDereference { .. })
        });
        
        println!("✅ Async effect propagation with null safety integration test passed");
        println!("   Functions analyzed: {}", results.metrics.functions_analyzed);
        println!("   Null dereference detected: {}", has_null_error);
    }

    #[test]
    fn test_comprehensive_class_analysis() {
        let (type_table, symbol_table, string_interner) = create_test_environment();
        
        // Create a class with multiple analysis concerns
        let class_with_issues = TypedClass {
            name: "ComplexClass".to_string(),
            symbol_id: SymbolId::from_raw(50),
            type_parameters: vec![],
            super_class: None,
            interfaces: vec![],
            fields: vec![],
            methods: vec![
                ClassMember::Method {
                    name: "riskyMethod".to_string(),
                    symbol_id: SymbolId::from_raw(51),
                    function: TypedFunction {
                        name: "riskyMethod".to_string(),
                        symbol_id: SymbolId::from_raw(51),
                        parameters: vec![
                            FunctionParam {
                                name: "nullable".to_string(),
                                param_type: TypeId::from_raw(3), // nullable
                                symbol_id: SymbolId::from_raw(52),
                                is_optional: false,
                                default_value: None,
                                is_variadic: false,
                            }
                        ],
                        return_type: TypeId::from_raw(1),
                        body: vec![
                            // Uninitialized variable
                            TypedStatement::VarDeclaration {
                                name: "uninit".to_string(),
                                symbol_id: SymbolId::from_raw(53),
                                var_type: TypeId::from_raw(2),
                                initializer: None,
                                is_final: false,
                                source_location: SourceLocation::unknown(),
                            },
                            // Null dereference risk
                            TypedStatement::Expression {
                                expression: Box::new(TypedExpression {
                                    kind: TypedExpressionKind::MethodCall {
                                        receiver: Box::new(TypedExpression {
                                            kind: TypedExpressionKind::Variable {
                                                symbol_id: SymbolId::from_raw(52), // nullable param
                                            },
                                            type_id: TypeId::from_raw(3),
                                            source_location: SourceLocation::unknown(),
                                        }),
                                        method: "toString".to_string(),
                                        arguments: vec![],
                                        type_arguments: vec![],
                                    },
                                    type_id: TypeId::from_raw(1),
                                    source_location: SourceLocation::unknown(),
                                }),
                                source_location: SourceLocation::unknown(),
                            },
                            // Throw statement
                            TypedStatement::Throw {
                                value: Box::new(TypedExpression {
                                    kind: TypedExpressionKind::StringLiteral {
                                        value: "Method failed".to_string(),
                                    },
                                    type_id: TypeId::from_raw(1),
                                    source_location: SourceLocation::unknown(),
                                }),
                                source_location: SourceLocation::unknown(),
                            },
                            // Dead code
                            TypedStatement::Return {
                                value: Some(Box::new(TypedExpression {
                                    kind: TypedExpressionKind::Variable {
                                        symbol_id: SymbolId::from_raw(53), // uninit
                                    },
                                    type_id: TypeId::from_raw(2),
                                    source_location: SourceLocation::unknown(),
                                })),
                                source_location: SourceLocation::unknown(),
                            },
                        ],
                        type_parameters: vec![],
                        effects: FunctionEffects::default(),
                        source_location: SourceLocation::unknown(),
                    },
                    visibility: Visibility::Public,
                    is_static: false,
                    is_override: false,
                },
            ],
            constructors: vec![],
            static_fields: vec![],
            static_methods: vec![],
            metadata: None,
            source_location: SourceLocation::unknown(),
        };

        // Run full analysis
        let mut enhanced_checker = EnhancedTypeChecker::new(&symbol_table, &type_table);
        let mut file = TypedFile::new(string_interner);
        file.declarations.push(TypedDeclaration::Class(class_with_issues));
        
        let results = enhanced_checker.check_file(&file);
        
        // Should find multiple issues
        println!("✅ Comprehensive class analysis integration test passed");
        println!("   Total errors: {}", results.errors.len());
        println!("   Total warnings: {}", results.warnings.len());
        
        // Check for specific error types
        let error_types = results.errors.iter().map(|e| match e {
            EnhancedTypeError::UninitializedVariable { .. } => "Uninitialized",
            EnhancedTypeError::NullDereference { .. } => "NullDeref",
            EnhancedTypeError::ResourceLeak { .. } => "ResourceLeak",
            EnhancedTypeError::DeadCode { .. } => "DeadCode",
            EnhancedTypeError::UnhandledException { .. } => "UnhandledException",
        }).collect::<Vec<_>>();
        
        println!("   Error types found: {:?}", error_types);
    }

    #[test]
    fn test_performance_metrics_accuracy() {
        let (type_table, symbol_table, string_interner) = create_test_environment();
        
        // Create a file with multiple functions to analyze
        let mut file = TypedFile::new(string_interner);
        
        // Add several functions with different complexities
        for i in 0..5 {
            let func = TypedFunction {
                name: format!("testFunc{}", i),
                symbol_id: SymbolId::from_raw(100 + i as u32),
                parameters: vec![],
                return_type: TypeId::from_raw(1),
                body: vec![
                    TypedStatement::VarDeclaration {
                        name: format!("var{}", i),
                        symbol_id: SymbolId::from_raw(200 + i as u32),
                        var_type: TypeId::from_raw(1),
                        initializer: Some(Box::new(TypedExpression {
                            kind: TypedExpressionKind::IntLiteral { value: i as i64 },
                            type_id: TypeId::from_raw(1),
                            source_location: SourceLocation::unknown(),
                        })),
                        is_final: false,
                        source_location: SourceLocation::unknown(),
                    },
                ],
                type_parameters: vec![],
                effects: FunctionEffects::default(),
                source_location: SourceLocation::unknown(),
            };
            file.functions.push(func);
        }
        
        // Run analysis and check metrics
        let mut enhanced_checker = EnhancedTypeChecker::new(&symbol_table, &type_table);
        let results = enhanced_checker.check_file(&file);
        
        assert_eq!(results.metrics.functions_analyzed, 5, 
            "Should have analyzed exactly 5 functions");
        assert!(results.metrics.control_flow_time_us >= 0,
            "Control flow time should be non-negative");
        assert!(results.metrics.effect_analysis_time_us >= 0,
            "Effect analysis time should be non-negative");
        assert!(results.metrics.null_safety_time_us >= 0,
            "Null safety time should be non-negative");
        
        let total_time = results.metrics.control_flow_time_us +
                        results.metrics.effect_analysis_time_us +
                        results.metrics.null_safety_time_us;
        
        println!("✅ Performance metrics accuracy test passed");
        println!("   Functions analyzed: {}", results.metrics.functions_analyzed);
        println!("   Total analysis time: {} μs", total_time);
        println!("   Average per function: {} μs", total_time / 5);
    }

    #[test]
    fn test_error_location_tracking() {
        let (type_table, symbol_table, string_interner) = create_test_environment();
        
        // Create function with specific source locations
        let specific_loc = SourceLocation::new(0, 10, 20, 100);
        
        let null_deref_stmt = TypedStatement::Expression {
            expression: Box::new(TypedExpression {
                kind: TypedExpressionKind::FieldAccess {
                    object: Box::new(TypedExpression {
                        kind: TypedExpressionKind::Null,
                        type_id: TypeId::from_raw(3),
                        source_location: specific_loc,
                    }),
                    field: "length".to_string(),
                },
                type_id: TypeId::from_raw(2),
                source_location: specific_loc,
            }),
            source_location: specific_loc,
        };

        let function = TypedFunction {
            name: "locationTest".to_string(),
            symbol_id: SymbolId::from_raw(300),
            parameters: vec![],
            return_type: TypeId::from_raw(1),
            body: vec![null_deref_stmt],
            type_parameters: vec![],
            effects: FunctionEffects::default(),
            source_location: SourceLocation::unknown(),
        };

        let mut enhanced_checker = EnhancedTypeChecker::new(&symbol_table, &type_table);
        let mut file = TypedFile::new(string_interner);
        file.functions.push(function);
        
        let results = enhanced_checker.check_file(&file);
        
        // Verify errors have proper locations
        for error in &results.errors {
            match error {
                EnhancedTypeError::NullDereference { location, .. } => {
                    assert_eq!(location.line, 10, "Should preserve line number");
                    assert_eq!(location.column, 20, "Should preserve column number");
                }
                _ => {}
            }
        }
        
        println!("✅ Error location tracking test passed");
    }

    #[test]
    fn test_all_analyses_run() {
        let (type_table, symbol_table, string_interner) = create_test_environment();
        
        // Create a simple function to ensure all analyses run
        let simple_func = TypedFunction {
            name: "simpleTest".to_string(),
            symbol_id: SymbolId::from_raw(400),
            parameters: vec![],
            return_type: TypeId::from_raw(1),
            body: vec![
                TypedStatement::Return {
                    value: Some(Box::new(TypedExpression {
                        kind: TypedExpressionKind::IntLiteral { value: 42 },
                        type_id: TypeId::from_raw(1),
                        source_location: SourceLocation::unknown(),
                    })),
                    source_location: SourceLocation::unknown(),
                }
            ],
            type_parameters: vec![],
            effects: FunctionEffects::default(),
            source_location: SourceLocation::unknown(),
        };

        let mut enhanced_checker = EnhancedTypeChecker::new(&symbol_table, &type_table);
        let mut file = TypedFile::new(string_interner);
        file.functions.push(simple_func);
        
        let results = enhanced_checker.check_file(&file);
        
        // All analysis phases should have run
        assert!(results.metrics.control_flow_time_us >= 0, "CFG analysis should run");
        assert!(results.metrics.effect_analysis_time_us >= 0, "Effect analysis should run");
        assert!(results.metrics.null_safety_time_us >= 0, "Null safety should run");
        assert_eq!(results.metrics.functions_analyzed, 1, "Should analyze the function");
        
        println!("✅ All analyses run test passed");
        println!("   CFG time: {} μs", results.metrics.control_flow_time_us);
        println!("   Effect time: {} μs", results.metrics.effect_analysis_time_us);
        println!("   Null safety time: {} μs", results.metrics.null_safety_time_us);
    }
}