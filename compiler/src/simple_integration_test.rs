//! Simple integration test to validate enhanced type checking fixes
//!
//! This directly tests the control flow analyzer to validate our integration gap fixes.

use crate::tast::{
    control_flow_analysis::ControlFlowAnalyzer,
    node::{TypedFunction, TypedStatement, TypedExpression, TypedExpressionKind, LiteralValue,
           Mutability, Visibility, FunctionEffects, BinaryOperator, VariableUsage, ExpressionMetadata},
    SymbolId, TypeId, SourceLocation, StringInterner,
};
use std::rc::Rc;
use std::cell::RefCell;

pub fn test_control_flow_analyzer_integration() -> bool {
    println!("=== Testing Control Flow Analyzer Integration ===");

    let string_interner = Rc::new(RefCell::new(StringInterner::new()));
    let func_name = string_interner.borrow_mut().intern("test");
    let x_symbol = SymbolId::from_raw(1);

    // Create function: function test(): Int { var x: Int; return x + 1; }
    let function = TypedFunction {
        symbol_id: SymbolId::from_raw(0),
        name: func_name,
        parameters: vec![],
        return_type: TypeId::from_raw(1), // Int
        body: vec![
            // var x: Int; (uninitialized)
            TypedStatement::VarDeclaration {
                symbol_id: x_symbol,
                var_type: TypeId::from_raw(1),
                initializer: None, // UNINITIALIZED!
                mutability: Mutability::Mutable,
                source_location: SourceLocation::new(0, 2, 5, 15),
            },
            // return x + 1; (use of uninitialized variable)
            TypedStatement::Return {
                value: Some(TypedExpression {
                    kind: TypedExpressionKind::BinaryOp {
                        left: Box::new(TypedExpression {
                            kind: TypedExpressionKind::Variable { symbol_id: x_symbol },
                            expr_type: TypeId::from_raw(1),
                            usage: VariableUsage::Copy,
                            lifetime_id: crate::tast::LifetimeId::from_raw(0),
                            source_location: SourceLocation::new(0, 3, 12, 20),
                            metadata: ExpressionMetadata::default(),
                        }),
                        right: Box::new(TypedExpression {
                            kind: TypedExpressionKind::Literal { value: LiteralValue::Int(1) },
                            expr_type: TypeId::from_raw(1),
                            usage: VariableUsage::Copy,
                            lifetime_id: crate::tast::LifetimeId::from_raw(0),
                            source_location: SourceLocation::new(0, 3, 16, 24),
                            metadata: ExpressionMetadata::default(),
                        }),
                        operator: BinaryOperator::Add,
                    },
                    expr_type: TypeId::from_raw(1),
                    usage: VariableUsage::Copy,
                    lifetime_id: crate::tast::LifetimeId::from_raw(0),
                    source_location: SourceLocation::new(0, 3, 10, 18),
                    metadata: ExpressionMetadata::default(),
                }),
                source_location: SourceLocation::new(0, 3, 5, 25),
            },
        ],
        type_parameters: vec![],
        effects: FunctionEffects::default(),
        source_location: SourceLocation::new(0, 1, 1, 1),
        visibility: Visibility::Public,
        is_static: false,
        metadata: crate::tast::node::FunctionMetadata::default(),
    };

    // Test control flow analyzer directly
    let mut analyzer = ControlFlowAnalyzer::new();
    let results = analyzer.analyze_function(&function);

    println!("Control Flow Analysis Results:");
    println!("- Uninitialized uses found: {}", results.uninitialized_uses.len());
    println!("- Dead code regions found: {}", results.dead_code.len());
    println!("- Null dereferences found: {}", results.null_dereferences.len());

    // Print detailed results
    for (i, uninit) in results.uninitialized_uses.iter().enumerate() {
        println!("  Uninitialized Use #{}: Variable {:?} at {}:{}",
            i + 1, uninit.variable, uninit.location.line, uninit.location.column);
    }

    // CRITICAL TEST: Check if we detected the uninitialized variable
    let detected_uninitialized = !results.uninitialized_uses.is_empty();

    if detected_uninitialized {
        let uninit_use = &results.uninitialized_uses[0];
        if uninit_use.variable == x_symbol {
            println!("🎉 SUCCESS: Detected uninitialized variable 'x' at line {}, column {}",
                uninit_use.location.line, uninit_use.location.column);
            println!("✅ Integration gap fixes are working correctly!");
            return true;
        } else {
            println!("⚠️  Detected uninitialized variable but wrong symbol");
            return false;
        }
    } else {
        println!("ℹ️  Control flow analyzer ran but didn't detect uninitialized variable");
        println!("   This indicates the integration fixes may need further refinement");
        return false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration_gap_fixes() {
        let success = test_control_flow_analyzer_integration();
        assert!(success, "Integration gap fixes should be working");
    }
}