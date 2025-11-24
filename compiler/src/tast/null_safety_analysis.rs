//! Null safety analysis for preventing null pointer exceptions
//!
//! This module provides comprehensive null safety analysis by tracking nullable values
//! through control flow and detecting potential null dereferences.

use crate::tast::{
    node::{TypedExpression, TypedExpressionKind, TypedStatement, TypedFunction, BinaryOperator},
    control_flow_analysis::{ControlFlowGraph, BlockId, VariableState},
    SymbolId, TypeId, SourceLocation, TypeTable, SymbolTable,
    core::TypeKind,
};
use std::collections::{HashMap, HashSet};
use std::cell::RefCell;

/// Null state of a variable or expression
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullState {
    /// Definitely null
    Null,
    /// Definitely not null  
    NotNull,
    /// Maybe null (unknown nullability)
    MaybeNull,
    /// Uninitialized (treated as potentially null)
    Uninitialized,
}

/// Information about a null check in the code
#[derive(Debug, Clone)]
pub struct NullCheck {
    /// Variable being checked
    pub variable: SymbolId,
    /// Whether this is a null check (x == null) or non-null check (x != null)
    pub is_null_check: bool,
    /// Location of the check
    pub location: SourceLocation,
}

/// Null safety violation
#[derive(Debug, Clone)]
pub struct NullSafetyViolation {
    /// Variable that might be null
    pub variable: SymbolId,
    /// Type of violation
    pub violation_kind: NullViolationKind,
    /// Location where violation occurs
    pub location: SourceLocation,
    /// Suggested fix
    pub suggestion: Option<String>,
}

/// Types of null safety violations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NullViolationKind {
    /// Dereferencing a potentially null value
    PotentialNullDereference,
    /// Calling method on potentially null value
    PotentialNullMethodCall,
    /// Accessing field of potentially null value
    PotentialNullFieldAccess,
    /// Array access on potentially null array
    PotentialNullArrayAccess,
    /// Passing null to non-nullable parameter
    NullArgumentToNonNullable,
    /// Returning null from non-nullable function
    NullReturnFromNonNullable,
}

/// Null safety analyzer
pub struct NullSafetyAnalyzer<'a> {
    /// Type table for checking nullable types
    type_table: &'a RefCell<TypeTable>,
    /// Symbol table for variable information
    symbol_table: &'a SymbolTable,
    /// Control flow graph
    cfg: &'a ControlFlowGraph,
    /// Null states for each variable at each program point
    null_states: HashMap<BlockId, HashMap<SymbolId, NullState>>,
    /// Detected violations
    violations: Vec<NullSafetyViolation>,
    /// Null checks found in the code
    null_checks: HashMap<BlockId, Vec<NullCheck>>,
}

impl<'a> NullSafetyAnalyzer<'a> {
    /// Create a new null safety analyzer
    pub fn new(
        type_table: &'a RefCell<TypeTable>,
        symbol_table: &'a SymbolTable,
        cfg: &'a ControlFlowGraph,
    ) -> Self {
        Self {
            type_table,
            symbol_table,
            cfg,
            null_states: HashMap::new(),
            violations: Vec::new(),
            null_checks: HashMap::new(),
        }
    }
    
    /// Analyze null safety for a function
    pub fn analyze_function(&mut self, function: &TypedFunction) -> Vec<NullSafetyViolation> {
        // Initialize null states for function parameters
        self.initialize_parameter_states(function);
        
        // Find all null checks in the function
        self.find_null_checks(function);
        
        // Perform data flow analysis to track null states
        self.analyze_null_flow();
        
        // Check for violations
        self.check_violations(function);
        
        std::mem::take(&mut self.violations)
    }
    
    /// Initialize null states for function parameters
    fn initialize_parameter_states(&mut self, function: &TypedFunction) {
        let entry_block = self.cfg.entry_block;
        
        // Collect parameter states first
        let mut param_states = HashMap::new();
        for param in &function.parameters {
            let null_state = if self.is_nullable_type(param.param_type) {
                NullState::MaybeNull
            } else {
                NullState::NotNull
            };
            param_states.insert(param.symbol_id, null_state);
        }
        
        // Then insert them into null_states
        let entry_states = self.null_states.entry(entry_block).or_insert_with(HashMap::new);
        entry_states.extend(param_states);
    }
    
