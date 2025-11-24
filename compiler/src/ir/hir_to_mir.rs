//! HIR to MIR Lowering
//!
//! This module converts High-level IR (HIR) to Mid-level IR (MIR).
//! 
//! According to the architecture plan:
//! - HIR: Close to source, with high-level constructs preserved
//! - MIR: SSA form with phi nodes, ready for optimization
//! - LIR: Target-specific, close to machine code
//!
//! The existing IR implementation (with IrBuilder, optimization passes, etc.) 
//! serves as our MIR level.

use crate::ir::hir::*;
use crate::ir::{
    IrBuilder, IrModule, IrFunction, IrBasicBlock, IrBlockId,
    IrInstruction, IrTerminator, IrPhiNode, IrId, IrType, IrValue,
    BinaryOp, UnaryOp, CompareOp, IrSourceLocation,
    FunctionSignatureBuilder, CallingConvention,
    IrGlobal, IrGlobalId, Linkage,
    IrTypeDef, IrTypeDefId, IrTypeDefinition, IrField, IrEnumVariant,
};
use crate::tast::{SymbolId, TypeId, SourceLocation, InternedString};
use std::collections::HashMap;

/// Context for lowering HIR to MIR
pub struct HirToMirContext {
    /// MIR builder
    builder: IrBuilder,

    /// Mapping from HIR symbols to MIR registers
    symbol_map: HashMap<SymbolId, IrId>,

    /// Mapping from HIR blocks to MIR blocks
    block_map: HashMap<usize, IrBlockId>,

    /// Loop context for break/continue
    loop_stack: Vec<LoopContext>,

    /// Current HIR module being processed
    current_module: Option<String>,

    /// Error accumulator
    errors: Vec<LoweringError>,

    /// SSA-derived optimization hints extracted from HIR metadata
    /// These are queried from DFG during HIR lowering and passed to MIR
    ssa_hints: SsaOptimizationHints,

    /// Counter for generating unique lambda names
    lambda_counter: u32,

    /// Dynamic global initializers (globals needing runtime initialization)
    dynamic_globals: Vec<(SymbolId, HirExpr)>,
}

/// SSA-derived optimization hints from DFG analysis
/// These guide MIR generation and optimization without rebuilding SSA
#[derive(Debug, Default)]
struct SsaOptimizationHints {
    /// Functions that are inline candidates (small, simple control flow)
    inline_candidates: std::collections::HashSet<SymbolId>,

    /// Functions with straight-line code (no branches, optimize aggressively)
    straight_line_functions: std::collections::HashSet<SymbolId>,

    /// Functions with complex control flow (many phi nodes, careful optimization)
    complex_control_flow_functions: std::collections::HashSet<SymbolId>,

    /// Functions with common subexpressions (CSE opportunities)
    cse_opportunities: std::collections::HashSet<SymbolId>,
}

#[derive(Debug)]
struct LoopContext {
    continue_block: IrBlockId,
    break_block: IrBlockId,
    label: Option<SymbolId>,
}

#[derive(Debug)]
pub struct LoweringError {
    pub message: String,
    pub location: SourceLocation,
}

impl HirToMirContext {
    /// Create a new lowering context
    pub fn new(module_name: String, source_file: String) -> Self {
        Self {
            builder: IrBuilder::new(module_name.clone(), source_file),
            symbol_map: HashMap::new(),
            block_map: HashMap::new(),
            loop_stack: Vec::new(),
            current_module: Some(module_name),
            errors: Vec::new(),
            ssa_hints: SsaOptimizationHints::default(),
            lambda_counter: 0,
            dynamic_globals: Vec::new(),
        }
    }

