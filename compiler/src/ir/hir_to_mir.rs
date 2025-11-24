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
    IrFunctionId, IrFunctionSignature, IrParameter, IrLocal,
};
use crate::tast::{
    SymbolId, TypeId, SourceLocation, InternedString, StringInterner,
    TypeTable, TypeKind, SymbolTable,
};
use crate::stdlib::{StdlibMapping, MethodSignature};
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

/// Context for lowering HIR to MIR
pub struct HirToMirContext<'a> {
    /// MIR builder
    builder: IrBuilder,

    /// Mapping from HIR symbols to MIR registers (for variables/parameters)
    symbol_map: HashMap<SymbolId, IrId>,

    /// Mapping from HIR function symbols to MIR function IDs
    function_map: HashMap<SymbolId, crate::ir::IrFunctionId>,

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

    /// String interner for resolving InternedString to actual strings
    string_interner: &'a StringInterner,

    /// Type table for proper type conversion
    type_table: &'a Rc<RefCell<TypeTable>>,

    /// Track closure registers and their environment pointers
    /// Maps: closure_function_pointer_register -> environment_pointer_register
    closure_environments: HashMap<IrId, IrId>,

    /// Mapping from field SymbolId to (class_type_id, field_index)
    /// This allows us to find the index of a field within its class
    field_index_map: HashMap<SymbolId, (TypeId, u32)>,

    /// Mapping from class TypeId to constructor IrFunctionId
    /// This allows new expressions to find the constructor by class type
    constructor_map: HashMap<TypeId, IrFunctionId>,

    /// Reference to HIR type declarations for inheritance lookup
    /// Needed to access parent class fields during field inheritance
    current_hir_types: &'a indexmap::IndexMap<TypeId, HirTypeDecl>,

    /// Standard library runtime function mapping
    stdlib_mapping: StdlibMapping,

    /// Symbol table for resolving symbols
    symbol_table: &'a SymbolTable,
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

impl<'a> HirToMirContext<'a> {
    /// Create a new lowering context
    pub fn new(
        module_name: String,
        source_file: String,
        string_interner: &'a StringInterner,
        type_table: &'a Rc<RefCell<TypeTable>>,
        hir_types: &'a indexmap::IndexMap<TypeId, HirTypeDecl>,
        symbol_table: &'a SymbolTable,
    ) -> Self {
        Self {
            builder: IrBuilder::new(module_name.clone(), source_file),
            symbol_map: HashMap::new(),
            function_map: HashMap::new(),
            block_map: HashMap::new(),
            loop_stack: Vec::new(),
            current_module: Some(module_name),
            errors: Vec::new(),
            ssa_hints: SsaOptimizationHints::default(),
            lambda_counter: 0,
            dynamic_globals: Vec::new(),
            string_interner,
            type_table,
            closure_environments: HashMap::new(),
            field_index_map: HashMap::new(),
            constructor_map: HashMap::new(),
            current_hir_types: hir_types,
            stdlib_mapping: StdlibMapping::new(),
            symbol_table,
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

        // IMPORTANT: Register type metadata FIRST before lowering any functions
        // This populates field_index_map which is needed for field access
        for (type_id, type_decl) in &hir_module.types {
            self.register_type_metadata(*type_id, type_decl);
        }

        // CRITICAL: Two-pass lowering to avoid non-deterministic function ordering issues
        // HashMap iteration over hir_module.types is random, so class methods might be
        // lowered after module functions that call them, causing "function not found" errors.
        //
        // Pass 1: Register ALL function signatures WITHOUT lowering bodies
        // This ensures function_map is fully populated before any calls are made

        // Pass 1a: Register class method signatures
        for (type_id, type_decl) in &hir_module.types {
            match type_decl {
                HirTypeDecl::Class(class) => {
                    eprintln!("DEBUG Pass1a: Registering methods for class {:?}",
                             self.string_interner.get(class.name).unwrap_or("<unknown>"));
                    for method in &class.methods {
                        eprintln!("DEBUG Pass1a:   - method {:?} (symbol={:?})",
                                 self.string_interner.get(method.function.name).unwrap_or("<unknown>"),
                                 method.function.symbol_id);
                        let this_type = if !method.is_static {
                            Some(*type_id)
                        } else {
                            None
                        };
                        self.register_function_signature(
                            method.function.symbol_id,
                            &method.function,
                            this_type
                        );
                    }

                    // Register constructor signature
                    if let Some(constructor) = &class.constructor {
                        self.register_constructor_signature(class.symbol_id, constructor, *type_id);
                    }
                }
                _ => {}
            }
        }

        // Pass 1b: Register module function signatures
        for (symbol_id, hir_func) in &hir_module.functions {
            self.register_function_signature(*symbol_id, hir_func, None);
        }

        // Pass 2: Now lower all function bodies (both class methods and module functions)
        // At this point, function_map is fully populated

        // Pass 2a: Lower class methods and constructors
        for (type_id, type_decl) in &hir_module.types {
            let name_str = if let HirTypeDecl::Class(c) = type_decl {
                self.string_interner.get(c.name).unwrap_or("<unknown>")
            } else {
                "<not-a-class>"
            };
            // eprintln!("DEBUG: Processing type - TypeId={:?}, name={:?}", type_id, name_str);
            match type_decl {
                HirTypeDecl::Class(class) => {
                    // Lower each method body
                    for method in &class.methods {
                        if method.is_static {
                            self.lower_function_body(
                                method.function.symbol_id,
                                &method.function,
                                None
                            );
                        } else {
                            self.lower_function_body(
                                method.function.symbol_id,
                                &method.function,
                                Some(*type_id)
                            );
                        }
                    }

                    // Lower constructor body
                    if let Some(constructor) = &class.constructor {
                        // eprintln!("DEBUG: Lowering constructor for class {:?}", class.name);
                        self.lower_constructor_body(class.symbol_id, constructor, *type_id, class.extends);
                    }
                }
                _ => {}
            }
        }

        // Pass 2b: Lower module function bodies
        for (symbol_id, hir_func) in &hir_module.functions {
            self.lower_function_body(*symbol_id, hir_func, None);
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

    /// Register a function signature without lowering the body (Pass 1)
    /// This creates the function stub and adds it to function_map
    fn register_function_signature(&mut self, symbol_id: SymbolId, hir_func: &HirFunction, this_type: Option<TypeId>) {
        let mut signature = self.build_function_signature(hir_func);

        // For instance methods, add implicit 'this' parameter
        if let Some(type_id) = this_type {
            let this_type = self.convert_type(type_id);
            signature.parameters.insert(0, IrParameter {
                name: "this".to_string(),
                ty: this_type,
                reg: IrId::new(0),  // Will be properly assigned when body is lowered
                by_ref: false,
            });
        }

        let func_name = self.string_interner.get(hir_func.name)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("func_{}", symbol_id.as_raw()));

        let func_id = self.builder.start_function(symbol_id, func_name, signature);
        self.function_map.insert(symbol_id, func_id);
        self.builder.finish_function();  // Close to allow next function to start
    }

    /// Register constructor signature (Pass 1)
    fn register_constructor_signature(&mut self, class_symbol: SymbolId, constructor: &HirConstructor, type_id: TypeId) {
        // Constructor signature: takes implicit 'this' parameter, returns void
        let this_type = self.convert_type(type_id);
        let mut signature = FunctionSignatureBuilder::new()
            .param("this".to_string(), this_type)
            .returns(IrType::Void)
            .build();

        // Assign register IDs to parameters
        for (i, param) in signature.parameters.iter_mut().enumerate() {
            param.reg = IrId::new(i as u32);
        }

        let func_id = self.builder.start_function(class_symbol, "new".to_string(), signature);
        self.function_map.insert(class_symbol, func_id);
        self.constructor_map.insert(type_id, func_id);
        self.builder.finish_function();  // Close the stub
    }

    /// Lower a function body after signature is registered (Pass 2)
    /// Reuses the existing function created in Pass 1
    fn lower_function_body(&mut self, symbol_id: SymbolId, hir_func: &HirFunction, this_type: Option<TypeId>) {
        // The function already exists from Pass 1, we just need to fill in the body
        let func_id = self.function_map.get(&symbol_id).copied()
            .expect("Function should have been registered in Pass 1");

        // Re-open the function for body lowering
        let func = self.builder.module.functions.get(&func_id)
            .expect("Function should exist").clone();

        self.builder.current_function = Some(func_id);
        self.builder.current_block = Some(func.entry_block());

        // Map 'this' parameter for instance methods
        if this_type.is_some() {
            // 'this' is parameter 0
            if let Some(this_param) = func.signature.parameters.get(0) {
                // Map 'this' to a special symbol ID (SymbolId(0))
                // This is what HirExprKind::This looks up
                self.symbol_map.insert(SymbolId::from_raw(0), this_param.reg);
            }
        }

        // Map parameters from the function signature (which already has register IDs assigned)
        let param_offset = if this_type.is_some() { 1 } else { 0 };
        for (i, param) in hir_func.params.iter().enumerate() {
            if let Some(sig_param) = func.signature.parameters.get(i + param_offset) {
                self.symbol_map.insert(param.symbol_id, sig_param.reg);
            }
        }

        // Lower body
        if let Some(body) = &hir_func.body {
            self.lower_block(body);
            self.ensure_terminator();
        }

        // Finish
        eprintln!("DEBUG ===== FINISHING FUNCTION =====");
        if let Some(func) = self.builder.current_function() {
            eprintln!("DEBUG   Function '{}' entry block terminator: {:?}",
                     func.name,
                     func.cfg.get_block(func.entry_block()).map(|b| &b.terminator));
        }
        self.builder.finish_function();
        eprintln!("DEBUG   Function finished, context cleared");

        self.symbol_map.clear();
        self.block_map.clear();
    }

    /// Lower constructor body (Pass 2)
    fn lower_constructor_body(&mut self, class_symbol: SymbolId, constructor: &HirConstructor, type_id: TypeId, parent_type: Option<TypeId>) {
        let func_id = self.constructor_map.get(&type_id).copied()
            .expect("Constructor should have been registered in Pass 1");

        let func = self.builder.module.functions.get(&func_id)
            .expect("Constructor function should exist").clone();

        self.builder.current_function = Some(func_id);
        self.builder.current_block = Some(func.entry_block());

        // 'this' is parameter 0
        let this_reg = IrId::new(0);

        // Map 'this' to symbol map for field access
        self.symbol_map.insert(SymbolId::from_raw(0), this_reg);

        // Map constructor parameters to registers
        for (i, param) in constructor.params.iter().enumerate() {
            let reg = IrId::new((i + 1) as u32);  // +1 because 'this' is parameter 0
            self.symbol_map.insert(param.symbol_id, reg);
        }

        // Handle super() call if present
        if let Some(super_call) = &constructor.super_call {
            // eprintln!("DEBUG: Processing super() call with {} args", super_call.args.len());

            if let Some(parent_type_id) = parent_type {
                // eprintln!("DEBUG: Parent class TypeId={:?}", parent_type_id);

                // Look up parent constructor
                if let Some(&parent_ctor_id) = self.constructor_map.get(&parent_type_id) {
                    // eprintln!("DEBUG: Found parent constructor IrFunctionId={:?}", parent_ctor_id);

                    // Lower super call arguments
                    let mut arg_regs = vec![this_reg];  // 'this' is first argument
                    for arg in &super_call.args {
                        if let Some(arg_reg) = self.lower_expression(arg) {
                            arg_regs.push(arg_reg);
                        }
                    }

                    // eprintln!("DEBUG: Calling parent constructor with {} args", arg_regs.len());
                    // Call parent constructor (returns void)
                    self.builder.build_call_direct(parent_ctor_id, arg_regs, crate::ir::IrType::Void);
                } else {
                    self.add_error(
                        &format!("Parent constructor not found for TypeId {:?}", parent_type_id),
                        crate::tast::SourceLocation::unknown()
                    );
                }
            } else {
                eprintln!("DEBUG: super() call but no parent class - this is an error in HIR");
            }
        }

        // Lower constructor body statements
        for stmt in &constructor.body.statements {
            self.lower_statement(stmt);
        }

        // Constructor implicitly returns void
        self.builder.build_return(None);

        eprintln!("DEBUG ===== FINISHING FUNCTION =====");
        if let Some(func) = self.builder.current_function() {
            eprintln!("DEBUG   Function '{}' entry block terminator: {:?}",
                     func.name,
                     func.cfg.get_block(func.entry_block()).map(|b| &b.terminator));
        }
        self.builder.finish_function();
        eprintln!("DEBUG   Function finished, context cleared");

        self.symbol_map.clear();
        self.block_map.clear();
    }

    /// Lower a HIR function to MIR (Legacy - combines Pass 1 and Pass 2)
    fn lower_function(&mut self, symbol_id: SymbolId, hir_func: &HirFunction) {
        let body_stmt_count = hir_func.body.as_ref().map(|b| b.statements.len()).unwrap_or(0);
        // eprintln!("DEBUG: lower_function - name={:?}, symbol={:?}, has_body={}, stmt_count={}",
        //           self.string_interner.get(hir_func.name),
        //           symbol_id,
        //           hir_func.body.is_some(),
        //           body_stmt_count);

        // Debug: Print each statement kind
        // if let Some(body) = &hir_func.body {
        //     for (i, stmt) in body.statements.iter().enumerate() {
        //         eprintln!("DEBUG: Statement {}: {:?}", i, std::mem::discriminant(stmt));
        //     }
        // }

        // Build MIR function signature
        let signature = self.build_function_signature(hir_func);

        // Start building the function
        let func_name = self.string_interner.get(hir_func.name)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("func_{}", symbol_id.as_raw()));
        // eprintln!("DEBUG ===== STARTING FUNCTION: {} (symbol {:?}) =====", func_name, symbol_id);
        let func_id = self.builder.start_function(
            symbol_id,
            func_name,
            signature,
        );
        // eprintln!("DEBUG   Function ID: {:?}, Entry block created", func_id);

        // Store function mapping for call resolution
        self.function_map.insert(symbol_id, func_id);

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

        // Set qualified name for debugging and profiling
        if let Some(qualified_name) = hir_func.qualified_name {
            if let Some(func) = self.builder.module.functions.get_mut(&func_id) {
                func.qualified_name = self.string_interner.get(qualified_name).map(|s| s.to_string());
            }
        }

        // Map parameters to MIR registers
        // Parameters now have symbol IDs (preserved from TAST)!
        /// eprintln!("DEBUG: Mapping {} parameters", hir_func.params.len());
        for (i, param) in hir_func.params.iter().enumerate() {
            if let Some(func) = self.builder.current_function() {
                if let Some(sig_param) = func.signature.parameters.get(i) {
                    let param_reg = sig_param.reg;
                    /// eprintln!("DEBUG: Parameter {} symbol {:?} '{}' -> register {:?}", i, param.symbol_id, param.name, param_reg);
                    // Map parameter symbol to its register
                    self.symbol_map.insert(param.symbol_id, param_reg);

                    // Also register parameter as a local so Cranelift can find its type
                    let param_type = self.convert_type(param.ty);
                    let src_loc = IrSourceLocation::unknown();
                    if let Some(func_mut) = self.builder.current_function_mut() {
                        func_mut.locals.insert(param_reg, super::IrLocal {
                            name: param.name.to_string(),
                            ty: param_type,
                            mutable: false, // Parameters are immutable by default
                            source_location: src_loc,
                            allocation: super::AllocationHint::Register,
                        });
                    }
                }
            }
        }

        // Lower function body if present
        if let Some(body) = &hir_func.body {
            // eprintln!("DEBUG: Lowering function body for {} (symbol {:?})", hir_func.name, symbol_id);
            let stmt_count = body.statements.len();
            let has_expr = body.expr.is_some();
            // eprintln!("  Body has {} statements, trailing expr: {}", stmt_count, has_expr);

            self.lower_block(body);
            // eprintln!("  After lower_block");

            // Add implicit return if needed
            self.ensure_terminator();
            // eprintln!("  After ensure_terminator");
        } else {
            // eprintln!("DEBUG: Function {} has no body", hir_func.name);
        }

        eprintln!("DEBUG ===== FINISHING FUNCTION =====");
        // Before finishing, dump the terminator for this function
        if let Some(func) = self.builder.current_function() {
            eprintln!("DEBUG   Function '{}' entry block terminator: {:?}",
                     func.name,
                     func.cfg.get_block(func.entry_block()).map(|b| &b.terminator));
        }
        self.builder.finish_function();
        eprintln!("DEBUG   Function finished, context cleared");

        // Clear per-function state
        self.symbol_map.clear();
        self.block_map.clear();
    }

    /// Lower an instance method (non-static class method) to MIR
    /// Instance methods receive an implicit 'this' parameter as their first argument
    fn lower_instance_method(&mut self, symbol_id: SymbolId, hir_func: &HirFunction, class_type_id: TypeId) {
        // Build MIR function signature with implicit 'this' parameter
        let signature = self.build_instance_method_signature(hir_func, class_type_id);

        // Start building the function
        let func_name = self.string_interner.get(hir_func.name)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("func_{}", symbol_id.as_raw()));
        eprintln!("DEBUG ===== STARTING FUNCTION: {} (symbol {:?}) =====", func_name, symbol_id);
        let func_id = self.builder.start_function(
            symbol_id,
            func_name,
            signature,
        );
        eprintln!("DEBUG   Function ID: {:?}, Entry block created", func_id);

        // Store function mapping for call resolution
        self.function_map.insert(symbol_id, func_id);

        // Set qualified name for debugging and profiling
        if let Some(qualified_name) = hir_func.qualified_name {
            if let Some(func) = self.builder.module.functions.get_mut(&func_id) {
                func.qualified_name = self.string_interner.get(qualified_name).map(|s| s.to_string());
            }
        }

        // Map parameters to MIR registers
        // The first parameter (index 0) is the implicit 'this'
        // User-defined parameters start at index 1
        if let Some(func) = self.builder.current_function() {
            // Map 'this' parameter - we need a special symbol for it
            // For now, we'll create a synthetic symbol ID for 'this'
            // TODO: This should ideally come from TAST/HIR where 'this' is properly resolved
            if let Some(this_param) = func.signature.parameters.get(0) {
                let this_symbol = SymbolId::from_raw(0); // Synthetic 'this' symbol
                self.symbol_map.insert(this_symbol, this_param.reg);
            }

            // Map regular parameters (offset by 1 due to 'this')
            for (i, param) in hir_func.params.iter().enumerate() {
                if let Some(sig_param) = func.signature.parameters.get(i + 1) {
                    let param_reg = sig_param.reg;
                    // Map parameter symbol to its register
                    self.symbol_map.insert(param.symbol_id, param_reg);
                }
            }
        }

        // Lower the function body
        if let Some(body) = &hir_func.body {
            self.lower_block(body);

            // Add implicit return if needed
            self.ensure_terminator();
        }

        eprintln!("DEBUG ===== FINISHING FUNCTION =====");
        // Before finishing, dump the terminator for this function
        if let Some(func) = self.builder.current_function() {
            eprintln!("DEBUG   Function '{}' entry block terminator: {:?}",
                     func.name,
                     func.cfg.get_block(func.entry_block()).map(|b| &b.terminator));
        }
        self.builder.finish_function();
        eprintln!("DEBUG   Function finished, context cleared");

        // Clear per-function state
        self.symbol_map.clear();
        self.block_map.clear();
    }