    /// Check if a type is nullable
    fn is_nullable_type(&self, type_id: TypeId) -> bool {
        let type_table = self.type_table.borrow();
        if let Some(type_info) = type_table.get(type_id) {
            match &type_info.kind {
                TypeKind::Optional { .. } => true,
                TypeKind::Dynamic => true, // Dynamic is always potentially null
                TypeKind::Class { .. } => true, // Class instances can be null in Haxe
                TypeKind::Interface { .. } => true,
                TypeKind::Array { .. } => true, // Arrays can be null
                TypeKind::Map { .. } => true, // Maps can be null
                TypeKind::Function { .. } => true, // Functions can be null
                // Primitives are generally not nullable unless optional
                TypeKind::Int | TypeKind::Float | TypeKind::Bool | TypeKind::String => false,
                _ => false,
            }
        } else {
            true // Unknown types are assumed nullable for safety
        }
    }
    
    /// Find all null checks in the function
    fn find_null_checks(&mut self, function: &TypedFunction) {
        for statement in &function.body {
            self.find_null_checks_in_statement(statement);
        }
    }
    
    /// Find null checks in a statement
    fn find_null_checks_in_statement(&mut self, statement: &TypedStatement) {
        match statement {
            TypedStatement::Expression { expression, .. } => {
                self.find_null_checks_in_expression(expression);
            }
            TypedStatement::If { condition, then_branch, else_branch, .. } => {
                self.find_null_checks_in_expression(condition);
                self.find_null_checks_in_statement(then_branch);
                if let Some(else_stmt) = else_branch {
                    self.find_null_checks_in_statement(else_stmt);
                }
            }
            TypedStatement::While { condition, body, .. } => {
                self.find_null_checks_in_expression(condition);
                self.find_null_checks_in_statement(body);
            }
            TypedStatement::Block { statements, .. } => {
                for stmt in statements {
                    self.find_null_checks_in_statement(stmt);
                }
            }
            _ => {
                // Handle other statement types
            }
        }
    }
    
    /// Find null checks in an expression
    fn find_null_checks_in_expression(&mut self, expression: &TypedExpression) {
        match &expression.kind {
            TypedExpressionKind::BinaryOp { left, right, operator } => {
                // Look for null equality checks: x == null, x != null
                if matches!(operator, BinaryOperator::Eq | BinaryOperator::Ne) {
                    if let (Some(var_id), true) = (
                        self.extract_variable_from_expression(left),
                        self.is_null_literal(right)
                    ) {
                        let null_check = NullCheck {
                            variable: var_id,
                            is_null_check: matches!(operator, BinaryOperator::Eq),
                            location: expression.source_location,
                        };
                        
                        // For now, we associate null checks with the entry block
                        // A more sophisticated implementation would track which block the check is in
                        self.null_checks.entry(self.cfg.entry_block)
                            .or_insert_with(Vec::new)
                            .push(null_check);
                    } else if let (Some(var_id), true) = (
                        self.extract_variable_from_expression(right),
                        self.is_null_literal(left)
                    ) {
                        let null_check = NullCheck {
                            variable: var_id,
                            is_null_check: matches!(operator, BinaryOperator::Eq),
                            location: expression.source_location,
                        };
                        
                        self.null_checks.entry(self.cfg.entry_block)
                            .or_insert_with(Vec::new)
                            .push(null_check);
                    }
                }
                
                // Recursively check operands
                self.find_null_checks_in_expression(left);
                self.find_null_checks_in_expression(right);
            }
            
            TypedExpressionKind::FieldAccess { object, .. } => {
                self.find_null_checks_in_expression(object);
            }
            
            TypedExpressionKind::MethodCall { receiver, arguments, .. } => {
                self.find_null_checks_in_expression(receiver);
                for arg in arguments {
                    self.find_null_checks_in_expression(arg);
                }
            }
            
            TypedExpressionKind::FunctionCall { function, arguments, .. } => {
                self.find_null_checks_in_expression(function);
                for arg in arguments {
                    self.find_null_checks_in_expression(arg);
                }
            }
            
            _ => {
                // Handle other expression types
            }
        }
    }
    
    /// Check if an expression is a null literal
    fn is_null_literal(&self, expression: &TypedExpression) -> bool {
        matches!(expression.kind, TypedExpressionKind::Null)
    }
    
    /// Extract variable symbol from expression
    fn extract_variable_from_expression(&self, expression: &TypedExpression) -> Option<SymbolId> {
        match &expression.kind {
            TypedExpressionKind::Variable { symbol_id } => Some(*symbol_id),
            _ => None,
        }
    }
    
