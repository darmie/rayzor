//! TAST to HIR Lowering
//!
//! This module converts the Typed AST (TAST) to High-level IR (HIR).
//! HIR preserves most source-level constructs while adding:
//! - Resolved symbols and types
//! - Lifetime and ownership information
//! - Desugared syntax (e.g., for-in to iterators)

use crate::ir::hir::*;
use crate::tast::{
    node::*, TypeId, SymbolId, SourceLocation,
    SymbolTable, TypeTable, InternedString, LifetimeId, ScopeId,
    Visibility, StringInterner,
};
use crate::semantic_graph::SemanticGraphs;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

/// Context for lowering TAST to HIR
pub struct TastToHirContext<'a> {
    /// Symbol table from TAST
    symbol_table: &'a SymbolTable,
    
    /// Type table from TAST
    type_table: &'a Rc<RefCell<TypeTable>>,
    
    /// String interner from TAST
    string_interner: &'a Rc<RefCell<StringInterner>>,
    
    /// Semantic graphs for additional information
    semantic_graphs: Option<&'a SemanticGraphs>,
    
    /// Current module being built
    module: HirModule,
    
    /// Current scope
    current_scope: ScopeId,
    
    /// Current lifetime
    current_lifetime: LifetimeId,
    
    /// Loop labels for break/continue
    loop_labels: Vec<Option<SymbolId>>,
    
    /// Error accumulator
    errors: Vec<LoweringError>,
    
    /// Counter for generating unique temporary variable names
    temp_var_counter: u32,
    
    /// Current file being processed (for validation)
    current_file: Option<&'a TypedFile>,
}

#[derive(Debug)]
pub struct LoweringError {
    pub message: String,
    pub location: SourceLocation,
}

/// Result type for lowering operations that can recover from errors
pub enum LoweringResult<T> {
    /// Successful lowering
    Ok(T),
    /// Complete failure - cannot produce any result
    Error(LoweringError),
    /// Partial success - produced a result but with errors
    Partial(T, Vec<LoweringError>),
}

impl<T> LoweringResult<T> {
    /// Convert to Result, treating partial success as success
    pub fn to_result(self) -> Result<T, Vec<LoweringError>> {
        match self {
            LoweringResult::Ok(value) => Ok(value),
            LoweringResult::Partial(value, _) => Ok(value),
            LoweringResult::Error(err) => Err(vec![err]),
        }
    }
    
    /// Check if this is a successful result (Ok or Partial)
    pub fn is_successful(&self) -> bool {
        !matches!(self, LoweringResult::Error(_))
    }
    
    /// Extract any errors from the result
    pub fn errors(&self) -> Vec<LoweringError> {
        match self {
            LoweringResult::Ok(_) => vec![],
            LoweringResult::Error(err) => vec![err.clone()],
            LoweringResult::Partial(_, errors) => errors.clone(),
        }
    }
}

impl Clone for LoweringError {
    fn clone(&self) -> Self {
        Self {
            message: self.message.clone(),
            location: self.location,
        }
    }
}

impl<'a> TastToHirContext<'a> {
    /// Create a new lowering context
    pub fn new(
        symbol_table: &'a SymbolTable,
        type_table: &'a Rc<RefCell<TypeTable>>,
        string_interner: &'a Rc<RefCell<StringInterner>>,
        module_name: String,
    ) -> Self {
        Self {
            symbol_table,
            type_table,
            string_interner,
            semantic_graphs: None,
            module: HirModule {
                name: module_name,
                imports: Vec::new(),
                types: HashMap::new(),
                functions: HashMap::new(),
                globals: HashMap::new(),
                metadata: HirMetadata {
                    source_file: String::new(),
                    language_version: "1.0".to_string(),
                    target_platforms: vec!["js".to_string()],
                    optimization_hints: Vec::new(),
                },
            },
            current_scope: ScopeId::from_raw(0),
            current_lifetime: LifetimeId::from_raw(0),
            loop_labels: Vec::new(),
            errors: Vec::new(),
            temp_var_counter: 0,
            current_file: None,
        }
    }
    
    /// Set semantic graphs for additional analysis info
    pub fn set_semantic_graphs(&mut self, graphs: &'a SemanticGraphs) {
        self.semantic_graphs = Some(graphs);
    }
    
    /// Helper methods to get builtin types from type table
    fn get_void_type(&self) -> TypeId {
        self.type_table.borrow().void_type()
    }
    
    fn get_bool_type(&self) -> TypeId {
        self.type_table.borrow().bool_type()
    }
    
    fn get_string_type(&self) -> TypeId {
        self.type_table.borrow().string_type()
    }
    
    fn get_null_type(&self) -> TypeId {
        // Use dynamic type for null for now
        self.type_table.borrow().dynamic_type()
    }
    
    fn get_dynamic_type(&self) -> TypeId {
        self.type_table.borrow().dynamic_type()
    }
    
    /// Check if class is final
    fn is_class_final(&self, class_symbol: SymbolId) -> bool {
        if let Some(hierarchy) = self.symbol_table.get_class_hierarchy(class_symbol) {
            hierarchy.is_final
        } else {
            false
        }
    }
    
    /// Check if class is abstract
    fn is_class_abstract(&self, class_symbol: SymbolId) -> bool {
        if let Some(hierarchy) = self.symbol_table.get_class_hierarchy(class_symbol) {
            hierarchy.is_abstract
        } else {
            false
        }
    }
    
    /// Lookup enum type from a constructor symbol
    fn lookup_enum_type_from_constructor(&self, constructor: SymbolId) -> TypeId {
        // Look up constructor symbol to find its parent enum type
        if let Some(symbol_info) = self.symbol_table.get_symbol(constructor) {
            // If this is an enum variant, its type should be the enum itself
            if symbol_info.type_id != TypeId::invalid() {
                return symbol_info.type_id;
            }
            // Try to find the parent enum through the symbol's scope
            // This would require more context about the symbol hierarchy
            // For now, return the symbol's type if available
        }
        // Fallback to dynamic type
        self.get_dynamic_type()
    }
    