    /// Extract SSA optimization hints from HIR module metadata
    /// This queries the hints that were previously extracted from DFG/SSA during HIR lowering
    fn extract_ssa_hints_from_hir(&mut self, hir_module: &HirModule) {
        for (symbol_id, func) in &hir_module.functions {
            // Parse optimization hints from function metadata
            for attr in &func.metadata {
                let attr_name = attr.name.to_string();
                match attr_name.as_str() {
                    "inline_candidate" => {
                        self.ssa_hints.inline_candidates.insert(*symbol_id);
                    }
                    "optimization_hint" => {
                        // Check the hint value
                        if let Some(HirAttributeArg::Literal(HirLiteral::String(hint))) = attr.args.first() {
                            match hint.to_string().as_str() {
                                "straight_line_code" => {
                                    self.ssa_hints.straight_line_functions.insert(*symbol_id);
                                }
                                "complex_control_flow" => {
                                    self.ssa_hints.complex_control_flow_functions.insert(*symbol_id);
                                }
                                "common_subexpressions" => {
                                    self.ssa_hints.cse_opportunities.insert(*symbol_id);
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    
    /// Lower a HIR module to MIR
    pub fn lower_module(&mut self, hir_module: &HirModule) -> Result<IrModule, Vec<LoweringError>> {
        // Extract SSA optimization hints from HIR metadata
        // These were populated during HIR lowering by querying DFG/SSA
        self.extract_ssa_hints_from_hir(hir_module);

        // Set module metadata
        self.builder.module.metadata.language_version =
            hir_module.metadata.language_version.clone();

        // Lower all functions
        for (symbol_id, hir_func) in &hir_module.functions {
            self.lower_function(*symbol_id, hir_func);
        }

        // Lower all type declarations to module metadata
        // MIR doesn't need full type declarations, just metadata for codegen
        for (type_id, type_decl) in &hir_module.types {
            self.register_type_metadata(*type_id, type_decl);
        }

        // Lower globals
        for (symbol_id, global) in &hir_module.globals {
            self.lower_global(*symbol_id, global);
        }

        // Generate __init__ function for dynamic global initialization
        if !self.dynamic_globals.is_empty() {
            self.generate_module_init_function();
        }

        if self.errors.is_empty() {
            Ok(std::mem::replace(
                &mut self.builder.module,
                IrModule::new(String::new(), String::new())
            ))
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }
    
    /// Lower a HIR function to MIR
    fn lower_function(&mut self, symbol_id: SymbolId, hir_func: &HirFunction) {
        // Build MIR function signature
        let signature = self.build_function_signature(hir_func);

        // Start building the function
        let func_id = self.builder.start_function(
            symbol_id,
            hir_func.name.to_string(),
            signature,
        );

        // Apply SSA-derived optimization hints to function attributes
        // These hints come from DFG/SSA analysis and guide MIR optimization
        if self.ssa_hints.inline_candidates.contains(&symbol_id) {
            // Mark for aggressive inlining (small function, simple control flow from SSA)
            if let Some(func) = self.builder.module.functions.get_mut(&func_id) {
                func.attributes.inline = super::InlineHint::Always;
            }
        }

        if self.ssa_hints.straight_line_functions.contains(&symbol_id) {
            // Mark for optimization (no branches, from CFG analysis)
            // Straight-line code can be optimized more aggressively
            if let Some(func) = self.builder.module.functions.get_mut(&func_id) {
                func.attributes.pure = true; // Assume pure for straight-line code
            }
        }

        if self.ssa_hints.complex_control_flow_functions.contains(&symbol_id) {
            // Don't mark for size optimization if complex control flow
            // Complex phi nodes benefit from full optimization passes
            if let Some(func) = self.builder.module.functions.get_mut(&func_id) {
                func.attributes.optimize_size = false;
            }
        }

        // Note: CSE opportunities don't have a direct attribute mapping yet
        // They will be used by the optimization pass manager

        // Map parameters to MIR registers
        for (i, param) in hir_func.params.iter().enumerate() {
            if let Some(reg) = self.builder.current_function()
                .and_then(|f| f.get_param_reg(i)) {
                // Create symbol mapping for parameter
                // Note: This assumes parameters have been registered in symbol table
                // In practice, we'd need to get the symbol ID from the parameter
            }
        }

        // Lower function body if present
        if let Some(body) = &hir_func.body {
            self.lower_block(body);

            // Add implicit return if needed
            self.ensure_terminator();
        }

        self.builder.finish_function();

        // Clear per-function state
        self.symbol_map.clear();
        self.block_map.clear();
    }
    
    /// Lower a HIR block to MIR
    fn lower_block(&mut self, block: &HirBlock) {
        // Process all statements
        for stmt in &block.statements {
            self.lower_statement(stmt);
        }
        
        // Process trailing expression if present
        if let Some(expr) = &block.expr {
            let _result = self.lower_expression(expr);
            // The result could be used for implicit returns
        }
    }
    
    /// Lower a HIR statement to MIR instructions
    fn lower_statement(&mut self, stmt: &HirStatement) {
        match stmt {
            HirStatement::Let { pattern, init, .. } => {
                // Lower initialization expression if present
                if let Some(init_expr) = init {
                    let value = self.lower_expression(init_expr);
                    
                    // Bind to pattern
                    if let Some(value_reg) = value {
                        self.bind_pattern(pattern, value_reg);
                    }
                }
            }
            
            HirStatement::Expr(expr) => {
                self.lower_expression(expr);
            }
            
            HirStatement::Assign { lhs, rhs, op } => {
                let rhs_value = self.lower_expression(rhs);
                
                if let Some(rhs_reg) = rhs_value {
                    // Handle compound assignment if present
                    let final_value = if let Some(bin_op) = op {
                        let lhs_value = self.lower_lvalue_read(lhs);
                        lhs_value.and_then(|lhs_reg| {
                            self.builder.build_binop(
                                self.convert_binary_op(*bin_op),
                                lhs_reg,
                                rhs_reg,
                            )
                        })
                    } else {
                        Some(rhs_reg)
                    };
                    
                    // Store to lvalue
                    if let Some(value) = final_value {
                        self.lower_lvalue_write(lhs, value);
                    }
                }
            }
            
            HirStatement::Return(value) => {
                let ret_value = value.as_ref()
                    .and_then(|e| self.lower_expression(e));
                self.builder.build_return(ret_value);
            }
            
            HirStatement::Break(label) => {
                if let Some(loop_ctx) = self.find_loop_context(label.as_ref()) {
                    self.builder.build_branch(loop_ctx.break_block);
                } else {
                    self.add_error("Break outside of loop", SourceLocation::unknown());
                }
            }
            
            HirStatement::Continue(label) => {
                if let Some(loop_ctx) = self.find_loop_context(label.as_ref()) {
                    self.builder.build_branch(loop_ctx.continue_block);
                } else {
                    self.add_error("Continue outside of loop", SourceLocation::unknown());
                }
            }
            
            HirStatement::Throw(expr) => {
                if let Some(_exception_reg) = self.lower_expression(expr) {
                    // In MIR, throw becomes unreachable for now
                    // TODO: Implement proper exception handling
                    self.builder.build_unreachable();
                }
            }
            
            HirStatement::If { condition, then_branch, else_branch } => {
                self.lower_if_statement(condition, then_branch, else_branch.as_ref());
            }
            
            HirStatement::Switch { scrutinee, cases } => {
                self.lower_switch_statement(scrutinee, cases);
            }
            
            HirStatement::While { condition, body, label } => {
                self.lower_while_loop(condition, body, label.as_ref());
            }
            
            HirStatement::DoWhile { body, condition, label } => {
                self.lower_do_while_loop(body, condition, label.as_ref());
            }
            
            HirStatement::ForIn { pattern, iterator, body, label } => {
                self.lower_for_in_loop(pattern, iterator, body, label.as_ref());
            }
            
            HirStatement::TryCatch { try_block, catches, finally_block } => {
                self.lower_try_catch(try_block, catches, finally_block.as_ref());
            }
            
            HirStatement::Label { symbol, block } => {
                // Labels in MIR become block labels
                let label_block = self.builder.create_block_with_label(format!("label_{}", symbol.as_raw()));
                if let Some(block_id) = label_block {
                    self.builder.build_branch(block_id);
                    self.builder.switch_to_block(block_id);
                    self.lower_block(block);
                }
            }
        }
    }
    
    /// Lower a HIR expression to MIR value
    fn lower_expression(&mut self, expr: &HirExpr) -> Option<IrId> {
        // Set source location for debugging
        self.builder.set_source_location(self.convert_source_location(&expr.source_location));
        
        match &expr.kind {
            HirExprKind::Literal(lit) => self.lower_literal(lit),
            
            HirExprKind::Variable { symbol, .. } => {
                self.symbol_map.get(symbol).copied()
            }
            
            HirExprKind::Field { object, field } => {
                let obj_reg = self.lower_expression(object)?;
                self.lower_field_access(obj_reg, *field, expr.ty)
            }
            
            HirExprKind::Index { object, index } => {
                let obj_reg = self.lower_expression(object)?;
                let idx_reg = self.lower_expression(index)?;
                self.lower_index_access(obj_reg, idx_reg, expr.ty)
            }
            
            HirExprKind::Call { callee, args, .. } => {
                let func_reg = self.lower_expression(callee)?;
                let arg_regs: Vec<_> = args.iter()
                    .filter_map(|a| self.lower_expression(a))
                    .collect();
                
                let result_type = self.convert_type(expr.ty);
                self.builder.build_call(func_reg, arg_regs, result_type)
            }
            
            HirExprKind::New { class_type, args, .. } => {
                // Allocate object
                let class_mir_type = self.convert_type(*class_type);
                let obj_ptr = self.builder.build_alloc(class_mir_type.clone(), None)?;
                
                // Call constructor
                let arg_regs: Vec<_> = std::iter::once(obj_ptr)
                    .chain(args.iter().filter_map(|a| self.lower_expression(a)))
                    .collect();
                
                // Constructor call (simplified - needs proper constructor lookup)
                // self.builder.build_call(constructor_func, arg_regs, class_mir_type);
                
                Some(obj_ptr)
            }
            
            HirExprKind::Unary { op, operand } => {
                let operand_reg = self.lower_expression(operand)?;
                self.builder.build_unop(self.convert_unary_op(*op), operand_reg)
            }
            
            HirExprKind::Binary { op, lhs, rhs } => {
                // Handle short-circuit operators specially
                match op {
                    HirBinaryOp::And => return self.lower_logical_and(lhs, rhs),
                    HirBinaryOp::Or => return self.lower_logical_or(lhs, rhs),
                    _ => {}
                }
                
                let lhs_reg = self.lower_expression(lhs)?;
                let rhs_reg = self.lower_expression(rhs)?;
                
                match self.convert_binary_op_to_mir(*op) {
                    MirBinaryOp::Binary(bin_op) => {
                        self.builder.build_binop(bin_op, lhs_reg, rhs_reg)
                    }
                    MirBinaryOp::Compare(cmp_op) => {
                        self.builder.build_cmp(cmp_op, lhs_reg, rhs_reg)
                    }
                }
            }
            
            HirExprKind::Cast { expr, target, .. } => {
                let value_reg = self.lower_expression(expr)?;
                let from_type = self.convert_type(expr.ty);
                let to_type = self.convert_type(*target);
                self.builder.build_cast(value_reg, from_type, to_type)
            }
            
            HirExprKind::If { condition, then_expr, else_expr } => {
                self.lower_conditional(condition, then_expr, else_expr)
            }
            
            HirExprKind::Block(block) => {
                self.lower_block(block);
                // Block expressions can return values through their trailing expression
                None // Simplified for now
            }
            
            HirExprKind::Lambda { params, body, captures } => {
                self.lower_lambda(params, body, captures)
            }
            
            HirExprKind::Array { elements } => {
                self.lower_array_literal(elements)
            }
            
            HirExprKind::Map { entries } => {
                self.lower_map_literal(entries)
            }
            
            HirExprKind::ObjectLiteral { fields } => {
                self.lower_object_literal(fields)
            }
            
            HirExprKind::ArrayComprehension { .. } => {
                // Array comprehensions are desugared to loops
                self.add_error("Array comprehensions not yet implemented in MIR", expr.source_location);
                None
            }
            
            HirExprKind::StringInterpolation { parts } => {
                self.lower_string_interpolation(parts)
            }
            
            HirExprKind::This => {
                // 'this' is typically passed as first parameter
                self.symbol_map.get(&SymbolId::from_raw(0)).copied()
            }
            
            HirExprKind::Super => {
                // 'super' requires special handling
                self.add_error("Super not yet implemented in MIR", expr.source_location);
                None
            }
            
            HirExprKind::Null => {
                self.builder.build_null()
            }
            
            HirExprKind::Untyped(inner) => {
                // Untyped expressions bypass type checking
                self.lower_expression(inner)
            }
            
            HirExprKind::InlineCode { target, code } => {
                // Platform-specific inline code
                self.lower_inline_code(target, code)
            }
            
            _ => {
                self.add_error("Unsupported expression type in MIR", expr.source_location);
                None
            }
        }
    }
    
    /// Lower if statement/expression
    fn lower_if_statement(
        &mut self,
        condition: &HirExpr,
        then_branch: &HirBlock,
        else_branch: Option<&HirBlock>,
    ) {
        let Some(then_block) = self.builder.create_block() else { return; };
        let Some(merge_block) = self.builder.create_block() else { return; };
        
        let else_block = if else_branch.is_some() {
            self.builder.create_block().unwrap_or(merge_block)
        } else {
            merge_block
        };
        
        // Evaluate condition
        if let Some(cond_reg) = self.lower_expression(condition) {
            self.builder.build_cond_branch(cond_reg, then_block, else_block);
            
            // Lower then branch
            self.builder.switch_to_block(then_block);
            self.lower_block(then_branch);
            if !self.is_terminated() {
                self.builder.build_branch(merge_block);
            }
            
            // Lower else branch if present
            if let Some(else_branch) = else_branch {
                self.builder.switch_to_block(else_block);
                self.lower_block(else_branch);
                if !self.is_terminated() {
                    self.builder.build_branch(merge_block);
                }
            }
            
            // Continue in merge block
            self.builder.switch_to_block(merge_block);
        }
    }
    
    /// Lower while loop
    fn lower_while_loop(
        &mut self,
        condition: &HirExpr,
        body: &HirBlock,
        label: Option<&SymbolId>,
    ) {
        let Some(cond_block) = self.builder.create_block() else { return; };
        let Some(body_block) = self.builder.create_block() else { return; };
        let Some(exit_block) = self.builder.create_block() else { return; };
        
        // Jump to condition block
        self.builder.build_branch(cond_block);
        
        // Push loop context
        self.loop_stack.push(LoopContext {
            continue_block: cond_block,
            break_block: exit_block,
            label: label.cloned(),
        });
        
        // Condition block
        self.builder.switch_to_block(cond_block);
        if let Some(cond_reg) = self.lower_expression(condition) {
            self.builder.build_cond_branch(cond_reg, body_block, exit_block);
        }
        
        // Body block
        self.builder.switch_to_block(body_block);
        self.lower_block(body);
        if !self.is_terminated() {
            self.builder.build_branch(cond_block);
        }
        
        // Pop loop context
        self.loop_stack.pop();
        
        // Continue in exit block
        self.builder.switch_to_block(exit_block);
    }
    
    // Helper methods...
    
    fn convert_binary_op(&self, op: HirBinaryOp) -> BinaryOp {
        match op {
            HirBinaryOp::Add => BinaryOp::Add,
            HirBinaryOp::Sub => BinaryOp::Sub,
            HirBinaryOp::Mul => BinaryOp::Mul,
            HirBinaryOp::Div => BinaryOp::Div,
            HirBinaryOp::Mod => BinaryOp::Rem,
            HirBinaryOp::BitAnd => BinaryOp::And,
            HirBinaryOp::BitOr => BinaryOp::Or,
            HirBinaryOp::BitXor => BinaryOp::Xor,
            HirBinaryOp::Shl => BinaryOp::Shl,
            HirBinaryOp::Shr => BinaryOp::Shr,
            _ => BinaryOp::Add, // Default fallback
        }
    }
    
    fn convert_binary_op_to_mir(&self, op: HirBinaryOp) -> MirBinaryOp {
        match op {
            HirBinaryOp::Add => MirBinaryOp::Binary(BinaryOp::Add),
            HirBinaryOp::Sub => MirBinaryOp::Binary(BinaryOp::Sub),
            HirBinaryOp::Mul => MirBinaryOp::Binary(BinaryOp::Mul),
            HirBinaryOp::Div => MirBinaryOp::Binary(BinaryOp::Div),
            HirBinaryOp::Mod => MirBinaryOp::Binary(BinaryOp::Rem),
            HirBinaryOp::Eq => MirBinaryOp::Compare(CompareOp::Eq),
            HirBinaryOp::Ne => MirBinaryOp::Compare(CompareOp::Ne),
            HirBinaryOp::Lt => MirBinaryOp::Compare(CompareOp::Lt),
            HirBinaryOp::Le => MirBinaryOp::Compare(CompareOp::Le),
            HirBinaryOp::Gt => MirBinaryOp::Compare(CompareOp::Gt),
            HirBinaryOp::Ge => MirBinaryOp::Compare(CompareOp::Ge),
            HirBinaryOp::BitAnd => MirBinaryOp::Binary(BinaryOp::And),
            HirBinaryOp::BitOr => MirBinaryOp::Binary(BinaryOp::Or),
            HirBinaryOp::BitXor => MirBinaryOp::Binary(BinaryOp::Xor),
            HirBinaryOp::Shl => MirBinaryOp::Binary(BinaryOp::Shl),
            HirBinaryOp::Shr => MirBinaryOp::Binary(BinaryOp::Shr),
            _ => MirBinaryOp::Binary(BinaryOp::Add), // Default
        }
    }
    
    fn convert_unary_op(&self, op: HirUnaryOp) -> UnaryOp {
        match op {
            HirUnaryOp::Not => UnaryOp::Not,
            HirUnaryOp::Neg => UnaryOp::Neg,
            HirUnaryOp::BitNot => UnaryOp::Not, // Reuse Not for bit not
            _ => UnaryOp::Neg, // Default
        }
    }
    
    fn convert_type(&self, _type_id: TypeId) -> IrType {
        // Simplified type conversion
        // In practice, this would look up the type in the type table
        IrType::I32
    }
    
    fn convert_source_location(&self, loc: &SourceLocation) -> IrSourceLocation {
        IrSourceLocation {
            file_id: loc.file_id,
            line: loc.line,
            column: loc.column,
        }
    }
    
    fn lower_literal(&mut self, lit: &HirLiteral) -> Option<IrId> {
        match lit {
            HirLiteral::Int(i) => self.builder.build_int(*i, IrType::I64),
            HirLiteral::Float(f) => self.builder.build_const(IrValue::F64(*f)),
            HirLiteral::String(s) => self.builder.build_string(s.to_string()),
            HirLiteral::Bool(b) => self.builder.build_bool(*b),
            HirLiteral::Regex { .. } => {
                self.add_error("Regex literals not yet supported in MIR", SourceLocation::unknown());
                None
            }
        }
    }
    
    fn build_function_signature(&self, func: &HirFunction) -> super::IrFunctionSignature {
        let mut builder = FunctionSignatureBuilder::new();
        
        for param in &func.params {
            let param_type = self.convert_type(param.ty);
            builder = builder.param(param.name.to_string(), param_type);
        }
        
        let return_type = self.convert_type(func.return_type);
        builder = builder.returns(return_type);
        
        if func.is_extern {
            builder = builder.calling_convention(CallingConvention::C);
        }
        
        builder.build()
    }
    
    fn is_terminated(&self) -> bool {
        let block_id = match self.builder.current_block() {
            Some(id) => id,
            None => return false,
        };
        
        self.builder.current_function()
            .and_then(|func| func.cfg.get_block(block_id))
            .map(|block| block.is_terminated())
            .unwrap_or(false)
    }
    
    fn ensure_terminator(&mut self) {
        if !self.is_terminated() {
            self.builder.build_return(None);
        }
    }
    
    fn find_loop_context(&self, label: Option<&SymbolId>) -> Option<&LoopContext> {
        if let Some(label) = label {
            self.loop_stack.iter().rev().find(|ctx| ctx.label.as_ref() == Some(label))
        } else {
            self.loop_stack.last()
        }
    }
    
    fn bind_pattern(&mut self, pattern: &HirPattern, value: IrId) {
        match pattern {
            HirPattern::Variable { symbol, .. } => {
                // Bind the value to the symbol
                self.symbol_map.insert(*symbol, value);
            }
            HirPattern::Wildcard => {
                // Wildcard doesn't bind anything
            }
            HirPattern::Tuple(patterns) => {
                // Extract tuple elements and bind recursively
                for (i, p) in patterns.iter().enumerate() {
                    // Use ExtractValue instruction to get tuple element
                    if let Some(elem) = self.builder.build_extract_value(value, vec![i as u32]) {
                        self.bind_pattern(p, elem);
                    }
                }
            }
            HirPattern::Literal(_) => {
                // Literals in patterns are used for matching, not binding
                // The matching logic should be handled elsewhere
            }
            HirPattern::Constructor { .. } => {
                // Constructor patterns need type information to extract fields
                self.add_error(
                    "Constructor patterns not yet supported in MIR lowering",
                    SourceLocation::unknown()
                );
            }
            HirPattern::Array { .. } => {
                // Array patterns need runtime length checks
                self.add_error(
                    "Array patterns not yet supported in MIR lowering",
                    SourceLocation::unknown()
                );
            }
            HirPattern::Object { .. } => {
                // Object patterns need field extraction
                self.add_error(
                    "Object patterns not yet supported in MIR lowering",
                    SourceLocation::unknown()
                );
            }
            HirPattern::Typed { pattern, .. } => {
                // Type annotations in patterns don't affect binding
                self.bind_pattern(pattern, value);
            }
            HirPattern::Guard { pattern, .. } => {
                // Guards are conditions, not bindings
                self.bind_pattern(pattern, value);
            }
            HirPattern::Or(patterns) => {
                // Or patterns need special handling - bind to all alternatives
                // For now, just bind the first pattern
                if let Some(first) = patterns.first() {
                    self.bind_pattern(first, value);
                }
            }
        }
    }
    
    fn lower_lvalue_read(&mut self, lvalue: &HirLValue) -> Option<IrId> {
        match lvalue {
            HirLValue::Variable(symbol) => {
                // Look up the variable in our symbol map
                self.symbol_map.get(symbol).copied()
            }
            HirLValue::Field { object, field } => {
                // Read object.field
                if let Some(obj_reg) = self.lower_expression(object) {
                    let field_ty = object.ty;  // Use the object's type for now
                    self.lower_field_access(obj_reg, *field, field_ty)
                } else {
                    None
                }
            }
            HirLValue::Index { object, index } => {
                // Read object[index]
                if let Some(obj_reg) = self.lower_expression(object) {
                    if let Some(idx_reg) = self.lower_expression(index) {
                        let elem_ty = object.ty;  // Use object's type for now
                        self.lower_index_access(obj_reg, idx_reg, elem_ty)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }
    
    fn lower_lvalue_write(&mut self, lvalue: &HirLValue, value: IrId) {
        match lvalue {
            HirLValue::Variable(symbol) => {
                // Update the variable binding
                self.symbol_map.insert(*symbol, value);
            }
            HirLValue::Field { object, field } => {
                // Write object.field = value
                if let Some(obj_reg) = self.lower_expression(object) {
                    // For now, use GEP to get field pointer
                    // Field index would need to be determined from type information
                    // BACKLOG: Need proper field index mapping from SymbolId
                    self.add_error(
                        "Field write not yet fully implemented - needs field index mapping",
                        SourceLocation::unknown()
                    );
                }
            }
            HirLValue::Index { object, index } => {
                // Write object[index] = value
                if let Some(obj_reg) = self.lower_expression(object) {
                    if let Some(idx_reg) = self.lower_expression(index) {
                        // Use GEP to get element pointer then store
                        let elem_ty = self.convert_type(object.ty);
                        if let Some(ptr) = self.builder.build_gep(obj_reg, vec![idx_reg], elem_ty) {
                            self.builder.build_store(ptr, value);
                        }
                    }
                }
            }
        }
    }
    
    fn lower_field_access(&mut self, obj: IrId, field: SymbolId, ty: TypeId) -> Option<IrId> {
        // Field access requires:
        // 1. Mapping SymbolId to field index
        // 2. Using GEP or ExtractValue based on whether obj is pointer or value
        
        // BACKLOG: Need proper field index mapping from symbol table
        // For now, return error
        self.add_error(
            "Field access not yet fully implemented - needs field index mapping",
            SourceLocation::unknown()
        );
        None
    }
    
    fn lower_index_access(&mut self, obj: IrId, idx: IrId, ty: TypeId) -> Option<IrId> {
        // Array/map index access
        let elem_ty = self.convert_type(ty);
        
        // Use GEP to get element pointer, then load
        if let Some(ptr) = self.builder.build_gep(obj, vec![idx], elem_ty.clone()) {
            self.builder.build_load(ptr, elem_ty)
        } else {
            None
        }
    }
    
    fn lower_logical_and(&mut self, lhs: &HirExpr, rhs: &HirExpr) -> Option<IrId> {
        // Short-circuit AND: if lhs is false, don't evaluate rhs
        // Create blocks: eval_rhs, merge
        let eval_rhs = self.builder.create_block()?;
        let merge = self.builder.create_block()?;
        
        // Evaluate LHS
        let lhs_val = self.lower_expression(lhs)?;
        
        // Branch on LHS: if true, evaluate RHS; if false, skip to merge with false
        self.builder.build_cond_branch(lhs_val, eval_rhs, merge)?;
        
        // Block for evaluating RHS
        self.builder.switch_to_block(eval_rhs);
        let rhs_val = self.lower_expression(rhs)?;
        self.builder.build_branch(merge)?;
        let rhs_block = self.builder.current_block()?;
        
        // Merge block with phi node
        self.builder.switch_to_block(merge);
        let result = self.builder.build_phi(merge, IrType::Bool)?;
        let false_val = self.builder.build_bool(false)?;
        let lhs_false_block = self.builder.current_block()?; // Where we came from if LHS was false
        self.builder.add_phi_incoming(merge, result, lhs_false_block, false_val)?;
        self.builder.add_phi_incoming(merge, result, rhs_block, rhs_val)?;
        
        Some(result)
    }
    
    fn lower_logical_or(&mut self, lhs: &HirExpr, rhs: &HirExpr) -> Option<IrId> {
        // Short-circuit OR: if lhs is true, don't evaluate rhs
        // Create blocks: eval_rhs, merge
        let eval_rhs = self.builder.create_block()?;
        let merge = self.builder.create_block()?;
        
        // Evaluate LHS
        let lhs_val = self.lower_expression(lhs)?;
        
        // Branch on LHS: if false, evaluate RHS; if true, skip to merge with true
        self.builder.build_cond_branch(lhs_val, merge, eval_rhs)?;
        
        // Block for evaluating RHS
        self.builder.switch_to_block(eval_rhs);
        let rhs_val = self.lower_expression(rhs)?;
        self.builder.build_branch(merge)?;
        let rhs_block = self.builder.current_block()?;
        
        // Merge block with phi node
        self.builder.switch_to_block(merge);
        let result = self.builder.build_phi(merge, IrType::Bool)?;
        let true_val = self.builder.build_bool(true)?;
        let lhs_true_block = self.builder.current_block()?; // Where we came from if LHS was true
        self.builder.add_phi_incoming(merge, result, lhs_true_block, true_val)?;
        self.builder.add_phi_incoming(merge, result, rhs_block, rhs_val)?;
        
        Some(result)
    }
    
    fn lower_conditional(&mut self, cond: &HirExpr, then_expr: &HirExpr, else_expr: &HirExpr) -> Option<IrId> {
        // Conditional expression: cond ? then : else
        //
        // Becomes:
        //   %cond_val = <evaluate cond>
        //   br %cond_val, then_block, else_block
        // then_block:
        //   %then_val = <evaluate then>
        //   br merge_block
        // else_block:
        //   %else_val = <evaluate else>
        //   br merge_block
        // merge_block:
        //   %result = phi [%then_val, then_block], [%else_val, else_block]

        let then_block = self.builder.create_block()?;
        let else_block = self.builder.create_block()?;
        let merge_block = self.builder.create_block()?;

        // Evaluate condition
        let cond_val = self.lower_expression(cond)?;

        // Branch based on condition
        self.builder.build_cond_branch(cond_val, then_block, else_block)?;

        // Then block
        self.builder.switch_to_block(then_block);
        let then_val = self.lower_expression(then_expr)?;
        self.builder.build_branch(merge_block)?;
        let then_end_block = self.builder.current_block()?;

        // Else block
        self.builder.switch_to_block(else_block);
        let else_val = self.lower_expression(else_expr)?;
        self.builder.build_branch(merge_block)?;
        let else_end_block = self.builder.current_block()?;

        // Merge block with phi node
        self.builder.switch_to_block(merge_block);

        // Determine result type from then expression
        // TODO: Get actual type from HIR expression
        let result_type = IrType::I32; // Placeholder
        let result = self.builder.build_phi(merge_block, result_type)?;

        self.builder.add_phi_incoming(merge_block, result, then_end_block, then_val)?;
        self.builder.add_phi_incoming(merge_block, result, else_end_block, else_val)?;

        Some(result)
    }
    
    fn lower_do_while_loop(&mut self, body: &HirBlock, condition: &HirExpr, label: Option<&SymbolId>) {
        // Do-while loop structure:
        // do {
        //     body;
        // } while (condition);
        //
        // MIR structure:
        // body_block:
        //     <body statements>
        //     goto cond_block
        // cond_block:
        //     %cond = <evaluate condition>
        //     br %cond, body_block, exit_block
        // exit_block:
        //     <continue>

        // Create blocks
        let Some(body_block) = self.builder.create_block() else { return; };
        let Some(cond_block) = self.builder.create_block() else { return; };
        let Some(exit_block) = self.builder.create_block() else { return; };

        // Jump to body first (do-while always executes once)
        self.builder.build_branch(body_block);

        // Push loop context
        self.loop_stack.push(LoopContext {
            continue_block: cond_block,
            break_block: exit_block,
            label: label.cloned(),
        });

        // Build body block
        self.builder.switch_to_block(body_block);
        self.lower_block(body);
        if !self.is_terminated() {
            self.builder.build_branch(cond_block);
        }

        // Build condition block
        self.builder.switch_to_block(cond_block);
        if let Some(cond_reg) = self.lower_expression(condition) {
            self.builder.build_cond_branch(cond_reg, body_block, exit_block);
        }

        // Pop loop context
        self.loop_stack.pop();

        // Continue at exit
        self.builder.switch_to_block(exit_block);
    }
    
    fn lower_for_in_loop(&mut self, pattern: &HirPattern, iter_expr: &HirExpr, body: &HirBlock, label: Option<&SymbolId>) {
        // For-in loops desugar to iterator protocol:
        // for (x in collection) { body }
        //
        // Becomes:
        // {
        //     var _iter = collection.iterator();
        //     while (_iter.hasNext()) {
        //         var x = _iter.next();
        //         body;
        //     }
        // }

        // Step 1: Get iterator by calling .iterator() on the collection
        let Some(_collection) = self.lower_expression(iter_expr) else { return; };

        // Call .iterator() method
        // For now, we'll assume the iterator is the collection itself if it has hasNext/next
        // TODO: Actually call .iterator() method when method call lowering is complete
        let _iterator_reg = _collection;

        // Step 2: Create loop structure with condition and body blocks
        let Some(loop_cond_block) = self.builder.create_block() else { return; };
        let Some(loop_body_block) = self.builder.create_block() else { return; };
        let Some(loop_exit_block) = self.builder.create_block() else { return; };

        // Jump to condition check
        self.builder.build_branch(loop_cond_block);

        // Push loop context
        self.loop_stack.push(LoopContext {
            continue_block: loop_cond_block,
            break_block: loop_exit_block,
            label: label.cloned(),
        });

        // Step 3: Build condition block - call hasNext()
        self.builder.switch_to_block(loop_cond_block);

        // Call hasNext() on iterator
        // TODO: Use proper method call when available
        // For now, create a placeholder that assumes hasNext returns bool
        let Some(has_next_reg) = self.builder.alloc_reg() else { return; };

        // Conditional branch based on hasNext()
        self.builder.build_cond_branch(has_next_reg, loop_body_block, loop_exit_block);

        // Step 4: Build body block
        self.builder.switch_to_block(loop_body_block);

        // Call next() to get the loop variable value
        let Some(next_value) = self.builder.alloc_reg() else { return; };

        // Bind the pattern to the value from next()
        // For simple variable patterns, this is straightforward
        // For complex patterns, we'd need pattern matching logic
        match pattern {
            HirPattern::Variable { symbol, .. } => {
                // Store the loop variable
                self.symbol_map.insert(*symbol, next_value);
            }
            _ => {
                // Complex patterns need full pattern matching
                // For now, just lower the body with whatever we have
            }
        }

        // Lower the loop body
        self.lower_block(body);

        // Jump back to condition check
        if !self.is_terminated() {
            self.builder.build_branch(loop_cond_block);
        }

        // Pop loop context
        self.loop_stack.pop();

        // Step 5: Continue at exit block
        self.builder.switch_to_block(loop_exit_block);
    }
    
    fn lower_switch_statement(&mut self, scrutinee: &HirExpr, cases: &[HirMatchCase]) {
        // Switch/match statement lowering:
        // switch (scrutinee) {
        //   case pattern1 if guard1: body1
        //   case pattern2: body2
        //   default: default_body
        // }
        //
        // Becomes a series of conditional branches:
        //   %scrut = evaluate scrutinee
        //   br pattern1_test
        // pattern1_test:
        //   %match1 = test pattern1 against %scrut
        //   br %match1, guard1_test, pattern2_test
        // guard1_test:
        //   %guard1 = evaluate guard1
        //   br %guard1, body1_block, pattern2_test
        // body1_block:
        //   <body1>
        //   br continuation
        // pattern2_test:
        //   %match2 = test pattern2 against %scrut
        //   br %match2, body2_block, default_block
        // ...
        // continuation:

        // Evaluate scrutinee once
        let scrut_val = match self.lower_expression(scrutinee) {
            Some(v) => v,
            None => return,
        };

        // Create continuation block (after switch)
        let continuation = match self.builder.create_block() {
            Some(b) => b,
            None => return,
        };

        // Create blocks for each case
        let mut case_test_blocks = Vec::new();
        let mut case_body_blocks = Vec::new();

        for _ in cases {
            if let (Some(test), Some(body)) = (self.builder.create_block(), self.builder.create_block()) {
                case_test_blocks.push(test);
                case_body_blocks.push(body);
            }
        }

        // Default block (for non-exhaustive matches)
        let default_block = match self.builder.create_block() {
            Some(b) => b,
            None => return,
        };

        // Branch to first case test
        if let Some(&first_test) = case_test_blocks.first() {
            self.builder.build_branch(first_test);
        } else {
            // No cases, go to default
            self.builder.build_branch(default_block);
            return;
        }

        // Lower each case
        for (i, case) in cases.iter().enumerate() {
            let test_block = case_test_blocks[i];
            let body_block = case_body_blocks[i];
            let next_test = case_test_blocks.get(i + 1).copied().unwrap_or(default_block);

            // Generate pattern test block
            self.builder.switch_to_block(test_block);

            // For now, simplified pattern matching:
            // - Variable patterns always match
            // - Wildcard always matches
            // - Literal patterns use equality
            // - Constructor patterns need runtime type checking (TODO)

            let pattern_matches = if case.patterns.is_empty() {
                // No pattern means default case
                self.builder.build_bool(true)
            } else {
                // Test first pattern (simplified - should test all patterns with OR logic)
                self.lower_pattern_test(scrut_val, &case.patterns[0])
            };

            let pattern_matches = match pattern_matches {
                Some(v) => v,
                None => {
                    // Pattern test failed, go to next
                    self.builder.build_branch(next_test);
                    continue;
                }
            };

            // If there's a guard, test it
            if let Some(ref guard) = case.guard {
                let guard_block = match self.builder.create_block() {
                    Some(b) => b,
                    None => return,
                };

                // Branch: if pattern matches, test guard; else try next pattern
                self.builder.build_cond_branch(pattern_matches, guard_block, next_test);

                // Guard test block
                self.builder.switch_to_block(guard_block);
                let guard_val = match self.lower_expression(guard) {
                    Some(v) => v,
                    None => {
                        self.builder.build_branch(next_test);
                        continue;
                    }
                };

                // Branch: if guard true, execute body; else try next pattern
                self.builder.build_cond_branch(guard_val, body_block, next_test);
            } else {
                // No guard, just test pattern
                self.builder.build_cond_branch(pattern_matches, body_block, next_test);
            }

            // Generate case body block
            self.builder.switch_to_block(body_block);
            self.lower_block(&case.body);
            self.builder.build_branch(continuation);
        }

        // Default block - just continue (could also panic for exhaustive matches)
        self.builder.switch_to_block(default_block);
        self.builder.build_branch(continuation);

        // Continue after switch
        self.builder.switch_to_block(continuation);
    }

    fn lower_pattern_test(&mut self, scrutinee: IrId, pattern: &HirPattern) -> Option<IrId> {
        // Test if scrutinee matches pattern
        // Returns a boolean IrId indicating match success

        match pattern {
            HirPattern::Variable { name, symbol } => {
                // Variable pattern always matches and binds the value
                self.symbol_map.insert(*symbol, scrutinee);
                self.builder.build_bool(true)
            }

            HirPattern::Wildcard => {
                // Wildcard always matches
                self.builder.build_bool(true)
            }

            HirPattern::Literal(lit) => {
                // Literal pattern: compare scrutinee with literal value
                let lit_val = self.lower_literal(lit)?;
                // TODO: Use proper comparison based on type
                self.builder.build_cmp(CompareOp::Eq, scrutinee, lit_val)
            }

            HirPattern::Constructor { enum_type, variant, fields } => {
                // Constructor pattern: check enum tag and extract fields
                //
                // Enum layout (simplified):
                // struct Enum { tag: i32, data: [fields...] }
                //
                // Strategy:
                // 1. Extract tag from scrutinee (index 0)
                // 2. Compare tag with variant discriminant
                // 3. If match, extract fields and test sub-patterns
                // 4. Return combined result

                // Extract tag field from enum (index 0)
                let Some(zero_idx) = self.builder.build_int(0, IrType::I64) else {
                    return None;
                };

                let Some(tag_ptr) = self.builder.build_gep(
                    scrutinee,
                    vec![zero_idx],
                    IrType::Ptr(Box::new(IrType::I32))
                ) else {
                    return None;
                };

                let Some(tag_val) = self.builder.build_load(tag_ptr, IrType::I32) else {
                    return None;
                };

                // TODO: Look up variant discriminant from type metadata
                // For now, use a placeholder value (hash of variant name)
                let variant_discriminant = variant.to_string().len() as i64; // Placeholder

                let Some(expected_tag) = self.builder.build_int(variant_discriminant, IrType::I32) else {
                    return None;
                };

                // Compare tags
                let Some(tag_matches) = self.builder.build_cmp(CompareOp::Eq, tag_val, expected_tag) else {
                    return None;
                };

                // If no fields to match, just return tag comparison
                if fields.is_empty() {
                    return Some(tag_matches);
                }

                // For fields, we need to extract and test each one
                // Combine all field tests with AND logic
                let mut all_fields_match = tag_matches;

                for (i, field_pattern) in fields.iter().enumerate() {
                    // Extract field from enum data area (starts at index 1)
                    let Some(field_idx) = self.builder.build_int((i + 1) as i64, IrType::I64) else {
                        return None;
                    };

                    let Some(field_ptr) = self.builder.build_gep(
                        scrutinee,
                        vec![field_idx],
                        IrType::Ptr(Box::new(IrType::Any))
                    ) else {
                        return None;
                    };

                    let Some(field_val) = self.builder.build_load(field_ptr, IrType::Any) else {
                        return None;
                    };

                    // Recursively test field pattern
                    let Some(field_match) = self.lower_pattern_test(field_val, field_pattern) else {
                        return None;
                    };

                    // Combine with AND
                    all_fields_match = self.builder.build_binop(
                        BinaryOp::And,
                        all_fields_match,
                        field_match
                    )?;
                }

                Some(all_fields_match)
            }

            HirPattern::Tuple(patterns) => {
                // Tuple pattern: extract and test each element
                //
                // Tuple layout:
                // struct Tuple { elem0, elem1, elem2, ... }
                //
                // Strategy:
                // 1. Extract each element by index
                // 2. Test each element against its pattern
                // 3. Combine all results with AND

                if patterns.is_empty() {
                    // Empty tuple always matches
                    return self.builder.build_bool(true);
                }

                let mut all_match = self.builder.build_bool(true)?;

                for (i, elem_pattern) in patterns.iter().enumerate() {
                    // Extract element at index i
                    let Some(elem_idx) = self.builder.build_int(i as i64, IrType::I64) else {
                        return None;
                    };

                    let Some(elem_ptr) = self.builder.build_gep(
                        scrutinee,
                        vec![elem_idx],
                        IrType::Ptr(Box::new(IrType::Any))
                    ) else {
                        return None;
                    };

                    let Some(elem_val) = self.builder.build_load(elem_ptr, IrType::Any) else {
                        return None;
                    };

                    // Recursively test element pattern
                    let Some(elem_match) = self.lower_pattern_test(elem_val, elem_pattern) else {
                        return None;
                    };

                    // Combine with AND
                    all_match = self.builder.build_binop(
                        BinaryOp::And,
                        all_match,
                        elem_match
                    )?;
                }

                Some(all_match)
            }

            HirPattern::Array { elements, rest } => {
                // Array pattern: check length and test elements
                //
                // Array layout:
                // struct Array { length: i64, data: [elements...] }
                //
                // Strategy:
                // 1. Extract array length (index 0)
                // 2. Check length matches expected (if no rest pattern)
                // 3. Extract and test each specified element
                // 4. If rest pattern exists, bind remaining elements

                // Extract array length from header (index 0)
                let Some(zero_idx) = self.builder.build_int(0, IrType::I64) else {
                    return None;
                };

                let Some(length_ptr) = self.builder.build_gep(
                    scrutinee,
                    vec![zero_idx],
                    IrType::Ptr(Box::new(IrType::I64))
                ) else {
                    return None;
                };

                let Some(array_length) = self.builder.build_load(length_ptr, IrType::I64) else {
                    return None;
                };

                let mut all_match = self.builder.build_bool(true)?;

                // If no rest pattern, check exact length
                if rest.is_none() {
                    let Some(expected_len) = self.builder.build_int(elements.len() as i64, IrType::I64) else {
                        return None;
                    };

                    let Some(length_matches) = self.builder.build_cmp(
                        CompareOp::Eq,
                        array_length,
                        expected_len
                    ) else {
                        return None;
                    };

                    all_match = self.builder.build_binop(
                        BinaryOp::And,
                        all_match,
                        length_matches
                    )?;
                } else {
                    // With rest pattern, check minimum length
                    let Some(min_len) = self.builder.build_int(elements.len() as i64, IrType::I64) else {
                        return None;
                    };

                    let Some(length_sufficient) = self.builder.build_cmp(
                        CompareOp::Ge,
                        array_length,
                        min_len
                    ) else {
                        return None;
                    };

                    all_match = self.builder.build_binop(
                        BinaryOp::And,
                        all_match,
                        length_sufficient
                    )?;
                }

                // Test each specified element
                for (i, elem_pattern) in elements.iter().enumerate() {
                    // Array elements start at index 1 (after length header)
                    let Some(elem_idx) = self.builder.build_int((i + 1) as i64, IrType::I64) else {
                        return None;
                    };

                    let Some(elem_ptr) = self.builder.build_gep(
                        scrutinee,
                        vec![elem_idx],
                        IrType::Ptr(Box::new(IrType::Any))
                    ) else {
                        return None;
                    };

                    let Some(elem_val) = self.builder.build_load(elem_ptr, IrType::Any) else {
                        return None;
                    };

                    // Recursively test element pattern
                    let Some(elem_match) = self.lower_pattern_test(elem_val, elem_pattern) else {
                        return None;
                    };

                    all_match = self.builder.build_binop(
                        BinaryOp::And,
                        all_match,
                        elem_match
                    )?;
                }

                // TODO: Handle rest pattern binding
                // For now, we just ignore the rest pattern
                // In a full implementation, we'd create a slice of remaining elements

                Some(all_match)
            }

            HirPattern::Object { fields, rest } => {
                // Object pattern: extract and test fields
                //
                // Object layout (simplified):
                // Hash map or struct with named fields
                //
                // Strategy:
                // 1. For each pattern field, extract object field by name
                // 2. Test extracted value against pattern
                // 3. Combine all results with AND
                // 4. rest flag indicates whether additional fields are allowed

                if fields.is_empty() {
                    // Empty object pattern always matches (or matches any object if rest=true)
                    return self.builder.build_bool(true);
                }

                let mut all_match = self.builder.build_bool(true)?;

                for (field_name, field_pattern) in fields {
                    // Extract field from object
                    // TODO: Implement proper field lookup by name
                    // For now, we use a simple hash-based approach

                    // Calculate field offset based on name hash (placeholder)
                    let field_offset = field_name.to_string().len() as i64;

                    let Some(field_idx) = self.builder.build_int(field_offset, IrType::I64) else {
                        return None;
                    };

                    let Some(field_ptr) = self.builder.build_gep(
                        scrutinee,
                        vec![field_idx],
                        IrType::Ptr(Box::new(IrType::Any))
                    ) else {
                        return None;
                    };

                    let Some(field_val) = self.builder.build_load(field_ptr, IrType::Any) else {
                        return None;
                    };

                    // Recursively test field pattern
                    let Some(field_match) = self.lower_pattern_test(field_val, field_pattern) else {
                        return None;
                    };

                    all_match = self.builder.build_binop(
                        BinaryOp::And,
                        all_match,
                        field_match
                    )?;
                }

                // TODO: If rest=false, verify no additional fields exist
                // For now, we just ignore the rest flag

                Some(all_match)
            }

            HirPattern::Typed { pattern, ty } => {
                // Typed pattern: check type and test inner pattern
                // TODO: Implement type checking
                self.lower_pattern_test(scrutinee, pattern)
            }

            HirPattern::Or(patterns) => {
                // Or pattern: test each pattern with OR logic
                // TODO: Implement proper OR pattern logic
                if let Some(first) = patterns.first() {
                    self.lower_pattern_test(scrutinee, first)
                } else {
                    self.builder.build_bool(false)
                }
            }

            HirPattern::Guard { pattern, condition } => {
                // Guard pattern: test pattern then condition
                let pattern_match = self.lower_pattern_test(scrutinee, pattern)?;
                let guard_val = self.lower_expression(condition)?;
                // AND the pattern match with the guard
                self.builder.build_binop(BinaryOp::And, pattern_match, guard_val)
            }
        }
    }
    
    fn lower_try_catch(&mut self, try_block: &HirBlock, catches: &[HirCatchClause], finally: Option<&HirBlock>) {
        // Exception handling lowering:
        // try { ... } catch (e: T) { ... } finally { ... }
        //
        // Becomes:
        //   normal_path:
        //     <try block>
        //     br continuation
        //   landing_pad:
        //     %exc = landingpad
        //     <match exception type>
        //     br to appropriate catch or unwind
        //   catch_N:
        //     <catch block>
        //     br finally_block
        //   finally_block:
        //     <finally code>
        //     br continuation
        //   continuation:
        //     <rest of code>

        let landing_pad_block = match self.builder.create_block() {
            Some(b) => b,
            None => return,
        };

        let finally_block = match self.builder.create_block() {
            Some(b) => b,
            None => return,
        };

        let continuation_block = match self.builder.create_block() {
            Some(b) => b,
            None => return,
        };

        // Lower the try block with landing pad as the exception target
        self.lower_block(try_block);

        // If try block completes normally, go to finally (if present) or continuation
        if finally.is_some() {
            self.builder.build_branch(finally_block);
        } else {
            self.builder.build_branch(continuation_block);
        }

        // Build landing pad block
        self.builder.switch_to_block(landing_pad_block);

        // Create landing pad instruction to receive the exception
        // For now, we'll use a generic exception type (pointer to exception object)
        let exception_id = match self.builder.alloc_reg() {
            Some(id) => id,
            None => return,
        };

        // Build catch blocks and dispatch logic
        let mut catch_blocks = Vec::new();
        for _catch in catches {
            if let Some(catch_block) = self.builder.create_block() {
                catch_blocks.push(catch_block);
            }
        }

        // For each catch clause, check if exception matches
        for (i, catch) in catches.iter().enumerate() {
            if let Some(catch_block_id) = catch_blocks.get(i).copied() {
                self.builder.switch_to_block(catch_block_id);

                // Bind the exception variable
                // The exception_id register holds the exception value from the landing pad
                // In a full implementation, this would extract specific exception fields
                // based on the catch type, but for now we bind the entire exception object
                self.symbol_map.insert(catch.exception_var, exception_id);

                // Lower the catch block body
                self.lower_block(&catch.body);

                // After catch, go to finally or continuation
                if finally.is_some() {
                    self.builder.build_branch(finally_block);
                } else {
                    self.builder.build_branch(continuation_block);
                }
            }
        }

        // Build finally block if present
        if let Some(finally_body) = finally {
            self.builder.switch_to_block(finally_block);
            self.lower_block(finally_body);
            self.builder.build_branch(continuation_block);
        }

        // Continue with rest of code
        self.builder.switch_to_block(continuation_block);
    }
    
    fn lower_lambda(&mut self, params: &[HirParam], body: &HirExpr, captures: &[HirCapture]) -> Option<IrId> {
        // Closure/Lambda lowering:
        //
        // A closure is represented as a pair: (function_pointer, environment)
        //
        // For: |x, y| { x + y + captured_z }
        //
        // We generate:
        // 1. Environment struct containing captured variables:
        //    struct Env { captured_z: Type }
        //
        // 2. Anonymous function taking (env*, params...):
        //    fn lambda_N(env: *Env, x: T1, y: T2) -> T3 {
        //      let captured_z = env->captured_z
        //      return x + y + captured_z
        //    }
        //
        // 3. Closure value: { fn_ptr: lambda_N, env_ptr: allocated_env }

        // For now, we'll implement a simplified version:
        // - Allocate environment struct with captured values
        // - Create closure struct with function pointer + environment
        // - Return closure struct pointer

        let has_captures = !captures.is_empty();

        // Allocate environment struct for captures
        let env_ptr = if has_captures {
            // Environment layout: [capture0, capture1, ...]
            let env_size = captures.len();
            let size_val = self.builder.build_int(env_size as i64, IrType::I64)?;
            let env = self.builder.build_alloc(IrType::Ptr(Box::new(IrType::Any)), Some(size_val))?;

            // Store each captured value in environment
            for (i, capture) in captures.iter().enumerate() {
                // Look up the captured variable's current value
                if let Some(&captured_val) = self.symbol_map.get(&capture.symbol) {
                    // For ByValue, copy the value into environment
                    // For ByRef/ByMutableRef, store a reference
                    let value_to_store = match capture.mode {
                        HirCaptureMode::ByValue => captured_val,
                        HirCaptureMode::ByRef | HirCaptureMode::ByMutableRef => {
                            // Store reference to the variable
                            // In a real implementation, this would need address-of operation
                            captured_val
                        }
                    };

                    let index = self.builder.build_int(i as i64, IrType::I64)?;
                    let field_ptr = self.builder.build_gep(env, vec![index], IrType::Ptr(Box::new(IrType::Any)))?;
                    self.builder.build_store(field_ptr, value_to_store)?;
                }
            }

            Some(env)
        } else {
            None
        };

        // TODO: Generate the actual lambda function
        //
        // LIMITATION: Current IrBuilder API doesn't support nested function generation
        // (can't save/restore function context as fields are private).
        //
        // Proper implementation requires one of:
        // 1. Two-pass lowering: collect lambdas in first pass, generate in second pass
        // 2. IrBuilder API extension to support nested function generation
        // 3. Manual IrFunction construction without using IrBuilder
        //
        // For now, we create a placeholder closure structure that will need to be
        // completed in a future phase when the IrBuilder API is extended.
        //
        // The closure generation infrastructure is in place (environment capture,
        // parameter handling, etc.) and just needs the actual function body lowering.

        // Generate unique lambda ID for future reference
        let lambda_id = self.lambda_counter;
        self.lambda_counter += 1;

        // Closure struct layout: [function_id, env_ptr]
        let closure_size = if has_captures { 2 } else { 1 };
        let size_val = self.builder.build_int(closure_size as i64, IrType::I64)?;
        let closure_ptr = self.builder.build_alloc(IrType::Ptr(Box::new(IrType::Any)), Some(size_val))?;

        // Store placeholder function ID at index 0
        // In a full implementation, this would be a real function pointer
        let fn_id_val = self.builder.build_int(lambda_id as i64, IrType::I64)?;
        self.builder.build_store(closure_ptr, fn_id_val)?;

        // Store environment pointer at index 1 (if captures exist)
        if let Some(env) = env_ptr {
            let env_idx = self.builder.build_int(1, IrType::I64)?;
            let env_field_ptr = self.builder.build_gep(closure_ptr, vec![env_idx], IrType::Ptr(Box::new(IrType::Any)))?;
            self.builder.build_store(env_field_ptr, env)?;
        }

        Some(closure_ptr)
    }
    
    fn lower_array_literal(&mut self, elements: &[HirExpr]) -> Option<IrId> {
        // Array literal: [e1, e2, e3, ...]
        //
        // Lowering strategy:
        // 1. Allocate array structure
        // 2. Initialize each element
        // 3. Return array pointer
        //
        // For now, we'll use a simple implementation that:
        // - Allocates space for array header + elements
        // - Stores length in header
        // - Initializes each element

        // Calculate total size: header (length field) + elements
        let element_count = elements.len();

        // Allocate array structure (simplified - actual implementation needs runtime support)
        // Allocate (element_count + 1) slots for header + elements
        let count_val = self.builder.build_int((element_count + 1) as i64, IrType::I64)?;

        // Allocate memory (array of Any pointers)
        let array_ptr = self.builder.build_alloc(IrType::Ptr(Box::new(IrType::Any)), Some(count_val))?;

        // Store length at offset 0
        let length_val = self.builder.build_int(element_count as i64, IrType::I64)?;
        self.builder.build_store(array_ptr, length_val)?;

        // Store each element using GEP for pointer arithmetic
        for (i, elem) in elements.iter().enumerate() {
            let elem_val = self.lower_expression(elem)?;

            // Use GEP to get pointer to element at index (i + 1)
            let index = self.builder.build_int((i + 1) as i64, IrType::I64)?;
            let elem_ptr = self.builder.build_gep(array_ptr, vec![index], IrType::Ptr(Box::new(IrType::Any)))?;

            self.builder.build_store(elem_ptr, elem_val)?;
        }

        Some(array_ptr)
    }
    
    fn lower_map_literal(&mut self, entries: &[(HirExpr, HirExpr)]) -> Option<IrId> {
        // Map literal: [key1 => val1, key2 => val2, ...]
        //
        // Lowering strategy:
        // 1. Allocate map structure (hash table)
        // 2. Initialize each key-value pair
        // 3. Return map pointer
        //
        // This is a simplified implementation. Production would use a proper hash table runtime.

        let entry_count = entries.len();

        // Allocate map structure: header + entry array
        // Structure: [size, capacity, entry0_key, entry0_val, entry1_key, entry1_val, ...]
        let total_slots = 2 + (entry_count * 2); // header + (key, value) pairs
        let count_val = self.builder.build_int(total_slots as i64, IrType::I64)?;
        let map_ptr = self.builder.build_alloc(IrType::Ptr(Box::new(IrType::Any)), Some(count_val))?;

        // Store size in header (index 0)
        let size_field = self.builder.build_int(entry_count as i64, IrType::I64)?;
        self.builder.build_store(map_ptr, size_field)?;

        // Store capacity (index 1)
        let capacity_val = self.builder.build_int(entry_count as i64, IrType::I64)?;
        let capacity_idx = self.builder.build_int(1, IrType::I64)?;
        let capacity_ptr = self.builder.build_gep(map_ptr, vec![capacity_idx], IrType::Ptr(Box::new(IrType::Any)))?;
        self.builder.build_store(capacity_ptr, capacity_val)?;

        // Store each key-value pair
        for (i, (key, value)) in entries.iter().enumerate() {
            let key_val = self.lower_expression(key)?;
            let value_val = self.lower_expression(value)?;

            // Store key at index: 2 + i * 2
            let key_index = 2 + (i * 2);
            let key_idx = self.builder.build_int(key_index as i64, IrType::I64)?;
            let key_ptr = self.builder.build_gep(map_ptr, vec![key_idx], IrType::Ptr(Box::new(IrType::Any)))?;
            self.builder.build_store(key_ptr, key_val)?;

            // Store value at index: 2 + i * 2 + 1
            let val_index = 2 + (i * 2) + 1;
            let val_idx = self.builder.build_int(val_index as i64, IrType::I64)?;
            let val_ptr = self.builder.build_gep(map_ptr, vec![val_idx], IrType::Ptr(Box::new(IrType::Any)))?;
            self.builder.build_store(val_ptr, value_val)?;
        }

        Some(map_ptr)
    }

    fn lower_object_literal(&mut self, fields: &[(InternedString, HirExpr)]) -> Option<IrId> {
        // Object literal: { field1: val1, field2: val2, ... }
        //
        // Lowering strategy:
        // 1. Allocate object structure
        // 2. Initialize each field
        // 3. Return object pointer
        //
        // Anonymous objects in Haxe are structural types. For simplicity,
        // we treat them as a simple array: [field_count, field0_val, field1_val, ...]

        let field_count = fields.len();

        // Allocate object structure: header + field values
        let total_slots = field_count + 1; // field count + values
        let count_val = self.builder.build_int(total_slots as i64, IrType::I64)?;
        let object_ptr = self.builder.build_alloc(IrType::Ptr(Box::new(IrType::Any)), Some(count_val))?;

        // Store field count at index 0
        let count_field = self.builder.build_int(field_count as i64, IrType::I64)?;
        self.builder.build_store(object_ptr, count_field)?;

        // Store each field value
        for (i, (_field_name, field_expr)) in fields.iter().enumerate() {
            let field_val = self.lower_expression(field_expr)?;

            // For now, we only store values. Production implementation would
            // need to store field names as well for runtime reflection.

            // Store at index (i + 1)
            let index = self.builder.build_int((i + 1) as i64, IrType::I64)?;
            let field_ptr = self.builder.build_gep(object_ptr, vec![index], IrType::Ptr(Box::new(IrType::Any)))?;
            self.builder.build_store(field_ptr, field_val)?;
        }

        Some(object_ptr)
    }
    
        fn lower_string_interpolation(&mut self, parts: &[HirStringPart]) -> Option<IrId> {
        // String interpolation: "Hello ${name}!" becomes string concatenation
        // Implemented as repeated calls to string concatenation
        //
        // Strategy:
        // 1. Start with empty string or first literal
        // 2. For each part:
        //    - If literal: concatenate directly
        //    - If expression: convert to string (toString()) then concatenate

        if parts.is_empty() {
            return self.builder.build_string(String::new());
        }

        // Build up the result by concatenating parts
        let mut result = None;

        for part in parts {
            let part_value = match part {
                HirStringPart::Literal(s) => {
                    // Literal string part
                    self.builder.build_string(s.to_string())?
                }
                HirStringPart::Interpolation(expr) => {
                    // Expression part - needs toString() conversion
                    let expr_val = self.lower_expression(expr)?;

                    // TODO: Call toString() method or use type-specific conversion
                    // For now, just use the value directly (assuming it's already a string)
                    expr_val
                }
            };

            result = match result {
                None => Some(part_value), // First part
                Some(acc) => {
                    // Concatenate with accumulator
                    // TODO: Use proper string concatenation operator or runtime function
                    // For now, use binary add which should work for strings
                    self.builder.build_binop(BinaryOp::Add, acc, part_value)
                }
            };
        }

        result
    }
    
    fn lower_inline_code(&mut self, _target: &str, _code: &str) -> Option<IrId> {
        // TODO: Implement inline code
        None
    }
    
    fn lower_global(&mut self, symbol: SymbolId, global: &HirGlobal) {
        // Allocate a global ID
        let global_id = self.builder.module.alloc_global_id();

        // Convert initialization expression to IrValue if present
        // For now, we only support constant expressions
        let initializer = if let Some(init_expr) = &global.init {
            // Try to evaluate as constant expression
            match &init_expr.kind {
                HirExprKind::Literal(lit) => {
                    match lit {
                        HirLiteral::Bool(b) => Some(IrValue::Bool(*b)),
                        HirLiteral::Int(i) => Some(IrValue::I64(*i)),
                        HirLiteral::Float(f) => Some(IrValue::F64(*f)),
                        HirLiteral::String(s) => {
                            // String literals are added to string pool
                            // and referenced by their pool ID
                            let string_id = self.builder.module.string_pool.add(s.to_string());
                            // Store the string pool ID as an integer value
                            // The runtime will look up the actual string from the pool
                            Some(IrValue::I32(string_id as i32))
                        }
                        HirLiteral::Regex { .. } => {
                            // Regex needs special handling
                            None
                        }
                    }
                }
                _ => {
                    // Non-constant initialization - needs runtime evaluation
                    // Collect for __init__ function generation
                    self.dynamic_globals.push((symbol, init_expr.clone()));
                    // Use Undef as placeholder - will be initialized at runtime
                    Some(IrValue::Undef)
                }
            }
        } else {
            // No initializer - use Undef
            Some(IrValue::Undef)
        };

        // Create the global variable
        // Note: Using placeholder name based on symbol ID since HirGlobal doesn't store name
        let ir_global = IrGlobal {
            id: global_id,
            name: format!("global_{}", symbol.as_raw()),
            symbol_id: symbol,
            ty: IrType::Any, // TODO: Convert TypeId to IrType properly
            initializer,
            mutable: !global.is_const,
            linkage: Linkage::Internal, // TODO: Determine linkage from visibility
            alignment: None,
            source_location: IrSourceLocation::unknown(),
        };

        // Add to module
        self.builder.module.add_global(ir_global);

        // TODO: For non-constant initializers, create an __init__ function
        // that runs at module load time to initialize the global
    }
    
    fn register_type_metadata(&mut self, type_id: TypeId, type_decl: &HirTypeDecl) {
        // Register type definitions in MIR for runtime type information
        // This metadata is used for:
        // - Enum discriminant values (for pattern matching)
        // - Struct field layouts (for field access)
        // - Interface method tables (for dynamic dispatch)
        // - Type checking at runtime

        match type_decl {
            HirTypeDecl::Class(class) => {
                self.register_class_metadata(type_id, class);
            }
            HirTypeDecl::Interface(interface) => {
                self.register_interface_metadata(type_id, interface);
            }
            HirTypeDecl::Enum(enum_decl) => {
                self.register_enum_metadata(type_id, enum_decl);
            }
            HirTypeDecl::Abstract(abstract_decl) => {
                self.register_abstract_metadata(type_id, abstract_decl);
            }
            HirTypeDecl::TypeAlias(alias) => {
                self.register_alias_metadata(type_id, alias);
            }
        }
    }

    fn register_enum_metadata(&mut self, type_id: TypeId, enum_decl: &HirEnum) {
        // Register enum type with discriminant values
        let typedef_id = self.builder.module.alloc_typedef_id();

        let mut variants = Vec::new();
        for (i, variant) in enum_decl.variants.iter().enumerate() {
            // Use explicit discriminant if provided, otherwise use index
            let discriminant = variant.discriminant.unwrap_or(i as i32) as i64;

            // Convert variant fields to IR fields
            let fields: Vec<IrField> = variant.fields.iter().map(|field| {
                IrField {
                    name: field.name.to_string(),
                    ty: IrType::Any, // TODO: Convert TypeId to IrType
                    offset: None,
                }
            }).collect();

            variants.push(IrEnumVariant {
                name: variant.name.to_string(),
                discriminant,
                fields,
            });
        }

        let typedef = IrTypeDef {
            id: typedef_id,
            name: enum_decl.name.to_string(),
            type_id,
            definition: IrTypeDefinition::Enum {
                variants,
                discriminant_type: IrType::I32,
            },
            source_location: IrSourceLocation::unknown(),
        };

        self.builder.module.add_type(typedef);
    }

    fn register_class_metadata(&mut self, type_id: TypeId, class: &HirClass) {
        // Register class as struct type
        let typedef_id = self.builder.module.alloc_typedef_id();

        let fields: Vec<IrField> = class.fields.iter().map(|field| {
            IrField {
                name: field.name.to_string(),
                ty: IrType::Any, // TODO: Convert TypeId to IrType
                offset: None,
            }
        }).collect();

        let typedef = IrTypeDef {
            id: typedef_id,
            name: class.name.to_string(),
            type_id,
            definition: IrTypeDefinition::Struct {
                fields,
                packed: false,
            },
            source_location: IrSourceLocation::unknown(),
        };

        self.builder.module.add_type(typedef);
    }

    fn register_interface_metadata(&mut self, type_id: TypeId, interface: &HirInterface) {
        // Interfaces are represented as method tables
        // For now, register as struct with method pointers
        let typedef_id = self.builder.module.alloc_typedef_id();

        let fields: Vec<IrField> = interface.methods.iter().map(|method| {
            IrField {
                name: method.name.to_string(),
                ty: IrType::Ptr(Box::new(IrType::Function {
                    params: vec![IrType::Any], // Placeholder
                    return_type: Box::new(IrType::Any),
                    varargs: false,
                })),
                offset: None,
            }
        }).collect();

        let typedef = IrTypeDef {
            id: typedef_id,
            name: interface.name.to_string(),
            type_id,
            definition: IrTypeDefinition::Struct {
                fields,
                packed: false,
            },
            source_location: IrSourceLocation::unknown(),
        };

        self.builder.module.add_type(typedef);
    }

    fn register_abstract_metadata(&mut self, type_id: TypeId, abstract_decl: &HirAbstract) {
        // Abstract types are type aliases with additional constraints
        let typedef_id = self.builder.module.alloc_typedef_id();

        let typedef = IrTypeDef {
            id: typedef_id,
            name: abstract_decl.name.to_string(),
            type_id,
            definition: IrTypeDefinition::Alias {
                aliased_type: IrType::Any, // TODO: Get underlying type
            },
            source_location: IrSourceLocation::unknown(),
        };

        self.builder.module.add_type(typedef);
    }

    fn register_alias_metadata(&mut self, type_id: TypeId, alias: &HirTypeAlias) {
        // Type aliases
        let typedef_id = self.builder.module.alloc_typedef_id();

        let typedef = IrTypeDef {
            id: typedef_id,
            name: alias.name.to_string(),
            type_id,
            definition: IrTypeDefinition::Alias {
                aliased_type: IrType::Any, // TODO: Convert aliased TypeId to IrType
            },
            source_location: IrSourceLocation::unknown(),
        };

        self.builder.module.add_type(typedef);
    }

    fn generate_module_init_function(&mut self) {
        // Generate __init__ function that initializes dynamic globals
        // This function is called once at module load time
        //
        // Function signature: fn __init__() -> void
        // Body: Initialize each dynamic global in order

        let init_sig = FunctionSignatureBuilder::new()
            .returns(IrType::Void)
            .calling_convention(CallingConvention::Haxe)
            .build();

        let init_symbol = SymbolId::from_raw(u32::MAX - 1); // Reserved symbol for __init__
        let _init_func_id = self.builder.start_function(
            init_symbol,
            "__init__".to_string(),
            init_sig,
        );

        // Save current symbol map (should be empty, but just in case)
        let saved_symbol_map = self.symbol_map.clone();
        self.symbol_map.clear();

        // Lower each dynamic global initialization
        for (_symbol, init_expr) in &self.dynamic_globals.clone() {
            // Evaluate the initialization expression
            let Some(_init_value) = self.lower_expression(init_expr) else {
                continue;
            };

            // TODO: Store the result to the global variable
            // This requires accessing the global by symbol ID
            // For now, we just evaluate the expression (side effects occur)
            // In a full implementation, we'd:
            // 1. Get the global's address
            // 2. Store init_value to that address
        }

        // Return void
        self.builder.build_return(None);

        // Finish the __init__ function
        self.builder.finish_function();

        // Restore symbol map
        self.symbol_map = saved_symbol_map;
    }

    fn add_error(&mut self, msg: &str, location: SourceLocation) {
        self.errors.push(LoweringError {
            message: msg.to_string(),
            location,
        });
    }
}

enum MirBinaryOp {
    Binary(BinaryOp),
    Compare(CompareOp),
}

/// Public API for HIR to MIR lowering
pub fn lower_hir_to_mir(
    hir_module: &HirModule,
) -> Result<IrModule, Vec<LoweringError>> {
    let mut context = HirToMirContext::new(
        hir_module.name.clone(),
        hir_module.metadata.source_file.clone(),
    );
    
    context.lower_module(hir_module)
}