    /// Perform data flow analysis to track null states
    fn analyze_null_flow(&mut self) {
        // Iterative data flow analysis
        let mut changed = true;
        while changed {
            changed = false;
            
            let block_ids: Vec<_> = self.cfg.blocks.keys().copied().collect();
            for block_id in block_ids {
                if self.update_null_states(block_id) {
                    changed = true;
                }
            }
        }
    }
    
    /// Update null states for a block
    fn update_null_states(&mut self, block_id: BlockId) -> bool {
        let mut changed = false;
        
        // Get predecessors and merge their exit states
        if let Some(block) = self.cfg.blocks.get(&block_id) {
            let predecessors = block.predecessors.clone();
            let mut merged_states = HashMap::new();
            
            for &pred_id in &predecessors {
                if let Some(pred_states) = self.null_states.get(&pred_id) {
                    for (&var_id, &state) in pred_states {
                        let current_state = merged_states.get(&var_id).cloned().unwrap_or(NullState::Uninitialized);
                        let merged_state = self.merge_null_states(current_state, state);
                        merged_states.insert(var_id, merged_state);
                    }
                }
            }
            
            // Apply null checks in this block
            if let Some(checks) = self.null_checks.get(&block_id) {
                for check in checks {
                    if check.is_null_check {
                        merged_states.insert(check.variable, NullState::Null);
                    } else {
                        merged_states.insert(check.variable, NullState::NotNull);
                    }
                }
            }
            
            // Check if states changed
            if self.null_states.get(&block_id) != Some(&merged_states) {
                self.null_states.insert(block_id, merged_states);
                changed = true;
            }
        }
        
        changed
    }
    
    /// Merge two null states
    fn merge_null_states(&self, state1: NullState, state2: NullState) -> NullState {
        match (state1, state2) {
            (NullState::Null, NullState::Null) => NullState::Null,
            (NullState::NotNull, NullState::NotNull) => NullState::NotNull,
            (NullState::Uninitialized, other) | (other, NullState::Uninitialized) => other,
            _ => NullState::MaybeNull, // Any mismatch becomes maybe null
        }
    }
    
    /// Check for null safety violations
    fn check_violations(&mut self, function: &TypedFunction) {
        for statement in &function.body {
            self.check_violations_in_statement(statement, function);
        }
    }
    
    /// Check for violations in a statement
    fn check_violations_in_statement(&mut self, statement: &TypedStatement, function: &TypedFunction) {
        match statement {
            TypedStatement::Expression { expression, .. } => {
                self.check_violations_in_expression(expression);
            }
            TypedStatement::Assignment { target, value, .. } => {
                self.check_violations_in_expression(target);
                self.check_violations_in_expression(value);
            }
            TypedStatement::If { condition, then_branch, else_branch, .. } => {
                self.check_violations_in_expression(condition);
                self.check_violations_in_statement(then_branch, function);
                if let Some(else_stmt) = else_branch {
                    self.check_violations_in_statement(else_stmt, function);
                }
            }
            TypedStatement::While { condition, body, .. } => {
                self.check_violations_in_expression(condition);
                self.check_violations_in_statement(body, function);
            }
            TypedStatement::Return { value, .. } => {
                if let Some(val) = value {
                    self.check_violations_in_expression(val);
                    
                    // Check if returning null from non-nullable function
                    if self.is_null_literal(val) && !self.is_nullable_type(function.return_type) {
                        self.violations.push(NullSafetyViolation {
                            variable: SymbolId::from_raw(0), // Use dummy variable for return
                            violation_kind: NullViolationKind::NullReturnFromNonNullable,
                            location: val.source_location,
                            suggestion: Some("Change return type to optional or return a non-null value".to_string()),
                        });
                    }
                }
            }
            TypedStatement::Block { statements, .. } => {
                for stmt in statements {
                    self.check_violations_in_statement(stmt, function);
                }
            }
            _ => {}
        }
    }
    