    /// Lower a typed file to HIR module  
    pub fn lower_file(&mut self, file: &'a TypedFile) -> Result<HirModule, Vec<LoweringError>> {
        // Set current file for validation
        self.current_file = Some(file);
        // Lower imports
        for import in &file.imports {
            self.lower_import(import);
        }
        
        // Lower type declarations (classes, interfaces, enums, etc.)
        for class in &file.classes {
            self.lower_class(class);
        }
        
        for interface in &file.interfaces {
            self.lower_interface(interface);
        }
        
        for enum_decl in &file.enums {
            self.lower_enum(enum_decl);
        }
        
        // Lower abstract types
        for abstract_decl in &file.abstracts {
            self.lower_abstract(abstract_decl);
        }
        
        // Lower type aliases
        for alias in &file.type_aliases {
            self.lower_type_alias(alias);
        }
        
        // Lower module-level functions
        for function in &file.functions {
            let hir_func = self.lower_function(function);
            self.module.functions.insert(function.symbol_id, hir_func);
        }
        
        // Lower module fields (global variables)
        for field in &file.module_fields {
            self.lower_module_field(field);
        }
        
        if self.errors.is_empty() {
            Ok(self.module.clone())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }
    
    /// Lower a class declaration
    fn lower_class(&mut self, class: &TypedClass) {
        let mut hir_fields = Vec::new();
        let mut hir_methods = Vec::new();
        let mut hir_constructor = None;
        
        // Process constructors
        for constructor in &class.constructors {
            hir_constructor = Some(self.lower_constructor(constructor));
            break; // Take only the first constructor for now
        }
        
        // Process methods
        for method in &class.methods {
            hir_methods.push(HirMethod {
                function: self.lower_function(method),
                visibility: self.convert_visibility(method.visibility),
                is_static: method.is_static,
                is_override: method.metadata.is_override,
                is_abstract: false // Abstract methods would have no body
            });
        }
        
        // Process fields
        for field in &class.fields {
            hir_fields.push(HirClassField {
                name: field.name.clone(),
                ty: field.field_type,
                init: field.initializer.as_ref().map(|e| self.lower_expression(e)),
                visibility: self.convert_visibility(field.visibility),
                is_static: field.is_static,
                is_final: !matches!(field.mutability, crate::tast::Mutability::Mutable),
            });
        }
        
        // Create type ID from symbol ID (simplified)
        let type_id = TypeId::from_raw(class.symbol_id.as_raw());
        
        let hir_class = HirClass {
            symbol_id: class.symbol_id,
            name: class.name.clone(),
            type_params: self.lower_type_params(&class.type_parameters),
            extends: class.super_class,
            implements: class.interfaces.clone(),
            fields: hir_fields,
            methods: hir_methods,
            constructor: hir_constructor,
            metadata: Vec::new(), // Convert from TypedMetadata when available
            // Look up class hierarchy info from SymbolTable
            is_final: self.is_class_final(class.symbol_id),
            is_abstract: self.is_class_abstract(class.symbol_id),
            is_extern: false, // Would be in effects or metadata when available
        };
        
        self.module.types.insert(
            type_id,
            HirTypeDecl::Class(hir_class),
        );
    }
    
    /// Lower an interface declaration
    fn lower_interface(&mut self, interface: &TypedInterface) {
        let hir_methods = Vec::new(); // TODO: Extract interface methods
        let hir_fields = Vec::new(); // TODO: Extract interface fields
        
        // Create type ID from symbol ID (simplified)
        let type_id = TypeId::from_raw(interface.symbol_id.as_raw());
        
        let hir_interface = HirInterface {
            symbol_id: interface.symbol_id,
            name: interface.name.clone(),
            type_params: self.lower_type_params(&interface.type_parameters),
            extends: interface.extends.clone(),
            fields: hir_fields,
            methods: hir_methods,
            metadata: Vec::new(), // TODO: Extract metadata
        };
        
        self.module.types.insert(
            type_id,
            HirTypeDecl::Interface(hir_interface),
        );
    }
    
    /// Lower an enum declaration
    fn lower_enum(&mut self, enum_decl: &TypedEnum) {
        let mut hir_variants = Vec::new();
        
        for (i, variant) in enum_decl.variants.iter().enumerate() {
            let hir_fields = Vec::new(); // TODO: Extract variant fields
            
            hir_variants.push(HirEnumVariant {
                name: variant.name.clone(),
                fields: hir_fields,
                discriminant: Some(i as i32),
            });
        }
        
        // Create type ID from symbol ID (simplified)
        let type_id = TypeId::from_raw(enum_decl.symbol_id.as_raw());
        
        let hir_enum = HirEnum {
            symbol_id: enum_decl.symbol_id,
            name: enum_decl.name.clone(),
            type_params: self.lower_type_params(&enum_decl.type_parameters),
            variants: hir_variants,
            metadata: Vec::new(), // TODO: Extract metadata
        };
        
        self.module.types.insert(
            type_id,
            HirTypeDecl::Enum(hir_enum),
        );
    }
    
    /// Lower an abstract type
    fn lower_abstract(&mut self, abstract_decl: &TypedAbstract) {
        // Create type ID from symbol ID (simplified)
        let type_id = TypeId::from_raw(abstract_decl.symbol_id.as_raw());
        
        let hir_abstract = HirAbstract {
            symbol_id: abstract_decl.symbol_id,
            name: abstract_decl.name.clone(),
            type_params: self.lower_type_params(&abstract_decl.type_parameters),
            underlying: abstract_decl.underlying_type.unwrap_or_else(|| self.get_dynamic_type()),
            from_rules: Vec::new(), // TODO: Extract from metadata
            to_rules: Vec::new(),   // TODO: Extract from metadata
            operators: Vec::new(),  // TODO: Extract operator overloads
            fields: Vec::new(),     // TODO: Extract abstract fields
            metadata: Vec::new(),   // TODO: Extract metadata
        };
        
        self.module.types.insert(
            type_id,
            HirTypeDecl::Abstract(hir_abstract),
        );
    }
    
    /// Lower a type alias
    fn lower_type_alias(&mut self, alias: &TypedTypeAlias) {
        // Create type ID from symbol ID (simplified)
        let type_id = TypeId::from_raw(alias.symbol_id.as_raw());
        
        let hir_alias = HirTypeAlias {
            symbol_id: alias.symbol_id,
            name: alias.name.clone(),
            type_params: self.lower_type_params(&alias.type_parameters),
            aliased_type: alias.target_type,
        };
        
        self.module.types.insert(
            type_id,
            HirTypeDecl::TypeAlias(hir_alias),
        );
    }
    
    /// Lower a function
    fn lower_function(&mut self, function: &TypedFunction) -> HirFunction {
        let hir_body = if !function.body.is_empty() {
            Some(self.lower_block(&function.body))
        } else {
            None
        };

        // Check if this is the main function
        let main_name = self.string_interner.borrow_mut().intern("main");
        let is_main = function.name == main_name;

        // Extract optimization hints from SemanticGraphs/DFG if available
        let mut metadata = self.extract_function_metadata(&function.metadata);
        if let Some(semantic_graphs) = self.semantic_graphs {
            metadata.extend(self.extract_ssa_optimization_hints(function, semantic_graphs));
        }

        HirFunction {
            symbol_id: function.symbol_id,
            name: function.name.clone(),
            type_params: self.lower_type_params(&function.type_parameters),
            params: function.parameters.iter().map(|p| self.lower_param(p)).collect(),
            return_type: function.return_type,
            body: hir_body,
            metadata,
            is_inline: function.effects.is_inline,
            is_macro: false, // TODO: Extract from function metadata
            is_extern: false, // TODO: Extract from function metadata
            calling_convention: HirCallingConvention::Haxe,
            is_main,
        }
    }

    /// Extract optimization hints from SSA analysis in SemanticGraphs
    /// This queries the DFG without rebuilding SSA - following the architectural principle
    fn extract_ssa_optimization_hints(
        &self,
        function: &TypedFunction,
        semantic_graphs: &SemanticGraphs,
    ) -> Vec<HirAttribute> {
        let mut hints = Vec::new();

        // Query DFG for this specific function using its symbol_id
        if let Some(dfg) = semantic_graphs.data_flow.get(&function.symbol_id) {
            // Check if function is in valid SSA form
            if dfg.is_valid_ssa() {
                // Extract optimization hints from SSA analysis

                // 1. Variable usage patterns from SSA
                let total_ssa_vars = dfg.ssa_variables.len();
                if total_ssa_vars < 10 {
                    let mut interner = self.string_interner.borrow_mut();
                    let hint_name = interner.intern("optimization_hint");
                    let hint_value = interner.intern("few_locals");
                    hints.push(HirAttribute {
                        name: hint_name,
                        args: vec![HirAttributeArg::Literal(HirLiteral::String(hint_value))],
                    });
                }

                // 2. Dead code detection from SSA
                let dead_nodes: usize = dfg.nodes.values()
                    .filter(|n| n.uses.is_empty() && !n.metadata.has_side_effects)
                    .count();
                if dead_nodes > 0 {
                    let dead_code_name = self.string_interner.borrow_mut().intern("dead_code_count");
                    hints.push(HirAttribute {
                        name: dead_code_name,
                        args: vec![HirAttributeArg::Literal(HirLiteral::Int(dead_nodes as i64))],
                    });
                }

                // 3. Phi node complexity (indicates control flow complexity)
                let phi_count = dfg.metadata.phi_node_count;
                if phi_count == 0 {
                    let mut interner = self.string_interner.borrow_mut();
                    let hint_name = interner.intern("optimization_hint");
                    let hint_value = interner.intern("straight_line_code");
                    hints.push(HirAttribute {
                        name: hint_name,
                        args: vec![HirAttributeArg::Literal(HirLiteral::String(hint_value))],
                    });
                } else if phi_count > 20 {
                    let mut interner = self.string_interner.borrow_mut();
                    let hint_name = interner.intern("optimization_hint");
                    let hint_value = interner.intern("complex_control_flow");
                    hints.push(HirAttribute {
                        name: hint_name,
                        args: vec![HirAttributeArg::Literal(HirLiteral::String(hint_value))],
                    });
                }

                // 4. Value numbering opportunities
                if dfg.value_numbering.expr_to_value.len() > 5 {
                    let mut interner = self.string_interner.borrow_mut();
                    let hint_name = interner.intern("optimization_hint");
                    let hint_value = interner.intern("common_subexpressions");
                    hints.push(HirAttribute {
                        name: hint_name,
                        args: vec![HirAttributeArg::Literal(HirLiteral::String(hint_value))],
                    });
                }

                // 5. Inlining hints based on function size and SSA complexity
                let node_count = dfg.nodes.len();
                if node_count < 10 && phi_count < 3 {
                    let inline_name = self.string_interner.borrow_mut().intern("inline_candidate");
                    hints.push(HirAttribute {
                        name: inline_name,
                        args: vec![HirAttributeArg::Literal(HirLiteral::Bool(true))],
                    });
                }
            }
        }

        // Query CFG for control flow patterns
        if let Some(cfg) = semantic_graphs.control_flow.get(&function.symbol_id) {
            let block_count = cfg.blocks.len();
            if block_count == 1 {
                let mut interner = self.string_interner.borrow_mut();
                let hint_name = interner.intern("optimization_hint");
                let hint_value = interner.intern("single_block");
                hints.push(HirAttribute {
                    name: hint_name,
                    args: vec![HirAttributeArg::Literal(HirLiteral::String(hint_value))],
                });
            }
        }

        hints
    }
    
    /// Lower a constructor
    fn lower_constructor(&mut self, method: &TypedFunction) -> HirConstructor {
        let body = self.lower_block(&method.body);
        
        HirConstructor {
            params: method.parameters.iter().map(|p| self.lower_param(p)).collect(),
            super_call: None, // TODO: Extract super call from body
            field_inits: Vec::new(), // TODO: Extract field initializations
            body,
        }
    }
    
    /// Lower a statement
    fn lower_statement(&mut self, stmt: &TypedStatement) -> HirStatement {
        match stmt {
            TypedStatement::VarDeclaration { symbol_id, var_type, initializer, mutability, .. } => {
                use crate::tast::Mutability;
                let is_mutable = matches!(mutability, Mutability::Mutable);
                
                HirStatement::Let {
                    pattern: HirPattern::Variable {
                        name: self.get_symbol_name(*symbol_id),
                        symbol: *symbol_id,
                    },
                    type_hint: Some(*var_type),
                    init: initializer.as_ref().map(|e| self.lower_expression(e)),
                    is_mutable,
                }
            }
            TypedStatement::Expression { expression, .. } => {
                HirStatement::Expr(self.lower_expression(expression))
            }
            TypedStatement::Return { value, .. } => {
                HirStatement::Return(value.as_ref().map(|e| self.lower_expression(e)))
            }
            TypedStatement::Break { target_loop, .. } => {
                // Use the SymbolId directly instead of converting to string
                HirStatement::Break(*target_loop)
            }
            TypedStatement::Continue { target_loop, .. } => {
                // Use the SymbolId directly
                HirStatement::Continue(*target_loop)
            }
            TypedStatement::Throw { exception, .. } => {
                HirStatement::Throw(self.lower_expression(exception))
            }
            TypedStatement::If { condition, then_branch, else_branch, .. } => {
                HirStatement::If {
                    condition: self.lower_expression(condition),
                    then_branch: self.lower_block(std::slice::from_ref(then_branch)),
                    else_branch: else_branch.as_ref().map(|s| 
                        self.lower_block(std::slice::from_ref(&**s))
                    ),
                }
            }
            TypedStatement::Switch { discriminant, cases, default_case, .. } => {
                let mut hir_cases = Vec::new();
                
                for case in cases {
                    // TAST TypedSwitchCase has case_value and body
                    let patterns = vec![self.lower_case_value_pattern(&case.case_value)];
                    
                    let guard = None; // TAST doesn't have guard on switch cases
                    let body = self.lower_statement_as_block(&case.body);
                    
                    hir_cases.push(HirMatchCase {
                        patterns,
                        guard,
                        body,
                    });
                }
                
                // Add default case if present
                if let Some(default) = default_case {
                    hir_cases.push(HirMatchCase {
                        patterns: vec![HirPattern::Wildcard],
                        guard: None,
                        body: self.lower_block(std::slice::from_ref(&**default)),
                    });
                }
                
                HirStatement::Switch {
                    scrutinee: self.lower_expression(discriminant),
                    cases: hir_cases,
                }
            }
            TypedStatement::While { condition, body, .. } => {
                self.loop_labels.push(None);
                let hir_stmt = HirStatement::While {
                    label: None,
                    condition: self.lower_expression(condition),
                    body: self.lower_block(std::slice::from_ref(body)),
                };
                self.loop_labels.pop();
                hir_stmt
            }
            // Note: DoWhile might not exist in TAST, handle via While
            TypedStatement::For { init, condition, update, body, .. } => {
                // Desugar for loop to while loop
                let mut statements = Vec::new();
                
                // Add init statement if present
                if let Some(init) = init {
                    statements.push(self.lower_statement(init));
                }
                
                // Create while loop
                self.loop_labels.push(None);
                let while_body = if let Some(update) = update {
                    // Add update at end of body
                    let mut body_stmts = vec![self.lower_statement(body)];
                    body_stmts.push(HirStatement::Expr(self.lower_expression(update)));
                    HirBlock::new(body_stmts, self.current_scope)
                } else {
                    self.lower_block(std::slice::from_ref(body))
                };
                
                let while_stmt = HirStatement::While {
                    label: None,
                    condition: condition.as_ref()
                        .map(|e| self.lower_expression(e))
                        .unwrap_or_else(|| self.make_bool_literal(true)),
                    body: while_body,
                };
                self.loop_labels.pop();
                
                statements.push(while_stmt);
                
                // Wrap in a block
                HirStatement::Expr(HirExpr::new(
                    HirExprKind::Block(HirBlock::new(statements, self.current_scope)),
                    self.get_void_type(),
                    self.current_lifetime,
                    stmt.source_location(),
                ))
            }
            TypedStatement::ForIn { value_var, key_var, iterable, body, .. } => {
                self.loop_labels.push(None);
                
                // Create pattern from value_var and optional key_var
                let pattern = if let Some(key_var) = key_var {
                    // Key-value iteration: key => value
                    HirPattern::Tuple(vec![
                        HirPattern::Variable {
                            name: self.get_symbol_name(*key_var),
                            symbol: *key_var,
                        },
                        HirPattern::Variable {
                            name: self.get_symbol_name(*value_var),
                            symbol: *value_var,
                        },
                    ])
                } else {
                    // Simple iteration
                    HirPattern::Variable {
                        name: self.get_symbol_name(*value_var),
                        symbol: *value_var,
                    }
                };
                
                let hir_stmt = HirStatement::ForIn {
                    label: None,
                    pattern,
                    iterator: self.lower_expression(iterable),
                    body: self.lower_block(std::slice::from_ref(body)),
                };
                self.loop_labels.pop();
                hir_stmt
            }
            TypedStatement::Try { body, catch_clauses, finally_block, .. } => {
                let hir_catches = catch_clauses.iter().map(|c| HirCatchClause {
                    exception_type: c.exception_type,
                    exception_var: c.exception_variable,
                    body: self.lower_block(std::slice::from_ref(&c.body)),
                }).collect();
                
                HirStatement::TryCatch {
                    try_block: self.lower_block(std::slice::from_ref(body)),
                    catches: hir_catches,
                    finally_block: finally_block.as_ref().map(|s| 
                        self.lower_block(std::slice::from_ref(&**s))
                    ),
                }
            }
            TypedStatement::Block { statements, .. } => {
                HirStatement::Expr(HirExpr::new(
                    HirExprKind::Block(self.lower_block(statements)),
                    self.get_void_type(),
                    self.current_lifetime,
                    stmt.source_location(),
                ))
            }
            TypedStatement::Assignment { target, value, .. } => {
                HirStatement::Assign {
                    lhs: self.lower_lvalue(target),
                    rhs: self.lower_expression(value),
                    op: None,
                }
            }
            TypedStatement::PatternMatch { value, patterns, source_location } => {
                // Desugar pattern matching to a series of if-else statements
                // match value { pattern1 => body1, pattern2 => body2 } becomes:
                // let _match = value;
                // if (pattern1 matches _match) { body1 } else if (pattern2 matches _match) { body2 }
                
                self.desugar_pattern_match(value, patterns, *source_location)
            }
            _ => {
                // Use error recovery to continue processing
                self.make_error_stmt("Unsupported statement type", stmt.source_location())
            }
        }
    }
    
    /// Lower an expression
    fn lower_expression(&mut self, expr: &TypedExpression) -> HirExpr {
        let kind = match &expr.kind {
            TypedExpressionKind::Literal { value } => {
                HirExprKind::Literal(self.lower_literal(value))
            }
            TypedExpressionKind::Variable { symbol_id } => {
                HirExprKind::Variable {
                    symbol: *symbol_id,
                    capture_mode: None, // TODO: Determine from context
                }
            }
            TypedExpressionKind::This { .. } => HirExprKind::This,
            TypedExpressionKind::Super { .. } => HirExprKind::Super,
            TypedExpressionKind::Null => HirExprKind::Null,
            TypedExpressionKind::FieldAccess { object, field_symbol, .. } => {
                HirExprKind::Field {
                    object: Box::new(self.lower_expression(object)),
                    field: *field_symbol,
                }
            }
            TypedExpressionKind::ArrayAccess { array, index } => {
                HirExprKind::Index {
                    object: Box::new(self.lower_expression(array)),
                    index: Box::new(self.lower_expression(index)),
                }
            }
            TypedExpressionKind::FunctionCall { function, type_arguments, arguments, .. } => {
                HirExprKind::Call {
                    callee: Box::new(self.lower_expression(function)),
                    type_args: type_arguments.clone(),
                    args: arguments.iter().map(|a| self.lower_expression(a)).collect(),
                    is_method: false, // TODO: Determine from context
                }
            }
            TypedExpressionKind::MethodCall { receiver, method_symbol, type_arguments, arguments } => {
                // Desugar method call to function call with receiver as first argument
                // receiver.method(args) becomes method(receiver, args)
                // This makes the calling convention explicit for later lowering
                
                let receiver_expr = self.lower_expression(receiver);
                let mut call_args = vec![receiver_expr];
                call_args.extend(arguments.iter().map(|a| self.lower_expression(a)));
                
                HirExprKind::Call {
                    callee: Box::new(HirExpr::new(
                        HirExprKind::Variable {
                            symbol: *method_symbol,
                            capture_mode: None,
                        },
                        // TODO: Get actual method type from symbol table
                        expr.expr_type,
                        self.current_lifetime,
                        expr.source_location,
                    )),
                    type_args: type_arguments.clone(),
                    args: call_args,
                    is_method: true, // Mark that this was originally a method call
                }
            }
            TypedExpressionKind::New { class_type, type_arguments, arguments } => {
                // Validate constructor exists and is accessible
                self.validate_constructor(*class_type, arguments.len(), expr.source_location);
                
                HirExprKind::New {
                    class_type: *class_type,
                    type_args: type_arguments.clone(),
                    args: arguments.iter().map(|a| self.lower_expression(a)).collect(),
                }
            }
            TypedExpressionKind::UnaryOp { operator, operand } => {
                HirExprKind::Unary {
                    op: self.convert_unary_op(operator),
                    operand: Box::new(self.lower_expression(operand)),
                }
            }
            TypedExpressionKind::BinaryOp { left, operator, right } => {
                // Check if this is an assignment operator
                match operator {
                    BinaryOperator::Assign | 
                    BinaryOperator::AddAssign |
                    BinaryOperator::SubAssign |
                    BinaryOperator::MulAssign |
                    BinaryOperator::DivAssign |
                    BinaryOperator::ModAssign => {
                        // Assignments in expression position need special handling
                        // In HIR, assignments are statements, not expressions
                        // We need to create a block that performs the assignment and returns the value
                        
                        // Create an assignment statement
                        let assign_stmt = HirStatement::Assign {
                            lhs: self.lower_lvalue(left),
                            rhs: self.lower_expression(right),
                            op: match operator {
                                BinaryOperator::AddAssign => Some(HirBinaryOp::Add),
                                BinaryOperator::SubAssign => Some(HirBinaryOp::Sub),
                                BinaryOperator::MulAssign => Some(HirBinaryOp::Mul),
                                BinaryOperator::DivAssign => Some(HirBinaryOp::Div),
                                BinaryOperator::ModAssign => Some(HirBinaryOp::Mod),
                                _ => None, // Simple assignment
                            }
                        };
                        
                        // Create a variable reference to the assigned value
                        let result_expr = self.lower_expression(left);
                        
                        // Wrap in a block that performs assignment and returns the value
                        HirExprKind::Block(HirBlock {
                            statements: vec![assign_stmt],
                            expr: Some(Box::new(result_expr)),
                            scope: self.current_scope,
                        })
                    }
                    _ => {
                        // Regular binary operators
                        HirExprKind::Binary {
                            op: self.convert_binary_op(operator),
                            lhs: Box::new(self.lower_expression(left)),
                            rhs: Box::new(self.lower_expression(right)),
                        }
                    }
                }
            }
            TypedExpressionKind::Cast { expression, target_type, cast_kind } => {
                use CastKind;
                HirExprKind::Cast {
                    expr: Box::new(self.lower_expression(expression)),
                    target: *target_type,
                    is_safe: !matches!(cast_kind, CastKind::Unsafe),
                }
            }
            TypedExpressionKind::Conditional { condition, then_expr, else_expr } => {
                HirExprKind::If {
                    condition: Box::new(self.lower_expression(condition)),
                    then_expr: Box::new(self.lower_expression(then_expr)),
                    else_expr: Box::new(else_expr.as_ref()
                        .map(|e| self.lower_expression(e))
                        .unwrap_or_else(|| self.make_null_literal())),
                }
            }
            TypedExpressionKind::FunctionLiteral { parameters, body, return_type: _ } => {
                HirExprKind::Lambda {
                    params: parameters.iter().map(|p| self.lower_param(p)).collect(),
                    body: Box::new(self.lower_statements_as_expr(body)),
                    captures: Vec::new(), // TODO: Compute captures from body
                }
            }
            TypedExpressionKind::ArrayLiteral { elements } => {
                HirExprKind::Array {
                    elements: elements.iter().map(|e| self.lower_expression(e)).collect(),
                }
            }
            TypedExpressionKind::ObjectLiteral { fields, .. } => {
                HirExprKind::ObjectLiteral {
                    fields: fields.iter().map(|f| {
                        (f.name.clone(), self.lower_expression(&f.value))
                    }).collect(),
                }
            }
            TypedExpressionKind::MapLiteral { entries } => {
                HirExprKind::Map {
                    entries: entries.iter().map(|entry| {
                        (self.lower_expression(&entry.key), self.lower_expression(&entry.value))
                    }).collect(),
                }
            }
            TypedExpressionKind::Block { statements, scope_id } => {
                let hir_block = HirBlock {
                    statements: statements.iter().map(|s| self.lower_statement(s)).collect(),
                    expr: None, // Block expression result handled separately
                    scope: *scope_id,
                };
                HirExprKind::Block(hir_block)
            }
            TypedExpressionKind::StringInterpolation { parts } => {
                // Desugar string interpolation to concatenation
                // "Hello ${name}!" becomes "Hello " + name + "!"
                // This simplifies later optimization and code generation
                
                if parts.is_empty() {
                    return HirExpr::new(
                        HirExprKind::Literal(HirLiteral::String(self.intern_str(""))),
                        self.get_string_type(),
                        self.current_lifetime,
                        expr.source_location,
                    );
                }
                
                let mut result = None;
                
                for part in parts {
                    let part_expr = match part {
                        StringInterpolationPart::String(s) => {
                            HirExpr::new(
                                HirExprKind::Literal(HirLiteral::String(self.intern_str(s))),
                                expr.expr_type, // Use the expression's string type
                                self.current_lifetime,
                                expr.source_location,
                            )
                        }
                        StringInterpolationPart::Expression(e) => {
                            // TODO: Add toString() conversion if needed
                            self.lower_expression(e)
                        }
                    };
                    
                    result = match result {
                        None => Some(part_expr),
                        Some(left) => {
                            // Create concatenation: left + part_expr
                            Some(HirExpr::new(
                                HirExprKind::Binary {
                                    op: HirBinaryOp::Add, // String concatenation
                                    lhs: Box::new(left),
                                    rhs: Box::new(part_expr),
                                },
                                self.get_string_type(),
                                self.current_lifetime,
                                expr.source_location,
                            ))
                        }
                    };
                }
                
                result.map(|e| e.kind).unwrap_or_else(|| 
                    HirExprKind::Literal(HirLiteral::String(self.intern_str("")))
                )
            }
            TypedExpressionKind::ArrayComprehension { for_parts, expression, element_type } => {
                // Desugar array comprehension to a loop that builds an array
                self.desugar_array_comprehension(for_parts, expression, *element_type)
            }
            TypedExpressionKind::Return { value } => {
                // Return as an expression creates a block that never returns normally
                let return_stmt = HirStatement::Return(value.as_ref().map(|v| self.lower_expression(v)));
                let block = HirBlock {
                    statements: vec![return_stmt],
                    expr: None,
                    scope: self.current_scope,
                };
                HirExprKind::Block(block)
            }
            TypedExpressionKind::Break => {
                // Break as an expression
                let break_stmt = HirStatement::Break(None); // TODO: Handle labeled breaks
                let block = HirBlock {
                    statements: vec![break_stmt],
                    expr: None,
                    scope: self.current_scope,
                };
                HirExprKind::Block(block)
            }
            TypedExpressionKind::Continue => {
                // Continue as an expression
                let continue_stmt = HirStatement::Continue(None); // TODO: Handle labeled continues
                let block = HirBlock {
                    statements: vec![continue_stmt],
                    expr: None,
                    scope: self.current_scope,
                };
                HirExprKind::Block(block)
            }
            TypedExpressionKind::Switch { discriminant, cases, default_case } => {
                // Convert switch expression to a block with if-then-else chain
                let discriminant_expr = self.lower_expression(discriminant);
                let mut current_expr = default_case.as_ref()
                    .map(|expr| self.lower_expression(expr))
                    .unwrap_or_else(|| self.make_null_literal());
                
                // Build if-then-else chain from right to left
                for case in cases.iter().rev() {
                    let case_value = self.lower_expression(&case.case_value);
                    let case_body = match &case.body {
                        TypedStatement::Expression { expression, .. } => {
                            self.lower_expression(expression)
                        }
                        _ => {
                            // For non-expression bodies, create a block
                            let block = HirBlock {
                                statements: vec![self.lower_statement(&case.body)],
                                expr: None,
                                scope: self.current_scope,
                            };
                            HirExpr::new(
                                HirExprKind::Block(block),
                                expr.expr_type, // Use the switch expression's type
                                self.current_lifetime,
                                SourceLocation::unknown(),
                            )
                        }
                    };
                    
                    // Create condition: discriminant == case_value
                    let condition = HirExpr::new(
                        HirExprKind::Binary {
                            lhs: Box::new(discriminant_expr.clone()),
                            op: crate::ir::hir::HirBinaryOp::Eq,
                            rhs: Box::new(case_value),
                        },
                        self.get_bool_type(),
                        self.current_lifetime,
                        SourceLocation::unknown(),
                    );
                    
                    current_expr = HirExpr::new(
                        HirExprKind::If {
                            condition: Box::new(condition),
                            then_expr: Box::new(case_body),
                            else_expr: Box::new(current_expr),
                        },
                        // TODO: Infer actual type from context
                        self.get_dynamic_type(),
                        self.current_lifetime,
                        SourceLocation::unknown(),
                    );
                }
                
                current_expr.kind
            }
            TypedExpressionKind::Throw { expression } => {
                // Throw as an expression creates a block that throws
                let throw_expr = self.lower_expression(expression);
                let throw_stmt = HirStatement::Throw(throw_expr);
                let block = HirBlock {
                    statements: vec![throw_stmt],
                    expr: None,
                    scope: self.current_scope,
                };
                HirExprKind::Block(block)
            }
            TypedExpressionKind::Try { try_expr, catch_clauses } => {
                // Lower try-catch expression to HIR
                // try { expr } catch(e:Type) { handle } becomes a TryCatch expression
                
                let try_body = self.lower_expression(try_expr);
                let catch_handlers = catch_clauses.iter().map(|clause| {
                    // Lower the catch body
                    let body = match &clause.body {
                        TypedStatement::Expression { expression, .. } => {
                            self.lower_expression(expression)
                        }
                        _ => {
                            let stmt = self.lower_statement(&clause.body);
                            // Wrap statement in a block expression
                            HirExpr::new(
                                HirExprKind::Block(HirBlock {
                                    statements: vec![stmt],
                                    expr: None,
                                    scope: self.current_scope, // Use current scope
                                }),
                                self.get_void_type(),
                                self.current_lifetime,
                                SourceLocation::unknown(),
                            )
                        }
                    };
                    
                    crate::ir::hir::HirCatchHandler {
                        exception_var: clause.exception_variable,
                        exception_type: clause.exception_type,
                        guard: clause.filter.as_ref().map(|f| Box::new(self.lower_expression(f))),
                        body: Box::new(body),
                    }
                }).collect();
                
                HirExprKind::TryCatch {
                    try_expr: Box::new(try_body),
                    catch_handlers,
                    finally_expr: None, // TODO: Add finally support if needed
                }
            }
            // TAST doesn't have Untyped, handle other cases
            _ => {
                let error_msg = format!("Unsupported expression type: {:?}", expr.kind);
                // Use error recovery but still return a valid HIR node
                return self.make_error_expr(&error_msg, expr.source_location);
            }
        };
        
        HirExpr::new(
            kind,
            expr.expr_type,
            expr.lifetime_id,
            expr.source_location,
        )
    }
    
    /// Lower a block of statements
    fn lower_block(&mut self, statements: &[TypedStatement]) -> HirBlock {
        let hir_stmts = statements.iter()
            .map(|s| self.lower_statement(s))
            .collect();
        
        HirBlock::new(hir_stmts, self.current_scope)
    }
    
    // Helper methods...
    
    fn lower_pattern(&mut self, pattern: &TypedPattern) -> HirPattern {
        match pattern {
            TypedPattern::Variable { symbol_id, .. } => {
                HirPattern::Variable {
                    name: self.get_symbol_name(*symbol_id),
                    symbol: *symbol_id,
                }
            }
            TypedPattern::Wildcard { .. } => HirPattern::Wildcard,
            TypedPattern::Literal { value, .. } => {
                HirPattern::Literal(self.lower_expression_as_literal(value))
            }
            TypedPattern::Constructor { constructor, args, .. } => {
                // Extract enum type from constructor symbol
                HirPattern::Constructor {
                    // TODO: Get actual enum type from constructor symbol
                    enum_type: self.lookup_enum_type_from_constructor(*constructor),
                    variant: self.get_symbol_name(*constructor),
                    fields: args.iter().map(|f| self.lower_pattern(f)).collect(),
                }
            }
            // TAST doesn't have Tuple pattern, handle via Array
            TypedPattern::Array { elements, rest, .. } => {
                HirPattern::Array {
                    elements: elements.iter().map(|e| self.lower_pattern(e)).collect(),
                    rest: rest.as_ref().map(|r| Box::new(self.lower_pattern(r))),
                }
            }
            TypedPattern::Object { fields, .. } => {
                HirPattern::Object {
                    fields: fields.iter().map(|f| {
                        (self.intern_str(&f.field_name), self.lower_pattern(&f.pattern))
                    }).collect(),
                    rest: false, // TODO: Extract from pattern
                }
            }
            // TAST doesn't have Or pattern directly
            TypedPattern::Guard { pattern, guard } => {
                HirPattern::Guard {
                    pattern: Box::new(self.lower_pattern(pattern)),
                    condition: self.lower_expression(guard),
                }
            }
            TypedPattern::Extractor { .. } => {
                // Extractor patterns need special handling
                HirPattern::Wildcard
            }
        }
    }
    
    fn lower_lvalue(&mut self, expr: &TypedExpression) -> HirLValue {
        match &expr.kind {
            TypedExpressionKind::Variable { symbol_id } => {
                HirLValue::Variable(*symbol_id)
            }
            TypedExpressionKind::FieldAccess { object, field_symbol, .. } => {
                HirLValue::Field {
                    object: Box::new(self.lower_expression(object)),
                    field: *field_symbol,
                }
            }
            TypedExpressionKind::ArrayAccess { array, index } => {
                HirLValue::Index {
                    object: Box::new(self.lower_expression(array)),
                    index: Box::new(self.lower_expression(index)),
                }
            }
            _ => {
                self.add_error("Invalid assignment target", expr.source_location);
                // Return invalid symbol - this will be caught during validation
                HirLValue::Variable(SymbolId::invalid())
            }
        }
    }
    
    fn lower_literal(&mut self, lit: &LiteralValue) -> HirLiteral {
        match lit {
            LiteralValue::Int(i) => HirLiteral::Int(*i),
            LiteralValue::Float(f) => HirLiteral::Float(*f),
            LiteralValue::String(s) => HirLiteral::String(self.intern_str(s)),
            LiteralValue::Bool(b) => HirLiteral::Bool(*b),
            LiteralValue::Char(c) => HirLiteral::String(self.intern_str(&c.to_string())),
            LiteralValue::Regex(pattern) => HirLiteral::Regex {
                pattern: self.intern_str(pattern),
                flags: self.intern_str(""),
            },
            LiteralValue::RegexWithFlags { pattern, flags } => HirLiteral::Regex {
                pattern: self.intern_str(pattern),
                flags: self.intern_str(flags),
            },
        }
    }
    
    fn convert_unary_op(&self, op: &UnaryOperator) -> HirUnaryOp {
        match op {
            UnaryOperator::Not => HirUnaryOp::Not,
            UnaryOperator::Neg => HirUnaryOp::Neg,
            UnaryOperator::BitNot => HirUnaryOp::BitNot,
            UnaryOperator::PreInc => HirUnaryOp::PreIncr,
            UnaryOperator::PreDec => HirUnaryOp::PreDecr,
            UnaryOperator::PostInc => HirUnaryOp::PostIncr,
            UnaryOperator::PostDec => HirUnaryOp::PostDecr,
        }
    }
    
    fn convert_binary_op(&self, op: &BinaryOperator) -> HirBinaryOp {
        match op {
            BinaryOperator::Add => HirBinaryOp::Add,
            BinaryOperator::Sub => HirBinaryOp::Sub,
            BinaryOperator::Mul => HirBinaryOp::Mul,
            BinaryOperator::Div => HirBinaryOp::Div,
            BinaryOperator::Mod => HirBinaryOp::Mod,
            BinaryOperator::Eq => HirBinaryOp::Eq,
            BinaryOperator::Ne => HirBinaryOp::Ne,
            BinaryOperator::Lt => HirBinaryOp::Lt,
            BinaryOperator::Le => HirBinaryOp::Le,
            BinaryOperator::Gt => HirBinaryOp::Gt,
            BinaryOperator::Ge => HirBinaryOp::Ge,
            BinaryOperator::And => HirBinaryOp::And,
            BinaryOperator::Or => HirBinaryOp::Or,
            BinaryOperator::BitAnd => HirBinaryOp::BitAnd,
            BinaryOperator::BitOr => HirBinaryOp::BitOr,
            BinaryOperator::BitXor => HirBinaryOp::BitXor,
            BinaryOperator::Shl => HirBinaryOp::Shl,
            BinaryOperator::Shr => HirBinaryOp::Shr,
            // Range and NullCoalesce might not exist in TAST
            _ => HirBinaryOp::Add, // Default fallback
        }
    }
    
    fn convert_visibility(&self, vis: Visibility) -> HirVisibility {
        match vis {
            Visibility::Public => HirVisibility::Public,
            Visibility::Private => HirVisibility::Private,
            Visibility::Protected => HirVisibility::Protected,
            Visibility::Internal => HirVisibility::Internal,
        }
    }
    
    fn lower_import(&mut self, _import: &TypedImport) {
        // TODO: Implement import lowering
    }
    
    fn lower_module_field(&mut self, _field: &TypedModuleField) {
        // TODO: Implement module field lowering
    }
    
    fn lower_type_params(&mut self, _params: &[TypedTypeParameter]) -> Vec<HirTypeParam> {
        // TODO: Implement type parameter lowering
        Vec::new()
    }
    
    fn lower_param(&mut self, param: &TypedParameter) -> HirParam {
        HirParam {
            name: param.name.clone(),
            ty: param.param_type,
            default: param.default_value.as_ref().map(|e| self.lower_expression(e)),
            is_optional: param.is_optional,
            is_rest: false, // TODO: Extract from parameter metadata
        }
    }
    
    fn lower_metadata(&mut self, _metadata: &[TypedMetadata]) -> Vec<HirAttribute> {
        // TODO: Implement metadata lowering
        Vec::new()
    }
    
    /// Desugar array comprehension into a loop that builds an array
    /// [for (x in xs) if (condition) expression] becomes:
    /// {
    ///     let result = [];
    ///     for (x in xs) {
    ///         if (condition) {
    ///             result.push(expression);
    ///         }
    ///     }
    ///     result
    /// }
    fn desugar_array_comprehension(
        &mut self, 
        for_parts: &[TypedComprehensionFor], 
        expression: &TypedExpression,
        element_type: TypeId
    ) -> HirExprKind {
        // Full desugaring: [for (x in xs) if (cond) expr] becomes:
        // {
        //     let _tmp = [];
        //     for (x in xs) {
        //         if (cond) {
        //             _tmp.push(expr);
        //         }
        //     }
        //     _tmp
        // }
        
        let mut statements = Vec::new();
        
        // 1. Create temporary array variable
        let (temp_name, temp_symbol) = self.gen_temp_var();
        let array_type = self.type_table.borrow_mut().create_array_type(element_type);
        
        // Create empty array literal
        let empty_array = HirExpr::new(
            HirExprKind::Array { elements: Vec::new() },
            array_type,
            self.current_lifetime,
            SourceLocation::unknown(),
        );
        
        // let _tmp = []
        statements.push(HirStatement::Let {
            pattern: HirPattern::Variable {
                name: temp_name.clone(),
                symbol: temp_symbol,
            },
            type_hint: Some(array_type),
            init: Some(empty_array),
            is_mutable: true,
        });
        
        // 2. Build nested for loops
        let mut current_body = match self.build_comprehension_body(
            expression,
            temp_symbol,
            temp_name.clone(),
            array_type
        ) {
            Ok(body) => body,
            Err(err) => {
                // If we can't build the comprehension body, return an error expression
                let error_expr = self.make_error_expr(&err, SourceLocation::unknown());
                return error_expr.kind;
            }
        };
        
        // Iterate through for parts in reverse to build nested structure
        for for_part in for_parts.iter().rev() {
            let pattern = if let Some(key_var) = for_part.key_var_symbol {
                // Key-value iteration
                HirPattern::Tuple(vec![
                    HirPattern::Variable {
                        name: self.get_symbol_name(key_var),
                        symbol: key_var,
                    },
                    HirPattern::Variable {
                        name: self.get_symbol_name(for_part.var_symbol),
                        symbol: for_part.var_symbol,
                    },
                ])
            } else {
                // Simple iteration
                HirPattern::Variable {
                    name: self.get_symbol_name(for_part.var_symbol),
                    symbol: for_part.var_symbol,
                }
            };
            
            // BACKLOG: TypedComprehensionFor doesn't have a condition field
            // Haxe array comprehensions support filters like: [for (x in xs) if (x > 0) x * 2]
            // Need to check how filters are represented in TAST
            // For now, no filter support in desugaring
            
            // Create for-in loop
            let for_stmt = HirStatement::ForIn {
                label: None,
                pattern,
                iterator: self.lower_expression(&for_part.iterator),
                body: current_body,
            };
            
            current_body = HirBlock::new(vec![for_stmt], self.current_scope);
        }
        
        // Add the nested loops to statements
        statements.extend(current_body.statements);
        
        // 3. Return the temporary array variable
        let result_expr = HirExpr::new(
            HirExprKind::Variable {
                symbol: temp_symbol,
                capture_mode: None,
            },
            array_type,
            self.current_lifetime,
            SourceLocation::unknown(),
        );
        
        // Create block that evaluates to the array
        let block = HirBlock {
            statements,
            expr: Some(Box::new(result_expr)),
            scope: self.current_scope,
        };
        
        HirExprKind::Block(block)
    }
    
    /// Build the innermost body for array comprehension that pushes to the array
    fn build_comprehension_body(
        &mut self,
        expression: &TypedExpression,
        array_symbol: SymbolId,
        array_name: InternedString,
        array_type: TypeId,
    ) -> Result<HirBlock, String> {
        // Create _tmp.push(expression)
        let array_ref = HirExpr::new(
            HirExprKind::Variable {
                symbol: array_symbol,
                capture_mode: None,
            },
            array_type,
            self.current_lifetime,
            SourceLocation::unknown(),
        );
        
        // BACKLOG: Need proper method resolution for Array.push
        // See src/ir/BACKLOG.md section 2a
        // For now, we cannot properly resolve the push method symbol
        // This blocks correct array comprehension desugaring
        
        // Attempt to look up push method from Array type
        let push_symbol = self.lookup_array_push_method(array_type)
            .ok_or_else(|| {
                format!("Cannot resolve Array.push method for comprehension desugaring. Array type methods must be loaded from haxe-std")
            })?;
        
        // Create method call: array.push(element)
        let push_call = HirExpr::new(
            HirExprKind::Call {
                callee: Box::new(HirExpr::new(
                    HirExprKind::Field {
                        object: Box::new(array_ref),
                        field: push_symbol,
                    },
                    self.get_dynamic_type(), // BACKLOG: Need actual method type
                    self.current_lifetime,
                    SourceLocation::unknown(),
                )),
                type_args: Vec::new(),
                args: vec![self.lower_expression(expression)],
                is_method: true,
            },
            self.get_void_type(),
            self.current_lifetime,
            SourceLocation::unknown(),
        );
        
        Ok(HirBlock::new(
            vec![HirStatement::Expr(push_call)],
            self.current_scope,
        ))
    }
    
    fn make_bool_literal(&self, value: bool) -> HirExpr {
        HirExpr::new(
            HirExprKind::Literal(HirLiteral::Bool(value)),
            self.get_bool_type(),
            self.current_lifetime,
            SourceLocation::unknown(),
        )
    }
    
    fn make_null_literal(&self) -> HirExpr {
        HirExpr::new(
            HirExprKind::Null,
            self.get_null_type(),
            self.current_lifetime,
            SourceLocation::unknown(),
        )
    }
    
    fn add_error(&mut self, msg: &str, location: SourceLocation) {
        self.errors.push(LoweringError {
            message: msg.to_string(),
            location,
        });
    }
    
    /// Create an error expression node that preserves structure during error recovery
    fn make_error_expr(&mut self, msg: &str, location: SourceLocation) -> HirExpr {
        self.add_error(msg, location);
        HirExpr::new(
            // Use Untyped as a placeholder for error expressions
            HirExprKind::Untyped(Box::new(HirExpr::new(
                HirExprKind::Null,
                self.get_dynamic_type(),
                self.current_lifetime,
                location,
            ))),
            self.get_dynamic_type(),
            self.current_lifetime,
            location,
        )
    }
    
    /// Create an error statement node
    fn make_error_stmt(&mut self, msg: &str, location: SourceLocation) -> HirStatement {
        self.add_error(msg, location);
        // Return a no-op statement
        HirStatement::Expr(self.make_error_expr(msg, location))
    }
    
    /// Get symbol name from symbol table
    fn get_symbol_name(&self, symbol_id: SymbolId) -> InternedString {
        // Look up symbol name from the symbol table
        if let Some(symbol_info) = self.symbol_table.get_symbol(symbol_id) {
            symbol_info.name
        } else {
            // Fallback for invalid symbols
            let name = format!("unknown_sym_{}", symbol_id.as_raw());
            self.intern_str(&name)
        }
    }
    
    /// Intern a string
    fn intern_str(&self, s: &str) -> InternedString {
        self.string_interner.borrow().intern(s)
    }
    
    /// Generate a unique temporary variable name
    fn gen_temp_var(&mut self) -> (InternedString, SymbolId) {
        let name = format!("_tmp{}", self.temp_var_counter);
        self.temp_var_counter += 1;
        let interned = self.intern_str(&name);
        // Create a synthetic symbol ID for the temporary
        let symbol_id = SymbolId::from_raw(u32::MAX - self.temp_var_counter);
        (interned, symbol_id)
    }
    
    /// Look up the push method for Array type
    fn lookup_array_push_method(&self, array_type: TypeId) -> Option<SymbolId> {
        // Array is loaded from haxe-std/Array.hx as an extern class
        // The push method should be registered in some scope
        
        let push_name = self.string_interner.borrow_mut().intern("push");
        
        // Try to find push in any scope
        // We'll iterate through a reasonable range of scope IDs
        // In practice, Array's scope should be one of the early ones
        for scope_id in 0..100 {
            let scope = ScopeId::from_raw(scope_id);
            if let Some(symbol_ref) = self.symbol_table.lookup_symbol(scope, push_name) {
                // Found a symbol named "push" - return it
                // In a proper implementation we'd verify it's the Array.push method
                return Some(symbol_ref.id);
            }
        }
        
        // If we still can't find it, it wasn't registered properly
        None
    }
    
    /// Desugar pattern matching statement to if-else chain
    fn desugar_pattern_match(
        &mut self, 
        value: &TypedExpression,
        patterns: &[TypedPatternCase],
        source_location: SourceLocation
    ) -> HirStatement {
        if patterns.is_empty() {
            return self.make_error_stmt("Pattern match with no cases", source_location);
        }
        
        // Generate temp variable to hold matched value
        let (match_var_name, match_var_symbol) = self.gen_temp_var();
        
        // Create let statement for matched value
        let match_let = HirStatement::Let {
            pattern: HirPattern::Variable {
                name: match_var_name.clone(),
                symbol: match_var_symbol,
            },
            type_hint: Some(value.expr_type),
            init: Some(self.lower_expression(value)),
            is_mutable: false,
        };
        
        // Build if-else chain from patterns
        let mut else_branch: Option<HirBlock> = None;
        
        // Process patterns in reverse to build nested if-else
        for (i, case) in patterns.iter().enumerate().rev() {
            let is_last = i == patterns.len() - 1;
            
            // Generate condition from pattern
            let (condition, bindings) = self.pattern_to_condition(
                &case.pattern,
                match_var_symbol,
                value.expr_type
            );
            
            // Add guard condition if present
            let final_condition = if let Some(guard) = &case.guard {
                // Combine pattern condition with guard: pattern_cond && guard
                HirExpr::new(
                    HirExprKind::Binary {
                        op: HirBinaryOp::And,
                        lhs: Box::new(condition),
                        rhs: Box::new(self.lower_expression(guard)),
                    },
                    self.get_bool_type(),
                    self.current_lifetime,
                    source_location,
                )
            } else {
                condition
            };
            
            // Build body with bindings
            let mut body_stmts = Vec::new();
            
            // Add variable bindings from pattern
            for (bind_symbol, bind_expr) in bindings {
                body_stmts.push(HirStatement::Let {
                    pattern: HirPattern::Variable {
                        name: self.get_symbol_name(bind_symbol),
                        symbol: bind_symbol,
                    },
                    type_hint: None, // Let type inference handle it
                    init: Some(bind_expr),
                    is_mutable: false,
                });
            }
            
            // Add the actual case body
            body_stmts.push(self.lower_statement(&case.body));
            
            let then_branch = HirBlock::new(body_stmts, self.current_scope);
            
            // Create if statement
            let if_stmt = HirStatement::If {
                condition: final_condition,
                then_branch,
                else_branch,
            };
            
            // This if becomes the else branch for the previous case
            else_branch = Some(HirBlock::new(vec![if_stmt], self.current_scope));
        }
        
        // Combine match variable declaration with the if-else chain
        let mut statements = vec![match_let];
        if let Some(else_block) = else_branch {
            // The else_block contains the entire if-else chain
            statements.extend(else_block.statements);
        }
        
        // Wrap in a block statement
        HirStatement::Expr(HirExpr::new(
            HirExprKind::Block(HirBlock::new(statements, self.current_scope)),
            self.get_void_type(),
            self.current_lifetime,
            source_location,
        ))
    }
    
    /// Convert a pattern to a condition expression and extract variable bindings
    fn pattern_to_condition(
        &mut self,
        pattern: &TypedPattern,
        match_var: SymbolId,
        match_type: TypeId,
    ) -> (HirExpr, Vec<(SymbolId, HirExpr)>) {
        let mut bindings = Vec::new();
        
        let condition = match pattern {
            TypedPattern::Wildcard { .. } => {
                // Wildcard always matches
                self.make_bool_literal(true)
            }
            TypedPattern::Variable { symbol_id, source_location, .. } => {
                // Variable pattern always matches and creates a binding
                let match_expr = HirExpr::new(
                    HirExprKind::Variable {
                        symbol: match_var,
                        capture_mode: None,
                    },
                    match_type,
                    self.current_lifetime,
                    *source_location,
                );
                bindings.push((*symbol_id, match_expr));
                self.make_bool_literal(true)
            }
            TypedPattern::Literal { value, source_location } => {
                // Literal pattern: match_var == literal
                let match_expr = HirExpr::new(
                    HirExprKind::Variable {
                        symbol: match_var,
                        capture_mode: None,
                    },
                    match_type,
                    self.current_lifetime,
                    *source_location,
                );
                HirExpr::new(
                    HirExprKind::Binary {
                        op: HirBinaryOp::Eq,
                        lhs: Box::new(match_expr),
                        rhs: Box::new(self.lower_expression(value)),
                    },
                    self.get_bool_type(),
                    self.current_lifetime,
                    *source_location,
                )
            }
            TypedPattern::Constructor { constructor, args, source_location, .. } => {
                // Constructor pattern: check type and extract fields
                // BACKLOG: Need proper enum variant checking
                // This requires:
                // 1. Runtime type information for enum variants
                // 2. Field extraction from enum constructors
                // 3. Nested pattern matching for constructor arguments
                
                // For now, create a placeholder that always fails
                self.add_error(
                    "Constructor patterns not yet supported in desugaring",
                    *source_location
                );
                self.make_bool_literal(false)
            }
            TypedPattern::Array { elements, rest, source_location, .. } => {
                // Array pattern: check length and elements
                // BACKLOG: Need proper array pattern matching
                // This requires:
                // 1. Array length checking
                // 2. Element extraction and matching
                // 3. Rest pattern handling
                
                self.add_error(
                    "Array patterns not yet supported in desugaring",
                    *source_location
                );
                self.make_bool_literal(false)
            }
            TypedPattern::Object { fields, source_location, .. } => {
                // Object pattern: check fields
                // BACKLOG: Need proper object pattern matching
                // This requires:
                // 1. Field existence checking
                // 2. Field extraction and matching
                // 3. Nested pattern matching for field values
                
                self.add_error(
                    "Object patterns not yet supported in desugaring",
                    *source_location
                );
                self.make_bool_literal(false)
            }
            TypedPattern::Guard { pattern, guard } => {
                // Guard pattern: pattern && guard
                let (pattern_cond, pattern_bindings) = self.pattern_to_condition(
                    pattern,
                    match_var,
                    match_type
                );
                bindings.extend(pattern_bindings);
                
                HirExpr::new(
                    HirExprKind::Binary {
                        op: HirBinaryOp::And,
                        lhs: Box::new(pattern_cond),
                        rhs: Box::new(self.lower_expression(guard)),
                    },
                    self.get_bool_type(),
                    self.current_lifetime,
                    guard.source_location,
                )
            }
            TypedPattern::Extractor { source_location, .. } => {
                // Extractor pattern: needs special handling
                // BACKLOG: Extractor patterns require method calls
                self.add_error(
                    "Extractor patterns not yet supported in desugaring",
                    *source_location
                );
                self.make_bool_literal(false)
            }
        };
        
        (condition, bindings)
    }
    
    /// Extract function metadata
    fn extract_function_metadata(&self, _metadata: &FunctionMetadata) -> Vec<HirAttribute> {
        // TODO: Convert function metadata to attributes
        Vec::new()
    }
    
    /// Lower a switch case value to a pattern
    fn lower_case_value_pattern(&mut self, expr: &TypedExpression) -> HirPattern {
        // Convert constant expression to pattern
        match &expr.kind {
            TypedExpressionKind::Literal { value } => {
                HirPattern::Literal(self.lower_literal(value))
            }
            _ => HirPattern::Wildcard,
        }
    }
    
    /// Convert a single statement to a block
    fn lower_statement_as_block(&mut self, stmt: &TypedStatement) -> HirBlock {
        HirBlock {
            statements: vec![self.lower_statement(stmt)],
            expr: None,
            scope: self.current_scope,
        }
    }
    
    /// Convert statements to an expression
    fn lower_statements_as_expr(&mut self, stmts: &[TypedStatement]) -> HirExpr {
        let block = self.lower_block(stmts);
        HirExpr::new(
            HirExprKind::Block(block),
            self.get_dynamic_type(), // Block type will be inferred
            self.current_lifetime,
            SourceLocation::unknown(),
        )
    }
    
    /// Look up method type from symbol table
    fn lookup_method_type(&mut self, method_symbol: SymbolId, receiver_type: TypeId) -> TypeId {
        // Try to get the method's type from the symbol table
        if let Some(symbol_info) = self.symbol_table.get_symbol(method_symbol) {
            if symbol_info.type_id != TypeId::invalid() {
                return symbol_info.type_id;
            }
        }
        
        // Fallback: return the receiver type (method should return something compatible)
        self.add_error(
            &format!("Method type not found for symbol {:?}", method_symbol),
            SourceLocation::unknown()
        );
        receiver_type
    }
    
    /// Look up enum type from constructor symbol
    fn lookup_enum_type(&mut self, constructor: SymbolId) -> TypeId {
        // Look up the constructor's parent enum type
        if let Some(symbol_info) = self.symbol_table.get_symbol(constructor) {
            if symbol_info.type_id != TypeId::invalid() {
                // The constructor's type should reference the enum
                return symbol_info.type_id;
            }
        }
        
        self.add_error(
            &format!("Enum type not found for constructor {:?}", constructor),
            SourceLocation::unknown()
        );
        self.get_dynamic_type() // Fallback to dynamic
    }
    
    /// Lower an expression to a literal pattern
    fn lower_expression_as_literal(&mut self, expr: &TypedExpression) -> HirLiteral {
        match &expr.kind {
            TypedExpressionKind::Literal { value } => self.lower_literal(value),
            _ => HirLiteral::Bool(false), // Default
        }
    }
    
    /// Validate that a constructor exists for the given class type and argument count
    fn validate_constructor(&mut self, class_type: TypeId, arg_count: usize, location: SourceLocation) {
        // Check if we have a current file being processed
        if let Some(file) = &self.current_file {
            // Look up the class type in the original TAST data
            for class in &file.classes {
                // Get the type symbol for this class
                if let Some(class_symbol_id) = self.type_table.borrow().get_type_symbol(class_type) {
                    // Check if this matches our class symbol
                    if class.symbol_id == class_symbol_id {
                        // Check if the class has a constructor
                        let has_constructor = !class.constructors.is_empty();
                        
                        if !has_constructor {
                            let class_name_str = self.string_interner.borrow()
                                .get(class.name)
                                .unwrap_or("?")
                                .to_string();
                            let error_msg = format!(
                                "Class '{}' has no constructor but 'new' was called with {} arguments", 
                                class_name_str,
                                arg_count
                            );
                            self.add_error(&error_msg, location);
                            return;
                        }
                        
                        // Basic validation passed
                        // Note: Enhanced validation features are tracked in BACKLOG.md
                        return;
                    }
                }
            }
        }
        
        // Class not found in current file - might be from imported module
        // Enhanced cross-module lookup tracked in BACKLOG.md
    }
    
}

/// Public entry point for TAST to HIR lowering
pub fn lower_tast_to_hir(
    file: &TypedFile,
    symbol_table: &SymbolTable,
    type_table: &Rc<RefCell<TypeTable>>,
    semantic_graphs: Option<&SemanticGraphs>,
) -> Result<HirModule, Vec<LoweringError>> {
    let mut context = TastToHirContext::new(
        symbol_table,
        type_table,
        &file.string_interner,
        file.metadata.package_name.as_ref()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "main".to_string()),
    );
    
    if let Some(graphs) = semantic_graphs {
        context.set_semantic_graphs(graphs);
    }
    
    context.lower_file(file)
}