    /// Lower a constructor to MIR
    /// Constructors are similar to instance methods but handle field initialization
    fn lower_constructor(&mut self, class_symbol: SymbolId, constructor: &HirConstructor, class_type_id: TypeId) {
        // eprintln!("DEBUG: lower_constructor - class_symbol={:?}", class_symbol);

        // Build signature using the builder
        let mut sig_builder = FunctionSignatureBuilder::new()
            .param("this".to_string(), self.convert_type(class_type_id));

        // Add constructor parameters
        for param in &constructor.params {
            let param_name = self.string_interner.get(param.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("param_{}", param.symbol_id.as_raw()));
            sig_builder = sig_builder.param(param_name, self.convert_type(param.ty));
        }

        // Constructor returns void
        let signature = sig_builder.returns(IrType::Void).build();

        // Start building the constructor function
        let func_name = "new".to_string();
        let func_id = self.builder.start_function(
            class_symbol,
            func_name,
            signature,
        );

        // Register this function in the function map
        self.function_map.insert(class_symbol, func_id);

        // Register in constructor_map by TypeId for new expressions
        self.constructor_map.insert(class_type_id, func_id);
        // eprintln!("DEBUG: Registered constructor - TypeId={:?}, FuncId={:?}", class_type_id, func_id);

        // Also register with TypeId derived from class SymbolId as a fallback
        // This handles cases where expression TypeIds differ from types map TypeIds
        let fallback_type_id = TypeId::from_raw(class_symbol.as_raw());
        if fallback_type_id != class_type_id {
            self.constructor_map.insert(fallback_type_id, func_id);
            // eprintln!("DEBUG: Also registered constructor - fallback TypeId={:?}, FuncId={:?}",
            //           fallback_type_id, func_id);
        }

        // Note: start_function already creates the entry block and switches to it
        // No need to create another block here - just use the current block

        // Map 'this' parameter (IrId(0) is the first parameter)
        // We'll use a temporary symbol ID for 'this'
        let this_reg = IrId::new(0);

        // Map constructor parameters
        for (i, param) in constructor.params.iter().enumerate() {
            let param_reg = IrId::new((i + 1) as u32); // Parameters start after 'this'
            self.symbol_map.insert(param.symbol_id, param_reg);
        }

        // Lower field initializations
        for field_init in &constructor.field_inits {
            if let Some(value_reg) = self.lower_expression(&field_init.value) {
                // Store to field using field_index_map
                if let Some(&(_class_type, field_index)) = self.field_index_map.get(&field_init.field) {
                    if let Some(index_const) = self.builder.build_const(IrValue::I32(field_index as i32)) {
                        // Use I32 as default field type (TODO: get actual type)
                        if let Some(field_ptr) = self.builder.build_gep(this_reg, vec![index_const], IrType::I32) {
                            self.builder.build_store(field_ptr, value_reg);
                        }
                    }
                }
            }
        }

        // Lower constructor body
        // eprintln!("DEBUG: Constructor body has {} statements", constructor.body.statements.len());
        for (i, stmt) in constructor.body.statements.iter().enumerate() {
            // eprintln!("DEBUG: Constructor stmt {}: {:?}", i, std::mem::discriminant(stmt));
        }
        self.lower_block(&constructor.body);

        // Ensure void return
        if !self.is_terminated() {
            self.builder.build_return(None);
        }

        eprintln!("DEBUG ===== FINISHING FUNCTION =====");
        // Before finishing, dump the terminator for this function
        if let Some(func) = self.builder.current_function() {
            eprintln!("DEBUG   Function '{}' entry block terminator: {:?}",
                     func.name,
                     func.cfg.get_block(func.entry_block()).map(|b| &b.terminator));
        }
        self.builder.finish_function();
        eprintln!("DEBUG   Function finished, context cleared");

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
        // eprintln!("DEBUG: lower_statement - {:?}", std::mem::discriminant(stmt));
        match stmt {
            HirStatement::Let { pattern, type_hint, init, is_mutable } => {
                // Lower initialization expression if present
                if let Some(init_expr) = init {
                    // eprintln!("DEBUG: Let statement - lowering init expression");
                    let value = self.lower_expression(init_expr);
                    // eprintln!("DEBUG: Let statement - init lowered to: {:?}", value);

                    // Bind to pattern and register as local
                    if let Some(value_reg) = value {
                        // eprintln!("DEBUG: Let statement - binding pattern with value_reg: {:?}", value_reg);
                        // Determine the type for the binding
                        let var_type = type_hint.or(Some(init_expr.ty));
                        self.bind_pattern_with_type(pattern, value_reg, var_type, *is_mutable);
                    } else {
                        // eprintln!("DEBUG: Let statement - NO VALUE from init expression!");
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
                // eprintln!("DEBUG: Return statement, has_value: {}", value.is_some());
                let ret_value = value.as_ref()
                    .and_then(|e| {
                        // eprintln!("DEBUG: Lowering return expression: {:?}", e);
                        let result = self.lower_expression(e);
                        // eprintln!("DEBUG: Return expression lowered to: {:?}", result);
                        // if result.is_none() {
                        //     eprintln!("ERROR: Failed to lower return expression!");
                        //     eprintln!("ERROR: Expression was: {:#?}", e);
                        // }
                        result
                    });
                // eprintln!("DEBUG: Building return instruction with value: {:?}", ret_value);
                self.builder.build_return(ret_value);
                // eprintln!("DEBUG: Return instruction built");
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
                if let Some(exception_reg) = self.lower_expression(expr) {
                    // Emit throw instruction
                    self.builder.build_throw(exception_reg);
                    // After throw, code is unreachable
                    self.builder.build_unreachable();
                }
            }
            
            HirStatement::If { condition, then_branch, else_branch } => {
                // eprintln!("DEBUG: About to call lower_if_statement, has_else={}", else_branch.is_some());
                self.lower_if_statement(condition, then_branch, else_branch.as_ref());
                // eprintln!("DEBUG: Returned from lower_if_statement");
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

    /// Check if a method symbol corresponds to a stdlib method with runtime mapping
    ///
    /// Returns (class_name, method_name, runtime_function_name) if this is a stdlib method
    fn get_stdlib_runtime_info(
        &self,
        method_symbol: SymbolId,
        receiver_type: TypeId,
    ) -> Option<(&'static str, &'static str, &'static str)> {
        // Get the method name from the symbol table
        let method_name = if let Some(symbol) = self.symbol_table.get_symbol(method_symbol) {
            self.string_interner.get(symbol.name)?
        } else {
            return None;
        };

        // Get the type name from the receiver type
        let type_table = self.type_table.borrow();
        let type_info = type_table.get(receiver_type)?;

        let class_name = match &type_info.kind {
            TypeKind::String => "String",
            TypeKind::Array { .. } => "Array",
            TypeKind::Class { symbol_id, .. } => {
                // Get class name from symbol
                if let Some(class_info) = self.symbol_table.get_symbol(*symbol_id) {
                    let name = self.string_interner.get(class_info.name)?;
                    match name {
                        "Math" => "Math",
                        "Sys" => "Sys",
                        _ => return None, // Not a stdlib class
                    }
                } else {
                    return None;
                }
            }
            _ => return None, // Not a stdlib type
        };

        drop(type_table);

        // Check if we have a mapping for this method
        let is_static = matches!(class_name, "Math" | "Sys");

        // Convert to static strings for the signature
        let class_static: &'static str = match class_name {
            "String" => "String",
            "Array" => "Array",
            "Math" => "Math",
            "Sys" => "Sys",
            _ => return None,
        };

        let method_static: &'static str = Box::leak(method_name.to_string().into_boxed_str());

        let sig = MethodSignature {
            class: class_static,
            method: method_static,
            is_static,
        };

        if let Some(mapping) = self.stdlib_mapping.get(&sig) {
            println!("DEBUG: Found stdlib runtime mapping {} .{} -> {}", class_static, method_static, mapping.runtime_name);
            Some((class_static, method_static, mapping.runtime_name))
        } else {
            None
        }
    }

    /// Get or register an external runtime function, returning its ID
    ///
    /// This allows calling external runtime functions (like haxe_math_abs) from MIR
    fn get_or_register_extern_function(
        &mut self,
        name: &str,
        param_types: Vec<IrType>,
        return_type: IrType,
    ) -> IrFunctionId {
        // Check if we already registered this extern function
        for (func_id, extern_func) in &self.builder.module.extern_functions {
            if extern_func.name == name {
                return *func_id;
            }
        }

        // Create new extern function entry
        let func_id = IrFunctionId(self.builder.module.next_function_id);
        self.builder.module.next_function_id += 1;

        let params = param_types.into_iter().enumerate().map(|(i, ty)| {
            IrParameter {
                name: format!("arg{}", i),
                ty,
                reg: IrId::new(i as u32),
                by_ref: false,
            }
        }).collect();

        let signature = IrFunctionSignature {
            parameters: params,
            return_type,
            calling_convention: CallingConvention::C, // External functions use C calling convention
            can_throw: false,
            type_params: vec![],
            uses_sret: false, // No struct return for C functions
        };

        let extern_func = crate::ir::modules::IrExternFunction {
            id: func_id,
            name: name.to_string(),
            symbol_id: SymbolId::from_raw(0), // Placeholder
            signature,
            source: "rayzor_runtime".to_string(),
        };

        self.builder.module.extern_functions.insert(func_id, extern_func);
        func_id
    }

    /// Lower a HIR expression to MIR value
    fn lower_expression(&mut self, expr: &HirExpr) -> Option<IrId> {
       // eprintln!("DEBUG: lower_expression - {:?}", std::mem::discriminant(&expr.kind));

        // Set source location for debugging
        self.builder.set_source_location(self.convert_source_location(&expr.source_location));

        let result = match &expr.kind {
            HirExprKind::Literal(lit) => self.lower_literal(lit, expr.ty),
            
            HirExprKind::Variable { symbol, .. } => {
                // Check if this symbol is a function reference
                if let Some(&func_id) = self.function_map.get(symbol) {
                    // Create a function pointer constant for static methods
                    return self.builder.build_function_ptr(func_id);
                }

                // Otherwise, it's a regular variable
                let reg = self.symbol_map.get(symbol).copied();
                if reg.is_none() {
                    eprintln!("WARNING: Variable {:?} not found in symbol_map! Available symbols: {:?}",
                             symbol, self.symbol_map.keys().collect::<Vec<_>>());
                }
                reg
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
            
            HirExprKind::Call { callee, args, is_method, .. } => {
                let result_type = self.convert_type(expr.ty);

                // Check if this is a method call (callee is a field access)
                if let HirExprKind::Field { object, field } = &callee.kind {
                    // eprintln!("DEBUG: Method call detected - field={:?}", field);
                    // eprintln!("DEBUG: function_map keys: {:?}", self.function_map.keys().collect::<Vec<_>>());

                    // This is a method call: object.method(args)
                    // The method symbol should be in our function_map
                    if let Some(&func_id) = self.function_map.get(field) {
                        // eprintln!("DEBUG: Found method in function_map - func_id={:?}", func_id);

                        // Lower the object (this will be the first parameter)
                        let obj_reg = self.lower_expression(object)?;

                        // Lower the arguments
                        let arg_regs: Vec<_> = std::iter::once(obj_reg)  // Add 'this' as first arg
                            .chain(args.iter().filter_map(|a| self.lower_expression(a)))
                            .collect();

                        // eprintln!("DEBUG: Calling method with {} args (including this)", arg_regs.len());
                        return self.builder.build_call_direct(func_id, arg_regs, result_type);
                    } else {
                        eprintln!("WARNING: Method {:?} not found in function_map", field);
                    }
                }

                // Check if callee is a direct function reference
                if let HirExprKind::Variable { symbol, .. } = &callee.kind {
                    // For instance method calls, check if this is a stdlib method
                    if *is_method && !args.is_empty() {
                        // The first arg is the receiver for instance method calls
                        let receiver_type = args[0].ty;

                        if let Some((_class, _method, runtime_func)) =
                            self.get_stdlib_runtime_info(*symbol, receiver_type)
                        {
                            println!("DEBUG: Generating runtime call to {}", runtime_func);
                            eprintln!("INFO: Stdlib method call detected: {} (runtime: {})", _method, runtime_func);
                            eprintln!("INFO: This will be properly handled once we register extern runtime functions");
                        }
                    }
                    // For static methods, check the symbol name to detect Math/Sys methods
                    else if !is_method {
                        if let Some(sym_info) = self.symbol_table.get_symbol(*symbol) {
                            if let Some(method_name) = self.string_interner.get(sym_info.name) {
                                // Check if this is a Math or Sys static method
                                // For static methods, we need to check the parent class
                                // For now, just log that we detected a potential static method
                                if method_name.starts_with("sin") || method_name.starts_with("cos") ||
                                   method_name.starts_with("sqrt") || method_name.starts_with("random") {
                                    eprintln!("DEBUG: Potential Math static method: {}", method_name);
                                }
                            }
                        }
                    }

                    // Check if this symbol is a function
                    if let Some(&func_id) = self.function_map.get(symbol) {
                        // Handle method calls where the object is passed as first argument
                        if *is_method {
                            // eprintln!("DEBUG: Method call (is_method=true) - symbol={:?}, args.len()={}", symbol, args.len());
                            // For method calls, args already includes the object as first arg
                            let arg_regs: Vec<_> = args.iter()
                                .filter_map(|a| self.lower_expression(a))
                                .collect();

                            // eprintln!("DEBUG: Method call lowered {} args", arg_regs.len());
                            return self.builder.build_call_direct(func_id, arg_regs, result_type);
                        } else {
                            // Direct function call
                            let arg_regs: Vec<_> = args.iter()
                                .filter_map(|a| self.lower_expression(a))
                                .collect();

                            return self.builder.build_call_direct(func_id, arg_regs, result_type);
                        }
                    } else {
                        // Function not in function_map - might be an extern/stdlib function
                        // Check if it's a stdlib static method (like Math.sin, Sys.println)
                        if let Some(sym_info) = self.symbol_table.get_symbol(*symbol) {
                            if let Some(method_name) = self.string_interner.get(sym_info.name) {
                                // Check if method name matches known Math/Sys methods
                                let is_math_method = matches!(method_name,
                                    "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2" |
                                    "sqrt" | "abs" | "min" | "max" | "floor" | "ceil" | "round" |
                                    "exp" | "log" | "pow" | "random" | "isNaN" | "isFinite"
                                );

                                let is_sys_method = matches!(method_name,
                                    "print" | "println" | "exit" | "time"
                                );

                                if is_math_method || is_sys_method {
                                    let class_name = if is_math_method { "Math" } else { "Sys" };

                                    // Look up the runtime function name from stdlib mapping
                                    let method_static: &'static str = Box::leak(method_name.to_string().into_boxed_str());
                                    let sig = crate::stdlib::MethodSignature {
                                        class: class_name,
                                        method: method_static,
                                        is_static: true,
                                    };

                                    if let Some(mapping) = self.stdlib_mapping.get(&sig) {
                                        let runtime_name = mapping.runtime_name;
                                        eprintln!("INFO: {} static method detected: {} (runtime: {})",
                                            class_name, method_name, runtime_name);

                                        // Lower arguments and get their types
                                        let mut arg_regs = Vec::new();
                                        let mut arg_types = Vec::new();
                                        for arg in args {
                                            if let Some(reg) = self.lower_expression(arg) {
                                                arg_regs.push(reg);
                                                arg_types.push(self.convert_type(arg.ty));
                                            }
                                        }

                                        // Register the external runtime function
                                        let extern_func_id = self.get_or_register_extern_function(
                                            runtime_name,
                                            arg_types,
                                            result_type.clone(),
                                        );

                                        // Generate call to external function
                                        return self.builder.build_call_direct(extern_func_id, arg_regs, result_type);
                                    } else {
                                        eprintln!("WARNING: {}.{} not found in stdlib mapping", class_name, method_name);
                                    }
                                } else {
                                    eprintln!("WARNING: Function/method {:?} not found in function_map (is_method={})", symbol, is_method);
                                    eprintln!("WARNING: Available symbols: {:?}", self.function_map.keys().collect::<Vec<_>>());
                                }
                            }
                        }
                    }
                }

                // Indirect function call (function pointer)
                // TODO: Get the full function signature from the callee's type
                // For now, we'll infer it from the arguments and return type
                // This is a temporary workaround until we pass type_table to HIR→MIR

                let func_ptr = self.lower_expression(callee)?;
                let arg_regs: Vec<_> = args.iter()
                    .filter_map(|a| self.lower_expression(a))
                    .collect();

                // Build function signature from arguments and return type
                // TODO: This should come from the type table lookup of callee.ty
                // For now, we infer parameter types as I32 and get return type from expr.ty
                let param_types = vec![IrType::I32; arg_regs.len()];
                let return_type = Box::new(self.convert_type(expr.ty));

                let func_signature = IrType::Function {
                    params: param_types,
                    return_type,
                    varargs: false,
                };

                self.builder.build_call_indirect(func_ptr, arg_regs, func_signature)
            }
            
            HirExprKind::New { class_type, args, .. } => {
                // eprintln!("DEBUG: New expression - class_type={:?}, expr.ty={:?}", class_type, expr.ty);

                // Check if this is an abstract type
                let type_table = self.type_table.borrow();
                let is_abstract = if let Some(type_ref) = type_table.get(*class_type) {
                    // eprintln!("DEBUG: Found type in table: kind={:?}", std::mem::discriminant(&type_ref.kind));
                    let is_abs = matches!(type_ref.kind, crate::tast::TypeKind::Abstract { .. });
                    // eprintln!("DEBUG: Is abstract? {}", is_abs);
                    is_abs
                } else {
                    // eprintln!("DEBUG: Type {:?} NOT found in type table", class_type);
                    false
                };
                drop(type_table);

                // SPECIAL CASE: Abstract type constructors
                // If this is an abstract type, OR if there's no constructor and we have exactly one argument,
                // treat this as a simple value wrap (no allocation).
                if is_abstract {
                    // eprintln!("DEBUG: Abstract type constructor detected - returning wrapped value");
                    if args.len() == 1 {
                        return self.lower_expression(&args[0]);
                    } else if args.is_empty() {
                        eprintln!("WARNING: Abstract constructor with no arguments, returning 0");
                        return self.builder.build_const(IrValue::I32(0));
                    } else {
                        eprintln!("WARNING: Abstract constructor with {} arguments, using first", args.len());
                        return self.lower_expression(&args[0]);
                    }
                }

                // Check if constructor exists
                let has_constructor = self.constructor_map.contains_key(class_type);

                // If no constructor exists and we have exactly one argument, treat as value wrap
                // This handles abstract types that weren't properly detected above
                if !has_constructor && args.len() == 1 {
                    // eprintln!("DEBUG: No constructor found, single argument - treating as value wrap");
                    // eprintln!("DEBUG: Argument expression: {:#?}", &args[0]);
                    let result = self.lower_expression(&args[0]);
                    // eprintln!("DEBUG: Value wrap result: {:?}", result);
                    return result;
                }

                // CLASS TYPE CONSTRUCTOR:
                // Allocate object
                let class_mir_type = self.convert_type(*class_type);
                let obj_ptr = self.builder.build_alloc(class_mir_type.clone(), None)?;

                // TEMPORARY WORKAROUND: Zero-initialize all fields
                // TODO: Remove this once constructor field initialization is fixed
                // The issue is that assignments in constructor bodies are lowered as
                // Expression statements instead of Assignment statements, so fields
                // don't get initialized properly. For now, just zero the memory.
                if let Some(zero) = self.builder.build_const(IrValue::I32(0)) {
                    if let Some(index_const) = self.builder.build_const(IrValue::I32(0)) {
                        if let Some(field_ptr) = self.builder.build_gep(obj_ptr, vec![index_const], IrType::I32) {
                            self.builder.build_store(field_ptr, zero);
                        }
                    }
                }

                // eprintln!("DEBUG: Class type constructor - allocated object");
                // eprintln!("DEBUG: Available constructors: {:?}", self.constructor_map.keys().collect::<Vec<_>>());

                // Look up constructor by TypeId
                // First try direct lookup
                let constructor_func_id = self.constructor_map.get(class_type).copied()
                    .or_else(|| {
                        // If not found, try to resolve through type table
                        // The TypeId in the expression might differ from the one in the types map
                        // Look up the class name and find the matching constructor
                        let type_table = self.type_table.borrow();
                        if let Some(type_ref) = type_table.get(*class_type) {
                            if let crate::tast::TypeKind::Class { symbol_id, .. } = &type_ref.kind {
                                // Try looking up by the class symbol converted to TypeId
                                let class_type_id = TypeId::from_raw(symbol_id.as_raw());
                                // eprintln!("DEBUG: Trying fallback lookup - symbol_id={:?}, derived TypeId={:?}",
                                //           symbol_id, class_type_id);
                                return self.constructor_map.get(&class_type_id).copied();
                            }
                        }
                        None
                    });

                if let Some(constructor_func_id) = constructor_func_id {
                    // eprintln!("DEBUG: Found constructor for TypeId {:?}", class_type);

                    // Call constructor with object as first argument
                    let arg_regs: Vec<_> = std::iter::once(obj_ptr)
                        .chain(args.iter().filter_map(|a| self.lower_expression(a)))
                        .collect();

                    // Constructor returns void, so we ignore the result
                    self.builder.build_call_direct(constructor_func_id, arg_regs, IrType::Void);
                } else {
                    eprintln!("WARNING: Constructor not found for TypeId {:?}", class_type);
                }

                Some(obj_ptr)
            }
            
            HirExprKind::Unary { op, operand } => {
                let operand_reg = self.lower_expression(operand)?;
                let result_reg = self.builder.build_unop(self.convert_unary_op(*op), operand_reg)?;

                // Register the result with its type so Cranelift can find it
                let result_type = self.convert_type(expr.ty);
                let src_loc = self.convert_source_location(&expr.source_location);
                if let Some(func) = self.builder.current_function_mut() {
                    func.locals.insert(result_reg, super::IrLocal {
                        name: format!("_temp{}", result_reg.0),
                        ty: result_type,
                        mutable: false,
                        source_location: src_loc,
                        allocation: super::AllocationHint::Stack,
                    });
                }

                Some(result_reg)
            }
            
            HirExprKind::Binary { op, lhs, rhs } => {
                // Handle short-circuit operators specially
                match op {
                    HirBinaryOp::And => return self.lower_logical_and(lhs, rhs),
                    HirBinaryOp::Or => return self.lower_logical_or(lhs, rhs),
                    _ => {}
                }

                let mut lhs_reg = self.lower_expression(lhs)?;
                let mut rhs_reg = self.lower_expression(rhs)?;

                // Special handling for division: Haxe always returns Float from division
                // If operands are integers, convert them to float first
                if matches!(op, HirBinaryOp::Div) {
                    let lhs_type = self.convert_type(lhs.ty);
                    let rhs_type = self.convert_type(rhs.ty);

                    // Convert integer operands to float
                    if matches!(lhs_type, IrType::I8 | IrType::I16 | IrType::I32 | IrType::I64 |
                                         IrType::U8 | IrType::U16 | IrType::U32 | IrType::U64) {
                        lhs_reg = self.builder.build_cast(lhs_reg, lhs_type, IrType::F64)?;
                    }
                    if matches!(rhs_type, IrType::I8 | IrType::I16 | IrType::I32 | IrType::I64 |
                                         IrType::U8 | IrType::U16 | IrType::U32 | IrType::U64) {
                        rhs_reg = self.builder.build_cast(rhs_reg, rhs_type, IrType::F64)?;
                    }
                }

                let result_reg = match self.convert_binary_op_to_mir(*op) {
                    MirBinaryOp::Binary(bin_op) => {
                        self.builder.build_binop(bin_op, lhs_reg, rhs_reg)?
                    }
                    MirBinaryOp::Compare(cmp_op) => {
                        self.builder.build_cmp(cmp_op, lhs_reg, rhs_reg)?
                    }
                };

                // Register the result with its type so Cranelift can find it
                let result_type = self.convert_type(expr.ty);
                let src_loc = self.convert_source_location(&expr.source_location);
                if let Some(func) = self.builder.current_function_mut() {
                    func.locals.insert(result_reg, super::IrLocal {
                        name: format!("_temp{}", result_reg.0),
                        ty: result_type,
                        mutable: false,
                        source_location: src_loc,
                        allocation: super::AllocationHint::Stack,
                    });
                }

                Some(result_reg)
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
                self.lower_lambda(params, body, captures, expr.ty)
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
                // 'super' should only appear in constructor super calls, which are handled
                // specially in lower_constructor_body. If we reach here, it's likely being
                // used incorrectly (e.g., super.method() which isn't supported yet)
                // eprintln!("WARNING: HirExprKind::Super encountered in expression lowering");
                // eprintln!("  This might be super.field or super.method() which isn't implemented yet");
                // For now, treat it like 'this' (same object, but calling parent methods)
                self.symbol_map.get(&SymbolId::from_raw(0)).copied()
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
               // eprintln!("DEBUG: Unsupported expression type in MIR");
                self.add_error("Unsupported expression type in MIR", expr.source_location);
                None
            }
        };

       // eprintln!("DEBUG: lower_expression result: {:?}", result);
        result
    }
    
    /// Lower if statement/expression
    fn lower_if_statement(
        &mut self,
        condition: &HirExpr,
        then_branch: &HirBlock,
        else_branch: Option<&HirBlock>,
    ) {
        // eprintln!("DEBUG HIR→MIR: lower_if_statement called, has_else={}", else_branch.is_some());
        let Some(then_block) = self.builder.create_block() else { return; };
        let Some(merge_block) = self.builder.create_block() else { return; };

        let else_block = if else_branch.is_some() {
            self.builder.create_block().unwrap_or(merge_block)
        } else {
            merge_block
        };

        // eprintln!("DEBUG IF: then_block={:?}, merge_block={:?}, else_block={:?}, has_else={}",
        //           then_block, merge_block, else_block, else_branch.is_some());

        // Get the current block before branching
        let entry_block = if let Some(block_id) = self.builder.current_block() {
            // eprintln!("DEBUG IF: Entry block is {:?}", block_id);
            block_id
        } else {
            return;
        };

        // Find all variables that are modified in either branch
        let mut modified_vars = std::collections::HashSet::new();
        for stmt in &then_branch.statements {
            self.find_modified_variables_in_statement(stmt, &mut modified_vars);
        }
        if let Some(else_br) = else_branch {
            for stmt in &else_br.statements {
                self.find_modified_variables_in_statement(stmt, &mut modified_vars);
            }
        }

        // Save initial values of variables that will be modified
        let mut var_initial_values: HashMap<SymbolId, (IrId, IrType)> = HashMap::new();
        for symbol_id in &modified_vars {
            if let Some(&reg) = self.symbol_map.get(symbol_id) {
                // Get the type from the locals table
                if let Some(func) = self.builder.current_function() {
                    if let Some(local) = func.locals.get(&reg) {
                        var_initial_values.insert(*symbol_id, (reg, local.ty.clone()));
                    }
                }
            }
        }

        // Evaluate condition
        if let Some(cond_reg) = self.lower_expression(condition) {
            self.builder.build_cond_branch(cond_reg, then_block, else_block);

            // Lower then branch
            self.builder.switch_to_block(then_block);
            self.lower_block(then_branch);
            let then_end_block = if !self.is_terminated() {
                let current = self.builder.current_block();
                self.builder.build_branch(merge_block);
                current
            } else {
                None
            };

            // Save values after then branch
            let mut then_values: HashMap<SymbolId, IrId> = HashMap::new();
            if then_end_block.is_some() {
                for symbol_id in &modified_vars {
                    if let Some(&reg) = self.symbol_map.get(symbol_id) {
                        then_values.insert(*symbol_id, reg);
                    }
                }
            }

            // Lower else branch if present
            let mut else_values: HashMap<SymbolId, IrId> = HashMap::new();
            let else_end_block = if let Some(else_branch) = else_branch {
                self.builder.switch_to_block(else_block);
                self.lower_block(else_branch);
                if !self.is_terminated() {
                    let current = self.builder.current_block();
                    self.builder.build_branch(merge_block);

                    // Save values after else branch
                    for symbol_id in &modified_vars {
                        if let Some(&reg) = self.symbol_map.get(symbol_id) {
                            else_values.insert(*symbol_id, reg);
                        }
                    }
                    current
                } else {
                    None
                }
            } else {
                // If no else branch, the else path just falls through to merge
                // with the original values
                for (symbol_id, (initial_reg, _)) in &var_initial_values {
                    else_values.insert(*symbol_id, *initial_reg);
                }
                Some(entry_block)
            };

            // Continue in merge block and create phi nodes
            self.builder.switch_to_block(merge_block);

            // Create phi nodes for modified variables
            for (symbol_id, (initial_reg, var_type)) in &var_initial_values {
                // Only create phi if at least one branch modified the variable
                let then_val = then_values.get(symbol_id).copied().unwrap_or(*initial_reg);
                let else_val = else_values.get(symbol_id).copied().unwrap_or(*initial_reg);

                // If both branches lead to the same value, no phi needed
                if then_val == else_val {
                    continue;
                }

                // Only create phi if we have valid incoming blocks
                if then_end_block.is_none() && else_end_block.is_none() {
                    continue;
                }

                // eprintln!("DEBUG: Creating phi for symbol {:?}, then_val {:?}, else_val {:?}", symbol_id, then_val, else_val);
                // eprintln!("DEBUG:   then_end_block {:?}, else_end_block {:?}", then_end_block, else_end_block);

                // Create phi node
                if let Some(phi_reg) = self.builder.build_phi(merge_block, var_type.clone()) {
                    // eprintln!("DEBUG:   Created phi node {:?} in merge block {:?}", phi_reg, merge_block);

                    // Add incoming values from both branches
                    if let Some(then_blk) = then_end_block {
                        // eprintln!("DEBUG:   Adding phi incoming from then block {:?}, value {:?}", then_blk, then_val);
                        self.builder.add_phi_incoming(merge_block, phi_reg, then_blk, then_val);
                    }
                    if let Some(else_blk) = else_end_block {
                        // eprintln!("DEBUG:   Adding phi incoming from else block {:?}, value {:?}", else_blk, else_val);
                        self.builder.add_phi_incoming(merge_block, phi_reg, else_blk, else_val);
                    }

                    // Register the phi node as a local
                    if let Some(func) = self.builder.current_function_mut() {
                        if let Some(local) = func.locals.get(initial_reg).cloned() {
                            func.locals.insert(phi_reg, super::IrLocal {
                                name: format!("{}_phi", local.name),
                                ty: var_type.clone(),
                                mutable: true,
                                source_location: local.source_location,
                                allocation: super::AllocationHint::Register,
                            });
                        }
                    }

                    // Update symbol map to use phi node
                    self.symbol_map.insert(*symbol_id, phi_reg);
                }
            }
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

        // Save the entry block (current block before loop)
        let entry_block = if let Some(block_id) = self.builder.current_block() {
            block_id
        } else {
            return;
        };

        // Find all variables that are referenced in the loop
        // For now, use a simple heuristic: any variable in the symbol_map
        // that is referenced in the condition or body is a potential loop variable
    //    // eprintln!("DEBUG: Loop body has {} statements", body.statements.len());
    //     for (i, stmt) in body.statements.iter().enumerate() {
    //        // eprintln!("DEBUG: Statement {}: {:?}", i, std::mem::discriminant(stmt));
    //     }

        // Collect all variables referenced in condition
        let mut referenced_vars = std::collections::HashSet::new();
        self.collect_referenced_variables_in_expr(condition, &mut referenced_vars);
       // eprintln!("DEBUG: Variables in condition: {:?}", referenced_vars);

        // Collect all variables referenced in body
        self.collect_referenced_variables_in_block(body, &mut referenced_vars);
       // eprintln!("DEBUG: Variables in condition + body: {:?}", referenced_vars);

        // Only include variables that were declared before the loop
        // (i.e., they're already in the symbol_map)
        // Exclude function parameters since they're immutable
        let modified_vars: std::collections::HashSet<SymbolId> = referenced_vars.into_iter()
            .filter(|sym| {
                let in_map = self.symbol_map.contains_key(sym);
                // Check if this is a function parameter by seeing if it's in the current function's parameters
                let is_param = if let Some(func) = self.builder.current_function() {
                    func.signature.parameters.iter().any(|p| {
                        // Check if the symbol maps to this parameter's register
                        self.symbol_map.get(sym) == Some(&p.reg)
                    })
                } else {
                    false
                };
               // eprintln!("DEBUG: Symbol {:?} in map: {}, is_param: {}", sym, in_map, is_param);
                in_map && !is_param
            })
            .collect();

       // eprintln!("DEBUG: Found {} loop variables: {:?}", modified_vars.len(), modified_vars);

        // Save initial values of loop variables before jumping to condition
        let mut loop_var_initial_values: HashMap<SymbolId, (IrId, IrType)> = HashMap::new();
        for symbol_id in &modified_vars {
            if let Some(&reg) = self.symbol_map.get(symbol_id) {
                // Get the type from the locals table
                if let Some(func) = self.builder.current_function() {
                    if let Some(local) = func.locals.get(&reg) {
                        loop_var_initial_values.insert(*symbol_id, (reg, local.ty.clone()));
                    }
                }
            }
        }

        // Jump to condition block
        self.builder.build_branch(cond_block);

        // Condition block - create phi nodes for loop variables
        self.builder.switch_to_block(cond_block);

        // Create phi nodes for all loop variables
        let mut phi_nodes: HashMap<SymbolId, IrId> = HashMap::new();
       // eprintln!("DEBUG: Creating phi nodes for {} variables", loop_var_initial_values.len());
        for (symbol_id, (initial_reg, var_type)) in &loop_var_initial_values {
           // eprintln!("DEBUG: Creating phi for symbol {:?}, initial reg {:?}", symbol_id, initial_reg);
            if let Some(phi_reg) = self.builder.build_phi(cond_block, var_type.clone()) {
               // eprintln!("DEBUG: Created phi node with dest {:?}", phi_reg);
                // Add incoming value from entry block
                self.builder.add_phi_incoming(cond_block, phi_reg, entry_block, *initial_reg);
               // eprintln!("DEBUG: Added incoming edge from entry block {:?}", entry_block);

                // Register the phi node as a local so Cranelift can find its type
                if let Some(func) = self.builder.current_function_mut() {
                    if let Some(local) = func.locals.get(initial_reg).cloned() {
                        func.locals.insert(phi_reg, super::IrLocal {
                            name: format!("{}_phi", local.name),
                            ty: var_type.clone(),
                            mutable: true,
                            source_location: local.source_location,
                            allocation: super::AllocationHint::Register,
                        });
                    }
                }

                // Update symbol map to use phi node
                phi_nodes.insert(*symbol_id, phi_reg);
                self.symbol_map.insert(*symbol_id, phi_reg);
            }
        }
       // eprintln!("DEBUG: Created {} phi nodes", phi_nodes.len());

        // Push loop context
        self.loop_stack.push(LoopContext {
            continue_block: cond_block,
            break_block: exit_block,
            label: label.cloned(),
        });

        // Evaluate condition
        if let Some(cond_reg) = self.lower_expression(condition) {
            self.builder.build_cond_branch(cond_reg, body_block, exit_block);
        }

        // Body block
        self.builder.switch_to_block(body_block);
        self.lower_block(body);

        // Get the end block of the loop body (might be different if there are nested blocks)
        let body_end_block = if let Some(block_id) = self.builder.current_block() {
            block_id
        } else {
            return;
        };

        // Add phi incoming edges for updated values from loop body
        for (symbol_id, phi_reg) in &phi_nodes {
            if let Some(&updated_reg) = self.symbol_map.get(symbol_id) {
                // Only add incoming if it's different from the phi node itself
                // (the symbol map now points to the updated value)
                if updated_reg != *phi_reg {
                    self.builder.add_phi_incoming(cond_block, *phi_reg, body_end_block, updated_reg);
                }
            }
        }

        // Branch back to condition if body didn't terminate
        if !self.is_terminated() {
            self.builder.build_branch(cond_block);
        }

        // Pop loop context
        self.loop_stack.pop();

        // Continue in exit block
        // The exit block will receive loop-carried values as block parameters when
        // the conditional branch from the loop header takes the false path
        self.builder.switch_to_block(exit_block);

        // Create block parameters in the exit block to receive final loop values
        // This is the Cranelift way: block parameters represent phi nodes
       // eprintln!("DEBUG: Creating exit block parameters for loop variables:");

        // We need to maintain the same order as the phi nodes for consistency
        let phi_nodes_ordered: Vec<_> = phi_nodes.iter().collect();

        for (symbol_id, loop_phi_reg) in &phi_nodes_ordered {
            if let Some((_, var_type)) = loop_var_initial_values.get(symbol_id) {
                // Allocate a new register for the exit block parameter (do this first to avoid double borrow)
                let exit_param_reg = self.builder.alloc_reg().unwrap();

               // eprintln!("DEBUG:   Created exit param {:?} for {:?} (from loop phi {:?})", exit_param_reg, symbol_id, loop_phi_reg);

                // Now create the phi node and register it
                if let Some(func) = self.builder.current_function_mut() {
                    if let Some(exit_block_data) = func.cfg.get_block_mut(exit_block) {
                        // Create a phi node in the exit block that receives the value from cond_block
                        let exit_phi = super::IrPhiNode {
                            dest: exit_param_reg,
                            incoming: vec![(cond_block, **loop_phi_reg)],
                            ty: var_type.clone(),
                        };
                        exit_block_data.add_phi(exit_phi);

                        // Register as a local
                        func.locals.insert(exit_param_reg, super::IrLocal {
                            name: format!("loop_exit_{}", (*symbol_id).as_raw()),
                            ty: var_type.clone(),
                            mutable: false,
                            source_location: super::IrSourceLocation::unknown(),
                            allocation: super::AllocationHint::Register,
                        });
                    }
                }

                // Update symbol map to use the exit parameter
                self.symbol_map.insert(**symbol_id, exit_param_reg);
            }
        }
    }
    
    // Helper methods...

    /// Collect all variables referenced in a block
    fn collect_referenced_variables_in_block(&self, block: &HirBlock, vars: &mut std::collections::HashSet<SymbolId>) {
        for stmt in &block.statements {
            self.collect_referenced_variables_in_stmt(stmt, vars);
        }
    }

    /// Collect all variables referenced in a statement
    fn collect_referenced_variables_in_stmt(&self, stmt: &HirStatement, vars: &mut std::collections::HashSet<SymbolId>) {
        match stmt {
            HirStatement::Let { init, .. } => {
                if let Some(expr) = init {
                    self.collect_referenced_variables_in_expr(expr, vars);
                }
            }
            HirStatement::Expr(expr) => {
                self.collect_referenced_variables_in_expr(expr, vars);
            }
            HirStatement::Assign { lhs, rhs, .. } => {
                if let HirLValue::Variable(sym) = lhs {
                    vars.insert(*sym);
                }
                self.collect_referenced_variables_in_expr(rhs, vars);
            }
            HirStatement::Return(Some(expr)) => {
                self.collect_referenced_variables_in_expr(expr, vars);
            }
            HirStatement::If { condition, then_branch, else_branch, .. } => {
                self.collect_referenced_variables_in_expr(condition, vars);
                self.collect_referenced_variables_in_block(then_branch, vars);
                if let Some(else_blk) = else_branch {
                    self.collect_referenced_variables_in_block(else_blk, vars);
                }
            }
            HirStatement::While { condition, body, .. } | HirStatement::DoWhile { condition, body, .. } => {
                self.collect_referenced_variables_in_expr(condition, vars);
                self.collect_referenced_variables_in_block(body, vars);
            }
            _ => {}
        }
    }

    /// Collect all variables referenced in an expression
    fn collect_referenced_variables_in_expr(&self, expr: &HirExpr, vars: &mut std::collections::HashSet<SymbolId>) {
        match &expr.kind {
            HirExprKind::Variable { symbol, .. } => {
                vars.insert(*symbol);
            }
            HirExprKind::Binary { lhs, rhs, .. } => {
                self.collect_referenced_variables_in_expr(lhs, vars);
                self.collect_referenced_variables_in_expr(rhs, vars);
            }
            HirExprKind::Unary { operand, .. } => {
                self.collect_referenced_variables_in_expr(operand, vars);
            }
            HirExprKind::If { condition, then_expr, else_expr, .. } => {
                self.collect_referenced_variables_in_expr(condition, vars);
                self.collect_referenced_variables_in_expr(then_expr, vars);
                self.collect_referenced_variables_in_expr(else_expr, vars);
            }
            HirExprKind::Call { callee, args, .. } => {
                self.collect_referenced_variables_in_expr(callee, vars);
                for arg in args {
                    self.collect_referenced_variables_in_expr(arg, vars);
                }
            }
            HirExprKind::Block(block) => {
                // Recursively collect variables from block expressions
                self.collect_referenced_variables_in_block(block, vars);
            }
            _ => {}
        }
    }

    /// Find all variables that are modified (assigned) in a block
    /// This is used for SSA phi node insertion in loops
    fn find_modified_variables_in_block(&self, block: &HirBlock) -> std::collections::HashSet<SymbolId> {
        let mut modified = std::collections::HashSet::new();

        for stmt in &block.statements {
            self.find_modified_variables_in_statement(stmt, &mut modified);
        }

        modified
    }

    /// Recursively find modified variables in a statement
    fn find_modified_variables_in_statement(
        &self,
        stmt: &HirStatement,
        modified: &mut std::collections::HashSet<SymbolId>,
    ) {
        match stmt {
            HirStatement::Let { pattern, .. } => {
                // Variable declarations are modifications
               // eprintln!("DEBUG: Found Let statement");
                self.collect_pattern_variables(pattern, modified);
            }
            HirStatement::Expr(expr) => {
               // eprintln!("DEBUG: Found Expr statement with kind: {:?}", std::mem::discriminant(&expr.kind));
                self.find_modified_variables_in_expression(expr, modified);
            }
            HirStatement::Assign { lhs, rhs, .. } => {
                // Assignments modify the left-hand side
                match lhs {
                    HirLValue::Variable(symbol) => {
                        modified.insert(*symbol);
                    }
                    _ => {}
                }
                self.find_modified_variables_in_expression(rhs, modified);
            }
            HirStatement::If { then_branch, else_branch, .. } => {
                for stmt in &then_branch.statements {
                    self.find_modified_variables_in_statement(stmt, modified);
                }
                if let Some(else_blk) = else_branch {
                    for stmt in &else_blk.statements {
                        self.find_modified_variables_in_statement(stmt, modified);
                    }
                }
            }
            HirStatement::While { body, .. } | HirStatement::DoWhile { body, .. } => {
                for stmt in &body.statements {
                    self.find_modified_variables_in_statement(stmt, modified);
                }
            }
            HirStatement::ForIn { body, .. } => {
                for stmt in &body.statements {
                    self.find_modified_variables_in_statement(stmt, modified);
                }
            }
            HirStatement::Label { block, .. } => {
                for stmt in &block.statements {
                    self.find_modified_variables_in_statement(stmt, modified);
                }
            }
            _ => {}
        }
    }

    /// Find modified variables in an expression (assignments)
    fn find_modified_variables_in_expression(
        &self,
        expr: &HirExpr,
        modified: &mut std::collections::HashSet<SymbolId>,
    ) {
        match &expr.kind {
            HirExprKind::Binary { lhs, rhs, .. } => {
                self.find_modified_variables_in_expression(lhs, modified);
                self.find_modified_variables_in_expression(rhs, modified);
            }
            HirExprKind::Unary { operand, .. } => {
                self.find_modified_variables_in_expression(operand, modified);
            }
            HirExprKind::If { then_expr, else_expr, .. } => {
                self.find_modified_variables_in_expression(then_expr, modified);
                self.find_modified_variables_in_expression(else_expr, modified);
            }
            HirExprKind::Call { args, .. } => {
                for arg in args {
                    self.find_modified_variables_in_expression(arg, modified);
                }
            }
            _ => {}
        }
    }

    /// Collect all variable symbols from a pattern
    fn collect_pattern_variables(
        &self,
        pattern: &HirPattern,
        variables: &mut std::collections::HashSet<SymbolId>,
    ) {
        match pattern {
            HirPattern::Variable { symbol, .. } => {
                variables.insert(*symbol);
            }
            HirPattern::Tuple(patterns) => {
                for p in patterns {
                    self.collect_pattern_variables(p, variables);
                }
            }
            HirPattern::Constructor { fields, .. } => {
                for p in fields {
                    self.collect_pattern_variables(p, variables);
                }
            }
            HirPattern::Array { elements, rest } => {
                for p in elements {
                    self.collect_pattern_variables(p, variables);
                }
                if let Some(rest_pat) = rest {
                    self.collect_pattern_variables(rest_pat, variables);
                }
            }
            _ => {}
        }
    }

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
    
    fn convert_type(&self, type_id: TypeId) -> IrType {
        use crate::tast::TypeKind;

        // Look up the type in the type table
        let type_table = self.type_table.borrow();
        let type_ref = type_table.get(type_id);

        match type_ref.as_ref().map(|t| &t.kind) {
            // Primitive types
            Some(TypeKind::Int) => IrType::I32,
            Some(TypeKind::Float) => IrType::F64,
            Some(TypeKind::Bool) => IrType::Bool,
            Some(TypeKind::Void) => IrType::Void,
            Some(TypeKind::String) => IrType::String,

            // Function types - represented as function pointers (i64)
            Some(TypeKind::Function { params, return_type, .. }) => {
                // Convert parameter types
                let param_types: Vec<IrType> = params.iter()
                    .map(|p| self.convert_type(*p))
                    .collect();

                // Convert return type
                let ret_type = Box::new(self.convert_type(*return_type));

                IrType::Function {
                    params: param_types,
                    return_type: ret_type,
                    varargs: false,
                }
            }

            // Complex types - represented as pointers (i64)
            Some(TypeKind::Class { .. }) => IrType::Ptr(Box::new(IrType::Void)),
            Some(TypeKind::Interface { .. }) => IrType::Ptr(Box::new(IrType::Void)),
            Some(TypeKind::Enum { .. }) => IrType::I32, // Enums as tagged unions
            Some(TypeKind::Array { element_type, .. }) => {
                let elem_ty = self.convert_type(*element_type);
                IrType::Ptr(Box::new(elem_ty))  // Arrays as pointers
            }
            Some(TypeKind::Optional { inner_type }) => {
                // Optional types (T?) as nullable pointers
                let inner = self.convert_type(*inner_type);
                IrType::Ptr(Box::new(inner))
            }

            // Abstract types - use their underlying type
            Some(TypeKind::Abstract { underlying, .. }) => {
                if let Some(underlying_type) = underlying {
                    // If underlying type is specified, use it
                    self.convert_type(*underlying_type)
                } else {
                    // No underlying type specified, this is likely Int or similar
                    // Check the abstract definition for hints, for now default to I32
                    // TODO: Look up abstract definition to get actual underlying type
                    IrType::I32
                }
            }

            // Type parameters and dynamic types
            Some(TypeKind::TypeParameter { .. }) => IrType::Any,
            Some(TypeKind::Dynamic) => IrType::Any,  // Dynamic type as Any

            // Unknown or error types
            Some(TypeKind::Unknown) | Some(TypeKind::Error) => {
                // Unknown or error types - default to I32 for safety
                eprintln!("Warning: Unknown type {:?}, defaulting to I32", type_id);
                IrType::I32
            }

            None => {
                // Type not found in type table
                // This might be a class type that wasn't registered in the type table
                // but exists in the HIR module. Default to pointer for unknown types
                // since they're likely to be objects/classes
                eprintln!("Warning: Type {:?} not found in type table, defaulting to Ptr(Void)", type_id);
                eprintln!("  This may indicate a lambda or class type that wasn't properly registered");
                IrType::Ptr(Box::new(IrType::Void))  // Unknown types as pointers (safer than I32)
            }

            // Catch-all for other types
            Some(other) => {
                eprintln!("Warning: Unhandled type kind for {:?}: {:?}, defaulting to I32", type_id, other);
                IrType::I32
            }
        }
    }
    
    fn convert_source_location(&self, loc: &SourceLocation) -> IrSourceLocation {
        IrSourceLocation {
            file_id: loc.file_id,
            line: loc.line,
            column: loc.column,
        }
    }
    
    fn lower_literal(&mut self, lit: &HirLiteral, type_id: TypeId) -> Option<IrId> {
        match lit {
            HirLiteral::Int(i) => {
                // Use the actual type from type checking instead of always using I64
                let ir_type = self.convert_type(type_id);
                self.builder.build_int(*i, ir_type)
            }
            HirLiteral::Float(f) => {
                let ir_type = self.convert_type(type_id);
                match ir_type {
                    IrType::F32 => self.builder.build_const(IrValue::F32(*f as f32)),
                    IrType::F64 => self.builder.build_const(IrValue::F64(*f)),
                    _ => self.builder.build_const(IrValue::F64(*f)), // Default to F64
                }
            }
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

    /// Build function signature for an instance method with implicit 'this' parameter
    fn build_instance_method_signature(&self, func: &HirFunction, class_type_id: TypeId) -> super::IrFunctionSignature {
        let mut builder = FunctionSignatureBuilder::new();

        // Add implicit 'this' parameter as first parameter
        // The type is a pointer/reference to the class instance
        let this_type = self.convert_type(class_type_id);
        builder = builder.param("this".to_string(), this_type);

        // Add regular parameters
        for param in &func.params {
            let param_name = self.string_interner.get(param.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("param_{}", param.symbol_id.as_raw()));
            let param_type = self.convert_type(param.ty);
            builder = builder.param(param_name, param_type);
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
        let is_term = self.is_terminated();
        eprintln!("DEBUG ensure_terminator: is_terminated={}, current_func={:?}",
                  is_term,
                  self.builder.current_function().map(|f| &f.name));
        if !is_term {
            eprintln!("DEBUG ensure_terminator: Adding implicit return(None)");
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
    
    /// Bind a pattern with type information (registers locals for Cranelift)
    fn bind_pattern_with_type(&mut self, pattern: &HirPattern, value: IrId, ty: Option<TypeId>, is_mutable: bool) {
        match pattern {
            HirPattern::Variable { name, symbol } => {
                eprintln!("DEBUG: Binding symbol {:?} to value {:?}", symbol, value);
                // Bind the value to the symbol
                self.symbol_map.insert(*symbol, value);

                // Register as local so Cranelift can find the type
                if let Some(type_id) = ty {
                    let var_type = self.convert_type(type_id);
                    if let Some(func) = self.builder.current_function_mut() {
                        func.locals.insert(value, super::IrLocal {
                            name: name.to_string(),
                            ty: var_type,
                            mutable: is_mutable,
                            source_location: IrSourceLocation::unknown(),
                            allocation: super::AllocationHint::Stack,
                        });
                    }
                }
            }
            _ => {
                // For other patterns, fall back to old behavior
                self.bind_pattern(pattern, value);
            }
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
                    // Look up the field index
                    if let Some(&(_class_type_id, field_index)) = self.field_index_map.get(field) {
                        eprintln!("DEBUG: Field write - field={:?}, index={}", field, field_index);

                        // Create constant for field index
                        if let Some(index_const) = self.builder.build_const(IrValue::I32(field_index as i32)) {
                            // Get the type of the field from the value being assigned
                            let field_ty = if let Some(func) = self.builder.current_function() {
                                func.locals.get(&value)
                                    .map(|local| local.ty.clone())
                                    .unwrap_or(IrType::I32)
                            } else {
                                IrType::I32
                            };

                            // Use GEP to get field pointer, then store
                            if let Some(field_ptr) = self.builder.build_gep(obj_reg, vec![index_const], field_ty) {
                                self.builder.build_store(field_ptr, value);
                                eprintln!("DEBUG: Field write successful");
                            }
                        }
                    } else {
                        eprintln!("WARNING: Field {:?} not found in field_index_map for write", field);
                        self.add_error(
                            &format!("Field {:?} index not found for write - class may not be registered", field),
                            SourceLocation::unknown()
                        );
                    }
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
        // Look up the field index from our field_index_map
        let (class_type_id, field_index) = match self.field_index_map.get(&field) {
            Some(&mapping) => mapping,
            None => {
                eprintln!("WARNING: Field {:?} not found in field_index_map", field);
                self.add_error(
                    &format!("Field {:?} index not found - class may not be registered", field),
                    SourceLocation::unknown()
                );
                return None;
            }
        };

        eprintln!("DEBUG: Field access - field={:?}, class_type={:?}, index={}", field, class_type_id, field_index);

        // Create constant for field index
        let index_const = self.builder.build_const(IrValue::I32(field_index as i32))?;

        // Get the type of the field
        let field_ty = self.convert_type(ty);

        // Use GetElementPtr to get pointer to the field
        // obj is a pointer to the struct, indices are [field_index]
        let field_ptr = self.builder.build_gep(obj, vec![index_const], field_ty.clone())?;

        // Load the value from the field pointer
        let field_value = self.builder.build_load(field_ptr, field_ty)?;

        eprintln!("DEBUG: Field access successful - value={:?}", field_value);
        Some(field_value)
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
        //   (plus phi nodes for any variables modified in branches)

        let then_block = self.builder.create_block()?;
        let else_block = self.builder.create_block()?;
        let merge_block = self.builder.create_block()?;

        // Snapshot symbol_map before branches
        let symbol_map_before = self.symbol_map.clone();
        eprintln!("DEBUG lower_conditional: symbol_map has {} entries before condition", symbol_map_before.len());

        // Evaluate condition
        let cond_val = self.lower_expression(cond)?;
        eprintln!("DEBUG lower_conditional: After evaluating condition, in block {:?}", self.builder.current_block());

        // Branch based on condition
        self.builder.build_cond_branch(cond_val, then_block, else_block)?;

        // Then block
        self.builder.switch_to_block(then_block);
        let then_val = self.lower_expression(then_expr);
        let then_terminated = self.is_terminated();
        eprintln!("DEBUG lower_conditional: then_terminated = {}", then_terminated);
        if !then_terminated {
            self.builder.build_branch(merge_block)?;
        }
        let then_end_block = self.builder.current_block()?;
        let symbol_map_after_then = self.symbol_map.clone();
        eprintln!("DEBUG lower_conditional: then_end_block = {:?}, symbol_map has {} entries", then_end_block, symbol_map_after_then.len());

        // Else block
        self.symbol_map = symbol_map_before.clone();  // Reset to before-branch state
        self.builder.switch_to_block(else_block);
        let else_val = self.lower_expression(else_expr);
        let else_terminated = self.is_terminated();
        eprintln!("DEBUG lower_conditional: else_terminated = {}", else_terminated);
        if !else_terminated {
            self.builder.build_branch(merge_block)?;
        }
        let else_end_block = self.builder.current_block()?;
        let symbol_map_after_else = self.symbol_map.clone();
        eprintln!("DEBUG lower_conditional: else_end_block = {:?}, symbol_map has {} entries", else_end_block, symbol_map_after_else.len());

        // If both branches terminated, no merge block needed
        if then_terminated && else_terminated {
            // Both branches returned/broke/continued
            // No value to return, and we shouldn't create unreachable merge block
            return None;
        }

        // Merge block with phi nodes
        self.builder.switch_to_block(merge_block);

        // Find variables that were modified in either branch
        let mut modified_symbols = std::collections::HashSet::new();
        eprintln!("DEBUG: Checking for modified symbols");
        eprintln!("  symbol_map_before: {} entries", symbol_map_before.len());
        eprintln!("  symbol_map_after_then: {} entries", symbol_map_after_then.len());
        eprintln!("  symbol_map_after_else: {} entries", symbol_map_after_else.len());

        for (sym, reg_after_then) in &symbol_map_after_then {
            if symbol_map_before.get(sym) != Some(reg_after_then) {
                eprintln!("  Modified in then branch: {:?} (before: {:?}, after: {:?})",
                         sym, symbol_map_before.get(sym), reg_after_then);
                modified_symbols.insert(*sym);
            }
        }
        for (sym, reg_after_else) in &symbol_map_after_else {
            if symbol_map_before.get(sym) != Some(reg_after_else) {
                eprintln!("  Modified in else branch: {:?} (before: {:?}, after: {:?})",
                         sym, symbol_map_before.get(sym), reg_after_else);
                modified_symbols.insert(*sym);
            }
        }
        eprintln!("DEBUG: Found {} modified symbols", modified_symbols.len());

        // Create phi nodes for modified variables
        eprintln!("DEBUG: Creating phi nodes for {} symbols", modified_symbols.len());
        for symbol_id in &modified_symbols {
            eprintln!("  Processing symbol {:?}", symbol_id);
            let before_reg = symbol_map_before.get(symbol_id).copied();
            let then_reg = symbol_map_after_then.get(symbol_id).copied();
            let else_reg = symbol_map_after_else.get(symbol_id).copied();

            // Get type from locals table using the "before" register (from variable declaration)
            // because new registers from assignments don't have local entries
            let type_lookup_reg = before_reg.or(then_reg).or(else_reg);
            let var_type = match type_lookup_reg
                .and_then(|r| self.builder.current_function()
                    .and_then(|f| f.locals.get(&r))
                    .map(|local| local.ty.clone())) {
                Some(t) => {
                    eprintln!("  Found type {:?} for symbol {:?}", t, symbol_id);
                    t
                }
                None => {
                    eprintln!("  No type found for symbol {:?} (tried {:?}), skipping", symbol_id, type_lookup_reg);
                    continue;
                }
            };

            let sample_reg = then_reg.or(else_reg).or(before_reg).unwrap();

            // Create phi node
            eprintln!("  Creating phi for {:?} with type {:?}", symbol_id, var_type);
            let phi_reg = match self.builder.build_phi(merge_block, var_type.clone()) {
                Some(r) => r,
                None => {
                    eprintln!("  Failed to create phi node");
                    continue;
                }
            };
            eprintln!("  Created phi node {:?}", phi_reg);

            // Add incoming edges for non-terminated branches
            eprintln!("  Adding phi incoming: then_terminated={}, else_terminated={}", then_terminated, else_terminated);
            if !then_terminated {
                let val = then_reg.unwrap_or(before_reg.unwrap_or(sample_reg));
                eprintln!("  Calling add_phi_incoming(merge={:?}, phi={:?}, from={:?}, val={:?})",
                         merge_block, phi_reg, then_end_block, val);
                match self.builder.add_phi_incoming(merge_block, phi_reg, then_end_block, val) {
                    Some(()) => eprintln!("  Successfully added phi incoming from then"),
                    None => eprintln!("  WARNING: Failed to add phi incoming from then block {:?}", then_end_block),
                }
            }
            if !else_terminated {
                let val = else_reg.unwrap_or(before_reg.unwrap_or(sample_reg));
                eprintln!("  Calling add_phi_incoming(merge={:?}, phi={:?}, from={:?}, val={:?})",
                         merge_block, phi_reg, else_end_block, val);
                match self.builder.add_phi_incoming(merge_block, phi_reg, else_end_block, val) {
                    Some(()) => eprintln!("  Successfully added phi incoming from else"),
                    None => eprintln!("  WARNING: Failed to add phi incoming from else block {:?}", else_end_block),
                }
            }

            // Register phi as local
            if let Some(func) = self.builder.current_function_mut() {
                if let Some(local) = func.locals.get(&sample_reg).cloned() {
                    func.locals.insert(phi_reg, super::IrLocal {
                        name: format!("{}_phi", local.name),
                        ty: var_type.clone(),
                        mutable: true,
                        source_location: local.source_location,
                        allocation: super::AllocationHint::Register,
                    });
                }
            }

            // Update symbol map to use phi
            self.symbol_map.insert(*symbol_id, phi_reg);
        }

        // Create phi for expression result if both branches returned values
        let mut result_phi = None;
        eprintln!("DEBUG: Checking if need result phi: then_val={:?}, else_val={:?}", then_val.is_some(), else_val.is_some());
        // Only create result phi if BOTH branches return values (for expression-style ifs)
        // If only one returns a value, that's a type error - skip result phi
        if then_val.is_some() && else_val.is_some() {
            // Determine result type from then expression
            // TODO: Get actual type from HIR expression
            let result_type = IrType::I32; // Placeholder
            let result = match self.builder.build_phi(merge_block, result_type.clone()) {
                Some(r) => {
                    eprintln!("DEBUG: Created result phi {:?}", r);
                    r
                }
                None => {
                    eprintln!("DEBUG: Failed to create result phi");
                    return None;
                }
            };

            eprintln!("DEBUG: Adding result phi incoming: then_term={}, else_term={}", then_terminated, else_terminated);
            // Both branches returned values, so add phi incoming from both
            if !then_terminated {
                let val = then_val.unwrap(); // Safe because we checked is_some() above
                eprintln!("DEBUG:   Adding from then: block={:?}, val={:?}", then_end_block, val);
                match self.builder.add_phi_incoming(merge_block, result, then_end_block, val) {
                    Some(()) => eprintln!("DEBUG:   Success"),
                    None => eprintln!("DEBUG:   FAILED!"),
                }
            }
            if !else_terminated {
                let val = else_val.unwrap(); // Safe because we checked is_some() above
                eprintln!("DEBUG:   Adding from else: block={:?}, val={:?}", else_end_block, val);
                match self.builder.add_phi_incoming(merge_block, result, else_end_block, val) {
                    Some(()) => eprintln!("DEBUG:   Success"),
                    None => eprintln!("DEBUG:   FAILED!"),
                }
            }
            result_phi = Some(result);
        }

        result_phi
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
                // TODO: Get proper type from pattern context
                // For now, use a default type based on the literal kind
                let default_type = match lit {
                    HirLiteral::Int(_) => TypeId::from_raw(1), // Assume Int type (ID 1)
                    HirLiteral::Float(_) => TypeId::from_raw(2), // Assume Float type
                    HirLiteral::Bool(_) => TypeId::from_raw(3), // Assume Bool type
                    HirLiteral::String(_) => TypeId::from_raw(4), // Assume String type
                    _ => TypeId::from_raw(1), // Default to Int
                };
                let lit_val = self.lower_literal(lit, default_type)?;
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
    
    fn lower_lambda(&mut self, params: &[HirParam], body: &HirExpr, captures: &[HirCapture], lambda_type: TypeId) -> Option<IrId> {
        // Closure/Lambda lowering using MakeClosure instruction:
        //
        // For: |x, y| { x + y + captured_z }
        //
        // Strategy:
        // 1. Generate a lambda function that takes (env*, params...) where
        //    env* is a struct containing all captured variables
        // 2. Collect the values to be captured (from current scope)
        // 3. Use MakeClosure instruction to create closure at runtime
        //
        // The MakeClosure instruction will:
        // - Allocate an environment struct
        // - Copy captured values into it
        // - Create a closure struct { func_ptr, env_ptr }
        // - Return the closure

        // Step 1: Generate the lambda function
        let lambda_func_id = self.generate_lambda_function(params, body, captures, lambda_type)?;

        // Step 2: Collect captured values from current scope
        let mut captured_values = Vec::new();
        for capture in captures {
            if let Some(&captured_val) = self.symbol_map.get(&capture.symbol) {
                captured_values.push(captured_val);
            } else {
                // Captured variable not found in current scope
                self.errors.push(LoweringError {
                    message: format!("Captured variable {:?} not found in scope", capture.symbol),
                    location: body.source_location.clone(),
                });
                return None;
            }
        }

        // Step 3: Use MakeClosure instruction to create closure
        self.builder.build_make_closure(lambda_func_id, captured_values)
    }

    /// Generate a lambda function
    ///
    /// Creates a new function that takes (env*, params...) as arguments,
    /// where env* is a pointer to a struct containing captured variables.
    fn generate_lambda_function(
        &mut self,
        params: &[HirParam],
        body: &HirExpr,
        captures: &[HirCapture],
        lambda_type: TypeId,
    ) -> Option<IrFunctionId> {
        // Generate unique lambda name
        let lambda_id = self.lambda_counter;
        self.lambda_counter += 1;
        let lambda_name = format!("<lambda_{}>", lambda_id);

        // Save current builder state (we'll need to restore it after generating the lambda)
        // PROBLEM: IrBuilder fields are private, so we can't directly save/restore state
        //
        // WORKAROUND: Generate lambda function by manually constructing IrFunction
        // without using IrBuilder, then adding it to the module

        // Build function signature
        // First parameter is environment pointer (if there are captures)
        let mut func_params = Vec::new();
        let mut next_reg_id = 0u32;

        if !captures.is_empty() {
            func_params.push(IrParameter {
                name: "env".to_string(),
                ty: IrType::Ptr(Box::new(IrType::Void)), // void* for environment
                reg: IrId::new(next_reg_id),
                by_ref: false,
            });
            next_reg_id += 1;
        }

        // Add lambda parameters
        for param in params {
            let param_type = self.convert_type(param.ty);
            let param_name = self.string_interner.get(param.name)
                .unwrap_or("<unknown>")
                .to_string();
            func_params.push(IrParameter {
                name: param_name,
                ty: param_type,
                reg: IrId::new(next_reg_id),
                by_ref: false,
            });
            next_reg_id += 1;
        }

        // Return type - extract from the lambda expression's function type
        // The lambda expression's ty field contains the FUNCTION type (e.g., Int -> Int)
        // We need to extract the return type from it for correct MIR lowering
        let return_type = {
            let type_table = self.type_table.borrow();
            let func_type_ref = type_table.get(lambda_type);

            let return_ty_id = if let Some(ty) = func_type_ref.as_ref() {
                // Extract return type from function type
                if let TypeKind::Function { return_type, .. } = &ty.kind {
                    *return_type
                } else {
                    // Fallback to body type if not a function
                    body.ty
                }
            } else {
                // Type not found, use body type as fallback
                body.ty
            };

            drop(type_table); // Release borrow before calling convert_type
            self.convert_type(return_ty_id)
        };

        // Create function signature
        let signature = IrFunctionSignature {
            parameters: func_params,
            return_type,
            calling_convention: CallingConvention::Haxe,
            can_throw: false,
            type_params: vec![],
            uses_sret: false, // Lambda functions don't use sret
        };

        // Allocate function ID from module
        let func_id = self.builder.module.alloc_function_id();
        let symbol_id = SymbolId::from_raw(1000000 + lambda_id);

        // Save current builder state (we need to restore it after generating the lambda)
        let saved_function = self.builder.current_function;
        let saved_block = self.builder.current_block;

        // Create the lambda function
        let mut lambda_function = IrFunction::new(
            func_id,
            symbol_id,
            lambda_name.clone(),
            signature.clone(),
        );

        // Get entry block
        let entry_block = lambda_function.entry_block();

        // Lower lambda body by temporarily switching builder context
        // We need to:
        // 1. Add lambda function to module temporarily (so builder can work with it)
        // 2. Switch builder to the lambda function context
        // 3. Set up parameter mappings
        // 4. Lower the body expression
        // 5. Create return instruction
        // 6. Retrieve modified function from module

        // Add the lambda function to module (builder needs it there to work with it)
        self.builder.module.add_function(lambda_function);

        // Switch builder context to the lambda function
        self.builder.current_function = Some(func_id);
        self.builder.current_block = Some(entry_block);

        // Save current symbol map (we'll need to restore it)
        let saved_symbol_map = self.symbol_map.clone();

        // Set up parameter mappings
        // Parameter 0 is environment pointer (if captures exist)
        let param_offset = if !captures.is_empty() { 1 } else { 0 };

        for (i, param) in params.iter().enumerate() {
            // Get the register ID for this parameter (assigned when we created IrParameter)
            let param_reg = IrId::new((param_offset + i) as u32);
            self.symbol_map.insert(param.symbol_id, param_reg);
        }

        // Set up captured variable access from environment
        // If we have captures, the first parameter (register 0) is the environment pointer
        if !captures.is_empty() {
            let env_ptr_reg = IrId::new(0); // First parameter is env pointer

            // For each captured variable, load it from the environment struct
            // Environment layout: struct { captured_0, captured_1, ... }
            for (field_index, capture) in captures.iter().enumerate() {
                // Calculate field offset (for simplicity, assume each field is 8 bytes)
                // This matches the layout created by MakeClosure instruction
                let field_offset = (field_index * 8) as i64;

                // Create offset constant
                let offset_reg = self.builder.build_int(field_offset, IrType::I64)?;

                // Calculate field pointer: env_ptr + offset
                let field_ptr_reg = self.builder.build_binop(BinaryOp::Add, env_ptr_reg, offset_reg)?;

                // Load value from the field pointer
                // For now, assume all captured values are I32 (we'll improve this later)
                let captured_value_reg = self.builder.build_load(field_ptr_reg, IrType::I32)?;

                // Add ALL registers to the lambda function's locals for type tracking
                // This is needed for Cranelift lowering to know register types
                if let Some(lambda_func) = self.builder.module.functions.get_mut(&func_id) {
                    lambda_func.locals.insert(offset_reg, IrLocal {
                        name: format!("offset_{}", field_index),
                        ty: IrType::I64,
                        mutable: false,
                        source_location: IrSourceLocation::unknown(),
                        allocation: crate::ir::AllocationHint::Register,
                    });
                    lambda_func.locals.insert(field_ptr_reg, IrLocal {
                        name: format!("field_ptr_{}", field_index),
                        ty: IrType::I64, // Pointer type
                        mutable: false,
                        source_location: IrSourceLocation::unknown(),
                        allocation: crate::ir::AllocationHint::Register,
                    });
                    lambda_func.locals.insert(captured_value_reg, IrLocal {
                        name: format!("captured_{}", field_index),
                        ty: IrType::I32, // Match the load type
                        mutable: false,
                        source_location: IrSourceLocation::unknown(),
                        allocation: crate::ir::AllocationHint::Register,
                    });
                }

                // Map the captured symbol to its loaded register
                self.symbol_map.insert(capture.symbol, captured_value_reg);
            }
        }

        // Lower the lambda body expression
        let body_result = self.lower_expression(body);

        // Get the lambda function back from module (it was modified by builder)
        let lambda_function = self.builder.module.functions.get_mut(&func_id)
            .expect("Lambda function should exist in module");

        // Check if the block already has a terminator (e.g., from a return statement in body)
        let has_terminator = {
            let entry_block_ref = lambda_function.cfg.get_block_mut(entry_block).unwrap();
            !matches!(entry_block_ref.terminator, IrTerminator::Unreachable)
        };

        if !has_terminator {
            // No terminator yet - add an implicit return
            let terminator = if signature.return_type == IrType::Void {
                IrTerminator::Return { value: None }
            } else if let Some(result_reg) = body_result {
                // Body produced a value - return it
                // Note: Type conversion happens during MIR lowering, we return the register as-is
                IrTerminator::Return { value: Some(result_reg) }
            } else {
                // Body didn't return a value but function expects one
                // This can happen for blocks that don't have a trailing expression
                eprintln!("Warning: Lambda body returned no value but function expects {:?}", signature.return_type);
                // Create a default zero value of the appropriate type
                // Pre-allocate the register and create the const value
                let default_reg = lambda_function.alloc_reg();
                let default_value = match &signature.return_type {
                    IrType::I32 => IrValue::I32(0),
                    IrType::I64 => IrValue::I64(0),
                    IrType::F32 => IrValue::F32(0.0),
                    IrType::F64 => IrValue::F64(0.0),
                    IrType::Bool => IrValue::Bool(false),
                    IrType::Any => IrValue::I64(0),
                    _ => IrValue::I32(0),
                };
                // Now get the block and add the instruction
                let entry_block_mut = lambda_function.cfg.get_block_mut(entry_block).unwrap();
                entry_block_mut.add_instruction(IrInstruction::Const {
                    dest: default_reg,
                    value: default_value,
                });
                IrTerminator::Return { value: Some(default_reg) }
            };

            // Set the terminator
            let entry_block_mut = lambda_function.cfg.get_block_mut(entry_block).unwrap();
            entry_block_mut.set_terminator(terminator);
        }

        // Restore symbol map
        self.symbol_map = saved_symbol_map;

        // Restore builder state to outer function context
        self.builder.current_function = saved_function;
        self.builder.current_block = saved_block;

        eprintln!("Info: Generated lambda function stub '{}'", lambda_name);
        eprintln!("  Signature: ({} params) -> {:?}", signature.parameters.len(), signature.return_type);
        eprintln!("  Captures: {} variables", captures.len());

        Some(func_id)
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

    /// Recursively collect fields from parent classes
    fn collect_inherited_fields(
        &mut self,
        parent_type: Option<TypeId>,
        child_type: TypeId,
        fields: &mut Vec<IrField>,
        field_index: &mut u32,
    ) {
        if let Some(parent_type_id) = parent_type {
            eprintln!("DEBUG: Looking for parent TypeId={:?} in current_hir_types", parent_type_id);

            // Try direct lookup first
            if let Some(HirTypeDecl::Class(parent_class)) = self.current_hir_types.get(&parent_type_id) {
                eprintln!("DEBUG: Found parent class directly: {:?}", parent_class.name);
                self.add_parent_fields(parent_class, child_type, fields, field_index);
            } else {
                // TypeId mismatch - the extends field might use instance type while
                // hir_module.types uses declaration type. Search by matching class type.
                eprintln!("DEBUG: Parent not found directly, searching all types...");

                // Get the type definition to find the class symbol
                if let Some(parent_type_def) = self.type_table.borrow().get(parent_type_id) {
                    if let crate::tast::TypeKind::Class { symbol_id: parent_symbol, .. } = &parent_type_def.kind {
                        eprintln!("DEBUG: Parent class symbol: {:?}", parent_symbol);

                        // Find the HIR class by symbol_id
                        for (decl_type_id, type_decl) in self.current_hir_types.iter() {
                            if let HirTypeDecl::Class(class) = type_decl {
                                if class.symbol_id == *parent_symbol {
                                    eprintln!("DEBUG: Found parent class by symbol: {:?} (TypeId={:?})", class.name, decl_type_id);
                                    self.add_parent_fields(class, child_type, fields, field_index);
                                    return;
                                }
                            }
                        }
                    }
                }

                eprintln!("WARNING: Could not find parent class for TypeId={:?}", parent_type_id);
            }
        }
    }

    /// Add parent class fields to the field list
    fn add_parent_fields(
        &mut self,
        parent_class: &HirClass,
        child_type: TypeId,
        fields: &mut Vec<IrField>,
        field_index: &mut u32,
    ) {
        eprintln!("DEBUG: add_parent_fields for parent {:?}, child TypeId={:?}", parent_class.name, child_type);
        eprintln!("DEBUG: Parent has {} fields", parent_class.fields.len());

        // First, recursively collect grandparent fields
        self.collect_inherited_fields(parent_class.extends, child_type, fields, field_index);

        // Then add parent's own fields
        for parent_field in &parent_class.fields {
            eprintln!("DEBUG: Adding parent field {:?} (SymbolId={:?}) at index {}",
                     parent_field.name, parent_field.symbol_id, *field_index);
            // Map parent field symbol to child class's type with the correct index
            self.field_index_map.insert(parent_field.symbol_id, (child_type, *field_index));

            fields.push(IrField {
                name: parent_field.name.to_string(),
                ty: self.convert_type(parent_field.ty),
                offset: None,
            });

            *field_index += 1;
        }
    }

    fn register_class_metadata(&mut self, type_id: TypeId, class: &HirClass) {
        // Register class as struct type
        let typedef_id = self.builder.module.alloc_typedef_id();

        let mut fields = Vec::new();
        let mut field_index = 0u32;

        // Collect all inherited fields from parent classes (recursively)
        eprintln!("DEBUG: register_class_metadata for {:?} (TypeId={:?}), extends={:?}",
                 class.name, type_id, class.extends);
        self.collect_inherited_fields(class.extends, type_id, &mut fields, &mut field_index);

        // Then add this class's own fields
        eprintln!("DEBUG: Adding {} own fields for class {:?} starting at index {}",
                 class.fields.len(), class.name, field_index);
        for field in &class.fields {
            eprintln!("DEBUG: Adding own field {:?} (SymbolId={:?}) at index {}",
                     field.name, field.symbol_id, field_index);
            // Store field index mapping for field access lowering
            self.field_index_map.insert(field.symbol_id, (type_id, field_index));

            fields.push(IrField {
                name: field.name.to_string(),
                ty: self.convert_type(field.ty),
                offset: None,
            });

            field_index += 1;
        }

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

        // IMPORTANT: Pre-register constructor mapping so that 'new' expressions
        // in function bodies can find the constructor before it's actually lowered.
        // The constructor will be lowered later in the second pass.
        // We use a placeholder IrFunctionId that will be updated when the actual
        // constructor is lowered.
        //
        // NOTE: We can't lower the constructor here because we're still in the
        // metadata registration phase and function lowering requires a different
        // builder state. So we just reserve the mapping.
        //
        // Actually, we can't pre-register with a placeholder because we don't have
        // a function ID yet. The real fix is to ensure constructors are lowered
        // before module-level functions. But for now, we'll keep the current
        // approach and ensure the ordering is correct at the module level.
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
    string_interner: &StringInterner,
    type_table: &Rc<RefCell<TypeTable>>,
    symbol_table: &SymbolTable,
) -> Result<IrModule, Vec<LoweringError>> {
    let mut context = HirToMirContext::new(
        hir_module.name.clone(),
        hir_module.metadata.source_file.clone(),
        string_interner,
        type_table,
        &hir_module.types,
        symbol_table,
    );

    context.lower_module(hir_module)
}