    /// Check for violations in an expression
    fn check_violations_in_expression(&mut self, expression: &TypedExpression) {
        match &expression.kind {
            TypedExpressionKind::FieldAccess { object, .. } => {
                if let Some(var_id) = self.extract_variable_from_expression(object) {
                    if self.is_potentially_null(var_id) {
                        self.violations.push(NullSafetyViolation {
                            variable: var_id,
                            violation_kind: NullViolationKind::PotentialNullFieldAccess,
                            location: expression.source_location,
                            suggestion: Some("Add null check before field access".to_string()),
                        });
                    }
                }
                self.check_violations_in_expression(object);
            }
            
            TypedExpressionKind::MethodCall { receiver, arguments, .. } => {
                if let Some(var_id) = self.extract_variable_from_expression(receiver) {
                    if self.is_potentially_null(var_id) {
                        self.violations.push(NullSafetyViolation {
                            variable: var_id,
                            violation_kind: NullViolationKind::PotentialNullMethodCall,
                            location: expression.source_location,
                            suggestion: Some("Add null check before method call".to_string()),
                        });
                    }
                }
                self.check_violations_in_expression(receiver);
                for arg in arguments {
                    self.check_violations_in_expression(arg);
                }
            }
            
            TypedExpressionKind::ArrayAccess { array, index } => {
                if let Some(var_id) = self.extract_variable_from_expression(array) {
                    if self.is_potentially_null(var_id) {
                        self.violations.push(NullSafetyViolation {
                            variable: var_id,
                            violation_kind: NullViolationKind::PotentialNullArrayAccess,
                            location: expression.source_location,
                            suggestion: Some("Add null check before array access".to_string()),
                        });
                    }
                }
                self.check_violations_in_expression(array);
                self.check_violations_in_expression(index);
            }
            
            TypedExpressionKind::BinaryOp { left, right, .. } => {
                self.check_violations_in_expression(left);
                self.check_violations_in_expression(right);
            }
            
            TypedExpressionKind::UnaryOp { operand, .. } => {
                self.check_violations_in_expression(operand);
            }
            
            TypedExpressionKind::FunctionCall { function, arguments, .. } => {
                self.check_violations_in_expression(function);
                for arg in arguments {
                    self.check_violations_in_expression(arg);
                }
            }
            
            _ => {
                // Handle other expression types
            }
        }
    }
    
    /// Check if a variable is potentially null
    fn is_potentially_null(&self, var_id: SymbolId) -> bool {
        // Check across all blocks (simplified - should check current block context)
        for states in self.null_states.values() {
            if let Some(state) = states.get(&var_id) {
                match state {
                    NullState::Null | NullState::MaybeNull | NullState::Uninitialized => return true,
                    NullState::NotNull => return false,
                }
            }
        }
        
        // If not found, assume potentially null for safety
        true
    }
}

/// Perform null safety analysis on a function
pub fn analyze_function_null_safety(
    function: &TypedFunction,
    cfg: &ControlFlowGraph,
    type_table: &RefCell<TypeTable>,
    symbol_table: &SymbolTable,
) -> Vec<NullSafetyViolation> {
    let mut analyzer = NullSafetyAnalyzer::new(type_table, symbol_table, cfg);
    analyzer.analyze_function(function)
}

/// Generate suggested fixes for null safety violations
pub fn suggest_null_safety_fixes(violations: &[NullSafetyViolation]) -> Vec<String> {
    let mut suggestions = Vec::new();
    
    for violation in violations {
        let suggestion = match violation.violation_kind {
            NullViolationKind::PotentialNullDereference => {
                format!("Add null check: if (variable != null) {{ /* safe access */ }}")
            }
            NullViolationKind::PotentialNullMethodCall => {
                format!("Use safe navigation: variable?.method() or add null check")
            }
            NullViolationKind::PotentialNullFieldAccess => {
                format!("Use safe navigation: variable?.field or add null check")
            }
            NullViolationKind::PotentialNullArrayAccess => {
                format!("Check array is not null before accessing: if (array != null) array[index]")
            }
            NullViolationKind::NullArgumentToNonNullable => {
                format!("Ensure argument is not null or change parameter type to optional")
            }
            NullViolationKind::NullReturnFromNonNullable => {
                format!("Return non-null value or change return type to optional")
            }
        };
        
        suggestions.push(suggestion);
    }
    
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    // fn test_null_state_merge() {
    //     let analyzer = NullSafetyAnalyzer::new(
    //         &RefCell::new(TypeTable::new()),
    //         &SymbolTable::new(),
    //         &ControlFlowGraph::new(),
    //     );
        
    //     assert_eq!(
    //         analyzer.merge_null_states(NullState::Null, NullState::Null),
    //         NullState::Null
    //     );
        
    //     assert_eq!(
    //         analyzer.merge_null_states(NullState::NotNull, NullState::Null),
    //         NullState::MaybeNull
    //     );
    // }
    
    #[test]
    fn test_null_violation_suggestion() {
        let violation = NullSafetyViolation {
            variable: SymbolId::from_raw(1),
            violation_kind: NullViolationKind::PotentialNullMethodCall,
            location: SourceLocation::unknown(),
            suggestion: None,
        };
        
        let suggestions = suggest_null_safety_fixes(&[violation]);
        assert!(!suggestions.is_empty());
        assert!(suggestions[0].contains("safe navigation"));
    }
}