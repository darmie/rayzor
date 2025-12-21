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
    BinaryOp, CallingConvention, CompareOp, EnvironmentLayout, FunctionSignatureBuilder, IrBasicBlock, IrBlockId,
    IrBuilder, IrEnumVariant, IrField, IrFunction, IrFunctionId, IrFunctionSignature, IrGlobal,
    IrGlobalId, IrId, IrInstruction, IrLocal, IrModule, IrParameter, IrPhiNode, IrSourceLocation,
    IrTerminator, IrType, IrTypeDef, IrTypeDefId, IrTypeDefinition, IrValue, Linkage, UnaryOp,
};
use crate::stdlib::{MethodSignature, StdlibMapping};
use crate::tast::{
    InternedString, SourceLocation, StringInterner, SymbolId, SymbolTable, TypeId, TypeKind,
    TypeTable,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Context for lowering HIR to MIR
pub struct HirToMirContext<'a> {
    /// MIR builder
    builder: IrBuilder,

    /// Mapping from HIR symbols to MIR registers (for variables/parameters)
    symbol_map: HashMap<SymbolId, IrId>,

    /// Mapping from HIR function symbols to MIR function IDs
    function_map: HashMap<SymbolId, crate::ir::IrFunctionId>,

    /// External function map from previously compiled modules (e.g., stdlib)
    /// These are functions defined in other modules that can be called from this module
    external_function_map: HashMap<SymbolId, crate::ir::IrFunctionId>,

    /// Name-based external function map for cross-file lookups
    /// This is used when SymbolIds don't match (e.g., "StringTools.startsWith" -> IrFunctionId)
    external_function_name_map: HashMap<String, crate::ir::IrFunctionId>,

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

    /// Mapping from (typedef_type_id, field_name) to field_index for anonymous struct fields
    /// This allows field access on typedef'd anonymous structs like FileStat
    /// where field symbols may be created at access sites rather than typedef declaration
    typedef_field_map: HashMap<(TypeId, InternedString), u32>,

    /// Mapping from field SymbolId to PropertyAccessInfo (for properties with custom getters/setters)
    /// This allows us to route property access through the appropriate getter/setter methods
    property_access_map: HashMap<SymbolId, crate::tast::PropertyAccessInfo>,

    /// Mapping from class TypeId to constructor IrFunctionId
    /// This allows new expressions to find the constructor by class type
    constructor_map: HashMap<TypeId, IrFunctionId>,

    /// Mapping from qualified class name to constructor IrFunctionId
    /// This is a fallback when TypeIds don't match (e.g., across separately compiled files)
    constructor_name_map: HashMap<String, IrFunctionId>,

    /// Reference to HIR type declarations for inheritance lookup
    /// Needed to access parent class fields during field inheritance
    current_hir_types: &'a indexmap::IndexMap<TypeId, HirTypeDecl>,

    /// Standard library runtime function mapping
    stdlib_mapping: StdlibMapping,

    /// Symbol table for resolving symbols
    symbol_table: &'a SymbolTable,

    /// Track when we're inside a lambda and what its environment layout is
    current_env_layout: Option<EnvironmentLayout>,

    /// Current class type for instance methods (used to resolve implicit field accesses)
    current_this_type: Option<TypeId>,

    /// Mapping from variable SymbolIds to their monomorphized stdlib class names
    /// Used to track Vec<Int> -> VecI32, Vec<Float> -> VecF64, etc.
    /// This is needed because extern generic classes don't have proper TypeIds in the type table
    monomorphized_var_types: HashMap<SymbolId, String>,

    /// Enums that need RTTI registration
    /// Maps enum SymbolId -> (runtime_type_id, enum_name, variant_names)
    enums_for_registration: HashMap<SymbolId, (u32, String, Vec<String>)>,

    /// Counter for generating unique wrapper function names
    next_wrapper_id: u32,
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
    /// Maps symbol IDs to their exit block phi registers
    /// When breaking, we need to add incoming edges to these phi nodes
    exit_phi_nodes: HashMap<SymbolId, IrId>,
}

#[derive(Debug)]
pub struct LoweringError {
    pub message: String,
    pub location: SourceLocation,
}

/// Context for lambda function generation (Two-Pass Architecture)
struct LambdaContext {
    /// The function ID of the lambda
    func_id: IrFunctionId,
    /// Entry block of the lambda
    entry_block: IrBlockId,
    /// Offset for parameter registers (0 if no env, 1 if has env)
    param_offset: u32,
    /// Environment layout (if captures exist)
    env_layout: Option<EnvironmentLayout>,
}

/// Saved state for restoring after lambda generation
struct SavedLoweringState {
    current_function: Option<IrFunctionId>,
    current_block: Option<IrBlockId>,
    symbol_map: HashMap<SymbolId, IrId>,
    current_env_layout: Option<EnvironmentLayout>,
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
            external_function_map: HashMap::new(),
            external_function_name_map: HashMap::new(),
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
            typedef_field_map: HashMap::new(),
            property_access_map: HashMap::new(),
            constructor_map: HashMap::new(),
            constructor_name_map: HashMap::new(),
            current_hir_types: hir_types,
            stdlib_mapping: StdlibMapping::new(),
            symbol_table,
            current_env_layout: None,
            current_this_type: None,
            monomorphized_var_types: HashMap::new(),
            enums_for_registration: HashMap::new(),
            next_wrapper_id: 0,
        }
    }

    /// Look up a function ID by symbol, checking both local and external function maps
    fn get_function_id(&self, symbol: &SymbolId) -> Option<IrFunctionId> {
        self.function_map.get(symbol)
            .copied()
            .or_else(|| self.external_function_map.get(symbol).copied())
    }

    /// Register a constructor by qualified name for cross-file resolution
    /// This is critical when the TypeId differs between files (e.g., loading StringIteratorUnicode
    /// as a dependency gives it a different TypeId than when StringTools.hx references it)
    fn register_constructor_by_name(&mut self, class_symbol: SymbolId, func_id: IrFunctionId) {
        if let Some(sym_info) = self.symbol_table.get_symbol(class_symbol) {
            if let Some(qual_name) = sym_info.qualified_name.and_then(|q| self.string_interner.get(q)) {
                self.constructor_name_map.insert(qual_name.to_string(), func_id);
            } else if let Some(name) = self.string_interner.get(sym_info.name) {
                // Fallback to simple name if no qualified name
                self.constructor_name_map.insert(name.to_string(), func_id);
            }
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
                        if let Some(HirAttributeArg::Literal(HirLiteral::String(hint))) =
                            attr.args.first()
                        {
                            match hint.to_string().as_str() {
                                "straight_line_code" => {
                                    self.ssa_hints.straight_line_functions.insert(*symbol_id);
                                }
                                "complex_control_flow" => {
                                    self.ssa_hints
                                        .complex_control_flow_functions
                                        .insert(*symbol_id);
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
                    // eprintln!(
                    //     "DEBUG Pass1a: Registering methods for class {:?}",
                    //     self.string_interner.get(class.name).unwrap_or("<unknown>")
                    // );
                    for method in &class.methods {
                        // eprintln!(
                        //     "DEBUG Pass1a:   - method {:?} (symbol={:?})",
                        //     self.string_interner
                        //         .get(method.function.name)
                        //         .unwrap_or("<unknown>"),
                        //     method.function.symbol_id
                        // );
                        let this_type = if !method.is_static {
                            Some(*type_id)
                        } else {
                            None
                        };
                        // Pass class type params for generic class methods
                        self.register_function_signature_with_class_type_params(
                            method.function.symbol_id,
                            &method.function,
                            this_type,
                            &class.type_params,
                        );
                    }

                    // Register constructor signature with class type params
                    if let Some(constructor) = &class.constructor {
                        self.register_constructor_signature_with_class_type_params(
                            class.symbol_id, constructor, *type_id, &class.type_params);
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
            // eprintln!("DEBUG Pass2a: Processing type - TypeId={:?}, name={:?}", type_id, name_str);
            match type_decl {
                HirTypeDecl::Class(class) => {
                    // Get qualified class name for runtime mapping checks
                    // Use qualified_name with underscores (e.g., "rayzor_Bytes") for precise matching
                    // This prevents "Bytes" from matching both rayzor.Bytes and haxe.io.Bytes
                    let qualified_class_name = self.symbol_table
                        .get_symbol(class.symbol_id)
                        .and_then(|sym| sym.qualified_name)
                        .and_then(|qn| self.string_interner.get(qn))
                        .map(|qn| qn.replace(".", "_"));

                    // Fallback to simple class name if no qualified name
                    let class_name = self.string_interner.get(class.name);

                    // Lower each method body
                    for method in &class.methods {
                        // SPECIAL CASE: Skip lowering method if this is an extern class
                        // with a runtime mapping for this method. For extern classes like FileSystem,
                        // methods are handled by the runtime mapping system, not by MIR stubs.
                        let should_skip_method = if method.function.body.is_none() {
                            // Method has no body - check if it has a runtime mapping
                            // First try qualified name (e.g., "rayzor_Bytes"), then fall back to simple name
                            let has_mapping = if let Some(ref qn) = qualified_class_name {
                                if let Some(method_name) = self.string_interner.get(method.function.name) {
                                    self.stdlib_mapping.has_mapping(qn, method_name, method.is_static)
                                } else {
                                    false
                                }
                            } else if let Some(class_name_str) = class_name {
                                if let Some(method_name) = self.string_interner.get(method.function.name) {
                                    self.stdlib_mapping.has_mapping(class_name_str, method_name, method.is_static)
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            // Also skip if this is a generic stdlib class (like Vec<T>)
                            // that has monomorphized variants with MIR wrappers
                            let is_generic_stdlib = class_name
                                .map(|n| self.stdlib_mapping.is_generic_stdlib_class(n))
                                .unwrap_or(false);

                            has_mapping || is_generic_stdlib
                        } else {
                            false
                        };

                        if should_skip_method {
                            continue;
                        }

                        if method.is_static {
                            self.lower_function_body(
                                method.function.symbol_id,
                                &method.function,
                                None,
                            );
                        } else {
                            self.lower_function_body(
                                method.function.symbol_id,
                                &method.function,
                                Some(*type_id),
                            );
                        }
                    }

                    // Lower constructor body
                    if let Some(constructor) = &class.constructor {
                        // SPECIAL CASE: Skip lowering constructor if this is an extern class
                        // with a runtime mapping for "new". For extern classes like Channel, Thread, etc.,
                        // the "new" constructor is handled by the runtime mapping system, not by
                        // generating a MIR constructor function.
                        // Use qualified class name first (e.g., "rayzor_Bytes"), then fall back to simple name
                        let should_skip_constructor = if let Some(ref qn) = qualified_class_name {
                            // Check if this class has a "new" constructor in the runtime mapping
                            if let Some(class_name_static) = self.stdlib_mapping.get_class_static_str(qn) {
                                let method_sig = crate::stdlib::runtime_mapping::MethodSignature {
                                    class: class_name_static,
                                    method: "new",
                                    is_static: true,
                                    is_constructor: true,
                                    param_count: 0,
                                };
                                let has_runtime_constructor = self.stdlib_mapping.get(&method_sig).is_some();
                                if has_runtime_constructor {
                                    eprintln!("DEBUG: Skipping constructor lowering for extern class '{}' - using runtime mapping", qn);
                                }
                                has_runtime_constructor
                            } else {
                                false
                            }
                        } else if let Some(class_name_str) = class_name {
                            // Fallback to simple class name
                            if let Some(class_name_static) = self.stdlib_mapping.get_class_static_str(class_name_str) {
                                let method_sig = crate::stdlib::runtime_mapping::MethodSignature {
                                    class: class_name_static,
                                    method: "new",
                                    is_static: true,
                                    is_constructor: true,
                                    param_count: 0,
                                };
                                let has_runtime_constructor = self.stdlib_mapping.get(&method_sig).is_some();
                                if has_runtime_constructor {
                                    eprintln!("DEBUG: Skipping constructor lowering for extern class '{}' - using runtime mapping", class_name_str);
                                }
                                has_runtime_constructor
                            } else {
                                // Also skip if this is a generic stdlib class (like Vec<T>)
                                self.stdlib_mapping.is_generic_stdlib_class(class_name_str)
                            }
                        } else {
                            false
                        };

                        if !should_skip_constructor {
                            // eprintln!("DEBUG: Lowering constructor for class {:?}", class.name);
                            self.lower_constructor_body(
                                class.symbol_id,
                                constructor,
                                *type_id,
                                class.extends,
                            );
                        }
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
            // eprintln!(
            //     "  ℹ️  Returning MIR module with {} functions, {} extern_functions",
            //     self.builder.module.functions.len(),
            //     self.builder.module.extern_functions.len()
            // );

            Ok(std::mem::replace(
                &mut self.builder.module,
                IrModule::new(String::new(), String::new()),
            ))
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    /// Register a function signature without lowering the body (Pass 1)
    /// This creates the function stub and adds it to function_map
    fn register_function_signature(
        &mut self,
        symbol_id: SymbolId,
        hir_func: &HirFunction,
        this_type: Option<TypeId>,
    ) {
        let mut signature = self.build_function_signature(hir_func);

        // For instance methods, add implicit 'this' parameter
        // 'this' is always a pointer to the class instance, regardless of generic parameters
        if let Some(type_id) = this_type {
            let this_type = match self.convert_type(type_id) {
                IrType::Ptr(_) => IrType::Ptr(Box::new(IrType::Void)),
                // If convert_type failed to resolve (e.g., generic class without instantiation),
                // default to pointer since 'this' is always a pointer to instance
                _ => IrType::Ptr(Box::new(IrType::Void)),
            };
            signature.parameters.insert(
                0,
                IrParameter {
                    name: "this".to_string(),
                    ty: this_type,
                    reg: IrId::new(0), // Will be properly assigned when body is lowered
                    by_ref: false,
                },
            );
        }

        let func_name = self
            .string_interner
            .get(hir_func.name)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("func_{}", symbol_id.as_raw()));

        let func_id = self.builder.start_function(symbol_id, func_name, signature);
        self.function_map.insert(symbol_id, func_id);
        self.builder.finish_function(); // Close to allow next function to start
    }

    /// Register a function signature with class type parameters (for generic class methods)
    /// This version includes the class's type parameters in the function signature
    fn register_function_signature_with_class_type_params(
        &mut self,
        symbol_id: SymbolId,
        hir_func: &HirFunction,
        this_type: Option<TypeId>,
        class_type_params: &[HirTypeParam],
    ) {
        let mut signature = self.build_function_signature_with_class_type_params(hir_func, class_type_params);

        // For instance methods, add implicit 'this' parameter
        if let Some(type_id) = this_type {
            let this_type = match self.convert_type(type_id) {
                IrType::Ptr(_) => IrType::Ptr(Box::new(IrType::Void)),
                _ => IrType::Ptr(Box::new(IrType::Void)),
            };
            signature.parameters.insert(
                0,
                IrParameter {
                    name: "this".to_string(),
                    ty: this_type,
                    reg: IrId::new(0),
                    by_ref: false,
                },
            );
        }

        let func_name = self
            .string_interner
            .get(hir_func.name)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("func_{}", symbol_id.as_raw()));

        let func_id = self.builder.start_function(symbol_id, func_name, signature);
        self.function_map.insert(symbol_id, func_id);
        self.builder.finish_function();
    }

    /// Register constructor signature with class type params (for generic classes)
    fn register_constructor_signature_with_class_type_params(
        &mut self,
        class_symbol: SymbolId,
        constructor: &HirConstructor,
        type_id: TypeId,
        class_type_params: &[HirTypeParam],
    ) {
        let this_type = match self.convert_type(type_id) {
            IrType::Ptr(_) => IrType::Ptr(Box::new(IrType::Void)),
            _ => IrType::Ptr(Box::new(IrType::Void)),
        };
        let mut sig_builder = FunctionSignatureBuilder::new()
            .param("this".to_string(), this_type);

        // Add class type parameters
        for type_param in class_type_params {
            let param_name = self.string_interner.get(type_param.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("T{}", type_param.name.as_raw()));
            sig_builder = sig_builder.type_param(param_name);
        }

        // Add constructor parameters
        for param in &constructor.params {
            let param_name = self
                .string_interner
                .get(param.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("param_{}", param.symbol_id.as_raw()));
            sig_builder = sig_builder.param(param_name, self.convert_type(param.ty));
        }

        let mut signature = sig_builder.returns(IrType::Void).build();

        // Assign register IDs
        for (i, param) in signature.parameters.iter_mut().enumerate() {
            param.reg = IrId::new(i as u32);
        }

        let func_id = self.builder.start_function(class_symbol, "new".to_string(), signature);
        self.function_map.insert(class_symbol, func_id);
        self.constructor_map.insert(type_id, func_id);
        self.register_constructor_by_name(class_symbol, func_id);

        let fallback_type_id = TypeId::from_raw(class_symbol.as_raw());
        if fallback_type_id != type_id {
            self.constructor_map.insert(fallback_type_id, func_id);
        }

        self.builder.finish_function();
    }

    /// Register constructor signature (Pass 1)
    fn register_constructor_signature(
        &mut self,
        class_symbol: SymbolId,
        constructor: &HirConstructor,
        type_id: TypeId,
    ) {
        // Constructor signature: takes implicit 'this' parameter + constructor params, returns void
        // 'this' is always a pointer to the class instance, regardless of generic parameters
        let this_type = match self.convert_type(type_id) {
            IrType::Ptr(_) => IrType::Ptr(Box::new(IrType::Void)),
            // If convert_type failed to resolve (e.g., generic class without instantiation),
            // default to pointer since 'this' is always a pointer to instance
            _ => IrType::Ptr(Box::new(IrType::Void)),
        };
        let mut sig_builder = FunctionSignatureBuilder::new()
            .param("this".to_string(), this_type);

        // Add constructor parameters
        for param in &constructor.params {
            let param_name = self
                .string_interner
                .get(param.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("param_{}", param.symbol_id.as_raw()));
            sig_builder = sig_builder.param(param_name, self.convert_type(param.ty));
        }

        let mut signature = sig_builder.returns(IrType::Void).build();

        // Assign register IDs to parameters
        for (i, param) in signature.parameters.iter_mut().enumerate() {
            param.reg = IrId::new(i as u32);
        }

        let func_id = self
            .builder
            .start_function(class_symbol, "new".to_string(), signature);
        self.function_map.insert(class_symbol, func_id);
        self.constructor_map.insert(type_id, func_id);
        self.register_constructor_by_name(class_symbol, func_id);

        // Also register with TypeId derived from class SymbolId as a fallback
        let fallback_type_id = TypeId::from_raw(class_symbol.as_raw());
        if fallback_type_id != type_id {
            self.constructor_map.insert(fallback_type_id, func_id);
        }

        self.builder.finish_function(); // Close the stub
    }

    /// Lower a function body after signature is registered (Pass 2)
    /// Reuses the existing function created in Pass 1
    fn lower_function_body(
        &mut self,
        symbol_id: SymbolId,
        hir_func: &HirFunction,
        this_type: Option<TypeId>,
    ) {
        // The function already exists from Pass 1, we just need to fill in the body
        let func_id = self
            .function_map
            .get(&symbol_id)
            .copied()
            .expect("Function should have been registered in Pass 1");

        // Re-open the function for body lowering
        let func = self
            .builder
            .module
            .functions
            .get(&func_id)
            .expect("Function should exist")
            .clone();

        self.builder.current_function = Some(func_id);
        self.builder.current_block = Some(func.entry_block());

        // Set current_this_type for implicit field access resolution
        self.current_this_type = this_type;

        // Map 'this' parameter for instance methods
        if this_type.is_some() {
            // 'this' is parameter 0
            if let Some(this_param) = func.signature.parameters.get(0) {
                // Map 'this' to a special symbol ID (SymbolId(0))
                // This is what HirExprKind::This looks up
                self.symbol_map
                    .insert(SymbolId::from_raw(0), this_param.reg);
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
        let func_name = self.string_interner.get(hir_func.name).unwrap_or("?").to_string();
        if let Some(body) = &hir_func.body {
            eprintln!("DEBUG [lower_function_body]: {} has body with {} statements, expr: {}",
                func_name, body.statements.len(), body.expr.is_some());
            for (i, stmt) in body.statements.iter().enumerate() {
                eprintln!("DEBUG [lower_function_body]: {} - stmt[{}] = {:?}", func_name, i, std::mem::discriminant(stmt));
            }
            self.lower_block(body);
            self.ensure_terminator();
        } else {
            eprintln!("DEBUG [lower_function_body]: {} has NO body", func_name);
            // Functions without body (extern, abstract) still need a terminator
            self.ensure_terminator();
        }

        // Finish
        // eprintln!("DEBUG ===== FINISHING FUNCTION =====");
        // if let Some(func) = self.builder.current_function() {
        //     eprintln!(
        //         "DEBUG   Function '{}' entry block terminator: {:?}",
        //         func.name,
        //         func.cfg
        //             .get_block(func.entry_block())
        //             .map(|b| &b.terminator)
        //     );
        // }
        self.builder.finish_function();
        // eprintln!("DEBUG   Function finished, context cleared");

        self.symbol_map.clear();
        self.block_map.clear();
        self.current_this_type = None;
    }

    /// Lower constructor body (Pass 2)
    fn lower_constructor_body(
        &mut self,
        class_symbol: SymbolId,
        constructor: &HirConstructor,
        type_id: TypeId,
        parent_type: Option<TypeId>,
    ) {
        let func_id = self
            .constructor_map
            .get(&type_id)
            .copied()
            .expect("Constructor should have been registered in Pass 1");

        let func = self
            .builder
            .module
            .functions
            .get(&func_id)
            .expect("Constructor function should exist")
            .clone();

        self.builder.current_function = Some(func_id);
        self.builder.current_block = Some(func.entry_block());

        // 'this' is parameter 0
        let this_reg = IrId::new(0);

        // Map 'this' to symbol map for field access
        self.symbol_map.insert(SymbolId::from_raw(0), this_reg);

        // Map constructor parameters to registers
        for (i, param) in constructor.params.iter().enumerate() {
            let reg = IrId::new((i + 1) as u32); // +1 because 'this' is parameter 0
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
                    let mut arg_regs = vec![this_reg]; // 'this' is first argument
                    for arg in &super_call.args {
                        if let Some(arg_reg) = self.lower_expression(arg) {
                            arg_regs.push(arg_reg);
                        }
                    }

                    // eprintln!("DEBUG: Calling parent constructor with {} args", arg_regs.len());
                    // Call parent constructor (returns void)
                    self.builder.build_call_direct(
                        parent_ctor_id,
                        arg_regs,
                        crate::ir::IrType::Void,
                    );
                } else {
                    self.add_error(
                        &format!(
                            "Parent constructor not found for TypeId {:?}",
                            parent_type_id
                        ),
                        crate::tast::SourceLocation::unknown(),
                    );
                }
            } else {
                // eprintln!("DEBUG: super() call but no parent class - this is an error in HIR");
            }
        }

        // Lower constructor body statements
        for stmt in &constructor.body.statements {
            self.lower_statement(stmt);
        }

        // Constructor implicitly returns void
        self.builder.build_return(None);

        // eprintln!("DEBUG ===== FINISHING FUNCTION =====");
        // if let Some(func) = self.builder.current_function() {
        //     eprintln!(
        //         "DEBUG   Function '{}' entry block terminator: {:?}",
        //         func.name,
        //         func.cfg
        //             .get_block(func.entry_block())
        //             .map(|b| &b.terminator)
        //     );
        // }
        self.builder.finish_function();
        // eprintln!("DEBUG   Function finished, context cleared");

        self.symbol_map.clear();
        self.block_map.clear();
    }

    /// Lower a HIR function to MIR (Legacy - combines Pass 1 and Pass 2)
    fn lower_function(&mut self, symbol_id: SymbolId, hir_func: &HirFunction) {
        let body_stmt_count = hir_func
            .body
            .as_ref()
            .map(|b| b.statements.len())
            .unwrap_or(0);
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
        let func_name = self
            .string_interner
            .get(hir_func.name)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("func_{}", symbol_id.as_raw()));
        // eprintln!("DEBUG ===== STARTING FUNCTION: {} (symbol {:?}) =====", func_name, symbol_id);
        let func_id = self.builder.start_function(symbol_id, func_name, signature);
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

        if self
            .ssa_hints
            .complex_control_flow_functions
            .contains(&symbol_id)
        {
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
                func.qualified_name = self
                    .string_interner
                    .get(qualified_name)
                    .map(|s| s.to_string());
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
                        func_mut.locals.insert(
                            param_reg,
                            super::IrLocal {
                                name: param.name.to_string(),
                                ty: param_type,
                                mutable: false, // Parameters are immutable by default
                                source_location: src_loc,
                                allocation: super::AllocationHint::Register,
                            },
                        );
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

        // eprintln!("DEBUG ===== FINISHING FUNCTION =====");
        // // Before finishing, dump the terminator for this function
        // if let Some(func) = self.builder.current_function() {
        //     eprintln!(
        //         "DEBUG   Function '{}' entry block terminator: {:?}",
        //         func.name,
        //         func.cfg
        //             .get_block(func.entry_block())
        //             .map(|b| &b.terminator)
        //     );
        // }
        self.builder.finish_function();
        // eprintln!("DEBUG   Function finished, context cleared");

        // Clear per-function state
        self.symbol_map.clear();
        self.block_map.clear();
    }

    /// Lower an instance method (non-static class method) to MIR
    /// Instance methods receive an implicit 'this' parameter as their first argument
    fn lower_instance_method(
        &mut self,
        symbol_id: SymbolId,
        hir_func: &HirFunction,
        class_type_id: TypeId,
    ) {
        // Build MIR function signature with implicit 'this' parameter
        let signature = self.build_instance_method_signature(hir_func, class_type_id);

        // Start building the function
        let func_name = self
            .string_interner
            .get(hir_func.name)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("func_{}", symbol_id.as_raw()));
        // eprintln!(
        //     "DEBUG ===== STARTING FUNCTION: {} (symbol {:?}) =====",
        //     func_name, symbol_id
        // );
        let func_id = self.builder.start_function(symbol_id, func_name, signature);
        // eprintln!("DEBUG   Function ID: {:?}, Entry block created", func_id);

        // Store function mapping for call resolution
        self.function_map.insert(symbol_id, func_id);

        // Set qualified name for debugging and profiling
        if let Some(qualified_name) = hir_func.qualified_name {
            if let Some(func) = self.builder.module.functions.get_mut(&func_id) {
                func.qualified_name = self
                    .string_interner
                    .get(qualified_name)
                    .map(|s| s.to_string());
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
            eprintln!("DEBUG lower_instance_method: {} has body with {} statements",
                     self.string_interner.get(hir_func.name).unwrap_or("?"),
                     body.statements.len());
            self.lower_block(body);
            eprintln!("DEBUG lower_instance_method: {} - after lower_block",
                     self.string_interner.get(hir_func.name).unwrap_or("?"));

            // Add implicit return if needed
            self.ensure_terminator();
            eprintln!("DEBUG lower_instance_method: {} - after ensure_terminator",
                     self.string_interner.get(hir_func.name).unwrap_or("?"));
        } else {
            eprintln!("DEBUG lower_instance_method: {} has NO body!",
                     self.string_interner.get(hir_func.name).unwrap_or("?"));
        }

        // Debug: check the function CFG
        if let Some(func) = self.builder.current_function() {
            eprintln!("DEBUG lower_instance_method: {} - CFG has {} blocks",
                     func.name, func.cfg.blocks.len());
            for (block_id, block) in &func.cfg.blocks {
                eprintln!("  Block {:?}: {} instructions, terminator: {:?}",
                         block_id, block.instructions.len(), block.terminator);
            }
        }

        // eprintln!("DEBUG ===== FINISHING FUNCTION =====");
        // Before finishing, dump the terminator for this function
        // if let Some(func) = self.builder.current_function() {
        //     eprintln!(
        //         "DEBUG   Function '{}' entry block terminator: {:?}",
        //         func.name,
        //         func.cfg
        //             .get_block(func.entry_block())
        //             .map(|b| &b.terminator)
        //     );
        // }
        self.builder.finish_function();
        // eprintln!("DEBUG   Function finished, context cleared");

        // Clear per-function state
        self.symbol_map.clear();
        self.block_map.clear();
    }

    /// Lower a constructor to MIR
    /// Constructors are similar to instance methods but handle field initialization
    fn lower_constructor(
        &mut self,
        class_symbol: SymbolId,
        constructor: &HirConstructor,
        class_type_id: TypeId,
    ) {
        // eprintln!("DEBUG: lower_constructor - class_symbol={:?}", class_symbol);

        // Build signature using the builder
        // 'this' is always a pointer to the class instance, regardless of generic parameters
        let this_type = match self.convert_type(class_type_id) {
            IrType::Ptr(_) => IrType::Ptr(Box::new(IrType::Void)),
            // If convert_type failed to resolve (e.g., generic class without instantiation),
            // default to pointer since 'this' is always a pointer to instance
            _ => IrType::Ptr(Box::new(IrType::Void)),
        };
        let mut sig_builder = FunctionSignatureBuilder::new()
            .param("this".to_string(), this_type);

        // Add constructor parameters
        for param in &constructor.params {
            let param_name = self
                .string_interner
                .get(param.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("param_{}", param.symbol_id.as_raw()));
            sig_builder = sig_builder.param(param_name, self.convert_type(param.ty));
        }

        // Constructor returns void
        let signature = sig_builder.returns(IrType::Void).build();

        // Start building the constructor function
        let func_name = "new".to_string();
        let func_id = self
            .builder
            .start_function(class_symbol, func_name, signature);

        // Register this function in the function map
        self.function_map.insert(class_symbol, func_id);

        // Register in constructor_map by TypeId for new expressions
        self.constructor_map.insert(class_type_id, func_id);
        self.register_constructor_by_name(class_symbol, func_id);

        // Also register with TypeId derived from class SymbolId as a fallback
        // This handles cases where expression TypeIds differ from types map TypeIds
        let fallback_type_id = TypeId::from_raw(class_symbol.as_raw());
        if fallback_type_id != class_type_id {
            self.constructor_map.insert(fallback_type_id, func_id);
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
                if let Some(&(_class_type, field_index)) =
                    self.field_index_map.get(&field_init.field)
                {
                    if let Some(index_const) =
                        self.builder.build_const(IrValue::I32(field_index as i32))
                    {
                        // Get the actual field type from the symbol table
                        let field_ty = self.symbol_table.get_symbol(field_init.field)
                            .map(|s| self.convert_type(s.type_id))
                            .unwrap_or(IrType::I32);
                        if let Some(field_ptr) =
                            self.builder
                                .build_gep(this_reg, vec![index_const], field_ty)
                        {
                            self.builder.build_store(field_ptr, value_reg);
                        }
                    }
                }
            }
        }

        // Lower constructor body
        // eprintln!("DEBUG: Constructor body has {} statements", constructor.body.statements.len());
        // for (i, stmt) in constructor.body.statements.iter().enumerate() {
        //     // eprintln!("DEBUG: Constructor stmt {}: {:?}", i, std::mem::discriminant(stmt));
        // }
        self.lower_block(&constructor.body);

        // Ensure void return
        if !self.is_terminated() {
            self.builder.build_return(None);
        }

        // eprintln!("DEBUG ===== FINISHING FUNCTION =====");
        // Before finishing, dump the terminator for this function
        // if let Some(func) = self.builder.current_function() {
        //     eprintln!(
        //         "DEBUG   Function '{}' entry block terminator: {:?}",
        //         func.name,
        //         func.cfg
        //             .get_block(func.entry_block())
        //             .map(|b| &b.terminator)
        //     );
        // }
        self.builder.finish_function();
        // eprintln!("DEBUG   Function finished, context cleared");

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
            HirStatement::Let {
                pattern,
                type_hint,
                init,
                is_mutable,
            } => {
                // Lower initialization expression if present
                if let Some(init_expr) = init {

                    // Check if this is a New expression for a generic stdlib class (Vec<T>)
                    // We need to track the monomorphized class name for later method calls
                    let monomorphized_class = if let HirExprKind::New { class_name, type_args, .. } = &init_expr.kind {
                        let class_name_str = class_name.and_then(|interned| self.string_interner.get(interned));
                        if class_name_str == Some("Vec") && !type_args.is_empty() {
                            // Determine the monomorphized Vec variant from type args
                            let first_arg = type_args[0];
                            let type_table = self.type_table.borrow();
                            let suffix = if let Some(arg_type) = type_table.get(first_arg) {
                                match &arg_type.kind {
                                    TypeKind::Int => Some("I32"),
                                    TypeKind::Float => Some("F64"),
                                    TypeKind::Bool => Some("Bool"),
                                    TypeKind::String => Some("Ptr"),
                                    TypeKind::Class { symbol_id, .. } => {
                                        if let Some(class_info) = self.symbol_table.get_symbol(*symbol_id) {
                                            if let Some(name) = self.string_interner.get(class_info.name) {
                                                if name == "Int64" { Some("I64") } else { Some("Ptr") }
                                            } else { Some("Ptr") }
                                        } else { Some("Ptr") }
                                    }
                                    _ => Some("Ptr"),
                                }
                            } else {
                                Some("Ptr")
                            };
                            drop(type_table);
                            suffix.map(|s| format!("Vec{}", s))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let value = self.lower_expression(init_expr);

                    // Bind to pattern and register as local
                    if let Some(value_reg) = value {
                        // Track monomorphized class name for this variable (by SymbolId)
                        if let Some(mono_class) = monomorphized_class {
                            if let HirPattern::Variable { symbol, .. } = pattern {
                                self.monomorphized_var_types.insert(*symbol, mono_class);
                            }
                        }

                        // Determine the type for the binding
                        let var_type = type_hint.or(Some(init_expr.ty));

                        // Auto-box if assigning concrete value to Dynamic variable
                        // Auto-unbox if assigning Dynamic value to concrete variable
                        let final_value = if let Some(target_ty) = var_type {
                            // Try boxing first (concrete → Dynamic)
                            let after_box = self.maybe_box_value(value_reg, init_expr.ty, target_ty)
                                .unwrap_or(value_reg);
                            // Then try unboxing (Dynamic → concrete)
                            self.maybe_unbox_value(after_box, init_expr.ty, target_ty)
                                .unwrap_or(after_box)
                        } else {
                            value_reg
                        };

                        self.bind_pattern_with_type(pattern, final_value, var_type, *is_mutable);
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
                            let result_reg = self.builder.build_binop(
                                self.convert_binary_op(*bin_op),
                                lhs_reg,
                                rhs_reg,
                            )?;

                            // Register the result type for Cranelift
                            let result_type = self.convert_type(rhs.ty);
                            if let Some(func) = self.builder.current_function_mut() {
                                func.locals.insert(
                                    result_reg,
                                    super::IrLocal {
                                        name: format!("_binop{}", result_reg.0),
                                        ty: result_type,
                                        mutable: false,
                                        source_location: super::IrSourceLocation {
                                            file_id: 0,
                                            line: 0,
                                            column: 0,
                                        },
                                        allocation: super::AllocationHint::Register,
                                    },
                                );
                            }

                            Some(result_reg)
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
                eprintln!("DEBUG [Return]: has_value: {}", value.is_some());
                let ret_value = value.as_ref().and_then(|e| {
                    eprintln!("DEBUG [Return]: Lowering return expression, expr kind: {:?}", std::mem::discriminant(&e.kind));
                    let result = self.lower_expression(e);
                    eprintln!("DEBUG [Return]: Return expression lowered to: {:?}", result);
                    if result.is_none() {
                        eprintln!("ERROR [Return]: Failed to lower return expression!");
                        eprintln!("ERROR [Return]: Expression was: {:#?}", e);
                    }
                    result
                });
                eprintln!("DEBUG [Return]: Building return instruction with value: {:?}", ret_value);
                self.builder.build_return(ret_value);
                // eprintln!("DEBUG: Return instruction built");
            }

            HirStatement::Break(label) => {
                if let Some(loop_ctx) = self.find_loop_context(label.as_ref()) {
                    let break_block = loop_ctx.break_block;
                    let exit_phi_nodes = loop_ctx.exit_phi_nodes.clone();

                    // Get the current block before branching
                    let current_block = self.builder.current_block().unwrap();

                    // Add incoming edges to exit block phi nodes with current symbol values
                    for (symbol_id, exit_phi_reg) in &exit_phi_nodes {
                        // Get the current value of this symbol
                        let current_value = if let Some(&reg) = self.symbol_map.get(symbol_id) {
                            reg
                        } else {
                            // If symbol not in map, use the phi register itself (shouldn't happen)
                            *exit_phi_reg
                        };

                        // Add incoming edge from current block to exit phi
                        self.builder.add_phi_incoming(
                            break_block,
                            *exit_phi_reg,
                            current_block,
                            current_value,
                        );
                    }

                    self.builder.build_branch(break_block);
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

            HirStatement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                // eprintln!("DEBUG: About to call lower_if_statement, has_else={}", else_branch.is_some());
                self.lower_if_statement(condition, then_branch, else_branch.as_ref());
                // eprintln!("DEBUG: Returned from lower_if_statement");
            }

            HirStatement::Switch { scrutinee, cases } => {
                self.lower_switch_statement(scrutinee, cases);
            }

            HirStatement::While {
                condition,
                body,
                label,
            } => {
                self.lower_while_loop(condition, body, label.as_ref());
            }

            HirStatement::DoWhile {
                body,
                condition,
                label,
            } => {
                self.lower_do_while_loop(body, condition, label.as_ref());
            }

            HirStatement::ForIn {
                pattern,
                iterator,
                body,
                label,
            } => {
                self.lower_for_in_loop(pattern, iterator, body, label.as_ref());
            }

            HirStatement::TryCatch {
                try_block,
                catches,
                finally_block,
            } => {
                self.lower_try_catch(try_block, catches, finally_block.as_ref());
            }

            HirStatement::Label { symbol, block } => {
                // Labels in MIR become block labels
                let label_block = self
                    .builder
                    .create_block_with_label(format!("label_{}", symbol.as_raw()));
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
    ) -> Option<(&'static str, &'static str, &crate::stdlib::RuntimeFunctionCall)> {
        // Get the method name and optional qualified name from the symbol table
        let (method_name, qualified_name) = if let Some(symbol) = self.symbol_table.get_symbol(method_symbol) {
            let name = self.string_interner.get(symbol.name)?;
            let qname = symbol.qualified_name.and_then(|qn| self.string_interner.get(qn));
            (name, qname)
        } else {
            return None;
        };

        // Get the class name and type args from the receiver type
        let type_table = self.type_table.borrow();
        let type_info = type_table.get(receiver_type);

        // FALLBACK: If receiver_type is invalid (extern classes like Vec), try to detect class from qualified name
        if type_info.is_none() {
            drop(type_table);
            eprintln!("DEBUG: [get_stdlib_runtime_info] receiver_type {:?} not in type_table, qualified_name={:?}", receiver_type, qualified_name);
            if let Some(qname) = qualified_name {
                // Pattern: "rayzor.Vec.get" or "test.Main.method"
                let parts: Vec<&str> = qname.split('.').collect();
                if parts.len() >= 2 {
                    // Check if second-to-last part is "Vec"
                    if let Some(&class_name) = parts.iter().rev().nth(1) {
                        if class_name == "Vec" {
                            // For Vec without type info, we can't monomorphize here
                            // Try Vec variants in the mapping
                            for variant in &["VecI32", "VecI64", "VecF64", "VecPtr", "VecBool"] {
                                if let Some((sig, mapping)) = self.stdlib_mapping.find_by_name(variant, method_name) {
                                    eprintln!("DEBUG: [FALLBACK] Found Vec method {} in {} mapping", method_name, variant);
                                    // Return the first match - caller will need to handle type selection
                                    return Some((sig.class, sig.method, mapping));
                                }
                            }
                        }
                    }
                }
            }
            return None;
        }

        let type_info = type_info.unwrap();

        // Extract class info from the type, following TypeAlias if needed
        let (base_class_name, qualified_class_name, type_args) = match &type_info.kind {
            TypeKind::String => (Some("String"), None, Vec::new()),
            TypeKind::Array { .. } => (Some("Array"), None, Vec::new()),
            TypeKind::Class { symbol_id, type_args, .. } => {
                // Get class name and qualified name from symbol
                let (name, qname) = if let Some(class_info) = self.symbol_table.get_symbol(*symbol_id) {
                    let n = self.string_interner.get(class_info.name);
                    // Get qualified name with underscore format (e.g., "rayzor_Bytes")
                    let qn = class_info.qualified_name
                        .and_then(|qn| self.string_interner.get(qn))
                        .map(|qn| qn.replace(".", "_"));
                    (n, qn)
                } else {
                    (None, None)
                };
                (name, qname, type_args.clone())
            }
            TypeKind::TypeAlias { target_type, .. } => {
                // For type aliases like `typedef Bytes = rayzor.Bytes`, follow the target type
                if let Some(target_info) = type_table.get(*target_type) {
                    match &target_info.kind {
                        TypeKind::Class { symbol_id, type_args, .. } => {
                            let (name, qname) = if let Some(class_info) = self.symbol_table.get_symbol(*symbol_id) {
                                let n = self.string_interner.get(class_info.name);
                                let qn = class_info.qualified_name
                                    .and_then(|qn| self.string_interner.get(qn))
                                    .map(|qn| qn.replace(".", "_"));
                                (n, qn)
                            } else {
                                (None, None)
                            };
                            (name, qname, type_args.clone())
                        }
                        TypeKind::Placeholder { name: placeholder_name } => {
                            // The typedef target wasn't resolved at compile time - try to look it up by name
                            // This handles cases like `typedef Bytes = rayzor.Bytes` where the target was loaded
                            // after the typedef was initially compiled
                            if let Some(target_name) = self.string_interner.get(*placeholder_name) {
                                // Convert "rayzor.Bytes" to "rayzor_Bytes" for stdlib mapping lookup
                                let qualified_name = target_name.replace(".", "_");
                                if let Some((_sig, mapping)) = self.stdlib_mapping.find_by_name(&qualified_name, method_name) {
                                    // Early return with the mapping
                                    drop(type_table);
                                    return Some((_sig.class, _sig.method, mapping));
                                }
                            }
                            (None, None, Vec::new())
                        }
                        _ => (None, None, Vec::new()),
                    }
                } else {
                    (None, None, Vec::new())
                }
            }
            _ => (None, None, Vec::new()),
        };

        let base_class_name = base_class_name?;

        // MONOMORPHIZATION: For generic extern classes like Vec<T>, monomorphize the class name
        // based on type arguments. Vec<Int> -> VecI32, Vec<Float> -> VecF64, etc.
        let monomorphized_class_name: Option<String> = if base_class_name == "Vec" && !type_args.is_empty() {
            let first_arg = type_args[0];
            let suffix = if let Some(arg_type) = type_table.get(first_arg) {
                match &arg_type.kind {
                    TypeKind::Int => Some("I32"),
                    TypeKind::Float => Some("F64"),
                    TypeKind::Bool => Some("Bool"),
                    TypeKind::String => Some("Ptr"), // Strings are reference types
                    TypeKind::Class { symbol_id, .. } => {
                        // Check if it's Int64 (a class type representing 64-bit int)
                        if let Some(class_info) = self.symbol_table.get_symbol(*symbol_id) {
                            if let Some(name) = self.string_interner.get(class_info.name) {
                                if name == "Int64" {
                                    Some("I64")
                                } else {
                                    Some("Ptr") // Other classes are reference types
                                }
                            } else {
                                Some("Ptr")
                            }
                        } else {
                            Some("Ptr")
                        }
                    }
                    _ => Some("Ptr"), // Default to pointer for other types
                }
            } else {
                Some("Ptr") // Unknown type, use pointer
            };
            suffix.map(|s| format!("Vec{}", s))
        } else {
            None
        };

        drop(type_table);

        // Use monomorphized name if available, otherwise use base name
        let class_name = monomorphized_class_name.as_deref().unwrap_or(base_class_name);

        // Try to find this method in the stdlib mapping
        // First try qualified name (e.g., "rayzor_Bytes"), then fall back to simple name
        // This avoids hardcoding class names and lets the StdlibMapping be the single source of truth
        if let Some(ref qn) = qualified_class_name {
            if let Some((sig, mapping)) = self.stdlib_mapping.find_by_name(qn, method_name) {
                eprintln!("DEBUG: [get_stdlib_runtime_info] Found {}.{} via qualified name -> {}", qn, method_name, mapping.runtime_name);
                return Some((sig.class, sig.method, mapping));
            }
        }
        if let Some((sig, mapping)) = self.stdlib_mapping.find_by_name(class_name, method_name) {
            Some((sig.class, sig.method, mapping))
        } else {
            None
        }
    }

    /// Check if a qualified name + method belongs to rayzor stdlib and return the runtime function name
    ///
    /// For static methods like Thread.spawn, Channel.init, etc.
    /// Uses StdlibMapping as single source of truth - no hardcoded mappings!
    fn get_static_stdlib_runtime_func(
        &self,
        qualified_name: &str,
        method_name: &str,
    ) -> Option<&'static str> {
        // Parse qualified name to extract class name
        // Patterns: "rayzor.concurrent.Thread.spawn", "test.Thread.spawn", "StringTools.startsWith"
        let parts: Vec<&str> = qualified_name.split('.').collect();
        let class_name = parts.iter().rev().nth(1)?; // Second-to-last part is class name

        // Use StdlibMapping to find the runtime function
        // This is the ONLY source of truth - all mappings come from the actual Rust implementations

        // First try simple class name (e.g., "Thread")
        if let Some((_sig, mapping)) = self.stdlib_mapping.find_by_name(class_name, method_name) {
            return Some(mapping.runtime_name);
        }

        // Then try qualified class name with underscore format (e.g., "sys_thread_Thread")
        // This handles sys.thread.Thread -> sys_thread_Thread mappings
        if parts.len() >= 2 {
            // Build qualified class name: all parts except the last (method name), joined with underscore
            let qualified_class_name = parts[..parts.len() - 1].join("_");
            if let Some((_sig, mapping)) = self.stdlib_mapping.find_by_name(&qualified_class_name, method_name) {
                return Some(mapping.runtime_name);
            }
        }

        None
    }

    /// Get the correct signature for a stdlib MIR wrapper function.
    /// These signatures MUST match what's defined in compiler/src/stdlib/{thread,channel,sync}.rs
    /// Returns (param_types, return_type) or None if not a known MIR wrapper.
    fn get_stdlib_mir_wrapper_signature(&self, name: &str) -> Option<(Vec<IrType>, IrType)> {
        let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
        let i32_type = IrType::I32;
        let bool_type = IrType::Bool;
        let u64_type = IrType::U64;
        let void_type = IrType::Void;

        match name {
            // Thread methods - all take/return pointers to opaque handles
            "Thread_spawn" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),  // (closure_obj) -> handle
            "Thread_join" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),    // (handle) -> result
            "Thread_isFinished" => Some((vec![ptr_u8.clone()], bool_type)),   // (handle) -> bool
            "Thread_sleep" => Some((vec![i32_type.clone()], void_type)),      // (millis) -> void
            "Thread_yieldNow" => Some((vec![], void_type)),                   // () -> void
            "Thread_currentId" => Some((vec![], u64_type)),                   // () -> id

            // Channel methods
            "Channel_init" => Some((vec![i32_type.clone()], ptr_u8.clone())),       // (capacity) -> channel
            "Channel_send" => Some((vec![ptr_u8.clone(), ptr_u8.clone()], void_type)), // (channel, value) -> void
            "Channel_receive" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),      // (channel) -> value
            "Channel_trySend" => Some((vec![ptr_u8.clone(), ptr_u8.clone()], bool_type)), // (channel, value) -> bool
            "Channel_tryReceive" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),   // (channel) -> value
            "Channel_close" => Some((vec![ptr_u8.clone()], void_type)),              // (channel) -> void
            "Channel_isClosed" => Some((vec![ptr_u8.clone()], bool_type)),           // (channel) -> bool
            "Channel_len" => Some((vec![ptr_u8.clone()], i32_type.clone())),         // (channel) -> len
            "Channel_capacity" => Some((vec![ptr_u8.clone()], i32_type.clone())),    // (channel) -> capacity
            "Channel_isEmpty" => Some((vec![ptr_u8.clone()], bool_type)),            // (channel) -> bool
            "Channel_isFull" => Some((vec![ptr_u8.clone()], bool_type)),             // (channel) -> bool

            // Mutex methods
            "Mutex_init" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),         // (value) -> mutex
            "Mutex_lock" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),         // (mutex) -> guard
            "Mutex_tryLock" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),      // (mutex) -> guard
            "Mutex_isLocked" => Some((vec![ptr_u8.clone()], bool_type)),          // (mutex) -> bool

            // MutexGuard methods
            "MutexGuard_get" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),     // (guard) -> value
            "MutexGuard_unlock" => Some((vec![ptr_u8.clone()], void_type)),       // (guard) -> void

            // Arc methods
            "Arc_init" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),           // (value) -> arc
            "Arc_clone" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),          // (arc) -> arc
            "Arc_get" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),            // (arc) -> value
            "Arc_strongCount" => Some((vec![ptr_u8.clone()], u64_type)),          // (arc) -> count
            "Arc_tryUnwrap" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),      // (arc) -> value
            "Arc_asPtr" => Some((vec![ptr_u8.clone()], u64_type)),                // (arc) -> ptr_as_u64

            // VecI32 methods
            "VecI32_new" => Some((vec![], ptr_u8.clone())),                        // () -> vec
            "VecI32_push" => Some((vec![ptr_u8.clone(), i32_type.clone()], void_type.clone())),  // (vec, value) -> void
            "VecI32_pop" => Some((vec![ptr_u8.clone()], i32_type.clone())),       // (vec) -> value
            "VecI32_get" => Some((vec![ptr_u8.clone(), IrType::I64], i32_type.clone())), // (vec, index) -> value
            "VecI32_set" => Some((vec![ptr_u8.clone(), IrType::I64, i32_type.clone()], void_type.clone())), // (vec, index, value) -> void
            "VecI32_length" => Some((vec![ptr_u8.clone()], IrType::I64)),         // (vec) -> len
            "VecI32_capacity" => Some((vec![ptr_u8.clone()], IrType::I64)),       // (vec) -> cap
            "VecI32_isEmpty" => Some((vec![ptr_u8.clone()], bool_type.clone())),  // (vec) -> bool
            "VecI32_first" => Some((vec![ptr_u8.clone()], i32_type.clone())),     // (vec) -> value
            "VecI32_last" => Some((vec![ptr_u8.clone()], i32_type.clone())),      // (vec) -> value
            "VecI32_clear" => Some((vec![ptr_u8.clone()], void_type.clone())),    // (vec) -> void
            "VecI32_sort" => Some((vec![ptr_u8.clone()], void_type.clone())),     // (vec) -> void
            "VecI32_sortBy" => Some((vec![ptr_u8.clone(), ptr_u8.clone(), ptr_u8.clone()], void_type.clone())), // (vec, cmp_fn, env) -> void

            // VecI64 methods
            "VecI64_new" => Some((vec![], ptr_u8.clone())),
            "VecI64_push" => Some((vec![ptr_u8.clone(), IrType::I64], void_type.clone())),
            "VecI64_pop" => Some((vec![ptr_u8.clone()], IrType::I64)),
            "VecI64_get" => Some((vec![ptr_u8.clone(), IrType::I64], IrType::I64)),
            "VecI64_set" => Some((vec![ptr_u8.clone(), IrType::I64, IrType::I64], void_type.clone())),
            "VecI64_length" => Some((vec![ptr_u8.clone()], IrType::I64)),
            "VecI64_isEmpty" => Some((vec![ptr_u8.clone()], bool_type.clone())),
            "VecI64_first" => Some((vec![ptr_u8.clone()], IrType::I64)),
            "VecI64_last" => Some((vec![ptr_u8.clone()], IrType::I64)),
            "VecI64_clear" => Some((vec![ptr_u8.clone()], void_type.clone())),

            // VecF64 methods
            "VecF64_new" => Some((vec![], ptr_u8.clone())),
            "VecF64_push" => Some((vec![ptr_u8.clone(), IrType::F64], void_type.clone())),
            "VecF64_pop" => Some((vec![ptr_u8.clone()], IrType::F64)),
            "VecF64_get" => Some((vec![ptr_u8.clone(), IrType::I64], IrType::F64)),
            "VecF64_set" => Some((vec![ptr_u8.clone(), IrType::I64, IrType::F64], void_type.clone())),
            "VecF64_length" => Some((vec![ptr_u8.clone()], IrType::I64)),
            "VecF64_isEmpty" => Some((vec![ptr_u8.clone()], bool_type.clone())),
            "VecF64_first" => Some((vec![ptr_u8.clone()], IrType::F64)),
            "VecF64_last" => Some((vec![ptr_u8.clone()], IrType::F64)),
            "VecF64_clear" => Some((vec![ptr_u8.clone()], void_type.clone())),
            "VecF64_sort" => Some((vec![ptr_u8.clone()], void_type.clone())),
            "VecF64_sortBy" => Some((vec![ptr_u8.clone(), ptr_u8.clone(), ptr_u8.clone()], void_type.clone())),

            // VecBool methods
            "VecBool_new" => Some((vec![], ptr_u8.clone())),
            "VecBool_push" => Some((vec![ptr_u8.clone(), bool_type.clone()], void_type.clone())),
            "VecBool_pop" => Some((vec![ptr_u8.clone()], bool_type.clone())),
            "VecBool_get" => Some((vec![ptr_u8.clone(), IrType::I64], bool_type.clone())),
            "VecBool_set" => Some((vec![ptr_u8.clone(), IrType::I64, bool_type.clone()], void_type.clone())),
            "VecBool_length" => Some((vec![ptr_u8.clone()], IrType::I64)),
            "VecBool_isEmpty" => Some((vec![ptr_u8.clone()], bool_type.clone())),
            "VecBool_clear" => Some((vec![ptr_u8.clone()], void_type.clone())),

            // VecPtr methods (for reference types)
            "VecPtr_new" => Some((vec![], ptr_u8.clone())),
            "VecPtr_push" => Some((vec![ptr_u8.clone(), ptr_u8.clone()], void_type.clone())),
            "VecPtr_pop" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),
            "VecPtr_get" => Some((vec![ptr_u8.clone(), IrType::I64], ptr_u8.clone())),
            "VecPtr_set" => Some((vec![ptr_u8.clone(), IrType::I64, ptr_u8.clone()], void_type.clone())),
            "VecPtr_length" => Some((vec![ptr_u8.clone()], IrType::I64)),
            "VecPtr_isEmpty" => Some((vec![ptr_u8.clone()], bool_type.clone())),
            "VecPtr_first" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),
            "VecPtr_last" => Some((vec![ptr_u8.clone()], ptr_u8.clone())),
            "VecPtr_clear" => Some((vec![ptr_u8.clone()], void_type.clone())),
            "VecPtr_sortBy" => Some((vec![ptr_u8.clone(), ptr_u8.clone(), ptr_u8.clone()], void_type.clone())),

            _ => None,
        }
    }

    /// Get known signature for extern runtime functions (not MIR wrappers)
    /// This is used to override inferred types for functions like Math that need f64
    fn get_extern_function_signature(&self, name: &str) -> Option<(Vec<IrType>, IrType)> {
        let bool_type = IrType::Bool;

        match name {
            // Math functions - all work with f64
            "haxe_math_abs" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_min" => Some((vec![IrType::F64, IrType::F64], IrType::F64)),
            "haxe_math_max" => Some((vec![IrType::F64, IrType::F64], IrType::F64)),
            "haxe_math_floor" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_ceil" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_round" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_sin" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_cos" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_tan" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_asin" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_acos" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_atan" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_atan2" => Some((vec![IrType::F64, IrType::F64], IrType::F64)),
            "haxe_math_exp" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_log" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_pow" => Some((vec![IrType::F64, IrType::F64], IrType::F64)),
            "haxe_math_sqrt" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_is_nan" => Some((vec![IrType::F64], bool_type.clone())),
            "haxe_math_is_finite" => Some((vec![IrType::F64], bool_type)),
            "haxe_math_fround" => Some((vec![IrType::F64], IrType::F64)),
            "haxe_math_random" => Some((vec![], IrType::F64)),

            // Sys functions
            "haxe_sys_time" => Some((vec![], IrType::F64)),
            "haxe_sys_cpu_time" => Some((vec![], IrType::F64)),
            "haxe_sys_exit" => Some((vec![IrType::I64], IrType::Void)),
            "haxe_sys_sleep" => Some((vec![IrType::F64], IrType::Void)),
            "haxe_sys_system_name" => Some((vec![], IrType::Ptr(Box::new(IrType::Void)))),
            "haxe_sys_get_cwd" => Some((vec![], IrType::Ptr(Box::new(IrType::Void)))),
            "haxe_sys_set_cwd" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_sys_get_env" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Ptr(Box::new(IrType::Void)))),
            "haxe_sys_put_env" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_sys_program_path" => Some((vec![], IrType::Ptr(Box::new(IrType::Void)))),

            // Std functions
            "haxe_std_int" => Some((vec![IrType::F64], IrType::I64)),
            "haxe_std_parse_int" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::I64)),
            "haxe_std_parse_float" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::F64)),
            "haxe_std_random" => Some((vec![IrType::I64], IrType::I64)),

            // File I/O functions (sys.io.File)
            "haxe_file_get_content" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Ptr(Box::new(IrType::Void)))),
            "haxe_file_save_content" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_file_copy" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),

            // FileSystem functions (sys.FileSystem)
            "haxe_filesystem_exists" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Bool)),
            "haxe_filesystem_is_directory" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Bool)),
            "haxe_filesystem_create_directory" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_filesystem_delete_file" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_filesystem_delete_directory" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_filesystem_rename" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_filesystem_full_path" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Ptr(Box::new(IrType::Void)))),
            "haxe_filesystem_absolute_path" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Ptr(Box::new(IrType::Void)))),

            // StringMap<T> functions (haxe.ds.StringMap)
            // StringMap is an opaque pointer handle to a Rust HashMap<String, *mut u8>
            "haxe_stringmap_new" => Some((vec![], IrType::Ptr(Box::new(IrType::Void)))),
            "haxe_stringmap_set" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_stringmap_get" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::Void))], IrType::Ptr(Box::new(IrType::Void)))),
            "haxe_stringmap_exists" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::Void))], IrType::Bool)),
            "haxe_stringmap_remove" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::Void))], IrType::Bool)),
            "haxe_stringmap_clear" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_stringmap_count" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::I64)),
            "haxe_stringmap_keys" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::I64))], IrType::Ptr(Box::new(IrType::Void)))),
            "haxe_stringmap_to_string" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Ptr(Box::new(IrType::Void)))),

            // IntMap<T> functions (haxe.ds.IntMap)
            // IntMap is an opaque pointer handle to a Rust HashMap<i64, *mut u8>
            "haxe_intmap_new" => Some((vec![], IrType::Ptr(Box::new(IrType::Void)))),
            "haxe_intmap_set" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::I64, IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_intmap_get" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::I64], IrType::Ptr(Box::new(IrType::Void)))),
            "haxe_intmap_exists" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::I64], IrType::Bool)),
            "haxe_intmap_remove" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::I64], IrType::Bool)),
            "haxe_intmap_clear" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Void)),
            "haxe_intmap_count" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::I64)),
            "haxe_intmap_keys" => Some((vec![IrType::Ptr(Box::new(IrType::Void)), IrType::Ptr(Box::new(IrType::I64))], IrType::Ptr(Box::new(IrType::I64)))),
            "haxe_intmap_to_string" => Some((vec![IrType::Ptr(Box::new(IrType::Void))], IrType::Ptr(Box::new(IrType::Void)))),

            // Boxing/unboxing functions for Dynamic types
            // These take i64 on ARM64 due to C ABI parameter extension
            "haxe_box_int_ptr" => Some((vec![IrType::I64], IrType::Ptr(Box::new(IrType::U8)))),
            "haxe_box_float_ptr" => Some((vec![IrType::F64], IrType::Ptr(Box::new(IrType::U8)))),
            "haxe_box_bool_ptr" => Some((vec![IrType::I64], IrType::Ptr(Box::new(IrType::U8)))),  // Bool extended to i64 on ARM64
            "haxe_unbox_int_ptr" => Some((vec![IrType::Ptr(Box::new(IrType::U8))], IrType::I64)),
            "haxe_unbox_float_ptr" => Some((vec![IrType::Ptr(Box::new(IrType::U8))], IrType::F64)),
            "haxe_unbox_bool_ptr" => Some((vec![IrType::Ptr(Box::new(IrType::U8))], IrType::I64)),  // Bool extended to i64 on ARM64

            _ => None,
        }
    }

    /// Register a forward reference to a stdlib MIR function that will be provided by module merging
    ///
    /// Unlike extern functions (which use C calling convention and are resolved by Cranelift),
    /// stdlib MIR functions use Haxe calling convention and are colocated functions that will
    /// be provided when the stdlib MIR module is merged.
    fn register_stdlib_mir_forward_ref(
        &mut self,
        name: &str,
        mut param_types: Vec<IrType>,
        mut return_type: IrType,
    ) -> IrFunctionId {
        // Check if already registered
        for (func_id, func) in &self.builder.module.functions {
            if func.name == name {
                return *func_id;
            }
        }

        // Override with correct signature if this is a known MIR wrapper
        // This fixes the bug where Thread_spawn gets wrong signature from inferred lambda type
        if let Some((correct_params, correct_return)) = self.get_stdlib_mir_wrapper_signature(name) {
            eprintln!("DEBUG: Using registered signature for {}: {} params -> {:?}",
                     name, correct_params.len(), correct_return);
            param_types = correct_params;
            return_type = correct_return;
        }

        // Create forward reference with Haxe calling convention (will be replaced during merge)
        let func_id = IrFunctionId(self.builder.module.next_function_id);
        self.builder.module.next_function_id += 1;

        let params = param_types
            .into_iter()
            .enumerate()
            .map(|(i, ty)| IrParameter {
                name: format!("arg{}", i),
                ty: ty.clone(),
                reg: IrId::new(i as u32),
                by_ref: false,
            })
            .collect();

        // Stdlib MIR wrappers use C calling convention (no env param)
        // This matches the actual definitions in thread.rs, channel.rs, sync.rs
        let signature = IrFunctionSignature {
            parameters: params,
            return_type: return_type.clone(),
            calling_convention: CallingConvention::C, // C calling convention for stdlib MIR wrappers
            can_throw: false,
            type_params: vec![],
            uses_sret: matches!(return_type, IrType::Struct { .. }),
        };

        use crate::ir::{IrControlFlowGraph, FunctionAttributes, Linkage, InlineHint};
        use crate::tast::SymbolId;

        // Create stub function (empty blocks = forward declaration)
        let mut attributes = FunctionAttributes::default();
        attributes.linkage = Linkage::Public;
        attributes.inline = InlineHint::Auto;

        let function = IrFunction {
            id: func_id,
            symbol_id: SymbolId::from_raw(0),
            name: name.to_string(),
            qualified_name: None,
            signature,
            cfg: IrControlFlowGraph::new(), // Empty - will be replaced during merge
            locals: HashMap::new(),
            register_types: HashMap::new(),
            attributes,
            source_location: IrSourceLocation::unknown(),
            next_reg_id: 0,
        };

        self.builder.module.functions.insert(func_id, function);
        func_id
    }

    /// Get or register an external runtime function, returning its ID
    ///
    /// This allows calling external runtime functions (like haxe_math_abs) from MIR
    fn get_or_register_extern_function(
        &mut self,
        name: &str,
        mut param_types: Vec<IrType>,
        mut return_type: IrType,
    ) -> IrFunctionId {
        if name.contains("Channel") || name.contains("init") {
            eprintln!("DEBUG: [get_or_register_extern] Called with name='{}', {} params", name, param_types.len());
        }

        // Override with correct signature if this is a known extern function
        // This is critical for Math functions to get f64 types instead of inferred i64
        if let Some((correct_params, correct_return)) = self.get_extern_function_signature(name) {
            eprintln!("DEBUG: [get_or_register_extern] Using registered signature for {}: {} params -> {:?}",
                     name, correct_params.len(), correct_return);
            param_types = correct_params;
            return_type = correct_return;
        }

        // FIRST: Check if a MIR wrapper with this name already exists (has blocks)
        // If it does, return that instead of creating an extern
        let existing_mir_wrapper: Option<IrFunctionId> = self.builder.module.functions
            .iter()
            .find(|(_, f)| f.name == name && !f.cfg.blocks.is_empty())
            .map(|(id, _)| *id);

        if let Some(func_id) = existing_mir_wrapper {
            return func_id;
        }

        // Check if we already registered this extern function
        // First, find if it exists and collect info (can't mutate while iterating)
        let existing_func: Option<(IrFunctionId, usize)> = self.builder.module.extern_functions
            .iter()
            .find(|(_, ef)| ef.name == name)
            .map(|(id, ef)| (*id, ef.signature.parameters.len()));

        if let Some((func_id, existing_param_count)) = existing_func {
            let new_param_count = param_types.len();

            // If new signature has MORE parameters, update the existing function
            if new_param_count > existing_param_count {
                eprintln!("DEBUG: Updating extern '{}' signature: {} params -> {} params",
                          name, existing_param_count, new_param_count);

                // Create updated parameters
                let params = param_types
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| IrParameter {
                        name: format!("arg{}", i),
                        ty: ty.clone(),
                        reg: IrId::new(i as u32),
                        by_ref: false,
                    })
                    .collect();

                let new_signature = IrFunctionSignature {
                    parameters: params,
                    return_type: return_type.clone(),
                    calling_convention: CallingConvention::C,
                    can_throw: false,
                    type_params: vec![],
                    uses_sret: false,
                };

                // Update in extern_functions map
                if let Some(ext_func) = self.builder.module.extern_functions.get_mut(&func_id) {
                    ext_func.signature = new_signature.clone();
                }

                // Update in functions map
                if let Some(func) = self.builder.module.functions.get_mut(&func_id) {
                    func.signature = new_signature;
                }
            }

            return func_id;
        }

        // Also check in functions (extern functions have empty blocks)
        let existing_func: Option<(IrFunctionId, usize)> = self.builder.module.functions
            .iter()
            .filter(|(_, f)| f.name == name && f.cfg.blocks.is_empty())
            .map(|(id, f)| (*id, f.signature.parameters.len()))
            .next();

        if let Some((func_id, existing_param_count)) = existing_func {
            let new_param_count = param_types.len();

            if new_param_count > existing_param_count {
                eprintln!("DEBUG: Updating function '{}' signature: {} params -> {} params",
                          name, existing_param_count, new_param_count);

                // Create updated parameters
                let params = param_types
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| IrParameter {
                        name: format!("arg{}", i),
                        ty: ty.clone(),
                        reg: IrId::new(i as u32),
                        by_ref: false,
                    })
                    .collect();

                let new_signature = IrFunctionSignature {
                    parameters: params,
                    return_type: return_type.clone(),
                    calling_convention: CallingConvention::C,
                    can_throw: false,
                    type_params: vec![],
                    uses_sret: false,
                };

                // Update in functions map
                if let Some(f) = self.builder.module.functions.get_mut(&func_id) {
                    f.signature = new_signature.clone();
                }

                // Also update in extern_functions if present
                if let Some(ext_func) = self.builder.module.extern_functions.get_mut(&func_id) {
                    ext_func.signature = new_signature;
                }
            }

            return func_id;
        }

        // println!("  ℹ️  Registering extern function: {}", name);

        // Create new extern function entry
        let func_id = IrFunctionId(self.builder.module.next_function_id);
        self.builder.module.next_function_id += 1;

        let params = param_types
            .into_iter()
            .enumerate()
            .map(|(i, ty)| IrParameter {
                name: format!("arg{}", i),
                ty: ty.clone(),
                reg: IrId::new(i as u32),
                by_ref: false,
            })
            .collect();

        let signature = IrFunctionSignature {
            parameters: params,
            return_type: return_type.clone(),
            calling_convention: CallingConvention::C, // External functions use C calling convention
            can_throw: false,
            type_params: vec![],
            uses_sret: false, // No struct return for C functions
        };

        // Create the IrFunction with empty blocks (extern marker)
        let func = crate::ir::functions::IrFunction {
            id: func_id,
            symbol_id: SymbolId::from_raw(0),
            name: name.to_string(),
            qualified_name: None,
            signature: signature.clone(),
            cfg: crate::ir::blocks::IrControlFlowGraph {
                blocks: std::collections::HashMap::new(), // Empty blocks = extern
                entry_block: IrBlockId(0),
                next_block_id: 0,
            },
            locals: std::collections::HashMap::new(),
            register_types: std::collections::HashMap::new(),
            attributes: crate::ir::functions::FunctionAttributes::default(),
            source_location: IrSourceLocation::unknown(),
            next_reg_id: 0,
        };

        // Add to both functions and extern_functions maps
        self.builder.module.functions.insert(func_id, func);

        let extern_func = crate::ir::modules::IrExternFunction {
            id: func_id,
            name: name.to_string(),
            symbol_id: SymbolId::from_raw(0), // Placeholder
            signature,
            source: "rayzor_runtime".to_string(),
        };

        self.builder
            .module
            .extern_functions
            .insert(func_id, extern_func);

        // eprintln!(
        //     "  ℹ️  After registration: module has {} functions, {} extern_functions",
        //     self.builder.module.functions.len(),
        //     self.builder.module.extern_functions.len()
        // );

        func_id
    }

    /// Record an enum for RTTI registration
    /// This collects enum metadata during lowering so it can be registered at module init
    fn record_enum_for_registration(&mut self, enum_symbol_id: SymbolId, _type_id: TypeId) {
        // Skip if already recorded
        if self.enums_for_registration.contains_key(&enum_symbol_id) {
            return;
        }

        // Calculate runtime type ID (symbol_id + 1000 offset)
        let runtime_type_id = enum_symbol_id.as_raw() + 1000;

        // Get enum name from symbol table
        let enum_name = self.symbol_table
            .get_symbol(enum_symbol_id)
            .and_then(|s| self.string_interner.get(s.name))
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Enum_{}", enum_symbol_id.as_raw()));

        // Get variant names from the symbol table's enum_variants map
        let variant_names: Vec<String> = if let Some(variant_ids) = self.symbol_table.get_enum_variants(enum_symbol_id) {
            variant_ids.iter()
                .filter_map(|&var_id| {
                    self.symbol_table.get_symbol(var_id)
                        .and_then(|sym| self.string_interner.get(sym.name))
                        .map(|s| s.to_string())
                })
                .collect()
        } else {
            Vec::new()
        };

        eprintln!("DEBUG: [RTTI] Recording enum '{}' (type_id={}) with variants: {:?}",
                  enum_name, runtime_type_id, variant_names);

        self.enums_for_registration.insert(
            enum_symbol_id,
            (runtime_type_id, enum_name, variant_names),
        );
    }

    /// Get collected enum RTTI data for registration
    pub fn get_enums_for_registration(&self) -> &HashMap<SymbolId, (u32, String, Vec<String>)> {
        &self.enums_for_registration
    }

    /// Lower a HIR expression to MIR value
    fn lower_expression(&mut self, expr: &HirExpr) -> Option<IrId> {
        // eprintln!("DEBUG: lower_expression - {:?}", std::mem::discriminant(&expr.kind));

        // DEBUG: Check if this is Field expression being lowered
        if matches!(&expr.kind, HirExprKind::Field { .. }) {
            eprintln!("DEBUG: [lower_expression] START - Field expression");
        }

        // Set source location for debugging
        self.builder
            .set_source_location(self.convert_source_location(&expr.source_location));

        let result = match &expr.kind {
            HirExprKind::Literal(lit) => self.lower_literal(lit, expr.ty),

            HirExprKind::Variable { symbol, .. } => {
                // Check if this symbol is a function reference (local or external)
                if let Some(func_id) = self.get_function_id(symbol) {
                    // Create a function pointer constant for static methods
                    return self.builder.build_function_ptr(func_id);
                }

                // Try to get from symbol_map first (local variables, parameters)
                if let Some(&reg) = self.symbol_map.get(symbol) {
                    // Check if we need to convert the type
                    // This handles cases where captured variables are stored as i64 in closure environment
                    // but need to be used as their original type (e.g., i32)
                    if let Some(actual_type) = self.builder.get_register_type(reg) {
                        let expected_type = self.convert_type(expr.ty);

                        // If types don't match, consider adding a cast instruction
                        // CRITICAL: Do NOT cast from Ptr to smaller types (I32, etc.)
                        // This can happen when generic type resolution fails (e.g., Thread<T> where T is unresolved)
                        // and the type system incorrectly infers I32 for a class instance pointer
                        if actual_type != expected_type {
                            // Skip casts in these cases to preserve actual type:
                            // 1. Actual is pointer, expected is scalar (would truncate pointer)
                            // 2. Actual is String, expected is Ptr(Void) (would lose string data)
                            // 3. Actual is more specific than Ptr(Void)
                            // 4. Actual is Ptr(String), expected is Ptr(Void) - preserve String type info for trace
                            let actual_is_ptr = matches!(&actual_type, IrType::Ptr(_));
                            let expected_is_ptr = matches!(&expected_type, IrType::Ptr(_));
                            let expected_is_void_ptr = matches!(&expected_type, IrType::Ptr(inner) if matches!(**inner, IrType::Void));
                            let actual_is_specific = matches!(&actual_type, IrType::String | IrType::I32 | IrType::I64 | IrType::F32 | IrType::F64 | IrType::Bool);
                            // Only skip cast for Ptr(String) specifically - NOT for other pointer types like Ptr(U8)
                            // which are used by concurrency primitives (Mutex, Thread, Channel, etc.)
                            let actual_is_string_ptr = matches!(&actual_type, IrType::Ptr(inner) if matches!(**inner, IrType::String));

                            let should_skip_cast = (actual_is_ptr && !expected_is_ptr)  // pointer to scalar
                                || (actual_is_specific && expected_is_void_ptr)          // specific type to void pointer
                                || (actual_is_string_ptr && expected_is_void_ptr);       // Ptr(String) to Ptr(Void)

                            if should_skip_cast {
                                eprintln!("DEBUG: Variable type mismatch - symbol={:?}, actual: {:?}, expected: {:?}, SKIPPING cast (would lose type info)", symbol, actual_type, expected_type);
                                Some(reg)
                            } else {
                                eprintln!("DEBUG: Variable type mismatch - symbol={:?}, actual: {:?}, expected: {:?}, inserting cast", symbol, actual_type, expected_type);
                                self.builder.build_cast(reg, actual_type, expected_type)
                            }
                        } else {
                            Some(reg)
                        }
                    } else {
                        // No type info, return as-is
                        Some(reg)
                    }
                } else {
                    // Symbol not in local scope - check if it's a class field
                    // If so, we need to access it via 'this' pointer

                    // First check field_index_map - this is more reliable than SymbolKind::Field
                    // because field symbols may be registered with SymbolKind::Variable
                    if let Some(&(field_class_type, _field_idx)) = self.field_index_map.get(symbol) {
                        // Get 'this' pointer (SymbolId(0) is the special 'this' mapping)
                        if let Some(&this_reg) = self.symbol_map.get(&SymbolId::from_raw(0)) {
                            // Use current_this_type if available, otherwise use field_class_type
                            let owner_type = self.current_this_type.unwrap_or(field_class_type);
                            return self.lower_field_access(this_reg, *symbol, owner_type, expr.ty);
                        }
                    }

                    // Fallback: check if symbol table says it's a field or enum variant
                    if let Some(sym) = self.symbol_table.get_symbol(*symbol) {
                        use crate::tast::SymbolKind;
                        if sym.kind == SymbolKind::Field {
                            if let Some(&this_reg) = self.symbol_map.get(&SymbolId::from_raw(0)) {
                                if let Some(owner_type) = self.current_this_type {
                                    return self.lower_field_access(this_reg, *symbol, owner_type, expr.ty);
                                }
                            }
                        } else if sym.kind == SymbolKind::EnumVariant {
                            // Enum variant without parameters - return its discriminant value
                            // Find the parent enum and get the variant index
                            if let Some(parent_enum_id) = self.symbol_table.find_parent_enum_for_constructor(*symbol) {
                                if let Some(variants) = self.symbol_table.get_enum_variants(parent_enum_id) {
                                    // Find the index of this variant
                                    for (idx, variant_id) in variants.iter().enumerate() {
                                        if *variant_id == *symbol {
                                            // Return the discriminant as an i64 constant (matches Haxe Int convention)
                                            return self.builder.build_const(IrValue::I64(idx as i64));
                                        }
                                    }
                                }
                            }
                            // If we can't find the variant info, try to get the discriminant from the type
                            eprintln!("DEBUG: EnumVariant {:?} - could not find discriminant", symbol);
                        }
                    }

                    // If we get here, we couldn't resolve the variable
                    eprintln!("ERROR: Could not resolve variable symbol {:?}", symbol);
                    None
                }
            }

            HirExprKind::Field { object, field } => {
                // Check if this is an enum variant access (e.g., Color.Red)
                // In that case, the object is an Enum type symbol, not a value
                if let HirExprKind::Variable { symbol, .. } = &object.kind {
                    if let Some(sym) = self.symbol_table.get_symbol(*symbol) {
                        use crate::tast::SymbolKind;
                        if sym.kind == SymbolKind::Enum {
                            // This is an enum variant access - get the variant discriminant
                            let enum_name = self.string_interner.get(sym.name).unwrap_or("<unknown>");
                            let field_sym = self.symbol_table.get_symbol(*field);
                            let field_name = field_sym.and_then(|s| self.string_interner.get(s.name)).unwrap_or("<unknown>");

                            if let Some(variants) = self.symbol_table.get_enum_variants(*symbol) {
                                for (idx, variant_id) in variants.iter().enumerate() {
                                    let variant_sym = self.symbol_table.get_symbol(*variant_id);
                                    let variant_name = variant_sym.and_then(|s| self.string_interner.get(s.name)).unwrap_or("<unknown>");
                                    // Compare by name since the field symbol might be different from the variant symbol
                                    if *variant_id == *field || variant_name == field_name {
                                        // Return the discriminant as an i64 constant (matches Haxe Int convention)
                                        return self.builder.build_const(IrValue::I64(idx as i64));
                                    }
                                }
                            }
                            // If field is not a variant, fall through to regular field access
                        }
                    }
                }

                // Regular field access
                eprintln!("DEBUG: [Field expression] About to lower object");
                let obj_reg = self.lower_expression(object)?;
                eprintln!("DEBUG: [Field expression] Object lowered to reg={}, now calling lower_field_access", obj_reg);
                let receiver_ty = object.ty; // The type of the object being accessed
                let result = self.lower_field_access(obj_reg, *field, receiver_ty, expr.ty);
                eprintln!("DEBUG: [Field expression] lower_field_access returned {:?}", result);
                result
            }

            HirExprKind::Index { object, index } => {
                let obj_reg = self.lower_expression(object)?;
                let idx_reg = self.lower_expression(index)?;
                self.lower_index_access(obj_reg, idx_reg, expr.ty)
            }

            HirExprKind::Call {
                callee,
                args,
                is_method,
                type_args: hir_type_args,
            } => {
                let result_type = self.convert_type(expr.ty);
                // Convert HIR type_args to IrType for use in CallDirect
                let converted_hir_type_args: Vec<IrType> = hir_type_args.iter()
                    .map(|&ty_id| self.convert_type(ty_id))
                    .collect();

                // DEBUG: Check if void function is being called with dest
                eprintln!("DEBUG: [CALL] expr.ty={:?}, result_type={:?}, is_method={}", expr.ty, result_type, is_method);

                eprintln!(
                    "DEBUG: Call expression - is_method={}, callee kind={:?}",
                    is_method,
                    std::mem::discriminant(&callee.kind)
                );

                // Check if this is a method call (callee is a field access)
                eprintln!("DEBUG: [CALL CHECK] callee.kind discriminant = {:?}", std::mem::discriminant(&callee.kind));
                if let HirExprKind::Field { object, field } = &callee.kind {
                    // This is a method call: object.method(args)
                    // The method symbol should be in our function_map (local or external)
                    let method_name = self.symbol_table.get_symbol(*field).and_then(|s| self.string_interner.get(s.name));
                    let in_local = self.function_map.contains_key(field);
                    let in_external = self.external_function_map.contains_key(field);
                    eprintln!("DEBUG: [Method call] method={:?}, field={:?}, in_local={}, in_external={}", method_name, field, in_local, in_external);
                    if let Some(func_id) = self.get_function_id(field) {
                        // eprintln!("DEBUG: Found method in function_map - func_id={:?}", func_id);

                        // FIRST: Try to route through runtime mapping for extern class methods
                        // Check if there's a runtime mapping using the standard approach
                        // BUT: for String methods with optional params, use param-count-aware lookup
                        let stdlib_info = {
                            let method_name_str = self.symbol_table.get_symbol(*field)
                                .and_then(|s| self.string_interner.get(s.name));

                            // Check if this is a String method with optional params
                            if let Some(mn) = method_name_str {
                                if (mn == "indexOf" || mn == "lastIndexOf") {
                                    // Use param-count-aware lookup for indexOf/lastIndexOf
                                    let arg_count = args.len();
                                    eprintln!("DEBUG: [indexOf/lastIndexOf lookup] method={}, arg_count={}", mn, arg_count);
                                    self.stdlib_mapping
                                        .find_by_name_and_params("String", mn, arg_count)
                                        .map(|(sig, mapping)| (sig.class, sig.method, mapping))
                                } else {
                                    self.get_stdlib_runtime_info(*field, object.ty)
                                }
                            } else {
                                self.get_stdlib_runtime_info(*field, object.ty)
                            }
                        };

                        if let Some((class_name, method_name, runtime_call)) = stdlib_info
                        {
                            let runtime_func = runtime_call.runtime_name;
                            eprintln!("DEBUG: [Extern method redirect] {}.{} -> {} (param_count={})", class_name, method_name, runtime_func, runtime_call.param_count);

                            // Lower the object (this will be the first parameter)
                            let obj_reg = self.lower_expression(object)?;

                            // Lower the arguments
                            let arg_regs: Vec<_> = std::iter::once(obj_reg)  // Add 'this' as first arg
                                .chain(args.iter().filter_map(|a| self.lower_expression(a)))
                                .collect();

                            // Determine parameter types from arguments
                            let mut param_types = vec![IrType::Ptr(Box::new(IrType::U8))]; // 'this' is always a pointer
                            for arg in args {
                                param_types.push(self.convert_type(arg.ty));
                            }

                            // Register the extern function
                            let extern_func_id = self.get_or_register_extern_function(
                                runtime_func,
                                param_types,
                                result_type.clone(),
                            );

                            return self.builder.build_call_direct(extern_func_id, arg_regs, result_type.clone());
                        }

                        // FALLBACK: For extern classes not in type_table (like rayzor.Bytes),
                        // try to extract class name from the MIR function's qualified_name
                        eprintln!("DEBUG: [FALLBACK check] func_id={:?}, in module={}", func_id, self.builder.module.functions.contains_key(&func_id));
                        if let Some(func) = self.builder.module.functions.get(&func_id) {
                            eprintln!("DEBUG: [FALLBACK] MIR function '{}' has qualified_name: {:?}", func.name, func.qualified_name);
                            if let Some(ref qn) = func.qualified_name {
                                // Pattern: "rayzor.Bytes.set" -> class="rayzor_Bytes", method="set"
                                let parts: Vec<&str> = qn.split('.').collect();
                                if parts.len() >= 2 {
                                    // Get method name (last part) and class name (all but last, joined with underscore)
                                    let mir_method_name = *parts.last().unwrap();
                                    let class_parts = &parts[..parts.len()-1];
                                    let qualified_class = class_parts.join("_");

                                    // Try to find in stdlib mapping
                                    if let Some((_sig, mapping)) = self.stdlib_mapping.find_by_name(&qualified_class, mir_method_name) {
                                        let runtime_func = mapping.runtime_name;
                                        eprintln!("DEBUG: [Extern method redirect via qualified_name] {}.{} -> {}", qualified_class, mir_method_name, runtime_func);

                                        // Lower the object (this will be the first parameter)
                                        let obj_reg = self.lower_expression(object)?;

                                        // Lower the arguments
                                        let arg_regs: Vec<_> = std::iter::once(obj_reg)
                                            .chain(args.iter().filter_map(|a| self.lower_expression(a)))
                                            .collect();

                                        // Determine parameter types from arguments
                                        let mut param_types = vec![IrType::Ptr(Box::new(IrType::U8))];
                                        for arg in args {
                                            param_types.push(self.convert_type(arg.ty));
                                        }

                                        // Register the extern function
                                        let extern_func_id = self.get_or_register_extern_function(
                                            runtime_func,
                                            param_types,
                                            result_type.clone(),
                                        );

                                        return self.builder.build_call_direct(extern_func_id, arg_regs, result_type.clone());
                                    }
                                }
                            }
                        }

                        // Regular method call (not extern or no runtime mapping)
                        // Lower the object (this will be the first parameter)
                        let obj_reg = self.lower_expression(object)?;

                        // Lower the arguments
                        let arg_regs: Vec<_> = std::iter::once(obj_reg)  // Add 'this' as first arg
                            .chain(args.iter().filter_map(|a| self.lower_expression(a)))
                            .collect();

                        // IMPORTANT: Use the function's actual return type, not expr.ty
                        // expr.ty can be incorrect (e.g., unresolved TypeParameter or wrong type)
                        let actual_return_type = if let Some(func) = self.builder.module.functions.get(&func_id) {
                            eprintln!("DEBUG: [Field method] Using actual return type {:?} for function {:?}", func.signature.return_type, func.name);
                            func.signature.return_type.clone()
                        } else {
                            eprintln!("DEBUG: [Field method] Function not found in module, using expr return type {:?}", result_type);
                            result_type.clone()
                        };

                        // eprintln!("DEBUG: Calling method with {} args (including this)", arg_regs.len());
                        return self
                            .builder
                            .build_call_direct(func_id, arg_regs, actual_return_type);
                    } else {
                        // Method not found by direct symbol lookup
                        // Check if this is a Dynamic method call or stdlib method
                        let object_type = object.ty;

                        // First check if the object is Dynamic - handle auto-unbox for method calls
                        let type_table = self.type_table.borrow();
                        if let Some(type_info) = type_table.get(object_type) {
                            if matches!(type_info.kind, TypeKind::Dynamic) {
                                // Dynamic method call - need to resolve method by name
                                drop(type_table);

                                let method_name = self.symbol_table.get_symbol(*field).map(|s| s.name);
                                if let Some(name) = method_name {
                                    // Look up function by name in function_map
                                    let mut found_func = None;
                                    for (sym, &func_id) in &self.function_map {
                                        if let Some(sym_info) = self.symbol_table.get_symbol(*sym) {
                                            if sym_info.name == name {
                                                found_func = Some(func_id);
                                                break;
                                            }
                                        }
                                    }

                                    if let Some(func_id) = found_func {
                                        // Lower the object and unbox it
                                        let obj_reg = self.lower_expression(object)?;

                                        // Unbox the Dynamic to get the actual object pointer
                                        let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                                        let unbox_func_id = self.get_or_register_extern_function(
                                            "haxe_unbox_reference_ptr",
                                            vec![ptr_u8.clone()],
                                            ptr_u8.clone(),
                                        );
                                        let unboxed_obj = self.builder.build_call_direct(
                                            unbox_func_id,
                                            vec![obj_reg],
                                            ptr_u8,
                                        )?;

                                        // Lower the arguments
                                        let arg_regs: Vec<_> = std::iter::once(unboxed_obj)  // Add unboxed 'this' as first arg
                                            .chain(args.iter().filter_map(|a| self.lower_expression(a)))
                                            .collect();

                                        // Get the function's actual return type
                                        let actual_return_type = if let Some(func) = self.builder.module.functions.get(&func_id) {
                                            func.signature.return_type.clone()
                                        } else {
                                            result_type.clone()
                                        };

                                        return self
                                            .builder
                                            .build_call_direct(func_id, arg_regs, actual_return_type);
                                    }
                                }
                            }
                        }

                        // Check if the object type is a String - handle String method calls
                        {
                            let type_table = self.type_table.borrow();
                            if let Some(type_info) = type_table.get(object_type) {
                                eprintln!("DEBUG: [CHECK STRING] object_type={:?}, kind={:?}", object_type, type_info.kind);
                                if matches!(type_info.kind, TypeKind::String) {
                                    // Get the method name from the field symbol
                                    let method_name = self.symbol_table.get_symbol(*field)
                                        .and_then(|s| self.string_interner.get(s.name));

                                    if let Some(method_name) = method_name {
                                        // For String methods with optional params (indexOf, lastIndexOf),
                                        // look up the mapping by param count to get the right variant
                                        let arg_count = args.len();
                                        let mapping_opt = self.stdlib_mapping
                                            .find_by_name_and_params("String", method_name, arg_count)
                                            .or_else(|| self.stdlib_mapping.find_by_name("String", method_name));

                                        // Look up the runtime function for this String method
                                        if let Some((_sig, mapping)) = mapping_opt {
                                            let runtime_func = mapping.runtime_name;

                                            eprintln!("DEBUG: [STRING METHOD] Found String.{} with {} args -> {}",
                                                     method_name, arg_count, runtime_func);

                                            drop(type_table);

                                            // Lower the object (the String pointer)
                                            let obj_reg = self.lower_expression(object)?;

                                            // Lower the method arguments
                                            let method_arg_regs: Vec<_> = args
                                                .iter()
                                                .filter_map(|a| self.lower_expression(a))
                                                .collect();

                                            // Build param types: string_ptr, ...args
                                            let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
                                            let mut param_types = vec![string_ptr_ty.clone()];
                                            for arg in &method_arg_regs {
                                                // Haxe Int is i32, default to I32 for integer args
                                                let arg_ty = self.builder.get_register_type(*arg)
                                                    .unwrap_or(IrType::I32);
                                                param_types.push(arg_ty);
                                            }

                                            // Determine return type - for String methods returning String,
                                            // they return a pointer to HaxeString
                                            let return_type = if result_type == IrType::String {
                                                string_ptr_ty.clone()
                                            } else {
                                                result_type.clone()
                                            };

                                            let runtime_func_id = self.get_or_register_extern_function(
                                                runtime_func,
                                                param_types,
                                                return_type.clone(),
                                            );

                                            let mut call_args = vec![obj_reg];
                                            call_args.extend(method_arg_regs);

                                            return self.builder.build_call_direct(
                                                runtime_func_id,
                                                call_args,
                                                return_type,
                                            );
                                        }
                                    }
                                }
                            }
                        }

                        // Check if the object type is a rayzor stdlib class
                        let type_table = self.type_table.borrow();
                        if let Some(type_info) = type_table.get(object_type) {
                            if let TypeKind::Class { symbol_id, .. } = &type_info.kind {
                                if let Some(class_symbol) = self.symbol_table.get_symbol(*symbol_id)
                                {
                                    if let Some(class_name) =
                                        self.string_interner.get(class_symbol.name)
                                    {
                                        // eprintln!(
                                        //     "DEBUG: Object is class '{}', qualified_name={:?}",
                                        //     class_name,
                                        //     class_symbol
                                        //         .qualified_name
                                        //         .and_then(|qn| self.string_interner.get(qn))
                                        // );

                                        // Check if it's a rayzor stdlib class by using qualified name
                                        let qualified_name_opt = class_symbol
                                            .qualified_name
                                            .and_then(|qn| self.string_interner.get(qn));

                                        if let Some(qualified_name) = qualified_name_opt {
                                            // Try to get the method name from the field symbol
                                            let method_name = if let Some(field_sym) =
                                                self.symbol_table.get_symbol(*field)
                                            {
                                                self.string_interner.get(field_sym.name)
                                            } else {
                                                None
                                            };

                                            if let Some(method_name) = method_name {
                                                // eprintln!("DEBUG: Checking stdlib class '{}' method '{}' with qualified name '{}'",
                                                //          class_name, method_name, qualified_name);

                                                // Use the proper mapping function that handles all methods
                                                if let Some(runtime_func) = self
                                                    .get_static_stdlib_runtime_func(
                                                        qualified_name,
                                                        method_name,
                                                    )
                                                {
                                                    // println!("✅ Generating runtime call to {} for {}.{}", runtime_func, class_name, method_name);
                                                    drop(type_table);

                                                    // Lower all arguments (don't include object for static methods like spawn)
                                                    let arg_regs: Vec<_> = args
                                                        .iter()
                                                        .filter_map(|a| self.lower_expression(a))
                                                        .collect();

                                                    // Get or register the extern runtime function
                                                    // Infer param types from actual arguments
                                                    let param_types: Vec<IrType> = arg_regs.iter()
                                                        .map(|reg| self.builder.get_register_type(*reg).unwrap_or(IrType::Any))
                                                        .collect();
                                                    let runtime_func_id = self
                                                        .get_or_register_extern_function(
                                                            &runtime_func,
                                                            param_types,
                                                            result_type.clone(),
                                                        );

                                                    // Generate the call to the runtime function
                                                    return self.builder.build_call_direct(
                                                        runtime_func_id,
                                                        arg_regs,
                                                        result_type,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        drop(type_table);

                        // eprintln!("WARNING: Method {:?} not found in function_map", field);
                    }
                }

                // Check if callee is a direct function reference
                if let HirExprKind::Variable { symbol, .. } = &callee.kind {
                    let symbol_name = self
                        .symbol_table
                        .get_symbol(*symbol)
                        .and_then(|s| self.string_interner.get(s.name))
                        .unwrap_or("<unknown>");
                    eprintln!(
                        "DEBUG: Callee is Variable, symbol={:?} ({}), is_method={}, args.len()={}",
                        symbol,
                        symbol_name,
                        is_method,
                        args.len()
                    );

                    // SPECIAL CASE: Handle global trace() function
                    // Route to type-specific trace functions based on argument type
                    if symbol_name == "trace" && args.len() == 1 {
                        let arg = &args[0];

                        // Check if arg is a class or enum type
                        // For classes: try to call toString() method
                        // For enums: for now, fall through to traceAny (enum toString not yet implemented)
                        let type_table = self.type_table.borrow();
                        let type_kind = type_table.get(arg.ty).map(|ti| ti.kind.clone());
                        drop(type_table);

                        eprintln!("DEBUG: [TRACE ARG TYPE] arg.ty={:?}, type_kind={:?}", arg.ty, type_kind);

                        let class_info = if let Some(crate::tast::core::TypeKind::Class { symbol_id, .. }) = &type_kind {
                            // Get class name
                            self.symbol_table.get_symbol(*symbol_id)
                                .and_then(|s| self.string_interner.get(s.name))
                                .map(|name| name.to_string())
                        } else {
                            None
                        };

                        // Check if the trace argument is an enum variant expression (e.g., Color.Red)
                        // If so, we can print the variant name directly
                        if let HirExprKind::Field { object, field } = &arg.kind {
                            if let HirExprKind::Variable { symbol: enum_symbol, .. } = &object.kind {
                                if let Some(enum_sym) = self.symbol_table.get_symbol(*enum_symbol) {
                                    use crate::tast::SymbolKind;
                                    if enum_sym.kind == SymbolKind::Enum {
                                        // Get the variant name
                                        let field_sym = self.symbol_table.get_symbol(*field);
                                        if let Some(variant_name) = field_sym.and_then(|s| self.string_interner.get(s.name)) {
                                            // Create a string constant with the variant name
                                            // IrValue::String will be converted by Cranelift to call haxe_string_literal
                                            // which returns a *mut HaxeString pointer
                                            let variant_name_str = variant_name.to_string();
                                            let string_ptr = self.builder.build_const(IrValue::String(variant_name_str))?;

                                            // Get or create the string trace function
                                            let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
                                            let string_trace_id = self.get_or_register_extern_function(
                                                "haxe_trace_string_struct",
                                                vec![string_ptr_ty],
                                                IrType::Void,
                                            );

                                            // Trace the string
                                            return self.builder.build_call_direct(
                                                string_trace_id,
                                                vec![string_ptr],
                                                IrType::Void,
                                            );
                                        }
                                    }
                                }
                            }
                        }

                        // Check if it's an enum variable - print discriminant for now
                        // Full variant name lookup for variables would require runtime RTTI
                        // Direct enum variant expressions (Color.Red) are handled above

                        // If this is a class type, try to call toString() on it
                        if let Some(class_name) = class_info {
                            eprintln!("DEBUG: [TRACE] Class '{}' detected, calling toString()", class_name);
                            // Lower the object first
                            let obj_reg = self.lower_expression(arg)?;

                            // Try to find a toString function - methods are named without class prefix in MIR
                            let tostring_func_name = "toString";

                            // toString() returns *String, takes this pointer
                            let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
                            let _this_ty = IrType::Ptr(Box::new(IrType::Void));

                            // Try to find the toString function in the current module
                            // Look through the module's functions to find one matching the name
                            let tostring_id = self.builder.module.functions.iter()
                                .find(|(_, func)| func.name == tostring_func_name)
                                .map(|(id, _)| *id);

                            eprintln!("DEBUG: [TRACE] Looking for '{}', found: {:?}, module has {} functions",
                                tostring_func_name, tostring_id, self.builder.module.functions.len());

                            // If not found, we'll fall through to traceAny
                            if let Some(tostring_id) = tostring_id {
                                // Call toString
                                let string_reg = self.builder.build_call_direct(
                                    tostring_id,
                                    vec![obj_reg],
                                    string_ptr_ty.clone(),
                                )?;

                                // Now trace the string result
                                let string_trace_id = self.get_or_register_extern_function(
                                    "haxe_trace_string_struct",
                                    vec![string_ptr_ty],
                                    IrType::Void,
                                );
                                return self.builder.build_call_direct(
                                    string_trace_id,
                                    vec![string_reg],
                                    IrType::Void,
                                );
                            } else {
                                eprintln!("DEBUG: [TRACE] toString function '{}' not found, falling back to traceAny", tostring_func_name);
                            }
                        }

                        // Lower the argument first to get the actual MIR register
                        // Check if this is a field access
                        let is_field = matches!(&arg.kind, HirExprKind::Field { .. });
                        if is_field {
                            if let HirExprKind::Field { object, field } = &arg.kind {
                                let field_sym = self.symbol_table.get_symbol(*field);
                                let field_name = field_sym.and_then(|s| self.string_interner.get(s.name)).unwrap_or("<unknown>");
                                eprintln!("DEBUG: [TRACE] Argument is Field access: field={}", field_name);

                                // Check what the object is
                                if let HirExprKind::Variable { symbol, .. } = &object.kind {
                                    let var_sym = self.symbol_table.get_symbol(*symbol);
                                    let var_name = var_sym.and_then(|s| self.string_interner.get(s.name)).unwrap_or("<unknown>");
                                    eprintln!("DEBUG: [TRACE] Field object is Variable: {}", var_name);
                                }
                            }
                        }
                        eprintln!("DEBUG: [TRACE] Lowering trace argument, expr kind: {:?}", std::mem::discriminant(&arg.kind));
                        let arg_reg = self.lower_expression(arg)?;
                        eprintln!("DEBUG: [TRACE] After lowering, arg_reg={}, checking type...", arg_reg);
                        if let Some(ty) = self.builder.get_register_type(arg_reg) {
                            eprintln!("DEBUG: [TRACE] arg_reg type from builder: {:?}", ty);
                        }

                        // Check if the HIR type is an enum
                        // Also check if the arg is a variable and look up its declared type
                        // (trace() takes Dynamic, so arg.ty might be Dynamic even if the variable is an enum)
                        let type_table = self.type_table.borrow();
                        let mut hir_type_kind = type_table.get(arg.ty).map(|ti| ti.kind.clone());

                        // If arg.ty is Dynamic but the argument is a variable, look up the variable's declared type
                        // This is needed because trace() accepts Dynamic, so the expression type might be Dynamic
                        // even when the underlying variable has a more specific type (like an enum)
                        if matches!(&hir_type_kind, Some(crate::tast::core::TypeKind::Dynamic) | None) {
                            if let HirExprKind::Variable { symbol, .. } = &arg.kind {
                                if let Some(sym) = self.symbol_table.get_symbol(*symbol) {
                                    let var_type_kind = type_table.get(sym.type_id).map(|ti| ti.kind.clone());
                                    if var_type_kind.is_some() {
                                        hir_type_kind = var_type_kind;
                                    }
                                }
                            }
                        }
                        drop(type_table);

                        // For enum variables, the discriminant will be printed as an integer
                        // Direct enum variant expressions (Color.Red) are handled above and print variant names
                        // TODO: Implement RTTI registration to print variant names for enum variables

                        // Get the actual MIR type from the register (not the HIR type)
                        // This is important because HIR types may be vague (Ptr(Void)) but
                        // MIR registers have the actual type (String, etc.)
                        let actual_reg_type = self.builder.get_register_type(arg_reg)
                            .unwrap_or_else(|| self.convert_type(arg.ty));

                        let mut arg_type = actual_reg_type.clone();

                        // If the MIR type is Ptr(Void) but we have better type info from the symbol,
                        // use the symbol's type instead. This handles cases like trace(t) where t is
                        // a float from Sys.time() but the trace() signature says Dynamic.
                        if matches!(arg_type, IrType::Ptr(_)) {
                            if let Some(ref type_kind) = hir_type_kind {
                                let better_type = match type_kind {
                                    crate::tast::core::TypeKind::Float => Some(IrType::F64),
                                    crate::tast::core::TypeKind::Int => Some(IrType::I64),
                                    crate::tast::core::TypeKind::Bool => Some(IrType::Bool),
                                    crate::tast::core::TypeKind::String => Some(IrType::String),
                                    _ => None,
                                };
                                if let Some(better) = better_type {
                                    arg_type = better;
                                }
                            }
                        }

                        // Determine which trace function to call based on type
                        let trace_method = {
                            match &arg_type {
                                IrType::I32 | IrType::I64 => "traceInt",
                                IrType::F32 | IrType::F64 => "traceFloat",
                                IrType::Bool => "traceBool",
                                IrType::String => "traceString", // String is ptr+len struct
                                // Also handle Ptr(String) - returned by String methods like toUpperCase()
                                IrType::Ptr(inner) if matches!(inner.as_ref(), IrType::String) => "traceString",
                                _ => "traceAny", // Fallback for Dynamic or unknown types
                            }
                        };

                        // Debug: Print trace method selection
                        eprintln!("[DEBUG trace] arg_reg={}, arg_type={:?}, trace_method={}",
                            arg_reg, arg_type, trace_method);

                        // Build the qualified name for the trace function
                        let trace_func_name = format!("rayzor.Trace.{}", trace_method);

                        // Look up the runtime function name
                        // For now, manually map to the runtime function
                        let runtime_func = match trace_method {
                            "traceInt" => "haxe_trace_int",
                            "traceFloat" => "haxe_trace_float",
                            "traceBool" => "haxe_trace_bool",
                            "traceString" => "haxe_trace_string",
                            "traceAny" => "haxe_trace_any",
                            _ => "haxe_trace_any",
                        };

                        // Special handling for String: use haxe_trace_string_struct that takes a pointer
                        if trace_method == "traceString" {
                            // String is represented as a pointer to HaxeString struct
                            let param_types = vec![IrType::Ptr(Box::new(IrType::String))];
                            let string_trace_id = self.get_or_register_extern_function(
                                "haxe_trace_string_struct",
                                param_types,
                                IrType::Void,
                            );
                            return self.builder.build_call_direct(
                                string_trace_id,
                                vec![arg_reg],
                                IrType::Void,
                            );
                        }

                        // Get or register the extern runtime function
                        // Note: Runtime trace functions expect specific types:
                        // - haxe_trace_int expects i64
                        // - haxe_trace_float expects f64
                        // We need to cast arguments to match
                        // Note: We don't need to cast arguments here - the Cranelift backend
                        // handles signature-aware type conversion automatically (see cranelift_backend.rs:1487-1491)
                        // It will insert sextend for i32->i64, fcvt for f32->f64, etc.
                        let param_types = match trace_method {
                            "traceInt" => vec![IrType::I64],
                            "traceFloat" => vec![IrType::F64],
                            "traceBool" => vec![IrType::Bool],
                            _ => vec![arg_type.clone()],
                        };

                        let final_arg_reg = arg_reg;

                        let runtime_func_id = self.get_or_register_extern_function(
                            runtime_func,
                            param_types,
                            IrType::Void,
                        );

                        // Generate the call
                        return self.builder.build_call_direct(
                            runtime_func_id,
                            vec![final_arg_reg],
                            IrType::Void,
                        );
                    }

                    // SPECIAL CASE: Handle Std.string() function
                    // Route to type-specific string conversion functions based on argument type
                    // Note: Std.string() comes as a static method call with 2 args (Std class + actual arg)
                    if symbol_name == "string" && (args.len() == 1 || (args.len() == 2 && *is_method)) {
                        eprintln!("DEBUG: [STD.STRING CHECK] Found 'string' call, is_method={}, args.len()={}", is_method, args.len());

                        // For static method calls, the actual argument is the second one (skip Std class)
                        let arg = if *is_method && args.len() == 2 { &args[1] } else { &args[0] };
                        let arg_type = self.convert_type(arg.ty);

                        // Determine which MIR wrapper function to call based on type
                        // These wrappers call the extern runtime functions
                        let mir_wrapper = match arg_type {
                            IrType::I32 | IrType::I64 => "int_to_string",
                            IrType::F32 | IrType::F64 => "float_to_string",
                            IrType::Bool => "bool_to_string",
                            IrType::String => "string_to_string",
                            // TODO: Handle null explicitly, handle Dynamic with runtime dispatch
                            _ => "int_to_string", // Fallback - will need Dynamic support later
                        };

                        eprintln!("DEBUG: [STD.STRING] Routing Std.string() call to {} for type {:?}", mir_wrapper, arg_type);

                        // Lower the argument
                        let arg_reg = self.lower_expression(arg)?;

                        // Get or register the MIR wrapper function
                        // These return String (a struct with ptr + len)
                        let param_types = vec![arg_type.clone()];
                        let return_type = IrType::String; // String is represented as ptr+len
                        let mir_wrapper_id = self.get_or_register_extern_function(
                            mir_wrapper,
                            param_types,
                            return_type.clone(),
                        );

                        // Generate the call to MIR wrapper
                        return self.builder.build_call_direct(
                            mir_wrapper_id,
                            vec![arg_reg],
                            return_type,
                        );
                    }

                    // For instance method calls, check if this is a stdlib method or Dynamic method
                    // Note: Static methods like Thread.spawn() can also come through here with is_method=true
                    if *is_method && !args.is_empty() {
                        // The first arg is the receiver for instance method calls
                        let receiver_type = args[0].ty;

                        // Debug: print receiver type info
                        {
                            let type_table = self.type_table.borrow();
                            if let Some(type_info) = type_table.get(receiver_type) {
                                eprintln!("DEBUG: [METHOD CALL] receiver_type={:?}, kind={:?}", receiver_type, type_info.kind);
                            } else {
                                // Print method name for calls with invalid receiver type
                                let method_name = self.symbol_table.get_symbol(*symbol).map(|s| self.string_interner.get(s.name));
                                eprintln!("DEBUG: [METHOD CALL] receiver_type={:?} NOT IN TYPE TABLE, method={:?}", receiver_type, method_name);
                            }
                        }

                        // SPECIAL CASE: Handle Dynamic method calls
                        // When receiver is Dynamic, we need to unbox and resolve method by name
                        {
                            let type_table = self.type_table.borrow();
                            if let Some(type_info) = type_table.get(receiver_type) {
                                if matches!(type_info.kind, TypeKind::Dynamic) {
                                    drop(type_table);

                                    // Look up method by name in function_map
                                    let method_name = self.symbol_table.get_symbol(*symbol).map(|s| s.name);
                                    if let Some(name) = method_name {
                                        let mut found_func = None;
                                        for (sym, &func_id) in &self.function_map {
                                            if let Some(sym_info) = self.symbol_table.get_symbol(*sym) {
                                                if sym_info.name == name {
                                                    found_func = Some(func_id);
                                                    break;
                                                }
                                            }
                                        }

                                        if let Some(func_id) = found_func {
                                            // Lower the receiver
                                            let receiver_reg = self.lower_expression(&args[0])?;

                                            // Check if the receiver was boxed by examining its MIR register type.
                                            // Boxing creates a Ptr(U8) value. If the receiver has a different
                                            // pointer type (like Ptr(Void) from a stdlib function return),
                                            // it wasn't boxed and shouldn't be unboxed.
                                            let receiver_mir_type = self.builder.get_register_type(receiver_reg);
                                            let should_unbox = receiver_mir_type.as_ref()
                                                .map(|t| {
                                                    // A boxed value has type Ptr(U8) from box_* functions
                                                    // Unboxed stdlib returns typically have Ptr(Void)
                                                    matches!(t, IrType::Ptr(inner) if matches!(**inner, IrType::U8))
                                                })
                                                .unwrap_or(true); // Assume boxed if type unknown

                                            eprintln!("DEBUG: [DYNAMIC METHOD] receiver_mir_type: {:?}, should_unbox: {}",
                                                     receiver_mir_type, should_unbox);

                                            let actual_receiver = if should_unbox {
                                                // Unbox the Dynamic to get the actual object pointer
                                                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                                                let unbox_func_id = self.get_or_register_extern_function(
                                                    "haxe_unbox_reference_ptr",
                                                    vec![ptr_u8.clone()],
                                                    ptr_u8.clone(),
                                                );
                                                self.builder.build_call_direct(
                                                    unbox_func_id,
                                                    vec![receiver_reg],
                                                    ptr_u8,
                                                )?
                                            } else {
                                                eprintln!("DEBUG: [DYNAMIC METHOD] Skipping unbox - stdlib container method");
                                                receiver_reg
                                            };

                                            // Lower the rest of arguments (skip receiver at index 0)
                                            let arg_regs: Vec<_> = std::iter::once(actual_receiver)
                                                .chain(args[1..].iter().filter_map(|a| self.lower_expression(a)))
                                                .collect();

                                            // Get the function's actual return type
                                            let actual_return_type = if let Some(func) = self.builder.module.functions.get(&func_id) {
                                                func.signature.return_type.clone()
                                            } else {
                                                result_type.clone()
                                            };

                                            return self.builder.build_call_direct(func_id, arg_regs, actual_return_type);
                                        }
                                    }
                                }
                            }
                        }

                        // SPECIAL CASE: Handle MutexGuard method calls (Deref-like semantics)
                        // When calling a method on MutexGuard<T> that doesn't exist on MutexGuard,
                        // we need to auto-call .get() to get the inner T and then call the method on T
                        {
                            let type_table = self.type_table.borrow();
                            if let Some(type_info) = type_table.get(receiver_type) {
                                // Check if receiver is MutexGuard class
                                if let TypeKind::Class { symbol_id, .. } = &type_info.kind {
                                    // Get class name from symbol
                                    let is_mutex_guard = self.symbol_table.get_symbol(*symbol_id)
                                        .and_then(|s| self.string_interner.get(s.name))
                                        .map(|n| n == "MutexGuard")
                                        .unwrap_or(false);

                                    if is_mutex_guard {
                                        // Get the method name being called
                                        let method_name = self.symbol_table.get_symbol(*symbol)
                                            .and_then(|s| self.string_interner.get(s.name))
                                            .map(|s| s.to_string());

                                        // Check if this is a MutexGuard method (get, unlock) or an inner type method
                                        let is_mutex_guard_method = method_name.as_ref()
                                            .map(|n| n == "get" || n == "unlock")
                                            .unwrap_or(false);

                                        if !is_mutex_guard_method {
                                            // Not a MutexGuard method - need to call .get() first
                                            eprintln!("DEBUG: [MUTEX_GUARD DEREF] Calling .get() before method '{}' on MutexGuard",
                                                     method_name.as_deref().unwrap_or("?"));

                                            drop(type_table);

                                            // Lower the MutexGuard receiver
                                            let guard_reg = self.lower_expression(&args[0])?;

                                            // Call MutexGuard_get to get the inner value
                                            // First find the MutexGuard_get function
                                            let mut guard_get_func = None;
                                            for (sym, &func_id) in &self.function_map {
                                                if let Some(sym_info) = self.symbol_table.get_symbol(*sym) {
                                                    if let Some(name) = self.string_interner.get(sym_info.name) {
                                                        if name == "MutexGuard_get" || name == "get" {
                                                            // Check if this is for MutexGuard
                                                            guard_get_func = Some(func_id);
                                                            break;
                                                        }
                                                    }
                                                }
                                            }

                                            // Also try stdlib mapping
                                            if guard_get_func.is_none() {
                                                if let Some((_sig, mapping)) = self.stdlib_mapping.find_by_name("MutexGuard", "get") {
                                                    let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                                                    guard_get_func = Some(self.get_or_register_extern_function(
                                                        mapping.runtime_name,
                                                        vec![ptr_u8.clone()],
                                                        ptr_u8,
                                                    ));
                                                }
                                            }

                                            if let Some(get_func_id) = guard_get_func {
                                                // Call .get() to get the inner value
                                                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                                                let inner_value = self.builder.build_call_direct(
                                                    get_func_id,
                                                    vec![guard_reg],
                                                    ptr_u8,
                                                )?;

                                                // Now call the actual method on the inner value
                                                // Lower the rest of arguments (skip receiver at index 0)
                                                let arg_regs: Vec<_> = std::iter::once(inner_value)
                                                    .chain(args[1..].iter().filter_map(|a| self.lower_expression(a)))
                                                    .collect();

                                                // Get the method function ID (local or external)
                                                if let Some(func_id) = self.get_function_id(symbol) {
                                                    let actual_return_type = if let Some(func) = self.builder.module.functions.get(&func_id) {
                                                        func.signature.return_type.clone()
                                                    } else {
                                                        result_type.clone()
                                                    };

                                                    return self.builder.build_call_direct(func_id, arg_regs, actual_return_type);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // SPECIAL CASE: Handle String method calls
                        // String methods like toUpperCase(), toLowerCase() need special routing
                        {
                            let type_table = self.type_table.borrow();
                            if let Some(type_info) = type_table.get(receiver_type) {
                                if matches!(type_info.kind, TypeKind::String) {
                                    // Get the method name
                                    let method_name = self.symbol_table.get_symbol(*symbol)
                                        .and_then(|s| self.string_interner.get(s.name));

                                    if let Some(method_name) = method_name {
                                        // For String methods with optional params (indexOf, lastIndexOf),
                                        // look up the mapping by param count (args[1..] is the method args)
                                        let arg_count = args.len().saturating_sub(1); // Exclude receiver
                                        eprintln!("DEBUG: [String arg_count] method={}, args.len()={}, arg_count={}", method_name, args.len(), arg_count);
                                        let mapping_opt = self.stdlib_mapping
                                            .find_by_name_and_params("String", method_name, arg_count)
                                            .or_else(|| self.stdlib_mapping.find_by_name("String", method_name));

                                        // Look up the runtime function for this String method
                                        if let Some((_sig, mapping)) = mapping_opt {
                                            let runtime_func = mapping.runtime_name;

                                            eprintln!("DEBUG: [STRING METHOD] Variable path - Found String.{} -> {}",
                                                     method_name, runtime_func);

                                            drop(type_table);

                                            // Lower the receiver (the String pointer, which is args[0])
                                            let obj_reg = self.lower_expression(&args[0])?;

                                            // Lower the method arguments (skip the receiver at index 0)
                                            let method_arg_regs: Vec<_> = args[1..]
                                                .iter()
                                                .filter_map(|a| self.lower_expression(a))
                                                .collect();

                                            // Build param types: string_ptr, ...args
                                            // Use TAST expression types for accuracy
                                            let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
                                            let mut param_types = vec![string_ptr_ty.clone()];
                                            for (i, arg) in args[1..].iter().enumerate() {
                                                // Get type from TAST expression
                                                let arg_ty = self.convert_type(arg.ty);
                                                // For String args, convert to Ptr(String)
                                                let param_ty = if arg_ty == IrType::String {
                                                    string_ptr_ty.clone()
                                                } else {
                                                    // Fall back to register type if TAST gives us something unusual
                                                    if matches!(arg_ty, IrType::Any | IrType::Void) {
                                                        method_arg_regs.get(i)
                                                            .and_then(|r| self.builder.get_register_type(*r))
                                                            .unwrap_or(IrType::I32)
                                                    } else {
                                                        arg_ty
                                                    }
                                                };
                                                param_types.push(param_ty);
                                            }

                                            // Determine return type based on method
                                            // Methods that return Int (i32): length, charCodeAt, indexOf, lastIndexOf
                                            // Methods that return String (Ptr<String>): charAt, substr, substring, toLowerCase, toUpperCase, toString
                                            // Methods that return Array (Ptr<Void>): split
                                            let return_type = match method_name {
                                                "length" | "charCodeAt" | "indexOf" | "lastIndexOf" => IrType::I32,
                                                "split" => {
                                                    // split returns Array<String>, convert to Ptr(Void)
                                                    IrType::Ptr(Box::new(IrType::Void))
                                                }
                                                _ => string_ptr_ty.clone(),
                                            };

                                            let runtime_func_id = self.get_or_register_extern_function(
                                                runtime_func,
                                                param_types,
                                                return_type.clone(),
                                            );

                                            let mut call_args = vec![obj_reg];
                                            call_args.extend(method_arg_regs);

                                            return self.builder.build_call_direct(
                                                runtime_func_id,
                                                call_args,
                                                return_type,
                                            );
                                        }
                                    }
                                }
                            }
                        }

                        // eprintln!(
                        //     "DEBUG: Instance method path - receiver_type={:?}",
                        //     receiver_type
                        // );

                        // PRIORITY CHECK: For extern generic classes like Vec<T>, the receiver type
                        // may be TypeId::MAX (invalid). In this case, try to use the tracked
                        // monomorphized class from variable assignment.
                        if receiver_type == TypeId::from_raw(u32::MAX) {
                            eprintln!("DEBUG: [MONO VAR CHECK] receiver_type is MAX, checking monomorphized_var_types ({} entries)",
                                     self.monomorphized_var_types.len());

                            // Try to extract the SymbolId from the receiver expression
                            // The receiver (args[0]) should be a variable reference like HirExprKind::Variable
                            let receiver_symbol = match &args[0].kind {
                                HirExprKind::Variable { symbol, .. } => Some(*symbol),
                                HirExprKind::Field { field, .. } => Some(*field),
                                _ => None,
                            };
                            eprintln!("DEBUG: [MONO VAR CHECK] Receiver expression symbol: {:?}", receiver_symbol);

                            if let Some(var_symbol) = receiver_symbol {
                                // Check if this variable has a tracked monomorphized class
                                if let Some(mono_class) = self.monomorphized_var_types.get(&var_symbol).cloned() {
                                    // Get the method name
                                    if let Some(method_sym) = self.symbol_table.get_symbol(*symbol) {
                                        if let Some(method_name) = self.string_interner.get(method_sym.name) {
                                            eprintln!("DEBUG: [MONO VAR DISPATCH] Found tracked class '{}' for variable {:?}, method '{}'",
                                                     mono_class, var_symbol, method_name);

                                            // Build the MIR wrapper function name: VecI32_push, VecF64_get, etc.
                                            let mir_func_name = format!("{}_{}", mono_class, method_name);

                                            // Get the signature from get_stdlib_mir_wrapper_signature
                                            if let Some((mir_param_types, mir_return_type)) = self.get_stdlib_mir_wrapper_signature(&mir_func_name) {
                                                eprintln!("DEBUG: [MONO VAR DISPATCH] Using MIR wrapper: {}", mir_func_name);

                                                // Lower all arguments (including receiver)
                                                let mut arg_regs = Vec::new();
                                                for arg in args {
                                                    if let Some(reg) = self.lower_expression(arg) {
                                                        arg_regs.push(reg);
                                                    }
                                                }

                                                // Register forward reference
                                                let mir_func_id = self.register_stdlib_mir_forward_ref(
                                                    &mir_func_name,
                                                    mir_param_types.clone(),
                                                    mir_return_type.clone(),
                                                );

                                                eprintln!("DEBUG: [MONO VAR DISPATCH] Registered forward ref to {} with ID {:?}", mir_func_name, mir_func_id);

                                                // Generate the call
                                                let result = self.builder.build_call_direct(
                                                    mir_func_id,
                                                    arg_regs,
                                                    mir_return_type,
                                                );
                                                eprintln!("DEBUG: [MONO VAR DISPATCH] Generated call, result: {:?}", result);
                                                return result;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // GUARD: Skip instance method handling if receiver is a Class type itself
                        // This can happen when static method calls come through with is_method=true
                        // e.g., Thread.spawn(closure) might be seen as Thread(receiver).spawn(closure)
                        let receiver_is_class_type = {
                            let type_table = self.type_table.borrow();
                            type_table.get(receiver_type)
                                .map(|ti| {
                                    // Check if the type is a class AND matches one of our MIR wrapper classes
                                    if let crate::tast::core::TypeKind::Class { symbol_id, .. } = &ti.kind {
                                        self.symbol_table.get_symbol(*symbol_id)
                                            .and_then(|s| self.string_interner.get(s.name))
                                            .map(|name| {
                                                // Check if it's Thread/Channel/Mutex/Arc/String
                                                let is_mir_wrapper = ["Thread", "Channel", "Mutex", "MutexGuard", "Arc", "String"].contains(&name);
                                                if is_mir_wrapper {
                                                    eprintln!("DEBUG: [GUARD] Receiver type is {} class, skipping instance method path", name);
                                                }
                                                is_mir_wrapper
                                            })
                                            .unwrap_or(false)
                                    } else {
                                        false
                                    }
                                })
                                .unwrap_or(false)
                        };

                        // Try the receiver type path first (for true instance methods)
                        // Skip if receiver is a MIR wrapper class type (those are static methods)
                        if !receiver_is_class_type {
                        if let Some((class_name, method_name, runtime_call)) =
                            self.get_stdlib_runtime_info(*symbol, receiver_type)
                        {
                            let runtime_func = runtime_call.runtime_name;
                            let ptr_conversion_mask = runtime_call.params_need_ptr_conversion;
                            let raw_value_mask = runtime_call.raw_value_params;
                            let returns_raw_value = runtime_call.returns_raw_value;
                            let extend_i64_mask = runtime_call.extend_to_i64_params;
                            let needs_out_param = runtime_call.needs_out_param;

                            // SPECIAL CASE: Instance methods that need out parameter (like Array.slice, String.split)
                            // These have void return but write result to first out parameter
                            // Generate inline wrapper: allocate + call runtime + return pointer
                            if needs_out_param {
                                eprintln!("DEBUG: [OUT PARAM] Instance method {}.{} needs out param inline wrapper", class_name, method_name);

                                // Lower all arguments (receiver + method args)
                                let mut call_arg_regs = Vec::new();
                                for arg in args {
                                    if let Some(reg) = self.lower_expression(arg) {
                                        call_arg_regs.push(reg);
                                    }
                                }

                                // Allocate space for the result object
                                // For arrays/strings, allocate an opaque pointer-sized value
                                let out_ptr_ty = IrType::Ptr(Box::new(IrType::Void));
                                let out_ptr = self.builder.build_alloc(out_ptr_ty.clone(), None)?;

                                // Register the extern runtime function
                                // Signature: void runtime_func(out: *Ptr(Void), receiver: Ptr(Void), ...params)
                                let mut extern_param_types = vec![out_ptr_ty.clone()]; // out parameter
                                for arg in args {
                                    extern_param_types.push(self.convert_type(arg.ty));
                                }

                                let extern_func_id = self.get_or_register_extern_function(
                                    runtime_func,
                                    extern_param_types,
                                    IrType::Void,
                                );

                                // Call runtime function: runtime_func(out_ptr, receiver, ...args)
                                let mut runtime_args = vec![out_ptr];
                                runtime_args.extend(call_arg_regs);

                                self.builder.build_call_direct(
                                    extern_func_id,
                                    runtime_args,
                                    IrType::Void,
                                );

                                // Load the result pointer from the out parameter
                                let result_ptr = self.builder.build_load(out_ptr, out_ptr_ty)?;

                                eprintln!("DEBUG: [OUT PARAM] Generated inline wrapper for {}, result_ptr: {:?}", runtime_func, result_ptr);

                                return Some(result_ptr);
                            }

                            // SPECIAL CASE: Check if this is a stdlib MIR function
                            // For these classes, we have MIR wrapper functions that forward to extern runtime functions.
                            // The wrappers take care of calling convention differences.
                            if self.stdlib_mapping.is_mir_wrapper_class(class_name) {
                                // Use PascalCase class name to match stdlib MIR wrapper naming convention
                                // e.g., VecI32_first, Thread_join, Channel_send
                                let mir_func_name = format!("{}_{}", class_name, method_name);
                                eprintln!("DEBUG: [STDLIB MIR] Detected stdlib MIR function (instance): {}", mir_func_name);

                                // Lower all arguments and collect their types
                                let mut arg_regs = Vec::new();
                                let mut param_types = Vec::new();
                                for arg in args {
                                    if let Some(reg) = self.lower_expression(arg) {
                                        arg_regs.push(reg);
                                        param_types.push(self.convert_type(arg.ty));
                                    }
                                }

                                // SPECIAL: For generic methods that return T (like Thread<T>.join() -> T),
                                // we need to resolve the type parameter from the receiver's generic arguments
                                let resolved_result_type = if result_type == IrType::Any {
                                    // Check if the receiver is a generic class with type parameters
                                    let type_table = self.type_table.borrow();
                                    if let Some(receiver_info) = type_table.get(receiver_type) {
                                        if let crate::tast::TypeKind::Class { type_args, .. } = &receiver_info.kind {
                                            // For Thread<T>.join(), type_args[0] is T
                                            if !type_args.is_empty() {
                                                let concrete_type = self.convert_type(type_args[0]);
                                                eprintln!("DEBUG: [GENERIC RESOLVE] Resolved return type from {:?} to {:?}", result_type, concrete_type);
                                                concrete_type
                                            } else {
                                                result_type.clone()
                                            }
                                        } else {
                                            result_type.clone()
                                        }
                                    } else {
                                        result_type.clone()
                                    }
                                } else {
                                    result_type.clone()
                                };

                                // Register forward reference - will be provided by merged stdlib module
                                let mir_func_id = self.register_stdlib_mir_forward_ref(
                                    &mir_func_name,
                                    param_types,
                                    resolved_result_type.clone(),
                                );

                                // IMPORTANT: For Void-returning functions, use the function's ACTUAL return type.
                                // For non-void functions, trust resolved_result_type (which handles generics correctly).
                                // This fixes the bug where void functions like Channel.send incorrectly get dest registers.
                                let final_return_type = if let Some(func) = self.builder.module.functions.get(&mir_func_id) {
                                    if func.signature.return_type == IrType::Void {
                                        eprintln!("DEBUG: [STDLIB MIR] Function {} returns Void, using actual signature", mir_func_name);
                                        IrType::Void
                                    } else if resolved_result_type == IrType::Any || matches!(resolved_result_type, IrType::Ptr(ref inner) if **inner == IrType::Void) {
                                        eprintln!("DEBUG: [STDLIB MIR] resolved_result_type is Any/Ptr(Void), using function signature {:?}", func.signature.return_type);
                                        func.signature.return_type.clone()
                                    } else {
                                        eprintln!("DEBUG: [STDLIB MIR] Using resolved_result_type {:?} (handles generics)", resolved_result_type);
                                        resolved_result_type.clone()
                                    }
                                } else {
                                    resolved_result_type.clone()
                                };

                                eprintln!("DEBUG: [STDLIB MIR] Registered forward ref (instance) to {} with ID {:?}, final return type: {:?}", mir_func_name, mir_func_id, final_return_type);

                                // Generate the call with the final return type
                                let result = self.builder.build_call_direct(
                                    mir_func_id,
                                    arg_regs,
                                    final_return_type,
                                );
                                eprintln!("DEBUG: [STDLIB MIR] Generated call (instance), result: {:?}", result);
                                return result;
                            }

                            // println!(
                            //     "✅ Generating runtime call to {} (receiver type path)",
                            //     runtime_func
                            // );

                            // Lower all arguments
                            let arg_regs: Vec<_> = args
                                .iter()
                                .filter_map(|a| self.lower_expression(a))
                                .collect();

                            // Apply raw value conversion for high-performance inline storage (StringMap, IntMap)
                            // Values are cast to u64 raw bits - no boxing, no heap allocation
                            let mut final_arg_regs = arg_regs.clone();
                            if raw_value_mask != 0 {
                                for i in 0..arg_regs.len() {
                                    if (raw_value_mask & (1 << i)) != 0 {
                                        let arg_reg = arg_regs[i];
                                        let arg_type = self.builder.get_register_type(arg_reg).unwrap_or(IrType::I64);

                                        // Cast value to U64 raw bits - zero-cost for same-size types
                                        let raw_reg = match &arg_type {
                                            IrType::I32 => {
                                                // Zero-extend i32 to u64
                                                self.builder.build_cast(arg_reg, IrType::I32, IrType::U64)
                                            }
                                            IrType::I64 => {
                                                // Reinterpret i64 as u64 (same bits) - use cast
                                                self.builder.build_cast(arg_reg, IrType::I64, IrType::U64)
                                            }
                                            IrType::F64 => {
                                                // Reinterpret f64 bits as u64 - use BitCast instruction
                                                self.builder.build_bitcast(arg_reg, IrType::U64)
                                            }
                                            IrType::F32 => {
                                                // Extend f32 to f64, then reinterpret as u64
                                                let f64_reg = self.builder.build_cast(arg_reg, IrType::F32, IrType::F64)
                                                    .unwrap_or(arg_reg);
                                                self.builder.build_bitcast(f64_reg, IrType::U64)
                                            }
                                            IrType::Bool => {
                                                // Zero-extend bool to u64
                                                self.builder.build_cast(arg_reg, IrType::Bool, IrType::U64)
                                            }
                                            IrType::Ptr(_) => {
                                                // Pointer to u64 (address as integer)
                                                self.builder.build_cast(arg_reg, arg_type.clone(), IrType::U64)
                                            }
                                            _ => {
                                                // For other types, try direct cast to U64
                                                self.builder.build_cast(arg_reg, arg_type.clone(), IrType::U64)
                                            }
                                        };

                                        if let Some(raw) = raw_reg {
                                            final_arg_regs[i] = raw;
                                        }
                                    }
                                }
                            }
                            // Apply pointer conversion for parameters that need it (DEPRECATED - use raw_value_params)
                            // This creates boxed Dynamic values for legacy runtime functions.
                            else if ptr_conversion_mask != 0 {
                                for i in 0..arg_regs.len() {
                                    // Check if bit i is set in the mask
                                    if (ptr_conversion_mask & (1 << i)) != 0 {
                                        let arg_reg = arg_regs[i];
                                        let arg_type = self.builder.get_register_type(arg_reg).unwrap_or(IrType::I64);

                                        // Use proper Dynamic boxing based on the argument type
                                        // This creates a tagged Dynamic value that can be unboxed later
                                        // Use the box_* wrapper functions which handle type conversion internally
                                        let boxed_reg = match &arg_type {
                                            IrType::I32 => {
                                                // Box int using box_int wrapper (which handles i32->i64 cast)
                                                let box_func = self.get_or_register_extern_function(
                                                    "box_int",
                                                    vec![IrType::I32],
                                                    IrType::Ptr(Box::new(IrType::U8)),
                                                );
                                                self.builder.build_call_direct(
                                                    box_func,
                                                    vec![arg_reg],
                                                    IrType::Ptr(Box::new(IrType::U8)),
                                                )
                                            }
                                            IrType::I64 => {
                                                // Box int64 - truncate to i32 and use box_int wrapper
                                                let truncated = self.builder.build_cast(arg_reg, IrType::I64, IrType::I32)
                                                    .unwrap_or(arg_reg);
                                                let box_func = self.get_or_register_extern_function(
                                                    "box_int",
                                                    vec![IrType::I32],
                                                    IrType::Ptr(Box::new(IrType::U8)),
                                                );
                                                self.builder.build_call_direct(
                                                    box_func,
                                                    vec![truncated],
                                                    IrType::Ptr(Box::new(IrType::U8)),
                                                )
                                            }
                                            IrType::F32 | IrType::F64 => {
                                                // Box float using box_float wrapper
                                                let float_val = if arg_type == IrType::F32 {
                                                    self.builder.build_cast(arg_reg, IrType::F32, IrType::F64)
                                                        .unwrap_or(arg_reg)
                                                } else {
                                                    arg_reg
                                                };
                                                let box_func = self.get_or_register_extern_function(
                                                    "box_float",
                                                    vec![IrType::F64],
                                                    IrType::Ptr(Box::new(IrType::U8)),
                                                );
                                                self.builder.build_call_direct(
                                                    box_func,
                                                    vec![float_val],
                                                    IrType::Ptr(Box::new(IrType::U8)),
                                                )
                                            }
                                            IrType::Bool => {
                                                // Box bool using box_bool wrapper
                                                let box_func = self.get_or_register_extern_function(
                                                    "box_bool",
                                                    vec![IrType::Bool],
                                                    IrType::Ptr(Box::new(IrType::U8)),
                                                );
                                                self.builder.build_call_direct(
                                                    box_func,
                                                    vec![arg_reg],
                                                    IrType::Ptr(Box::new(IrType::U8)),
                                                )
                                            }
                                            IrType::Ptr(_) | IrType::Struct { .. } => {
                                                // Pointer/reference types still need stack allocation for ptr_params
                                                // because the runtime function expects a pointer TO the value,
                                                // and the value itself is a pointer we need to pass BY REFERENCE.
                                                // Example: haxe_array_push(arr, data) where data = &value
                                                // For Array<Thread>, value is a pointer, so data = &pointer
                                                if let Some(stack_slot) = self.builder.build_alloc(arg_type.clone(), None) {
                                                    self.builder.build_store(stack_slot, arg_reg);
                                                    Some(stack_slot)
                                                } else {
                                                    Some(arg_reg)
                                                }
                                            }
                                            _ => {
                                                // For other types, fallback to stack allocation
                                                // (This preserves the old behavior for edge cases)
                                                if let Some(stack_slot) = self.builder.build_alloc(arg_type.clone(), None) {
                                                    self.builder.build_store(stack_slot, arg_reg);
                                                    Some(stack_slot)
                                                } else {
                                                    Some(arg_reg)
                                                }
                                            }
                                        };

                                        if let Some(boxed) = boxed_reg {
                                            final_arg_regs[i] = boxed;
                                        }
                                    }
                                }
                            }

                            // Apply i32 -> i64 extension for IntMap key parameters
                            // This is needed because Haxe Int is 32-bit but the runtime uses 64-bit keys
                            if extend_i64_mask != 0 {
                                for i in 0..final_arg_regs.len() {
                                    if (extend_i64_mask & (1 << i)) != 0 {
                                        let arg_reg = final_arg_regs[i];
                                        let arg_type = self.builder.get_register_type(arg_reg).unwrap_or(IrType::I32);

                                        // Only extend i32 to i64, skip if already i64
                                        if arg_type == IrType::I32 {
                                            if let Some(extended) = self.builder.build_cast(arg_reg, IrType::I32, IrType::I64) {
                                                final_arg_regs[i] = extended;
                                            }
                                        }
                                    }
                                }
                            }

                            // Get or register the extern runtime function
                            // Use actual argument types from TAST, applying type conversion where needed
                            let param_types: Vec<IrType> = args.iter().enumerate()
                                .map(|(i, arg)| {
                                    // Raw value params are passed as U64 (high-performance inline storage)
                                    if raw_value_mask != 0 && (raw_value_mask & (1 << i)) != 0 {
                                        IrType::U64
                                    }
                                    // Extended i64 params need i64 type in signature
                                    else if extend_i64_mask != 0 && (extend_i64_mask & (1 << i)) != 0 {
                                        IrType::I64
                                    }
                                    // Legacy ptr_conversion params are passed as Ptr (boxed Dynamic)
                                    else if ptr_conversion_mask != 0 && (ptr_conversion_mask & (1 << i)) != 0 {
                                        IrType::Ptr(Box::new(IrType::U8))
                                    } else {
                                        self.convert_type(arg.ty)
                                    }
                                })
                                .collect();

                            // For functions that return raw values (u64), we need to:
                            // 1. Resolve the actual type parameter T from the receiver's generic args
                            // 2. Call with U64 return type
                            // 3. Cast the result to the resolved type
                            let resolved_return_type = if returns_raw_value {
                                // Try to resolve T from receiver's generic arguments
                                let type_table = self.type_table.borrow();
                                if let Some(receiver_info) = type_table.get(receiver_type) {
                                    if let crate::tast::TypeKind::Class { type_args, .. } = &receiver_info.kind {
                                        if !type_args.is_empty() {
                                            let concrete_type = self.convert_type(type_args[0]);
                                            concrete_type
                                        } else {
                                            result_type.clone()
                                        }
                                    } else {
                                        result_type.clone()
                                    }
                                } else {
                                    result_type.clone()
                                }
                            } else {
                                result_type.clone()
                            };

                            let call_return_type = if returns_raw_value {
                                IrType::U64
                            } else {
                                resolved_return_type.clone()
                            };

                            let runtime_func_id = self.get_or_register_extern_function(
                                &runtime_func,
                                param_types,
                                call_return_type.clone(),
                            );

                            // Generate the call to the runtime function
                            let call_result = self.builder.build_call_direct(
                                runtime_func_id,
                                final_arg_regs,
                                call_return_type,
                            );

                            // If this returns raw value, cast U64 back to the resolved type parameter
                            if returns_raw_value {
                                if let Some(raw_reg) = call_result {
                                    // Cast U64 to the resolved type parameter
                                    let final_result = match &resolved_return_type {
                                        IrType::I32 => {
                                            self.builder.build_cast(raw_reg, IrType::U64, IrType::I32)
                                        }
                                        IrType::I64 => {
                                            self.builder.build_cast(raw_reg, IrType::U64, IrType::I64)
                                        }
                                        IrType::F64 => {
                                            self.builder.build_bitcast(raw_reg, IrType::F64)
                                        }
                                        IrType::F32 => {
                                            // Bitcast to F64, then convert to F32
                                            if let Some(f64_reg) = self.builder.build_bitcast(raw_reg, IrType::F64) {
                                                self.builder.build_cast(f64_reg, IrType::F64, IrType::F32)
                                            } else {
                                                None
                                            }
                                        }
                                        IrType::Bool => {
                                            self.builder.build_cast(raw_reg, IrType::U64, IrType::Bool)
                                        }
                                        IrType::Ptr(_) => {
                                            // Interpret as pointer
                                            self.builder.build_cast(raw_reg, IrType::U64, resolved_return_type.clone())
                                        }
                                        _ => {
                                            // For unknown types, return the raw u64
                                            Some(raw_reg)
                                        }
                                    };
                                    return final_result;
                                }
                            }

                            return call_result;
                        }

                        // Fallback: Use stdlib mapping to try all possible class/method combinations
                        // This is necessary when qualified names aren't set properly
                        if let Some(method_sym) = self.symbol_table.get_symbol(*symbol) {
                            if let Some(method_name) = self.string_interner.get(method_sym.name) {
                                // eprintln!("DEBUG: Trying stdlib mapping for method '{}', symbol={:?}, kind={:?}, qualified_name={:?}",
                                //          method_name,
                                //          symbol,
                                //          method_sym.kind,
                                //          method_sym.qualified_name.and_then(|qn| self.string_interner.get(qn)));

                                // First try to use the qualified name if available
                                if let Some(qual_name) = method_sym
                                    .qualified_name
                                    .and_then(|qn| self.string_interner.get(qn))
                                {
                                    if let Some(runtime_func) =
                                        self.get_static_stdlib_runtime_func(qual_name, method_name)
                                    {
                                        // println!("✅ Generating runtime call to {} for {} (qualified name path)", runtime_func, qual_name);

                                        // CHECK: Is this a MIR wrapper function or a true extern?
                                        // We check this by asking get_stdlib_mir_wrapper_signature - if it knows about
                                        // this function, it's a MIR wrapper. If not, it's an extern.
                                        // This keeps all the knowledge about MIR wrappers centralized.
                                        if let Some((mir_param_types, mir_return_type)) = self.get_stdlib_mir_wrapper_signature(runtime_func) {
                                            eprintln!("DEBUG: [QUALIFIED NAME PATH] Detected MIR wrapper: {}", runtime_func);

                                            // Lower all arguments and collect their types
                                            let mut arg_regs = Vec::new();
                                            let mut param_types = Vec::new();
                                            for arg in args {
                                                if let Some(reg) = self.lower_expression(arg) {
                                                    arg_regs.push(reg);
                                                    param_types.push(self.convert_type(arg.ty));
                                                }
                                            }

                                            // Register forward reference - will be provided by merged stdlib module
                                            let mir_func_id = self.register_stdlib_mir_forward_ref(
                                                runtime_func,
                                                param_types,
                                                result_type.clone(),
                                            );

                                            eprintln!("DEBUG: [QUALIFIED NAME PATH] Registered forward ref to {} with ID {:?}", runtime_func, mir_func_id);

                                            // Generate the call
                                            let result = self.builder.build_call_direct(
                                                mir_func_id,
                                                arg_regs,
                                                result_type,
                                            );
                                            eprintln!("DEBUG: [QUALIFIED NAME PATH] Generated call, result: {:?}", result);
                                            return result;
                                        }

                                        // Lower all arguments
                                        let arg_regs: Vec<_> = args
                                            .iter()
                                            .filter_map(|a| self.lower_expression(a))
                                            .collect();

                                        // Apply pointer conversion for parameters that need it (metadata-driven)
                                        // Look up the RuntimeFunctionCall metadata by runtime function name
                                        // This means the runtime function expects a POINTER TO the value, not the value directly.
                                        let mut final_arg_regs = arg_regs.clone();
                                        let ptr_conversion_mask = self.stdlib_mapping.find_by_runtime_name(&runtime_func)
                                            .map(|m| m.params_need_ptr_conversion)
                                            .unwrap_or(0);
                                        if ptr_conversion_mask != 0 {
                                            for i in 0..arg_regs.len() {
                                                // Check if bit i is set in the mask
                                                if (ptr_conversion_mask & (1 << i)) != 0 {
                                                    let arg_reg = arg_regs[i];
                                                    // Default to I64 (pointer-sized) if type is unknown.
                                                    // This is safer than I32 since pointers and most values are 64-bit.
                                                    let arg_type = self.builder.get_register_type(arg_reg).unwrap_or(IrType::I64);

                                                    // For array operations, always allocate 8 bytes (elem_size is always 8)
                                                    // and extend smaller values to 64-bit
                                                    let (alloc_type, value_to_store) = match arg_type {
                                                        IrType::I32 => {
                                                            let ext_val = self.builder.build_cast(arg_reg, IrType::I32, IrType::I64);
                                                            (IrType::I64, ext_val.unwrap_or(arg_reg))
                                                        }
                                                        IrType::F32 => {
                                                            let ext_val = self.builder.build_cast(arg_reg, IrType::F32, IrType::F64);
                                                            (IrType::F64, ext_val.unwrap_or(arg_reg))
                                                        }
                                                        _ => (arg_type.clone(), arg_reg)
                                                    };

                                                    // Allocate stack space and pass a pointer to the value.
                                                    if let Some(stack_slot) = self.builder.build_alloc(alloc_type.clone(), None) {
                                                        // Store the value into the stack slot
                                                        self.builder.build_store(stack_slot, value_to_store);
                                                        // Use the pointer for the call
                                                        final_arg_regs[i] = stack_slot;
                                                    }
                                                }
                                            }
                                        }

                                        // Get or register the extern runtime function
                                        // Use actual argument types from TAST, applying ptr conversion where needed
                                        let param_types: Vec<IrType> = args.iter().enumerate()
                                            .map(|(i, arg)| {
                                                // If this param was converted to a pointer, the type is Ptr
                                                if ptr_conversion_mask != 0 && (ptr_conversion_mask & (1 << i)) != 0 {
                                                    IrType::Ptr(Box::new(IrType::U8))
                                                } else {
                                                    self.convert_type(arg.ty)
                                                }
                                            })
                                            .collect();
                                        let runtime_func_id = self.get_or_register_extern_function(
                                            &runtime_func,
                                            param_types,
                                            result_type.clone(),
                                        );

                                        // Generate the call to the runtime function
                                        return self.builder.build_call_direct(
                                            runtime_func_id,
                                            final_arg_regs,
                                            result_type,
                                        );
                                    }
                                }

                                // Fallback: try each possible stdlib class (only if qualified name didn't work)
                                // For static methods like Arc.init, Mutex.init, etc, try to infer the class from the return type
                                // eprintln!("DEBUG: Qualified name not available, trying to infer class from return type={:?}", expr.ty);

                                let inferred_class = {
                                    let type_table = self.type_table.borrow();
                                    eprintln!("DEBUG: [INFER CLASS] Checking return type expr.ty={:?}", expr.ty);
                                    if let Some(type_info) = type_table.get(expr.ty) {
                                        eprintln!("DEBUG: [INFER CLASS] Return type kind={:?}", type_info.kind);
                                        if let TypeKind::Class { symbol_id, .. } = &type_info.kind {
                                            if let Some(class_sym) =
                                                self.symbol_table.get_symbol(*symbol_id)
                                            {
                                                let class_name =
                                                    self.string_interner.get(class_sym.name);
                                                eprintln!(
                                                    "DEBUG: [INFER CLASS] Inferred class from return type: {:?}",
                                                    class_name
                                                );
                                                class_name
                                            } else {
                                                eprintln!("DEBUG: [INFER CLASS] Class symbol not found");
                                                None
                                            }
                                        } else {
                                            eprintln!("DEBUG: [INFER CLASS] Return type is not a class");
                                            None
                                        }
                                    } else {
                                        eprintln!("DEBUG: [INFER CLASS] Type info not found for expr.ty={:?}", expr.ty);
                                        None
                                    }
                                };

                                if let Some(class_name) = inferred_class {
                                    // SPECIAL CASE: Check if this is a stdlib MIR function
                                    if self.stdlib_mapping.is_mir_wrapper_class(class_name) {
                                        // Use lowercase class name to match stdlib MIR wrapper naming convention
                                        let mir_func_name = format!("{}_{}", class_name.to_lowercase(), method_name);
                                        eprintln!("DEBUG: [STDLIB MIR] Detected stdlib MIR function: {}", mir_func_name);

                                        // Lower all arguments and collect their types
                                        let mut arg_regs = Vec::new();
                                        let mut param_types = Vec::new();
                                        for arg in args {
                                            if let Some(reg) = self.lower_expression(arg) {
                                                arg_regs.push(reg);
                                                param_types.push(self.convert_type(arg.ty));
                                            }
                                        }

                                        // Register forward reference - will be provided by merged stdlib module
                                        let mir_func_id = self.register_stdlib_mir_forward_ref(
                                            &mir_func_name,
                                            param_types,
                                            result_type.clone(),
                                        );

                                        eprintln!("DEBUG: [STDLIB MIR] Registered forward ref to {} with ID {:?}", mir_func_name, mir_func_id);

                                        // Generate the call
                                        let result = self.builder.build_call_direct(
                                            mir_func_id,
                                            arg_regs,
                                            result_type,
                                        );
                                        eprintln!("DEBUG: [STDLIB MIR] Generated call, result: {:?}", result);
                                        return result;
                                    }

                                    // Try the inferred class first
                                    let fake_qual_name =
                                        format!("rayzor.concurrent.{}.{}", class_name, method_name);
                                    if let Some(runtime_func) = self.get_static_stdlib_runtime_func(
                                        &fake_qual_name,
                                        method_name,
                                    ) {
                                        eprintln!("DEBUG: [INFERRED CLASS PATH] Got runtime_func='{}' for class={}, method={}", runtime_func, class_name, method_name);
                                        // println!("✅ Generating runtime call to {} for {}.{} (inferred from return type)", runtime_func, class_name, method_name);

                                        // Lower all arguments
                                        let arg_regs: Vec<_> = args
                                            .iter()
                                            .filter_map(|a| self.lower_expression(a))
                                            .collect();

                                        // Apply pointer conversion for parameters that need it (metadata-driven)
                                        // Look up the RuntimeFunctionCall metadata by runtime function name
                                        // This means the runtime function expects a POINTER TO the value, not the value directly.
                                        let mut final_arg_regs = arg_regs.clone();
                                        let ptr_conversion_mask = self.stdlib_mapping.find_by_runtime_name(&runtime_func)
                                            .map(|m| m.params_need_ptr_conversion)
                                            .unwrap_or(0);
                                        if ptr_conversion_mask != 0 {
                                            for i in 0..arg_regs.len() {
                                                // Check if bit i is set in the mask
                                                if (ptr_conversion_mask & (1 << i)) != 0 {
                                                    let arg_reg = arg_regs[i];
                                                    // Default to I64 (pointer-sized) if type is unknown.
                                                    // This is safer than I32 since pointers and most values are 64-bit.
                                                    let arg_type = self.builder.get_register_type(arg_reg).unwrap_or(IrType::I64);

                                                    // For array operations, always allocate 8 bytes (elem_size is always 8)
                                                    // and extend smaller values to 64-bit
                                                    let (alloc_type, value_to_store) = match arg_type {
                                                        IrType::I32 => {
                                                            let ext_val = self.builder.build_cast(arg_reg, IrType::I32, IrType::I64);
                                                            (IrType::I64, ext_val.unwrap_or(arg_reg))
                                                        }
                                                        IrType::F32 => {
                                                            let ext_val = self.builder.build_cast(arg_reg, IrType::F32, IrType::F64);
                                                            (IrType::F64, ext_val.unwrap_or(arg_reg))
                                                        }
                                                        _ => (arg_type.clone(), arg_reg)
                                                    };

                                                    // Allocate stack space and pass a pointer to the value.
                                                    if let Some(stack_slot) = self.builder.build_alloc(alloc_type.clone(), None) {
                                                        // Store the value into the stack slot
                                                        self.builder.build_store(stack_slot, value_to_store);
                                                        // Use the pointer for the call
                                                        final_arg_regs[i] = stack_slot;
                                                    }
                                                }
                                            }
                                        }

                                        // Get or register the extern runtime function
                                        // Use actual argument types from TAST, applying ptr conversion where needed
                                        let param_types: Vec<IrType> = args.iter().enumerate()
                                            .map(|(i, arg)| {
                                                // If this param was converted to a pointer, the type is Ptr
                                                if ptr_conversion_mask != 0 && (ptr_conversion_mask & (1 << i)) != 0 {
                                                    IrType::Ptr(Box::new(IrType::U8))
                                                } else {
                                                    self.convert_type(arg.ty)
                                                }
                                            })
                                            .collect();
                                        let runtime_func_id = self.get_or_register_extern_function(
                                            &runtime_func,
                                            param_types,
                                            result_type.clone(),
                                        );

                                        // Generate the call to the runtime function
                                        return self.builder.build_call_direct(
                                            runtime_func_id,
                                            final_arg_regs,
                                            result_type,
                                        );
                                    }
                                }

                                // Last resort: try all stdlib classes with param count matching
                                // NOTE: We must match by param count to disambiguate overloaded methods
                                // (e.g., Array.join(sep) with 1 param vs Thread.join() with 0 params)
                                let actual_arg_count = args.len().saturating_sub(1); // Subtract 1 for receiver (self)
                                eprintln!(
                                    "DEBUG: [LAST RESORT] Could not infer class for method '{}' with {} args, trying all stdlib classes",
                                    method_name, actual_arg_count
                                );
                                // Get all stdlib classes dynamically from the mapping
                                // NOTE: We do NOT add stdlib MIR detection here because we don't know which class
                                // to use - the fallback tries all classes and would match the wrong one
                                let stdlib_classes = self.stdlib_mapping.get_all_classes();
                                for class_name in &stdlib_classes {
                                    // Use find_by_name_and_params to ensure param count matches
                                    // This prevents Array.join(1 param) from matching Thread.join(0 params)
                                    if let Some((sig, mapping)) = self.stdlib_mapping.find_by_name_and_params(
                                        class_name,
                                        method_name,
                                        actual_arg_count,
                                    ) {
                                        let runtime_func = mapping.runtime_name;

                                        // CHECK: Is this a MIR wrapper or an extern?
                                        if let Some((mir_param_types, mir_return_type)) = self.get_stdlib_mir_wrapper_signature(&runtime_func) {
                                            eprintln!("DEBUG: [FALLBACK PATH] Detected MIR wrapper: {}", runtime_func);

                                            // Lower all arguments
                                            let mut arg_regs = Vec::new();
                                            for arg in args {
                                                if let Some(reg) = self.lower_expression(arg) {
                                                    arg_regs.push(reg);
                                                }
                                            }

                                            // Register forward reference - signature comes from get_stdlib_mir_wrapper_signature
                                            let mir_func_id = self.register_stdlib_mir_forward_ref(
                                                &runtime_func,
                                                mir_param_types,
                                                mir_return_type,
                                            );

                                            eprintln!("DEBUG: [FALLBACK PATH] Registered forward ref to {} with ID {:?}", runtime_func, mir_func_id);

                                            // Generate the call
                                            let result = self.builder.build_call_direct(
                                                mir_func_id,
                                                arg_regs,
                                                result_type,
                                            );
                                            eprintln!("DEBUG: [FALLBACK PATH] Generated call, result: {:?}", result);
                                            return result;
                                        }

                                        // Lower all arguments
                                        let arg_regs: Vec<_> = args
                                            .iter()
                                            .filter_map(|a| self.lower_expression(a))
                                            .collect();

                                        // Apply pointer conversion for parameters that need it (metadata-driven)
                                        // Look up the RuntimeFunctionCall metadata by runtime function name
                                        // This means the runtime function expects a POINTER TO the value, not the value directly.
                                        let mut final_arg_regs = arg_regs.clone();
                                        let ptr_conversion_mask = self.stdlib_mapping.find_by_runtime_name(&runtime_func)
                                            .map(|m| m.params_need_ptr_conversion)
                                            .unwrap_or(0);
                                        if ptr_conversion_mask != 0 {
                                            for i in 0..arg_regs.len() {
                                                // Check if bit i is set in the mask
                                                if (ptr_conversion_mask & (1 << i)) != 0 {
                                                    let arg_reg = arg_regs[i];
                                                    // Default to I64 (pointer-sized) if type is unknown.
                                                    // This is safer than I32 since pointers and most values are 64-bit.
                                                    let arg_type = self.builder.get_register_type(arg_reg).unwrap_or(IrType::I64);

                                                    // For array operations, always allocate 8 bytes (elem_size is always 8)
                                                    // and extend smaller values to 64-bit
                                                    let (alloc_type, value_to_store) = match arg_type {
                                                        IrType::I32 => {
                                                            let ext_val = self.builder.build_cast(arg_reg, IrType::I32, IrType::I64);
                                                            (IrType::I64, ext_val.unwrap_or(arg_reg))
                                                        }
                                                        IrType::F32 => {
                                                            let ext_val = self.builder.build_cast(arg_reg, IrType::F32, IrType::F64);
                                                            (IrType::F64, ext_val.unwrap_or(arg_reg))
                                                        }
                                                        _ => (arg_type.clone(), arg_reg)
                                                    };

                                                    // Allocate stack space and pass a pointer to the value.
                                                    if let Some(stack_slot) = self.builder.build_alloc(alloc_type.clone(), None) {
                                                        // Store the value into the stack slot
                                                        self.builder.build_store(stack_slot, value_to_store);
                                                        // Use the pointer for the call
                                                        final_arg_regs[i] = stack_slot;
                                                    }
                                                }
                                            }
                                        }

                                        // Get or register the extern runtime function
                                        // Use actual argument types from TAST, applying ptr conversion where needed
                                        let param_types: Vec<IrType> = args.iter().enumerate()
                                            .map(|(i, arg)| {
                                                // If this param was converted to a pointer, the type is Ptr
                                                if ptr_conversion_mask != 0 && (ptr_conversion_mask & (1 << i)) != 0 {
                                                    IrType::Ptr(Box::new(IrType::U8))
                                                } else {
                                                    self.convert_type(arg.ty)
                                                }
                                            })
                                            .collect();
                                        let runtime_func_id = self.get_or_register_extern_function(
                                            &runtime_func,
                                            param_types,
                                            result_type.clone(),
                                        );

                                        // Generate the call to the runtime function
                                        return self.builder.build_call_direct(
                                            runtime_func_id,
                                            final_arg_regs,
                                            result_type,
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        // receiver_is_class_type == true
                        // This is an instance method call on a MIR wrapper class (Thread, Channel, etc.)
                        // Route to the MIR wrapper function (Thread_join, Channel_send, etc.)
                        if let Some(sym_info) = self.symbol_table.get_symbol(*symbol) {
                            if let Some(method_name) = self.string_interner.get(sym_info.name) {
                                // Get the class name from the receiver type
                                let class_name = {
                                    let type_table = self.type_table.borrow();
                                    type_table.get(receiver_type)
                                        .and_then(|ti| {
                                            if let crate::tast::core::TypeKind::Class { symbol_id, .. } = &ti.kind {
                                                self.symbol_table.get_symbol(*symbol_id)
                                                    .and_then(|s| self.string_interner.get(s.name))
                                                    .map(|s| s.to_string())
                                            } else {
                                                None
                                            }
                                        })
                                };

                                if let Some(class_name) = class_name {
                                    // Build MIR wrapper function name: Thread_join, Channel_send, etc.
                                    let mir_func_name = format!("{}_{}", class_name, method_name);
                                    eprintln!("DEBUG: [MIR WRAPPER INSTANCE] Routing {}.{} to {}", class_name, method_name, mir_func_name);

                                    // Get the registered signature for this MIR wrapper
                                    if let Some((mir_param_types, mir_return_type)) = self.get_stdlib_mir_wrapper_signature(&mir_func_name) {
                                        // Lower all arguments (first arg is receiver/self)
                                        let mut arg_regs = Vec::new();
                                        for arg in args {
                                            if let Some(reg) = self.lower_expression(arg) {
                                                arg_regs.push(reg);
                                            }
                                        }

                                        // Register forward reference to MIR wrapper
                                        let mir_func_id = self.register_stdlib_mir_forward_ref(
                                            &mir_func_name,
                                            mir_param_types,
                                            mir_return_type,
                                        );

                                        eprintln!("DEBUG: [MIR WRAPPER INSTANCE] Registered forward ref to {} with ID {:?}", mir_func_name, mir_func_id);

                                        // Generate the call
                                        let result = self.builder.build_call_direct(
                                            mir_func_id,
                                            arg_regs,
                                            result_type,
                                        );
                                        eprintln!("DEBUG: [MIR WRAPPER INSTANCE] Generated call, result: {:?}", result);
                                        return result;
                                    } else {
                                        eprintln!("DEBUG: [MIR WRAPPER INSTANCE] No signature found for {}, falling through", mir_func_name);
                                    }
                                }
                            }
                        }
                    } // end if receiver_is_class_type else block
                    }
                    // For static methods, check if it's a stdlib static method
                    if !*is_method {
                        // eprintln!("DEBUG: Static method path (is_method=false)");
                        if let Some(sym_info) = self.symbol_table.get_symbol(*symbol) {
                            if let Some(method_name) = self.string_interner.get(sym_info.name) {
                                eprintln!("DEBUG: [PRE-CHECK] Static method candidate: name='{}', symbol={:?}",
                                    method_name, symbol);

                                // Try to get the qualified name to determine the class
                                if let Some(qual_name) = sym_info.qualified_name {
                                    if let Some(qual_name_str) = self.string_interner.get(qual_name)
                                    {
                                        eprintln!("DEBUG: [PRE-CHECK] Qualified name: '{}'", qual_name_str);

                                        // SPECIAL CASE: Thread/Channel/Mutex/Arc methods are MIR wrappers, not runtime_mapping
                                        // These are implemented in stdlib MIR (thread.rs, channel.rs, etc.)
                                        // Pattern: "rayzor.concurrent.Thread.spawn" -> "Thread_spawn"
                                        // NOTE: This only applies to rayzor.concurrent.*, NOT sys.thread.*
                                        let parts: Vec<&str> = qual_name_str.split('.').collect();
                                        if parts.len() >= 2 {
                                            let class_name = parts[parts.len() - 2];

                                            // Check if this is a rayzor.concurrent.* class (NOT sys.thread.*)
                                            // sys.thread.Thread uses runtime mapping directly, not MIR wrappers
                                            let is_rayzor_concurrent = qual_name_str.starts_with("rayzor.concurrent.");
                                            if is_rayzor_concurrent && ["Thread", "Channel", "Mutex", "Arc"].contains(&class_name) {
                                                // Use capitalized class names for rayzor.concurrent (Thread, Channel, etc.)
                                                let mir_func_name = format!("{}_{}", class_name, method_name);
                                                eprintln!("DEBUG: [STDLIB MIR] Detected stdlib MIR function: {}, args.len()={}", mir_func_name, args.len());
                                                for (idx, arg) in args.iter().enumerate() {
                                                    eprintln!("DEBUG: [STDLIB MIR PRE] arg[{}] kind={:?}, ty={:?}", idx, std::mem::discriminant(&arg.kind), arg.ty);
                                                }

                                                // WORKAROUND: During dependency loading retries, static method calls like
                                                // Thread.spawn(closure) can be incorrectly treated as instance method calls
                                                // with args = [Thread_class, closure] instead of just [closure].
                                                // Detect and fix this by checking if first arg is the class itself.
                                                let actual_args = if args.len() >= 2 {
                                                    // Check if first arg might be the class type
                                                    let type_table = self.type_table.borrow();
                                                    let first_arg_is_class = type_table.get(args[0].ty)
                                                        .map(|ti| {
                                                            // Check if this type is a Class type matching our static method class
                                                            if let crate::tast::core::TypeKind::Class { symbol_id, .. } = &ti.kind {
                                                                self.symbol_table.get_symbol(*symbol_id)
                                                                    .and_then(|s| self.string_interner.get(s.name))
                                                                    .map(|name| name == class_name)
                                                                    .unwrap_or(false)
                                                            } else {
                                                                false
                                                            }
                                                        })
                                                        .unwrap_or(false);
                                                    drop(type_table);

                                                    if first_arg_is_class {
                                                        eprintln!("DEBUG: [STDLIB MIR FIX] Detected spurious class argument, skipping first arg");
                                                        &args[1..]  // Skip the class "receiver" argument
                                                    } else {
                                                        &args[..]
                                                    }
                                                } else {
                                                    &args[..]
                                                };

                                                // Lower all arguments and collect their types
                                                let mut arg_regs = Vec::new();
                                                let mut param_types = Vec::new();
                                                for (idx, arg) in actual_args.iter().enumerate() {
                                                    eprintln!("DEBUG: [STDLIB MIR] arg[{}] ty={:?}", idx, arg.ty);
                                                    if let Some(reg) = self.lower_expression(arg) {
                                                        arg_regs.push(reg);
                                                        param_types.push(self.convert_type(arg.ty));
                                                    }
                                                }

                                                // Register forward reference - will be provided by merged stdlib module
                                                // We infer the signature from the call site arguments
                                                let mir_func_id = self.register_stdlib_mir_forward_ref(
                                                    &mir_func_name,
                                                    param_types,
                                                    result_type.clone(),
                                                );

                                                eprintln!("DEBUG: [STDLIB MIR] Registered forward ref to {} with ID {:?}", mir_func_name, mir_func_id);

                                                // Generate the call
                                                let result = self.builder.build_call_direct(
                                                    mir_func_id,
                                                    arg_regs,
                                                    result_type,
                                                );
                                                eprintln!("DEBUG: [STDLIB MIR] Generated call, result: {:?}", result);
                                                return result;
                                            }
                                        }

                                        // Check if this is a stdlib class method by looking at qualified name
                                        // e.g., "rayzor.concurrent.Thread.spawn" or "test.Thread.spawn"
                                        let lookup_result = self.get_static_stdlib_runtime_func(
                                            qual_name_str,
                                            method_name,
                                        );
                                        eprintln!("DEBUG: [PRE-CHECK] get_static_stdlib_runtime_func returned: {:?}", lookup_result);

                                        if let Some(runtime_func) = lookup_result
                                        {
                                            eprintln!("DEBUG: [STATIC METHOD] Found stdlib runtime func: {}.{} -> {}",
                                                qual_name_str, method_name, runtime_func);

                                            // Get the expected signature from our registered extern functions
                                            // This ensures we use the correct types (e.g., I64 for Std.random)
                                            let (expected_param_types, expected_return_type) = self
                                                .get_extern_function_signature(&runtime_func)
                                                .unwrap_or_else(|| {
                                                    // Fall back to inferred types from TAST
                                                    let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
                                                    let param_types: Vec<IrType> = args.iter()
                                                        .map(|a| {
                                                            let arg_ty = self.convert_type(a.ty);
                                                            if arg_ty == IrType::String {
                                                                string_ptr_ty.clone()
                                                            } else {
                                                                arg_ty
                                                            }
                                                        })
                                                        .collect();
                                                    (param_types, result_type.clone())
                                                });

                                            // Lower all arguments
                                            let arg_regs: Vec<_> = args
                                                .iter()
                                                .filter_map(|a| self.lower_expression(a))
                                                .collect();

                                            eprintln!("DEBUG: [STATIC METHOD] Lowered {} args: {:?}", arg_regs.len(), arg_regs);

                                            // Cast arguments to expected types if needed
                                            let final_arg_regs: Vec<_> = arg_regs.iter().enumerate()
                                                .map(|(i, &reg)| {
                                                    if let (Some(expected_ty), Some(actual_ty)) = (
                                                        expected_param_types.get(i),
                                                        self.builder.get_register_type(reg)
                                                    ) {
                                                        // If types differ, insert a cast
                                                        if *expected_ty != actual_ty {
                                                            eprintln!("DEBUG: [STATIC METHOD] Casting arg {} from {:?} to {:?}", i, actual_ty, expected_ty);
                                                            if let Some(casted) = self.builder.build_cast(reg, actual_ty.clone(), expected_ty.clone()) {
                                                                return casted;
                                                            }
                                                        }
                                                    }
                                                    reg
                                                })
                                                .collect();

                                            let runtime_func_id = self
                                                .get_or_register_extern_function(
                                                    &runtime_func,
                                                    expected_param_types,
                                                    expected_return_type.clone(),
                                                );

                                            eprintln!("DEBUG: [STATIC METHOD] Registered runtime func {} with ID {:?}", runtime_func, runtime_func_id);

                                            // Generate the call to the runtime function
                                            let result = self.builder.build_call_direct(
                                                runtime_func_id,
                                                final_arg_regs,
                                                expected_return_type,
                                            );
                                            eprintln!("DEBUG: [STATIC METHOD] Generated call, result: {:?}", result);
                                            return result;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Check if this symbol is a function (local or external)
                    // First try direct symbol ID lookup
                    let mut func_id_opt = self.get_function_id(symbol);

                    // If not found by symbol ID, try lookup by qualified name
                    // This handles cross-module calls where symbol IDs differ between modules
                    if func_id_opt.is_none() {
                        if let Some(sym_info) = self.symbol_table.get_symbol(*symbol) {
                            if let Some(qual_name) = sym_info.qualified_name {
                                if let Some(qual_name_str) = self.string_interner.get(qual_name) {
                                    // Search external_function_map by qualified name
                                    for (ext_sym, &ext_func_id) in &self.external_function_map {
                                        if let Some(ext_sym_info) = self.symbol_table.get_symbol(*ext_sym) {
                                            if let Some(ext_qual) = ext_sym_info.qualified_name {
                                                if let Some(ext_qual_str) = self.string_interner.get(ext_qual) {
                                                    if ext_qual_str == qual_name_str {
                                                        eprintln!("DEBUG: [CROSS-MODULE] Found function by qualified name '{}': symbol {:?} -> func_id={:?}",
                                                            qual_name_str, ext_sym, ext_func_id);
                                                        func_id_opt = Some(ext_func_id);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(func_id) = func_id_opt {
                        let sym_name = self.symbol_table.get_symbol(*symbol)
                            .and_then(|s| self.string_interner.get(s.name))
                            .unwrap_or("<unknown>");
                        let qual_name = self.symbol_table.get_symbol(*symbol)
                            .and_then(|s| s.qualified_name)
                            .and_then(|qn| self.string_interner.get(qn))
                            .unwrap_or("<none>");
                        let is_external = self.external_function_map.contains_key(symbol);

                        eprintln!("DEBUG: [FUNCTION_MAP LOOKUP] Found symbol {:?} '{}' (qual: '{}') -> func_id={:?}, is_method={}, external={}",
                            symbol, sym_name, qual_name, func_id, is_method, is_external);

                        // IMPORTANT: Use the function's actual return type, not expr.ty
                        let actual_return_type = if let Some(func) = self.builder.module.functions.get(&func_id) {
                            eprintln!("DEBUG: [FUNCTION_MAP] Using actual return type {:?} for function {:?}", func.signature.return_type, func.name);
                            func.signature.return_type.clone()
                        } else {
                            // Function not in module yet (probably forward ref to stdlib MIR wrapper)
                            // Try to look up the correct signature by function name
                            eprintln!("DEBUG: [FUNCTION_MAP] Function {:?} not found in module, checking stdlib signatures", func_id);
                            if let Some((_params, ret_ty)) = self.get_stdlib_mir_wrapper_signature(&sym_name) {
                                eprintln!("DEBUG: [FUNCTION_MAP] Found stdlib signature for '{}': returns {:?}", sym_name, ret_ty);
                                ret_ty
                            } else {
                                eprintln!("DEBUG: [FUNCTION_MAP] No stdlib signature found, using expr return type {:?}", result_type);
                                result_type.clone()
                            }
                        };

                        // Handle method calls where the object is passed as first argument
                        if *is_method {
                            // eprintln!("DEBUG: Method call (is_method=true) - symbol={:?}, args.len()={}", symbol, args.len());
                            // For method calls, args already includes the object as first arg
                            let arg_regs: Vec<_> = args
                                .iter()
                                .filter_map(|a| self.lower_expression(a))
                                .collect();

                            // Extract type_args from receiver's class type for generic method calls
                            let ir_type_args = if !args.is_empty() {
                                let receiver_type = args[0].ty;
                                let type_args_copy = {
                                    let type_table = self.type_table.borrow();
                                    if let Some(receiver_info) = type_table.get(receiver_type) {
                                        if let crate::tast::TypeKind::Class { type_args, .. } = &receiver_info.kind {
                                            // Clone type_args before releasing borrow
                                            type_args.clone()
                                        } else {
                                            Vec::new()
                                        }
                                    } else {
                                        Vec::new()
                                    }
                                };
                                // Convert TypeId type_args to IrType (borrow released)
                                type_args_copy.iter().map(|&ty_id| self.convert_type(ty_id)).collect::<Vec<_>>()
                            } else {
                                Vec::new()
                            };

                            eprintln!("DEBUG: [FUNCTION_MAP] Method call lowered {} args: {:?}, type_args: {:?}", arg_regs.len(), arg_regs, ir_type_args);
                            let result = if ir_type_args.is_empty() {
                                self.builder.build_call_direct(func_id, arg_regs, actual_return_type.clone())
                            } else {
                                self.builder.build_call_direct_with_type_args(func_id, arg_regs, actual_return_type.clone(), ir_type_args)
                            };
                            eprintln!("DEBUG: [FUNCTION_MAP] Result: {:?}", result);
                            return result;
                        } else {
                            // Direct function call (static method or free function)
                            let arg_regs: Vec<_> = args
                                .iter()
                                .filter_map(|a| self.lower_expression(a))
                                .collect();

                            // Infer type_args for static generic calls if not already provided
                            let final_type_args = if converted_hir_type_args.is_empty() {
                                // Check if the function has type parameters
                                if let Some(func) = self.builder.module.functions.get(&func_id) {
                                    if !func.signature.type_params.is_empty() && !args.is_empty() {
                                        // Try to infer type_args from argument types
                                        eprintln!("DEBUG: [TYPE INFERENCE] Function {} has type_params: {:?}", func.name, func.signature.type_params);
                                        eprintln!("DEBUG: [TYPE INFERENCE] Function params: {:?}", func.signature.parameters.iter().map(|p| (&p.name, &p.ty)).collect::<Vec<_>>());

                                        let mut inferred: Vec<IrType> = Vec::new();
                                        for (_param_idx, type_param) in func.signature.type_params.iter().enumerate() {
                                            // Look for a parameter using this type variable
                                            let mut found = false;
                                            for (i, sig_param) in func.signature.parameters.iter().enumerate() {
                                                eprintln!("DEBUG: [TYPE INFERENCE] Checking param {} type {:?} against type_param {}", sig_param.name, sig_param.ty, type_param.name);
                                                if let IrType::TypeVar(ref name) = sig_param.ty {
                                                    if name == &type_param.name && i < args.len() {
                                                        // Use the concrete type of the corresponding argument
                                                        let arg_type = self.convert_type(args[i].ty);
                                                        eprintln!("DEBUG: [TYPE INFERENCE] Inferred {}={:?} from arg {}", type_param.name, arg_type, i);
                                                        inferred.push(arg_type);
                                                        found = true;
                                                        break;
                                                    }
                                                }
                                            }
                                            if !found {
                                                // Couldn't infer this type param from signature params
                                                // Try using the first argument's type as a fallback for single type param
                                                if func.signature.type_params.len() == 1 && !args.is_empty() {
                                                    let arg_type = self.convert_type(args[0].ty);
                                                    eprintln!("DEBUG: [TYPE INFERENCE] Fallback: Inferred {}={:?} from first arg", type_param.name, arg_type);
                                                    inferred.push(arg_type);
                                                } else {
                                                    eprintln!("DEBUG: [TYPE INFERENCE] Could not infer {}, using Any", type_param.name);
                                                    inferred.push(IrType::Any);
                                                }
                                            }
                                        }
                                        inferred
                                    } else {
                                        Vec::new()
                                    }
                                } else {
                                    Vec::new()
                                }
                            } else {
                                converted_hir_type_args.clone()
                            };

                            // Use HIR type_args or inferred type_args for static generic calls
                            eprintln!("DEBUG: [FUNCTION_MAP] Direct call lowered {} args: {:?}, final_type_args: {:?}", arg_regs.len(), arg_regs, final_type_args);
                            let result = if final_type_args.is_empty() {
                                self.builder.build_call_direct(func_id, arg_regs, actual_return_type)
                            } else {
                                self.builder.build_call_direct_with_type_args(func_id, arg_regs, actual_return_type, final_type_args)
                            };
                            eprintln!("DEBUG: [FUNCTION_MAP] Result: {:?}", result);
                            return result;
                        }
                    } else {
                        // Function not in function_map - might be an extern/stdlib function
                        // Check if it's a stdlib static method (like Math.sin, Sys.println)
                        if let Some(sym_info) = self.symbol_table.get_symbol(*symbol) {
                            if let Some(method_name) = self.string_interner.get(sym_info.name) {
                                // Check if method name matches known Math/Sys methods
                                // Try to find this method in ANY stdlib class with static methods
                                // This replaces the hardcoded is_math_method and is_sys_method checks
                                let method_static: &'static str =
                                    Box::leak(method_name.to_string().into_boxed_str());

                                // Try all stdlib classes that have static methods
                                let mut found_mapping = None;
                                for class_name in self.stdlib_mapping.get_all_classes() {
                                    if self.stdlib_mapping.class_has_static_methods(class_name) {
                                        let sig = crate::stdlib::MethodSignature {
                                            class: class_name,
                                            method: method_static,
                                            is_static: true,
                                            is_constructor: false,  // Normal static method, not constructor
                                            param_count: args.len(),
                                        };
                                        if let Some(mapping) = self.stdlib_mapping.get(&sig) {
                                            found_mapping = Some((class_name, mapping));
                                            break;
                                        }
                                    }
                                }

                                if let Some((class_name, mapping)) = found_mapping {
                                    let runtime_name = mapping.runtime_name;
                                    // eprintln!(
                                    //     "INFO: {} static method detected: {} (runtime: {})",
                                    //     class_name, method_name, runtime_name
                                    // );

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
                                    return self.builder.build_call_direct(
                                        extern_func_id,
                                        arg_regs,
                                        result_type,
                                    );
                                }
                            }
                        }
                    }
                }

                // Before falling through to indirect call, try to look up by name or register a forward reference
                // for unresolved static method calls (cross-module dependencies during stdlib compilation)
                if let HirExprKind::Variable { symbol, .. } = &callee.kind {
                    if let Some(sym_info) = self.symbol_table.get_symbol(*symbol) {
                        if let Some(qual_name) = sym_info.qualified_name {
                            if let Some(qual_name_str) = self.string_interner.get(qual_name) {
                                let _method_name = self.string_interner.get(sym_info.name).unwrap_or("<unknown>");
                                eprintln!("DEBUG: [PRE-CHECK] Qualified name: '{}'", qual_name_str);

                                // FIRST: Check if this function is already compiled and in the name map
                                if let Some(&existing_func_id) = self.external_function_name_map.get(qual_name_str) {
                                    eprintln!("DEBUG: [NAME MAP HIT] Found '{}' in external_function_name_map -> {:?}", qual_name_str, existing_func_id);

                                    // Lower arguments
                                    let arg_regs: Vec<_> = args.iter()
                                        .filter_map(|a| self.lower_expression(a))
                                        .collect();

                                    // Generate the call to the external function
                                    return self.builder.build_call_direct(
                                        existing_func_id,
                                        arg_regs,
                                        result_type,
                                    );
                                }

                                eprintln!("DEBUG: [FORWARD REF] Registering forward reference for unresolved call to '{}'", qual_name_str);

                                // Lower arguments and collect their types
                                let mut arg_regs = Vec::new();
                                let mut param_types = Vec::new();
                                for arg in args {
                                    if let Some(reg) = self.lower_expression(arg) {
                                        arg_regs.push(reg);
                                        param_types.push(self.convert_type(arg.ty));
                                    }
                                }

                                // Register as a forward reference using qualified name
                                // This will be resolved later during module linking
                                let forward_func_id = self.register_stdlib_mir_forward_ref(
                                    qual_name_str,
                                    param_types,
                                    result_type.clone(),
                                );

                                eprintln!("DEBUG: [FORWARD REF] Registered forward ref to '{}' with ID {:?}", qual_name_str, forward_func_id);

                                // Generate the call to the forward reference
                                return self.builder.build_call_direct(
                                    forward_func_id,
                                    arg_regs,
                                    result_type,
                                );
                            }
                        }
                    }
                }

                // Indirect function call (function pointer)
                // TODO: Get the full function signature from the callee's type
                // For now, we'll infer it from the arguments and return type
                // This is a temporary workaround until we pass type_table to HIR→MIR

                eprintln!("DEBUG: Taking indirect function call path - callee kind={:?}, args.len()={}",
                         std::mem::discriminant(&callee.kind), args.len());

                // Lower arguments FIRST, before trying to lower the callee
                // This ensures lambdas in arguments get generated even if callee lowering fails
                eprintln!("DEBUG: About to lower {} indirect call arguments", args.len());
                for (i, a) in args.iter().enumerate() {
                    eprintln!("DEBUG:   arg[{}] kind={:?}", i, std::mem::discriminant(&a.kind));
                }
                let arg_regs: Vec<_> = args
                    .iter()
                    .filter_map(|a| {
                        eprintln!("DEBUG: NOW lowering arg with kind={:?}", std::mem::discriminant(&a.kind));
                        self.lower_expression(a)
                    })
                    .collect();
                eprintln!("DEBUG: Lowered {} indirect call arguments successfully", arg_regs.len());

                // Now try to lower the callee - if this fails, the call won't be generated
                // but the lambda functions in arguments will have been created
                let func_ptr = self.lower_expression(callee)?;

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

                self.builder
                    .build_call_indirect(func_ptr, arg_regs, func_signature)
            }

            HirExprKind::New {
                class_type, type_args: hir_type_args, args, class_name: hir_class_name, ..
            } => {
                let debug_class_name = hir_class_name.and_then(|interned| self.string_interner.get(interned));
                eprintln!("DEBUG [NEW EXPR]: class_type={:?}, args.len={}, hir_class_name={:?}, hir_type_args={:?}", class_type, args.len(), debug_class_name, hir_type_args);

                // Check if this is an abstract type
                let type_table = self.type_table.borrow();
                let (is_abstract, actual_symbol_id) = if let Some(type_ref) = type_table.get(*class_type) {
                    let symbol_id = match &type_ref.kind {
                        crate::tast::TypeKind::Class { symbol_id, .. } => Some(*symbol_id),
                        crate::tast::TypeKind::Abstract { symbol_id, .. } => Some(*symbol_id),
                        _ => None
                    };
                    let is_abs = matches!(type_ref.kind, crate::tast::TypeKind::Abstract { .. });
                    (is_abs, symbol_id)
                } else {
                    (false, None)
                };
                drop(type_table);

                // SPECIAL CASE: Abstract type constructors
                // If this is an abstract type, treat this as a simple value wrap (no allocation).
                if is_abstract {
                    // eprintln!("DEBUG: Abstract type constructor detected - returning wrapped value");
                    if args.len() == 1 {
                        return self.lower_expression(&args[0]);
                    } else if args.is_empty() {
                        // eprintln!("WARNING: Abstract constructor with no arguments, returning 0");
                        return self.builder.build_const(IrValue::I32(0));
                    } else {
                        // eprintln!(
                        //     "WARNING: Abstract constructor with {} arguments, using first",
                        //     args.len()
                        // );
                        return self.lower_expression(&args[0]);
                    }
                }

                // SPECIAL CASE: Check if this is an extern stdlib class constructor BEFORE fallback
                // For extern stdlib classes (Channel, Thread, Arc, Mutex), we call the MIR wrapper
                // function (e.g., Channel_init) instead of allocating and calling a constructor.
                // This MUST come before the value-wrap fallback to prevent returning the argument value!

                // PRIORITY #1: Use class_name from HIR if available (preserves actual class name even when TypeId is invalid)
                let mut class_name = hir_class_name.and_then(|interned| self.string_interner.get(interned));

                // FALLBACK #1: Try to get class name from TypeId if HIR didn't have it
                if class_name.is_none() {
                    let type_table = self.type_table.borrow();
                    class_name = if let Some(type_ref) = type_table.get(*class_type) {
                        if let crate::tast::TypeKind::Class { symbol_id, .. } = &type_ref.kind {
                            self.symbol_table.get_symbol(*symbol_id)
                                .and_then(|sym| self.string_interner.get(sym.name))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    drop(type_table);
                }

                // FALLBACK #2: If TypeId lookup failed (e.g., for extern stdlib classes that aren't
                // pre-registered because Channel.hx is skipped), try getting class name from the
                // actual_symbol_id which comes from the HIR New expression
                if class_name.is_none() {
                    if let Some(sym_id) = actual_symbol_id {
                        class_name = self.symbol_table.get_symbol(sym_id)
                            .and_then(|sym| self.string_interner.get(sym.name));
                    }
                }

                // FALLBACK #3: If still no class name and TypeId is invalid (u32::MAX),
                // try checking all stdlib registered class names to see if ANY constructor matches
                // This is a last resort for extern stdlib classes that weren't pre-registered
                if class_name.is_none() && *class_type == TypeId::from_raw(u32::MAX) {
                    let stdlib_mapping = crate::stdlib::runtime_mapping::StdlibMapping::new();

                    // Get ALL classes that have registered constructors from the stdlib mapping
                    let constructor_classes = stdlib_mapping.get_constructor_classes();

                    // Try each registered constructor class
                    for potential_class in constructor_classes {
                        let method_sig = crate::stdlib::runtime_mapping::MethodSignature {
                            class: potential_class,
                            method: "new",
                            is_static: true,
                            is_constructor: true,
                            param_count: 0,
                        };
                        if stdlib_mapping.get(&method_sig).is_some() {
                            class_name = Some(potential_class);
                            break;
                        }
                    }
                }
                eprintln!("DEBUG [NEW EXPR]: resolved class_name={:?}", class_name);

                // MONOMORPHIZATION: For generic extern classes like Vec<T>, monomorphize the class name
                // based on type arguments. Vec<Int> -> VecI32, Vec<Float> -> VecF64, etc.
                // Use hir_type_args directly (from HIR) instead of type_table lookup (which may fail for extern classes)
                let monomorphized_class_name: Option<String> = if let Some(base_name) = class_name {
                    if base_name == "Vec" && !hir_type_args.is_empty() {
                        // Get the first type argument and determine the monomorphized suffix
                        let first_arg = hir_type_args[0];
                        let type_table = self.type_table.borrow();
                        let suffix = if let Some(arg_type) = type_table.get(first_arg) {
                            match &arg_type.kind {
                                crate::tast::TypeKind::Int => Some("I32"),
                                crate::tast::TypeKind::Float => Some("F64"),
                                crate::tast::TypeKind::Bool => Some("Bool"),
                                crate::tast::TypeKind::String => Some("Ptr"),
                                crate::tast::TypeKind::Class { symbol_id, .. } => {
                                    // Check if it's Int64 (a class type representing 64-bit int)
                                    if let Some(class_info) = self.symbol_table.get_symbol(*symbol_id) {
                                        if let Some(name) = self.string_interner.get(class_info.name) {
                                            if name == "Int64" {
                                                Some("I64")
                                            } else {
                                                Some("Ptr") // Other classes are reference types
                                            }
                                        } else {
                                            Some("Ptr")
                                        }
                                    } else {
                                        Some("Ptr")
                                    }
                                }
                                _ => Some("Ptr"),
                            }
                        } else {
                            Some("Ptr") // If type not found, default to Ptr variant
                        };
                        drop(type_table);
                        suffix.map(|s| format!("Vec{}", s))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Use monomorphized name if available, otherwise use original class name
                let final_class_name = monomorphized_class_name.as_deref().or(class_name);
                eprintln!("DEBUG [NEW EXPR]: final_class_name={:?} (monomorphized from {:?})", final_class_name, class_name);

                if let Some(class_name) = final_class_name {
                    // Check if this class has a "new" constructor registered in the runtime mapping
                    let stdlib_mapping = crate::stdlib::runtime_mapping::StdlibMapping::new();

                    // Use find_constructor to look up the registered constructor mapping
                    // This returns both the MethodSignature and RuntimeFunctionCall from the registry
                    // PRIORITY: Try qualified class name FIRST (e.g., "sys_thread_Mutex")
                    // This ensures sys.thread.Mutex uses sys_mutex_alloc, not Mutex_init (rayzor.concurrent.Mutex)
                    let mut found_constructor: Option<(&crate::stdlib::MethodSignature, &crate::stdlib::RuntimeFunctionCall)> = None;
                    if let Some(sym_id) = actual_symbol_id {
                        if let Some(sym) = self.symbol_table.get_symbol(sym_id) {
                            if let Some(qn) = sym.qualified_name {
                                if let Some(qual_name) = self.string_interner.get(qn) {
                                    // Convert "sys.thread.Mutex" to "sys_thread_Mutex"
                                    let qualified_class_name = qual_name.replace(".", "_");
                                    found_constructor = stdlib_mapping.find_constructor(&qualified_class_name);
                                    eprintln!("DEBUG [NEW EXPR]: find_constructor(qualified='{}') = {:?}",
                                        qualified_class_name, found_constructor.as_ref().map(|(_, rc)| rc.runtime_name));
                                }
                            }
                        }
                    }

                    // FALLBACK: If not found via qualified name, try simple class name (e.g., "Mutex")
                    if found_constructor.is_none() {
                        found_constructor = stdlib_mapping.find_constructor(class_name);
                        eprintln!("DEBUG [NEW EXPR]: find_constructor('{}') = {:?}", class_name, found_constructor.as_ref().map(|(_, rc)| rc.runtime_name));
                    }
                    if let Some((_method_sig, runtime_call)) = found_constructor {
                        // Found a constructor mapping!
                        let wrapper_name = runtime_call.runtime_name;

                        // Lower arguments
                        let arg_regs: Vec<_> = args
                            .iter()
                            .filter_map(|a| self.lower_expression(a))
                            .collect();

                        // Register forward ref if not already present
                        let param_types: Vec<IrType> = arg_regs
                            .iter()
                            .map(|reg| self.builder.get_register_type(*reg).unwrap_or(IrType::Any))
                            .collect();

                        // For extern classes, the return type should be a pointer (opaque handle),
                        // not the class struct itself
                        let result_type = IrType::Ptr(Box::new(IrType::U8));

                        // For constructors that return primitive (direct extern call), use extern function
                        // For constructors that need_out_param (wrapper), use MIR forward ref
                        let wrapper_func_id = if runtime_call.needs_out_param {
                            // Complex constructors need a MIR wrapper
                            self.register_stdlib_mir_forward_ref(
                                wrapper_name,
                                param_types,
                                result_type.clone(),
                            )
                        } else {
                            // Simple constructors are direct extern calls
                            self.get_or_register_extern_function(
                                wrapper_name,
                                param_types,
                                result_type.clone(),
                            )
                        };

                        // Call the wrapper and return the result
                        let result = self.builder.build_call_direct(
                            wrapper_func_id,
                            arg_regs,
                            result_type,
                        );
                        return result;
                    }
                }

                // Check if constructor exists - try both TypeId and TypeId derived from SymbolId
                let mut constructor_type_id = *class_type;
                let mut has_constructor = self.constructor_map.contains_key(class_type);

                // If not found and we have a SymbolId, try TypeId derived from SymbolId as fallback
                if !has_constructor {
                    if let Some(sym_id) = actual_symbol_id {
                        let type_id_from_symbol = TypeId::from_raw(sym_id.as_raw());
                        if self.constructor_map.contains_key(&type_id_from_symbol) {
                            constructor_type_id = type_id_from_symbol;
                            has_constructor = true;
                        }
                    }
                }

                // If no constructor exists and we have exactly one argument, treat as value wrap
                // This handles abstract types that weren't properly detected above
                if !has_constructor && args.len() == 1 {
                    // eprintln!("DEBUG: No constructor found for TypeId={:?}, single argument - treating as value wrap", class_type);
                    let result = self.lower_expression(&args[0]);
                    return result;
                }

                // SPECIAL CASE: Array constructor (@:coreType extern class)
                // Array needs special handling - call haxe_array_new() runtime function
                let type_table = self.type_table.borrow();
                let is_array = if let Some(type_ref) = type_table.get(*class_type) {
                    matches!(type_ref.kind, crate::tast::TypeKind::Array { .. })
                } else {
                    false
                };
                drop(type_table);

                if is_array {
                    // Allocate HaxeArray struct on stack and zero-initialize it
                    // The runtime functions (push, etc.) will handle initialization on first use
                    let class_mir_type = self.convert_type(*class_type);
                    let array_ptr = self.builder.build_alloc(class_mir_type.clone(), None)?;

                    // Zero-initialize the HaxeArray struct (ptr=null, len=0, cap=0, elem_size=8)
                    // This represents an empty uninitialized array
                    if let Some(zero_i64) = self.builder.build_const(IrValue::I64(0)) {
                        // Zero out ptr field (offset 0)
                        if let Some(index_0) = self.builder.build_const(IrValue::I32(0)) {
                            if let Some(ptr_field) = self.builder.build_gep(array_ptr, vec![index_0], IrType::I64) {
                                self.builder.build_store(ptr_field, zero_i64);
                            }
                        }
                        // Zero out len field (offset 8)
                        if let Some(index_1) = self.builder.build_const(IrValue::I32(1)) {
                            if let Some(len_field) = self.builder.build_gep(array_ptr, vec![index_1], IrType::I64) {
                                self.builder.build_store(len_field, zero_i64);
                            }
                        }
                        // Zero out cap field (offset 16)
                        if let Some(index_2) = self.builder.build_const(IrValue::I32(2)) {
                            if let Some(cap_field) = self.builder.build_gep(array_ptr, vec![index_2], IrType::I64) {
                                self.builder.build_store(cap_field, zero_i64);
                            }
                        }
                        // Set elem_size field to 8 bytes (offset 24) - assume pointer size for now
                        if let Some(elem_size_val) = self.builder.build_const(IrValue::I64(8)) {
                            if let Some(index_3) = self.builder.build_const(IrValue::I32(3)) {
                                if let Some(elem_size_field) = self.builder.build_gep(array_ptr, vec![index_3], IrType::I64) {
                                    self.builder.build_store(elem_size_field, elem_size_val);
                                }
                            }
                        }
                    }

                    // Return the zero-initialized array pointer
                    return Some(array_ptr);
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
                        if let Some(field_ptr) =
                            self.builder
                                .build_gep(obj_ptr, vec![index_const], IrType::I32)
                        {
                            self.builder.build_store(field_ptr, zero);
                        }
                    }
                }

                // eprintln!("DEBUG: Class type constructor - allocated object");
                // eprintln!("DEBUG: Available constructors: {:?}", self.constructor_map.keys().collect::<Vec<_>>());

                // Look up constructor by TypeId - use the resolved constructor_type_id
                let constructor_func_id = self.constructor_map.get(&constructor_type_id).copied();

                if let Some(constructor_func_id) = constructor_func_id {
                    // eprintln!("DEBUG: Found constructor FuncId={:?} for TypeId={:?}", constructor_func_id, constructor_type_id);

                    // Call constructor with object as first argument
                    let arg_regs: Vec<_> = std::iter::once(obj_ptr)
                        .chain(args.iter().filter_map(|a| self.lower_expression(a)))
                        .collect();

                    // Constructor returns void, so we ignore the result
                    self.builder
                        .build_call_direct(constructor_func_id, arg_regs, IrType::Void);
                } else {
                    // eprintln!("WARNING: Constructor not found for TypeId {:?}", class_type);
                }

                Some(obj_ptr)
            }

            HirExprKind::Unary { op, operand } => {
                // Handle increment/decrement operators specially
                match op {
                    HirUnaryOp::PostIncr | HirUnaryOp::PreIncr | HirUnaryOp::PostDecr | HirUnaryOp::PreDecr => {
                        // For increment/decrement, we need to:
                        // 1. Load the current value
                        // 2. Compute new value (old ± 1)
                        // 3. Store the new value back
                        // 4. Return old value (post) or new value (pre)

                        let old_value = self.lower_expression(operand)?;
                        let one = self.builder.build_const(IrValue::I32(1))?;

                        let is_increment = matches!(op, HirUnaryOp::PostIncr | HirUnaryOp::PreIncr);
                        let new_value = if is_increment {
                            self.builder.build_binop(BinaryOp::Add, old_value, one)?
                        } else {
                            self.builder.build_binop(BinaryOp::Sub, old_value, one)?
                        };

                        // Register the new_value with its type
                        let result_type = self.convert_type(expr.ty);
                        let src_loc = self.convert_source_location(&expr.source_location);
                        if let Some(func) = self.builder.current_function_mut() {
                            func.locals.insert(
                                new_value,
                                super::IrLocal {
                                    name: format!("_incr{}", new_value.0),
                                    ty: result_type.clone(),
                                    mutable: false,
                                    source_location: src_loc.clone(),
                                    allocation: super::AllocationHint::Stack,
                                },
                            );
                        }

                        // Update the variable binding (SSA style)
                        if let HirExprKind::Variable { symbol, .. } = &operand.kind {
                            // If we're inside a lambda with captured variables, also store back to environment
                            if let Some(ref env_layout) = self.current_env_layout {
                                if env_layout.find_field(*symbol).is_some() {
                                    // This is a captured variable - store it back to environment
                                    let env_ptr = IrId::new(0);  // First parameter in lambda is environment pointer
                                    env_layout.store_field(&mut self.builder, env_ptr, *symbol, new_value);
                                }
                            }

                            self.symbol_map.insert(*symbol, new_value);
                        }

                        // Return appropriate value
                        let result_reg = match op {
                            HirUnaryOp::PostIncr | HirUnaryOp::PostDecr => old_value,  // Post: return old value
                            HirUnaryOp::PreIncr | HirUnaryOp::PreDecr => new_value,    // Pre: return new value
                            _ => unreachable!(),
                        };

                        Some(result_reg)
                    }
                    _ => {
                        // Handle other unary operators normally
                        let operand_reg = self.lower_expression(operand)?;
                        let result_reg = self
                            .builder
                            .build_unop(self.convert_unary_op(*op), operand_reg)?;

                        // Register the result with its type so Cranelift can find it
                        let result_type = self.convert_type(expr.ty);
                        let src_loc = self.convert_source_location(&expr.source_location);
                        if let Some(func) = self.builder.current_function_mut() {
                            func.locals.insert(
                                result_reg,
                                super::IrLocal {
                                    name: format!("_temp{}", result_reg.0),
                                    ty: result_type,
                                    mutable: false,
                                    source_location: src_loc,
                                    allocation: super::AllocationHint::Stack,
                                },
                            );
                        }

                        Some(result_reg)
                    }
                }
            }

            HirExprKind::Binary { op, lhs, rhs } => {
                // Handle short-circuit operators specially
                match op {
                    HirBinaryOp::And => return self.lower_logical_and(lhs, rhs),
                    HirBinaryOp::Or => return self.lower_logical_or(lhs, rhs),
                    _ => {}
                }

                // Special handling for string concatenation with +
                if matches!(op, HirBinaryOp::Add) {
                    let lhs_type = self.convert_type(lhs.ty);
                    let rhs_type = self.convert_type(rhs.ty);

                    let lhs_is_string = matches!(&lhs_type, IrType::String) ||
                        matches!(&lhs_type, IrType::Ptr(inner) if matches!(inner.as_ref(), IrType::String));
                    let rhs_is_string = matches!(&rhs_type, IrType::String) ||
                        matches!(&rhs_type, IrType::Ptr(inner) if matches!(inner.as_ref(), IrType::String));

                    if lhs_is_string || rhs_is_string {
                        eprintln!("DEBUG: [STRING CONCAT] Detected string concatenation, lhs_type={:?}, rhs_type={:?}", lhs_type, rhs_type);

                        // Lower both operands
                        let lhs_reg = self.lower_expression(lhs)?;
                        let rhs_reg = self.lower_expression(rhs)?;

                        // Convert non-string operand to string if needed
                        let lhs_str_val = if !lhs_is_string {
                            self.convert_to_string(lhs_reg, &lhs_type)?
                        } else {
                            lhs_reg
                        };

                        let rhs_str_val = if !rhs_is_string {
                            self.convert_to_string(rhs_reg, &rhs_type)?
                        } else {
                            rhs_reg
                        };

                        // String values are already pointers (*HaxeString):
                        // - string literals from haxe_string_literal return *mut HaxeString
                        // - conversion functions like int_to_string also return pointers
                        // Pass them directly to string_concat which expects *HaxeString args
                        let string_ptr_ty = IrType::Ptr(Box::new(IrType::String));
                        let concat_func_id = self.register_stdlib_mir_forward_ref(
                            "string_concat",
                            vec![string_ptr_ty.clone(), string_ptr_ty.clone()],
                            string_ptr_ty.clone(),
                        );

                        return self.builder.build_call_direct(
                            concat_func_id,
                            vec![lhs_str_val, rhs_str_val],
                            string_ptr_ty,
                        );
                    }
                }

                let mut lhs_reg = self.lower_expression(lhs)?;
                let mut rhs_reg = self.lower_expression(rhs)?;

                // Special handling for division: Haxe always returns Float from division
                // If operands are integers, convert them to float first
                if matches!(op, HirBinaryOp::Div) {
                    let lhs_type = self.convert_type(lhs.ty);
                    let rhs_type = self.convert_type(rhs.ty);

                    // Convert integer operands to float
                    if matches!(
                        lhs_type,
                        IrType::I8
                            | IrType::I16
                            | IrType::I32
                            | IrType::I64
                            | IrType::U8
                            | IrType::U16
                            | IrType::U32
                            | IrType::U64
                    ) {
                        lhs_reg = self.builder.build_cast(lhs_reg, lhs_type, IrType::F64)?;
                    }
                    if matches!(
                        rhs_type,
                        IrType::I8
                            | IrType::I16
                            | IrType::I32
                            | IrType::I64
                            | IrType::U8
                            | IrType::U16
                            | IrType::U32
                            | IrType::U64
                    ) {
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
                    func.locals.insert(
                        result_reg,
                        super::IrLocal {
                            name: format!("_temp{}", result_reg.0),
                            ty: result_type,
                            mutable: false,
                            source_location: src_loc,
                            allocation: super::AllocationHint::Stack,
                        },
                    );
                }

                Some(result_reg)
            }

            HirExprKind::Cast { expr, target, .. } => {
                let value_reg = self.lower_expression(expr)?;
                let from_type = self.convert_type(expr.ty);
                let to_type = self.convert_type(*target);
                self.builder.build_cast(value_reg, from_type, to_type)
            }

            HirExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => self.lower_conditional(condition, then_expr, else_expr),

            HirExprKind::Block(block) => {
                self.lower_block(block);
                // Block expressions can return values through their trailing expression
                None // Simplified for now
            }

            HirExprKind::Lambda {
                params,
                body,
                captures,
            } => {
                eprintln!("DEBUG: Lowering lambda with {} params, {} captures", params.len(), captures.len());
                self.lower_lambda(params, body, captures, expr.ty)
            },

            HirExprKind::Array { elements } => self.lower_array_literal(elements),

            HirExprKind::Map { entries } => self.lower_map_literal(entries),

            HirExprKind::ObjectLiteral { fields } => self.lower_object_literal(fields),

            HirExprKind::ArrayComprehension { .. } => {
                // Array comprehensions are desugared to loops
                self.add_error(
                    "Array comprehensions not yet implemented in MIR",
                    expr.source_location,
                );
                None
            }

            HirExprKind::StringInterpolation { parts } => self.lower_string_interpolation(parts),

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

            HirExprKind::Null => self.builder.build_null(),

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
        eprintln!("DEBUG: lower_if_statement called, has_else={}", else_branch.is_some());
        let Some(then_block) = self.builder.create_block() else {
            return;
        };
        let Some(merge_block) = self.builder.create_block() else {
            return;
        };

        let else_block = if else_branch.is_some() {
            self.builder.create_block().unwrap_or(merge_block)
        } else {
            merge_block
        };

        // Get the current block before branching
        let entry_block = if let Some(block_id) = self.builder.current_block() {
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

        eprintln!("DEBUG: modified_vars.len() = {}", modified_vars.len());

        // Save initial values of variables that will be modified
        let mut var_initial_values: HashMap<SymbolId, (IrId, IrType)> = HashMap::new();
        for symbol_id in &modified_vars {
            if let Some(&reg) = self.symbol_map.get(symbol_id) {
                // Get the type from the locals table
                if let Some(func) = self.builder.current_function() {
                    if let Some(local) = func.locals.get(&reg) {
                        eprintln!("DEBUG: var {:?} has initial value {:?}", symbol_id, reg);
                        var_initial_values.insert(*symbol_id, (reg, local.ty.clone()));
                    }
                }
            } else {
                eprintln!("DEBUG: var {:?} NOT in symbol_map (new in branch)", symbol_id);
            }
        }

        eprintln!("DEBUG: var_initial_values.len() = {}", var_initial_values.len());

        // Evaluate condition
        if let Some(cond_reg) = self.lower_expression(condition) {
            self.builder
                .build_cond_branch(cond_reg, then_block, else_block);

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
                // Only collect values for variables that existed BEFORE the if/else
                // Variables defined within the then branch should not be added
                for symbol_id in var_initial_values.keys() {
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
                    // Only collect values for variables that existed BEFORE the if/else
                    // Variables defined within one branch should not leak into the other
                    for symbol_id in var_initial_values.keys() {
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
            eprintln!("DEBUG: var_initial_values.len() = {}", var_initial_values.len());
            for (symbol_id, (initial_reg, var_type)) in &var_initial_values {
                eprintln!("DEBUG: Checking var {:?} for phi node", symbol_id);
                // Only create phi if at least one branch modified the variable
                let then_val = then_values.get(symbol_id).copied().unwrap_or(*initial_reg);
                let else_val = else_values.get(symbol_id).copied().unwrap_or(*initial_reg);

                eprintln!("DEBUG:   then_val {:?}, else_val {:?}", then_val, else_val);

                // If both branches lead to the same value, no phi needed
                if then_val == else_val {
                    eprintln!("DEBUG:   Skipping phi - same value");
                    continue;
                }

                // Only create phi if we have valid incoming blocks
                if then_end_block.is_none() && else_end_block.is_none() {
                    eprintln!("DEBUG:   Skipping phi - no valid incoming blocks");
                    continue;
                }

                eprintln!("DEBUG: Creating phi for symbol {:?}, then_val {:?}, else_val {:?}", symbol_id, then_val, else_val);
                eprintln!("DEBUG:   then_end_block {:?}, else_end_block {:?}", then_end_block, else_end_block);

                // Create phi node
                if let Some(phi_reg) = self.builder.build_phi(merge_block, var_type.clone()) {
                    eprintln!("DEBUG:   Created phi node {:?} in merge block {:?}", phi_reg, merge_block);

                    // Add incoming values from both branches
                    if let Some(then_blk) = then_end_block {
                        eprintln!("DEBUG:   Adding phi incoming from then block {:?}, value {:?}", then_blk, then_val);
                        self.builder
                            .add_phi_incoming(merge_block, phi_reg, then_blk, then_val);
                    }
                    if let Some(else_blk) = else_end_block {
                        eprintln!("DEBUG:   Adding phi incoming from else block {:?}, value {:?}", else_blk, else_val);
                        self.builder
                            .add_phi_incoming(merge_block, phi_reg, else_blk, else_val);
                    }

                    // Register the phi node as a local
                    if let Some(func) = self.builder.current_function_mut() {
                        if let Some(local) = func.locals.get(initial_reg).cloned() {
                            func.locals.insert(
                                phi_reg,
                                super::IrLocal {
                                    name: format!("{}_phi", local.name),
                                    ty: var_type.clone(),
                                    mutable: true,
                                    source_location: local.source_location,
                                    allocation: super::AllocationHint::Register,
                                },
                            );
                        }
                    }

                    // Update symbol map to use phi node
                    self.symbol_map.insert(*symbol_id, phi_reg);
                }
            }
        }
    }

    /// Lower while loop
    fn lower_while_loop(&mut self, condition: &HirExpr, body: &HirBlock, label: Option<&SymbolId>) {
        let Some(cond_block) = self.builder.create_block() else {
            return;
        };
        let Some(body_block) = self.builder.create_block() else {
            return;
        };
        let Some(exit_block) = self.builder.create_block() else {
            return;
        };

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

        // Collect all variables referenced in body
        self.collect_referenced_variables_in_block(body, &mut referenced_vars);

        // Only include variables that were declared before the loop
        // (i.e., they're already in the symbol_map)
        // Exclude function parameters since they're immutable
        let modified_vars: std::collections::HashSet<SymbolId> = referenced_vars
            .into_iter()
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
                in_map && !is_param
            })
            .collect();

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
                self.builder
                    .add_phi_incoming(cond_block, phi_reg, entry_block, *initial_reg);
                // eprintln!("DEBUG: Added incoming edge from entry block {:?}", entry_block);

                // Register the phi node as a local so Cranelift can find its type
                if let Some(func) = self.builder.current_function_mut() {
                    if let Some(local) = func.locals.get(initial_reg).cloned() {
                        func.locals.insert(
                            phi_reg,
                            super::IrLocal {
                                name: format!("{}_phi", local.name),
                                ty: var_type.clone(),
                                mutable: true,
                                source_location: local.source_location,
                                allocation: super::AllocationHint::Register,
                            },
                        );
                    }
                }

                // Update symbol map to use phi node
                phi_nodes.insert(*symbol_id, phi_reg);
                self.symbol_map.insert(*symbol_id, phi_reg);
            }
        }
        // eprintln!("DEBUG: Created {} phi nodes", phi_nodes.len());

        // Push loop context (exit phi nodes will be added after condition evaluation)
        // We need to evaluate the condition FIRST to know which block we're actually in
        // (short-circuit operators like && create additional blocks)
        self.loop_stack.push(LoopContext {
            continue_block: cond_block,
            break_block: exit_block,
            label: label.cloned(),
            exit_phi_nodes: HashMap::new(), // Will be populated after condition eval
        });

        // Evaluate condition - this may create additional blocks for short-circuit operators!
        // After this, we may be in a different block than cond_block
        let cond_result = self.lower_expression(condition);

        // Capture the block we're actually in AFTER condition evaluation
        // This is where the conditional branch to body/exit will happen
        let cond_end_block = self.builder.current_block().unwrap_or(cond_block);

        // Now create exit block phi nodes with the correct predecessor block
        let mut exit_phi_nodes: HashMap<SymbolId, IrId> = HashMap::new();
        for (symbol_id, loop_phi_reg) in &phi_nodes {
            if let Some((_, var_type)) = loop_var_initial_values.get(symbol_id) {
                // Allocate a new register for the exit block parameter
                let exit_param_reg = self.builder.alloc_reg().unwrap();

                // Create the phi node in the exit block with incoming edge from the ACTUAL
                // block that branches to exit (cond_end_block, not necessarily cond_block)
                if let Some(func) = self.builder.current_function_mut() {
                    if let Some(exit_block_data) = func.cfg.get_block_mut(exit_block) {
                        let exit_phi = super::IrPhiNode {
                            dest: exit_param_reg,
                            incoming: vec![(cond_end_block, *loop_phi_reg)],
                            ty: var_type.clone(),
                        };
                        exit_block_data.add_phi(exit_phi);

                        // Register as a local
                        func.locals.insert(
                            exit_param_reg,
                            super::IrLocal {
                                name: format!("loop_exit_{}", symbol_id.as_raw()),
                                ty: var_type.clone(),
                                mutable: false,
                                source_location: super::IrSourceLocation::unknown(),
                                allocation: super::AllocationHint::Register,
                            },
                        );
                    }
                }

                exit_phi_nodes.insert(*symbol_id, exit_param_reg);
            }
        }

        // Update loop context with the exit phi nodes
        if let Some(loop_ctx) = self.loop_stack.last_mut() {
            loop_ctx.exit_phi_nodes = exit_phi_nodes.clone();
        }

        // Build conditional branch from the block we're actually in
        if let Some(cond_reg) = cond_result {
            self.builder
                .build_cond_branch(cond_reg, body_block, exit_block);
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
            // Get the current value of the variable after the loop body
            let back_edge_value = if let Some(&updated_reg) = self.symbol_map.get(symbol_id) {
                // If the variable was updated, use the new value
                updated_reg
            } else {
                // If not updated, use the phi node itself (the value from the previous iteration)
                *phi_reg
            };

            // ALWAYS add the back-edge, even if the variable wasn't modified
            // This is required for SSA correctness - every phi node must have an incoming
            // value from each predecessor block
            self.builder.add_phi_incoming(
                cond_block,
                *phi_reg,
                body_end_block,
                back_edge_value,
            );
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

        // Update symbol map to use exit phi nodes (already created before loop body)
        for (symbol_id, exit_param_reg) in &exit_phi_nodes {
            self.symbol_map.insert(*symbol_id, *exit_param_reg);
        }
    }

    // Helper methods...

    /// Collect all variables referenced in a block
    fn collect_referenced_variables_in_block(
        &self,
        block: &HirBlock,
        vars: &mut std::collections::HashSet<SymbolId>,
    ) {
        for stmt in &block.statements {
            self.collect_referenced_variables_in_stmt(stmt, vars);
        }
    }

    /// Collect all variables referenced in a statement
    fn collect_referenced_variables_in_stmt(
        &self,
        stmt: &HirStatement,
        vars: &mut std::collections::HashSet<SymbolId>,
    ) {
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
            HirStatement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.collect_referenced_variables_in_expr(condition, vars);
                self.collect_referenced_variables_in_block(then_branch, vars);
                if let Some(else_blk) = else_branch {
                    self.collect_referenced_variables_in_block(else_blk, vars);
                }
            }
            HirStatement::While {
                condition, body, ..
            }
            | HirStatement::DoWhile {
                condition, body, ..
            } => {
                self.collect_referenced_variables_in_expr(condition, vars);
                self.collect_referenced_variables_in_block(body, vars);
            }
            _ => {}
        }
    }

    /// Collect all variables referenced in an expression
    fn collect_referenced_variables_in_expr(
        &self,
        expr: &HirExpr,
        vars: &mut std::collections::HashSet<SymbolId>,
    ) {
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
            HirExprKind::If {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
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
    fn find_modified_variables_in_block(
        &self,
        block: &HirBlock,
    ) -> std::collections::HashSet<SymbolId> {
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
            HirStatement::If {
                then_branch,
                else_branch,
                ..
            } => {
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
            HirExprKind::If {
                then_expr,
                else_expr,
                ..
            } => {
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
            _ => UnaryOp::Neg,                  // Default
        }
    }

    fn convert_type(&self, type_id: TypeId) -> IrType {
        use crate::tast::TypeKind;

        // Look up the type in the type table
        let type_table = self.type_table.borrow();
        let type_ref = type_table.get(type_id);

        // DEBUG: Trace type conversion
        // eprintln!("DEBUG [convert_type] type_id={:?}, type_kind={:?}", type_id, type_ref.as_ref().map(|t| &t.kind));

        match type_ref.as_ref().map(|t| &t.kind) {
            // Primitive types
            Some(TypeKind::Int) => IrType::I32,
            Some(TypeKind::Float) => IrType::F64,
            Some(TypeKind::Bool) => IrType::Bool,
            Some(TypeKind::Void) => IrType::Void,
            Some(TypeKind::String) => IrType::String,

            // Function types - represented as function pointers (i64)
            Some(TypeKind::Function {
                params,
                return_type,
                ..
            }) => {
                // Convert parameter types
                let param_types: Vec<IrType> =
                    params.iter().map(|p| self.convert_type(*p)).collect();

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
            Some(TypeKind::Enum { .. }) => IrType::I64, // Enums as discriminant values (i64 to match Haxe Int)
            Some(TypeKind::Array { element_type, .. }) => {
                // HaxeArray is an opaque runtime structure, represented as Ptr(Void)
                // regardless of element type. Element type information is tracked at runtime.
                IrType::Ptr(Box::new(IrType::Void))
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
            Some(TypeKind::TypeParameter { symbol_id, .. }) => {
                // Type parameters like T, U, etc. - convert to IrType::TypeVar for monomorphization
                // Look up the symbol name to get the type parameter name
                let type_param_name = self.symbol_table.get_symbol(*symbol_id)
                    .and_then(|sym| self.string_interner.get(sym.name))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("T{}", symbol_id.as_raw()));

                eprintln!("DEBUG: Converting TypeParameter {:?} to TypeVar(\"{}\")", symbol_id, type_param_name);
                IrType::TypeVar(type_param_name)
            },
            Some(TypeKind::Dynamic) => {
                // Dynamic type is used as a placeholder for unresolved generic type parameters
                // in stdlib (e.g., ArrayIterator<T>.next() where T is unresolved).
                // Since dynamic values can be any type including objects/pointers, we treat
                // them as pointer-sized values to avoid truncation bugs.
                IrType::Ptr(Box::new(IrType::Void))
            }

            // Unknown or error types
            Some(TypeKind::Unknown) | Some(TypeKind::Error) => {
                // Unknown or error types - treat as pointer-sized values to avoid truncation.
                // These may be unresolved generic class instances that need full 64-bit values.
                eprintln!("Warning: Unknown/Error type {:?}, defaulting to Ptr(Void)", type_id);
                IrType::Ptr(Box::new(IrType::Void))
            }

            // Generic instance types (ArrayIterator<T>, Map<K,V>, etc.) - pointer to instantiated class
            Some(TypeKind::GenericInstance { .. }) => IrType::Ptr(Box::new(IrType::Void)),

            // Map type - pointer to map structure
            Some(TypeKind::Map { .. }) => IrType::Ptr(Box::new(IrType::Void)),

            // Anonymous structure type - pointer to struct
            Some(TypeKind::Anonymous { .. }) => IrType::Ptr(Box::new(IrType::Void)),

            // Union type - pointer (can hold any of the union members)
            Some(TypeKind::Union { .. }) => IrType::Ptr(Box::new(IrType::Void)),

            // Intersection type - pointer to combined type
            Some(TypeKind::Intersection { .. }) => IrType::Ptr(Box::new(IrType::Void)),

            // Type alias - resolve to target type
            Some(TypeKind::TypeAlias { target_type, .. }) => {
                self.convert_type(*target_type)
            }

            // Placeholder type - treat as pointer (will be resolved later)
            Some(TypeKind::Placeholder { .. }) => IrType::Ptr(Box::new(IrType::Void)),

            // Char type - single character, represented as i32
            Some(TypeKind::Char) => IrType::I32,

            None => {
                // Type not found in type table
                // This often happens for generic type parameters that weren't resolved,
                // like T in ArrayIterator<T>.next() returning array[current++].
                // Use Ptr(Void) to avoid truncation when these are actually pointers/objects.
                // TODO: Properly resolve generic type parameters from instantiation context.
                eprintln!("Warning: Type {:?} not found in type table, defaulting to Ptr(Void)", type_id);
                IrType::Ptr(Box::new(IrType::Void))
            }

            // Catch-all for other types
            Some(other) => {
                eprintln!(
                    "Warning: Unhandled type kind for {:?}: {:?}, defaulting to Ptr(Void)",
                    type_id, other
                );
                IrType::Ptr(Box::new(IrType::Void))
            }
        }
    }

    /// Extract the element type from an Array type.
    /// If the type is Array<T>, returns Some(T). Otherwise returns None.
    fn get_array_element_type(&self, type_id: TypeId) -> Option<TypeId> {
        use crate::tast::TypeKind;
        let type_table = self.type_table.borrow();
        let type_ref = type_table.get(type_id)?;
        match &type_ref.kind {
            TypeKind::Array { element_type, .. } => Some(*element_type),
            _ => None,
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
            HirLiteral::String(s) => {
                // Resolve the interned string to get the actual content
                let string_content = self.string_interner.get(*s)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| String::new());
                self.builder.build_string(string_content)
            }
            HirLiteral::Bool(b) => self.builder.build_bool(*b),
            HirLiteral::Regex { .. } => {
                self.add_error(
                    "Regex literals not yet supported in MIR",
                    SourceLocation::unknown(),
                );
                None
            }
        }
    }

    fn build_function_signature(&self, func: &HirFunction) -> super::IrFunctionSignature {
        let mut builder = FunctionSignatureBuilder::new();

        // Add type parameters from the HIR function (for generic functions)
        for type_param in &func.type_params {
            let param_name = self.string_interner.get(type_param.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("T{}", type_param.name.as_raw()));
            builder = builder.type_param(param_name);
        }

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

    /// Build function signature with class type parameters (for generic class methods)
    fn build_function_signature_with_class_type_params(
        &self,
        func: &HirFunction,
        class_type_params: &[HirTypeParam],
    ) -> super::IrFunctionSignature {
        let mut builder = FunctionSignatureBuilder::new();

        // Add class type parameters first (T, U, etc from the generic class)
        for type_param in class_type_params {
            let param_name = self.string_interner.get(type_param.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("T{}", type_param.name.as_raw()));
            builder = builder.type_param(param_name);
        }

        // Add method's own type parameters (if any)
        for type_param in &func.type_params {
            let param_name = self.string_interner.get(type_param.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("T{}", type_param.name.as_raw()));
            builder = builder.type_param(param_name);
        }

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
    fn build_instance_method_signature(
        &self,
        func: &HirFunction,
        class_type_id: TypeId,
    ) -> super::IrFunctionSignature {
        let mut builder = FunctionSignatureBuilder::new();

        // Add type parameters from the HIR function (for generic methods)
        for type_param in &func.type_params {
            let param_name = self.string_interner.get(type_param.name)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("T{}", type_param.name.as_raw()));
            builder = builder.type_param(param_name);
        }

        // Add implicit 'this' parameter as first parameter
        // 'this' is always a pointer to the class instance, regardless of generic parameters
        let this_type = match self.convert_type(class_type_id) {
            IrType::Ptr(_) => IrType::Ptr(Box::new(IrType::Void)),
            // If convert_type failed to resolve (e.g., generic class without instantiation),
            // default to pointer since 'this' is always a pointer to instance
            _ => IrType::Ptr(Box::new(IrType::Void)),
        };

        builder = builder.param("this".to_string(), this_type);

        // Add regular parameters
        for param in &func.params {
            let param_name = self
                .string_interner
                .get(param.name)
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

        self.builder
            .current_function()
            .and_then(|func| func.cfg.get_block(block_id))
            .map(|block| block.is_terminated())
            .unwrap_or(false)
    }

    fn ensure_terminator(&mut self) {
        let is_term = self.is_terminated();
        // eprintln!(
        //     "DEBUG ensure_terminator: is_terminated={}, current_func={:?}",
        //     is_term,
        //     self.builder.current_function().map(|f| &f.name)
        // );
        if !is_term {
            // eprintln!("DEBUG ensure_terminator: Adding implicit return(None)");
            self.builder.build_return(None);
        }
    }

    fn find_loop_context(&self, label: Option<&SymbolId>) -> Option<&LoopContext> {
        if let Some(label) = label {
            self.loop_stack
                .iter()
                .rev()
                .find(|ctx| ctx.label.as_ref() == Some(label))
        } else {
            self.loop_stack.last()
        }
    }


    /// Insert automatic boxing if needed when assigning to Dynamic
    /// Returns the (potentially boxed) value and whether boxing was applied
    /// Convert a value to a string pointer
    /// Uses the appropriate *_to_string MIR wrapper based on the source type
    fn convert_to_string(&mut self, value: IrId, from_type: &IrType) -> Option<IrId> {
        let mir_wrapper = match from_type {
            IrType::I32 | IrType::I64 => "int_to_string",
            IrType::F32 | IrType::F64 => "float_to_string",
            IrType::Bool => "bool_to_string",
            IrType::String => {
                // Already a string
                return Some(value);
            }
            IrType::Ptr(inner) if matches!(inner.as_ref(), IrType::String) => {
                // Pointer to string - already a string pointer
                return Some(value);
            }
            IrType::Ptr(inner) if matches!(inner.as_ref(), IrType::Void) => {
                // Ptr(Void) is often an unresolved generic - treat as i64 (Dynamic/pointer value)
                // For now, assume it's an integer that needs conversion
                eprintln!("DEBUG: [CONVERT TO STRING] Ptr(Void) detected, treating as int");
                "int_to_string"
            }
            IrType::Ptr(_) => {
                // Other pointer types - treat as opaque integer for conversion
                "int_to_string"
            }
            _ => "int_to_string", // Fallback
        };

        eprintln!("DEBUG: [CONVERT TO STRING] Using {} for type {:?}", mir_wrapper, from_type);

        // Cast to proper type if needed (int_to_string expects i32)
        let final_value = if mir_wrapper == "int_to_string" {
            match from_type {
                IrType::I64 => {
                    // int_to_string takes i32, so reduce i64 to i32
                    self.builder.build_cast(value, IrType::I64, IrType::I32).unwrap_or(value)
                }
                IrType::Ptr(_) => {
                    // Pointer value (likely unresolved generic like Ptr(Void)) - treat as i64, then cast to i32
                    self.builder.build_cast(value, IrType::I64, IrType::I32).unwrap_or(value)
                }
                _ => value,
            }
        } else {
            value
        };

        // Get the parameter and return types for the MIR wrapper
        // Note: *_to_string wrappers return String, not Ptr(String)
        let (param_type, return_type) = match mir_wrapper {
            "int_to_string" => (IrType::I32, IrType::String),
            "float_to_string" => (IrType::F64, IrType::String),
            "bool_to_string" => (IrType::Bool, IrType::String),
            _ => (IrType::I32, IrType::String),
        };

        // Register forward reference to the MIR wrapper
        let func_id = self.register_stdlib_mir_forward_ref(
            mir_wrapper,
            vec![param_type],
            return_type.clone(),
        );

        // Generate the call
        self.builder.build_call_direct(
            func_id,
            vec![final_value],
            return_type,
        )
    }

    fn maybe_box_value(&mut self, value: IrId, value_ty: TypeId, target_ty: TypeId) -> Option<IrId> {
        use crate::tast::TypeKind;

        // Check if target is Dynamic and value is concrete
        // Clone TypeKind to avoid borrow checker issues
        let (target_is_dynamic, value_kind_cloned) = {
            let type_table = self.type_table.borrow();
            let target_is_dyn = matches!(type_table.get(target_ty).map(|t| &t.kind), Some(TypeKind::Dynamic));
            let value_kind = type_table.get(value_ty).map(|t| t.kind.clone());
            (target_is_dyn, value_kind)
        };

        let value_is_dynamic = matches!(&value_kind_cloned, Some(TypeKind::Dynamic));
        let needs_boxing = target_is_dynamic && !value_is_dynamic;

        if !needs_boxing {
            return Some(value);
        }

        // Determine which boxing function to call based on value type
        match &value_kind_cloned {
            // Value types (need malloc + copy)
            Some(TypeKind::Int) => {
                eprintln!("DEBUG: [BOXING] Auto-boxing Int to Dynamic using box_int");
                let value_mir_type = self.builder.get_register_type(value)
                    .unwrap_or_else(|| self.convert_type(value_ty));
                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                let box_func_id = self.get_or_register_extern_function("box_int", vec![value_mir_type], ptr_u8);
                self.builder.build_call_direct(box_func_id, vec![value], IrType::Ptr(Box::new(IrType::U8)))
            }
            Some(TypeKind::Float) => {
                eprintln!("DEBUG: [BOXING] Auto-boxing Float to Dynamic using box_float");
                let value_mir_type = self.builder.get_register_type(value)
                    .unwrap_or_else(|| self.convert_type(value_ty));
                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                let box_func_id = self.get_or_register_extern_function("box_float", vec![value_mir_type], ptr_u8);
                self.builder.build_call_direct(box_func_id, vec![value], IrType::Ptr(Box::new(IrType::U8)))
            }
            Some(TypeKind::Bool) => {
                eprintln!("DEBUG: [BOXING] Auto-boxing Bool to Dynamic using box_bool");
                let value_mir_type = self.builder.get_register_type(value)
                    .unwrap_or_else(|| self.convert_type(value_ty));
                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                let box_func_id = self.get_or_register_extern_function("box_bool", vec![value_mir_type], ptr_u8);
                self.builder.build_call_direct(box_func_id, vec![value], IrType::Ptr(Box::new(IrType::U8)))
            }

            // Reference types (already pointers, just wrap with type_id)
            Some(TypeKind::Class { .. })
            | Some(TypeKind::Enum { .. })
            | Some(TypeKind::Interface { .. })
            | Some(TypeKind::Anonymous { .. })
            | Some(TypeKind::Array { .. }) => {
                eprintln!("DEBUG: [BOXING] Auto-boxing reference type {:?} to Dynamic using box_reference", value_kind_cloned);

                // Get TypeId as u32
                let type_id_u32 = value_ty.as_raw();

                // Create constant for type_id
                let type_id_const = self.builder.build_const(IrValue::U32(type_id_u32))?;

                // Get pointer type
                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));

                // Call box_reference_ptr(value_ptr, type_id)
                let box_func_id = self.get_or_register_extern_function(
                    "haxe_box_reference_ptr",
                    vec![ptr_u8.clone(), IrType::U32],
                    ptr_u8.clone(),
                );

                self.builder.build_call_direct(box_func_id, vec![value, type_id_const], ptr_u8)
            }

            // TODO: String (special struct handling), Abstract (depends on underlying type)
            _ => {
                eprintln!("DEBUG: [BOXING] Unsupported type for boxing: {:?}", value_kind_cloned);
                Some(value) // Skip boxing for unsupported types
            }
        }
    }

    /// Insert automatic unboxing if needed when reading from Dynamic
    /// Returns the (potentially unboxed) value
    fn maybe_unbox_value(&mut self, value: IrId, value_ty: TypeId, target_ty: TypeId) -> Option<IrId> {
        use crate::tast::TypeKind;

        // Check if value is Dynamic and target is concrete
        // Clone TypeKind to avoid borrow checker issues
        let (value_is_dynamic, target_kind_cloned) = {
            let type_table = self.type_table.borrow();
            let value_is_dyn = matches!(type_table.get(value_ty).map(|t| &t.kind), Some(TypeKind::Dynamic));
            let target_kind = type_table.get(target_ty).map(|t| t.kind.clone());
            (value_is_dyn, target_kind)
        };

        let target_is_dynamic = matches!(&target_kind_cloned, Some(TypeKind::Dynamic));
        let needs_unboxing = value_is_dynamic && !target_is_dynamic;

        if !needs_unboxing {
            return Some(value);
        }

        // Determine which unboxing function to call based on target type
        match &target_kind_cloned {
            // Value types (need to extract from heap)
            Some(TypeKind::Int) => {
                eprintln!("DEBUG: [UNBOXING] Auto-unboxing Dynamic to Int using unbox_int");
                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                let unbox_func_id = self.get_or_register_extern_function("unbox_int", vec![ptr_u8], IrType::I32);
                self.builder.build_call_direct(unbox_func_id, vec![value], IrType::I32)
            }
            Some(TypeKind::Float) => {
                eprintln!("DEBUG: [UNBOXING] Auto-unboxing Dynamic to Float using unbox_float");
                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                let unbox_func_id = self.get_or_register_extern_function("unbox_float", vec![ptr_u8], IrType::F64);
                self.builder.build_call_direct(unbox_func_id, vec![value], IrType::F64)
            }
            Some(TypeKind::Bool) => {
                eprintln!("DEBUG: [UNBOXING] Auto-unboxing Dynamic to Bool using unbox_bool");
                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                let unbox_func_id = self.get_or_register_extern_function("unbox_bool", vec![ptr_u8], IrType::Bool);
                self.builder.build_call_direct(unbox_func_id, vec![value], IrType::Bool)
            }

            // Enums: Check if this is actually an enum discriminant (i64) rather than a boxed value
            // When accessing Color.Green, the HIR may incorrectly type it as Dynamic,
            // but the actual MIR value is an i64 discriminant, not a boxed pointer
            Some(TypeKind::Enum { .. }) => {
                // Check the actual register type - if it's i64, don't unbox
                let actual_type = self.builder.get_register_type(value);
                if matches!(actual_type, Some(IrType::I64) | Some(IrType::I32)) {
                    eprintln!("DEBUG: [UNBOXING] Skipping unbox for enum - value is already i64 discriminant");
                    return Some(value);
                }
                eprintln!("DEBUG: [UNBOXING] Auto-unboxing Dynamic to Enum using unbox_reference");

                // Call haxe_unbox_reference_ptr to extract the pointer
                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                let unbox_func_id = self.get_or_register_extern_function(
                    "haxe_unbox_reference_ptr",
                    vec![ptr_u8.clone()],
                    ptr_u8.clone(),
                );
                self.builder.build_call_direct(unbox_func_id, vec![value], ptr_u8)
            }

            // Reference types (just extract the pointer)
            Some(TypeKind::Class { .. })
            | Some(TypeKind::Interface { .. })
            | Some(TypeKind::Anonymous { .. })
            | Some(TypeKind::Array { .. }) => {
                eprintln!("DEBUG: [UNBOXING] Auto-unboxing Dynamic to reference type {:?} using unbox_reference", target_kind_cloned);

                // Call haxe_unbox_reference_ptr to extract the pointer
                let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                let unbox_func_id = self.get_or_register_extern_function(
                    "haxe_unbox_reference_ptr",
                    vec![ptr_u8.clone()],
                    ptr_u8.clone(),
                );

                self.builder.build_call_direct(unbox_func_id, vec![value], ptr_u8)
            }

            // TODO: String (special struct handling), Abstract (depends on underlying type)
            _ => {
                eprintln!("DEBUG: [UNBOXING] Unsupported type for unboxing: {:?}", target_kind_cloned);
                Some(value) // Skip unboxing for unsupported types
            }
        }
    }

    /// Bind a pattern with type information (registers locals for Cranelift)
    fn bind_pattern_with_type(
        &mut self,
        pattern: &HirPattern,
        value: IrId,
        ty: Option<TypeId>,
        is_mutable: bool,
    ) {
        match pattern {
            HirPattern::Variable { name, symbol } => {
                // Bind the value to the symbol
                self.symbol_map.insert(*symbol, value);

                // Register as local so Cranelift can find the type
                if let Some(type_id) = ty {
                    let var_type_from_hint = self.convert_type(type_id);

                    // Check if the actual register type is more specific than the hint
                    // This can happen when:
                    // - Generic method return types aren't properly resolved (Thread<T>)
                    // - HIR type is vague (Ptr(Void)) but actual MIR type is specific (String)
                    let actual_reg_type = self.builder.get_register_type(value);
                    let var_type = if let Some(ref actual_type) = actual_reg_type {
                        // If hint is Ptr(Void) but actual type is more specific, use actual type
                        let hint_is_void_ptr = matches!(&var_type_from_hint, IrType::Ptr(inner) if matches!(**inner, IrType::Void));
                        let actual_is_specific = !matches!(actual_type, IrType::Ptr(inner) if matches!(**inner, IrType::Void));

                        // Also handle case where actual is pointer but hint is scalar
                        let actual_is_ptr = matches!(actual_type, IrType::Ptr(_));
                        let hint_is_scalar = matches!(&var_type_from_hint, IrType::I32 | IrType::I64 | IrType::Bool | IrType::F32 | IrType::F64);

                        if (hint_is_void_ptr && actual_is_specific) || (actual_is_ptr && hint_is_scalar) {
                            actual_type.clone()
                        } else {
                            var_type_from_hint
                        }
                    } else {
                        var_type_from_hint
                    };

                    if let Some(func) = self.builder.current_function_mut() {
                        func.locals.insert(
                            value,
                            super::IrLocal {
                                name: name.to_string(),
                                ty: var_type.clone(),
                                mutable: is_mutable,
                                source_location: IrSourceLocation::unknown(),
                                allocation: super::AllocationHint::Stack,
                            },
                        );
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
                    SourceLocation::unknown(),
                );
            }
            HirPattern::Array { .. } => {
                // Array patterns need runtime length checks
                self.add_error(
                    "Array patterns not yet supported in MIR lowering",
                    SourceLocation::unknown(),
                );
            }
            HirPattern::Object { .. } => {
                // Object patterns need field extraction
                self.add_error(
                    "Object patterns not yet supported in MIR lowering",
                    SourceLocation::unknown(),
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
                    let receiver_ty = object.ty; // The type of the object being accessed
                    // TODO: Look up field type from symbol table
                    // For now, use invalid TypeId - runtime call path doesn't need it
                    let field_ty = TypeId(u32::MAX); // Field's result type (placeholder)
                    self.lower_field_access(obj_reg, *field, receiver_ty, field_ty)
                } else {
                    None
                }
            }
            HirLValue::Index { object, index } => {
                // Read object[index]
                if let Some(obj_reg) = self.lower_expression(object) {
                    if let Some(idx_reg) = self.lower_expression(index) {
                        let elem_ty = object.ty; // Use object's type for now
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
                // Get the old register before updating (for type inference)
                let old_reg = self.symbol_map.get(symbol).copied();

                // Update the variable binding
                self.symbol_map.insert(*symbol, value);

                // Ensure the new value register has a local entry for phi node tracking
                // Get the existing local type from the symbol, or infer from value
                if let Some(func) = self.builder.current_function_mut() {
                    // Only add if not already present
                    if !func.locals.contains_key(&value) {
                        // Try to get the type from an existing binding of this symbol
                        let var_type = old_reg
                            .and_then(|r| func.locals.get(&r))
                            .map(|local| local.ty.clone())
                            .unwrap_or(IrType::Ptr(Box::new(IrType::Void)));

                        let var_name = self.symbol_table.get_symbol(*symbol)
                            .and_then(|s| self.string_interner.get(s.name))
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("var_{}", symbol.as_raw()));

                        func.locals.insert(
                            value,
                            super::IrLocal {
                                name: format!("{}_v{}", var_name, value.0),
                                ty: var_type,
                                mutable: true,
                                source_location: super::IrSourceLocation::unknown(),
                                allocation: super::AllocationHint::Register,
                            },
                        );
                    }
                }
            }
            HirLValue::Field { object, field } => {
                // Write object.field = value
                if let Some(obj_reg) = self.lower_expression(object) {
                    // Check if this is a property with a custom setter
                    if let Some(property_info) = self.property_access_map.get(field) {
                        match &property_info.setter {
                            crate::tast::PropertyAccessor::Method(setter_method_name) => {
                                // Look up the setter method by name in function_map
                                let setter_func_id = self.function_map.iter()
                                    .find(|(sym_id, _)| {
                                        if let Some(symbol) = self.symbol_table.get_symbol(**sym_id) {
                                            symbol.name == *setter_method_name
                                        } else {
                                            false
                                        }
                                    })
                                    .map(|(_, func_id)| *func_id);

                                if let Some(func_id) = setter_func_id {
                                    // Generate a call to the setter method
                                    // Setters are called with: (this, value)
                                    // and return the value that was set
                                    let return_type = if let Some(func) = self.builder.current_function() {
                                        func.locals
                                            .get(&value)
                                            .map(|local| local.ty.clone())
                                            .unwrap_or(IrType::I32)
                                    } else {
                                        IrType::I32
                                    };

                                    self.builder.build_call_direct(
                                        func_id,
                                        vec![obj_reg, value],  // Pass object as 'this', value as parameter
                                        return_type,
                                    );
                                    return;  // Setter called successfully
                                } else {
                                    // Setter method not found - this is an error
                                    let method_name_str = self.string_interner.get(*setter_method_name)
                                        .unwrap_or("<unknown>");
                                    self.add_error(
                                        &format!("Property setter method '{}' not found", method_name_str),
                                        SourceLocation::unknown()
                                    );
                                    return;
                                }
                            }
                            crate::tast::PropertyAccessor::Null | crate::tast::PropertyAccessor::Never => {
                                self.add_error(
                                    "Cannot write to read-only property (Null or Never setter)",
                                    SourceLocation::unknown()
                                );
                                return;
                            }
                            crate::tast::PropertyAccessor::Default | crate::tast::PropertyAccessor::Dynamic => {
                                // Fall through to direct field access
                            }
                        }
                    }

                    // Look up the field index (with fallback to name lookup)
                    let field_index_opt = self.field_index_map.get(field).map(|&(_, idx)| idx)
                        .or_else(|| {
                            // Fallback: Try to find field by name
                            let field_name = self.symbol_table.get_symbol(*field)
                                .map(|s| s.name)?;

                            // eprintln!("DEBUG: Field write for {:?} ({}) not found by SymbolId, trying name lookup", field, field_name);

                            for (sym, (_, idx)) in &self.field_index_map {
                                if let Some(sym_info) = self.symbol_table.get_symbol(*sym) {
                                    if sym_info.name == field_name {
                                       
                                        return Some(*idx);
                                    }
                                }
                            }
                            None
                        });

                    if let Some(field_index) = field_index_opt {
                        // eprintln!(
                        //     "DEBUG: Field write - field={:?}, index={}",
                        //     field, field_index
                        // );

                        // Create constant for field index
                        if let Some(index_const) =
                            self.builder.build_const(IrValue::I32(field_index as i32))
                        {
                            // Get the type of the field from the FIELD'S DECLARED TYPE in symbol table
                            // NOT from the value's type, which may be incorrect (e.g., I32 for String parameter)
                            let field_ty = self.symbol_table.get_symbol(*field)
                                .map(|s| self.convert_type(s.type_id))
                                .unwrap_or_else(|| {
                                    // Fallback: find field by name
                                    let field_name = self.symbol_table.get_symbol(*field).map(|s| s.name);
                                    for (sym, _) in &self.field_index_map {
                                        if let Some(sym_info) = self.symbol_table.get_symbol(*sym) {
                                            if field_name == Some(sym_info.name) {
                                                return self.convert_type(sym_info.type_id);
                                            }
                                        }
                                    }
                                    IrType::I32
                                });

                            // Use GEP to get field pointer, then store
                            if let Some(field_ptr) =
                                self.builder.build_gep(obj_reg, vec![index_const], field_ty)
                            {
                                self.builder.build_store(field_ptr, value);
                            }
                        }
                    } else {
                        let field_name = self
                            .symbol_table
                            .get_symbol(*field)
                            .map(|s| format!("{}", s.name))
                            .unwrap_or_else(|| format!("{:?}", field));
                        // eprintln!(
                        //     "WARNING: Field '{}' ({:?}) not found in field_index_map for write",
                        //     field_name, field
                        // );
                        self.add_error(
                            &format!("Field '{}' ({:?}) index not found for write - class may not be registered", field_name, field),
                            SourceLocation::unknown()
                        );
                    }
                }
            }
            HirLValue::Index { object, index } => {
                // Write object[index] = value
                // Call haxe_array_set runtime function for HaxeArray
                // Signature: fn haxe_array_set(arr: *mut HaxeArray, index: usize, data: *const u8) -> bool
                if let Some(obj_reg) = self.lower_expression(object) {
                    if let Some(idx_reg) = self.lower_expression(index) {
                        // Box the value if it's a primitive type (Int, Float, Bool)
                        // HaxeArray stores elements as boxed pointers
                        let value_ir_type = self.builder.get_register_type(value);
                        let boxed_value = match &value_ir_type {
                            Some(IrType::I32) | Some(IrType::I64) => {
                                // Box integer value
                                let box_func_id = self.get_or_register_extern_function(
                                    "box_int",
                                    vec![IrType::I64],
                                    IrType::Ptr(Box::new(IrType::U8)),
                                );
                                self.builder.build_call_direct(box_func_id, vec![value], IrType::Ptr(Box::new(IrType::U8)))
                            }
                            Some(IrType::F32) | Some(IrType::F64) => {
                                // Box float value
                                let box_func_id = self.get_or_register_extern_function(
                                    "box_float",
                                    vec![IrType::F64],
                                    IrType::Ptr(Box::new(IrType::U8)),
                                );
                                self.builder.build_call_direct(box_func_id, vec![value], IrType::Ptr(Box::new(IrType::U8)))
                            }
                            Some(IrType::Bool) => {
                                // Box bool value
                                let box_func_id = self.get_or_register_extern_function(
                                    "box_bool",
                                    vec![IrType::I64],
                                    IrType::Ptr(Box::new(IrType::U8)),
                                );
                                self.builder.build_call_direct(box_func_id, vec![value], IrType::Ptr(Box::new(IrType::U8)))
                            }
                            _ => {
                                // Already a pointer or unknown type, use as-is
                                Some(value)
                            }
                        }.unwrap_or(value);

                        // Get or declare the haxe_array_set extern function
                        let func_id = self.get_or_register_extern_function(
                            "haxe_array_set",
                            vec![
                                IrType::Ptr(Box::new(IrType::Void)),  // array
                                IrType::I64,                          // index
                                IrType::Ptr(Box::new(IrType::U8)),    // data pointer
                            ],
                            IrType::Bool,  // returns bool (success indicator)
                        );

                        // Call haxe_array_set(array, index, boxed_value)
                        self.builder.build_call_direct(
                            func_id,
                            vec![obj_reg, idx_reg, boxed_value],
                            IrType::Bool,
                        );
                    }
                }
            }
        }
    }

    fn lower_field_access(&mut self, obj: IrId, field: SymbolId, receiver_ty: TypeId, field_ty: TypeId) -> Option<IrId> {
        // SPECIAL CASE: Auto-unbox Dynamic for field access
        // If receiver is Dynamic, automatically unbox to get the actual object pointer
        let (obj, receiver_ty) = {
            let type_table = self.type_table.borrow();
            let obj_ir_type = self.builder.get_register_type(obj);
            if let Some(ty) = type_table.get(receiver_ty) {
                if matches!(ty.kind, TypeKind::Dynamic) {
                    // Check if the object's IR type is already a non-boxed pointer
                    // If the IR type is Ptr(Void), this is likely a raw pointer from StringMap/IntMap.get(),
                    // NOT a boxed Dynamic value. In that case, skip unboxing.
                    if let Some(IrType::Ptr(inner)) = &obj_ir_type {
                        if matches!(**inner, IrType::Void) {
                            // This is a raw pointer (e.g., from StringMap<Point>.get()),
                            // not a boxed Dynamic value. Skip unboxing.
                            drop(type_table);
                            return self.lower_field_access_for_class(obj, field, field_ty);
                        }
                    }
                    drop(type_table);

                    // Unbox to get the actual object pointer
                    let ptr_u8 = IrType::Ptr(Box::new(IrType::U8));
                    let unbox_func_id = self.get_or_register_extern_function(
                        "haxe_unbox_reference_ptr",
                        vec![ptr_u8.clone()],
                        ptr_u8.clone(),
                    );
                    let unboxed_obj = self.builder.build_call_direct(unbox_func_id, vec![obj], ptr_u8.clone())?;

                    // Get the actual class type from the field's class
                    // The field_index_map tells us which class this field belongs to
                    // For Dynamic types, the field symbol may be a newly created placeholder,
                    // so we need to look up by field name instead
                    let (actual_type, _resolved_field) = if let Some(&(class_type_id, _field_idx)) = self.field_index_map.get(&field) {
                        (class_type_id, field)
                    } else {
                        // Field not found by SymbolId - try looking up by name
                        // This handles Dynamic field access where a new symbol was created
                        let field_name = self.symbol_table.get_symbol(field).map(|s| s.name);

                        if let Some(name) = field_name {
                            // Search for any field with the same name in field_index_map
                            let mut found = None;
                            for (sym, &(class_ty, _idx)) in &self.field_index_map {
                                if let Some(sym_info) = self.symbol_table.get_symbol(*sym) {
                                    if sym_info.name == name {
                                        // Get the field's actual type from the symbol
                                        let resolved_field_ty = sym_info.type_id;
                                        found = Some((class_ty, *sym, resolved_field_ty));
                                        break;
                                    }
                                }
                            }

                            if let Some((class_ty, resolved_sym, resolved_field_ty)) = found {
                                // Early return with the correct field symbol AND correct field type
                                return self.lower_field_access(unboxed_obj, resolved_sym, class_ty, resolved_field_ty);
                            } else {
                                (receiver_ty, field)
                            }
                        } else {
                            (receiver_ty, field)
                        }
                    };

                    // If we reach here, we couldn't resolve the field - fall through to normal handling
                    // This shouldn't happen for valid Dynamic field access, but provides a fallback
                    (unboxed_obj, actual_type)
                } else {
                    drop(type_table);
                    (obj, receiver_ty)
                }
            } else {
                drop(type_table);
                (obj, receiver_ty)
            }
        };

        // SPECIAL CASE: Check if this is a property access on a @:coreType extern class
        // For example, Array.length should map to haxe_array_length() runtime call
        // These classes have no actual fields - all access must go through runtime functions
        let field_name_debug = self.symbol_table.get_symbol(field)
            .and_then(|s| self.string_interner.get(s.name))
            .unwrap_or("<unknown>");
        eprintln!("DEBUG: [lower_field_access] Checking stdlib for field='{}', field={:?}, receiver_ty={:?}", field_name_debug, field, receiver_ty);

        if let Some((_class, _method, runtime_call)) = self.get_stdlib_runtime_info(field, receiver_ty) {
            let runtime_func = runtime_call.runtime_name;
            eprintln!("DEBUG: [lower_field_access] Found stdlib property! runtime_func={}", runtime_func);

            // Determine result type based on whether it returns a primitive or complex type
            // If needs_out_param is false and has_return is true, it returns a primitive (i32/i64/f64)
            // Otherwise it returns a complex type (Ptr) or void
            let result_type = if !runtime_call.needs_out_param && runtime_call.has_return {
                // Returns a primitive - get the actual primitive type from field_ty
                let field_kind = {
                    let type_table = self.type_table.borrow();
                    type_table.get(field_ty).map(|t| t.kind.clone())
                };

                // Map TAST primitive types to IR types correctly
                match field_kind {
                    Some(crate::tast::TypeKind::Int) => IrType::I64,
                    Some(crate::tast::TypeKind::Float) => IrType::F64,
                    Some(crate::tast::TypeKind::Bool) => IrType::Bool,
                    _ => {
                        eprintln!("WARNING: Unexpected field kind {:?} for primitive-returning function {}", field_kind, runtime_func);
                        self.convert_type(field_ty)
                    }
                }
            } else {
                // Returns a complex type or void
                self.convert_type(field_ty)
            };

            eprintln!("DEBUG: [lower_field_access] result_type for {} = {:?} (needs_out_param={}, has_return={})",
                runtime_func, result_type, runtime_call.needs_out_param, runtime_call.has_return);

            // Generate a call to the runtime property getter
            // Property getters take the object as the only parameter
            // Use explicit Ptr(Void) type for opaque stdlib objects (Array, String, etc.)
            let param_types = vec![IrType::Ptr(Box::new(IrType::Void))];
            let runtime_func_id = self.get_or_register_extern_function(
                &runtime_func,
                param_types,
                result_type.clone(),
            );

            // Call the property getter with just the object
            let result_reg = self.builder.build_call_direct(
                runtime_func_id,
                vec![obj],
                result_type.clone(),
            );

            // DEBUG: Check actual type of result register
            if let Some(reg) = result_reg {
                if let Some(reg_type) = self.builder.get_register_type(reg) {
                    eprintln!("DEBUG: [lower_field_access] result_reg={}, register_type={:?}", reg, reg_type);
                } else {
                    eprintln!("DEBUG: [lower_field_access] result_reg={} has no type in builder", reg);
                }
            }

            return result_reg;
        } else {
            eprintln!("DEBUG: [lower_field_access] get_stdlib_runtime_info returned None for field='{}' ({:?}), receiver_ty={:?}", field_name_debug, field, receiver_ty);

            // FALLBACK: receiver_ty didn't match, but this might be a stdlib property from an out-param function
            // Try checking all common stdlib class types
            let common_stdlib_types = [
                crate::tast::TypeKind::Array { element_type: TypeId::from_raw(0) },
                crate::tast::TypeKind::String,
            ];

            for ref_kind in &common_stdlib_types {
                // Find a type ID matching this kind
                let matching_type_id = {
                    let type_table = self.type_table.borrow();
                    let mut found = None;
                    for (type_id, type_info) in type_table.iter() {
                        let matches = match (&type_info.kind, ref_kind) {
                            (crate::tast::TypeKind::Array { .. }, crate::tast::TypeKind::Array { .. }) => true,
                            (crate::tast::TypeKind::String, crate::tast::TypeKind::String) => true,
                            _ => false,
                        };
                        if matches {
                            found = Some(type_id);
                            break;
                        }
                    }
                    found
                };

                // Check if this field is a stdlib property for this class
                if let Some(class_ty) = matching_type_id {
                    if let Some((_class, _method, runtime_call)) = self.get_stdlib_runtime_info(field, class_ty) {
                        let runtime_func = runtime_call.runtime_name;
                        eprintln!("DEBUG: [lower_field_access fallback] Found stdlib property! runtime_func={}", runtime_func);
                        // Use explicit pointer type for parameter (matches stdlib signatures)
                        let param_types = vec![IrType::Ptr(Box::new(IrType::Void))];
                        let result_type = self.convert_type(field_ty);
                        let runtime_func_id = self.get_or_register_extern_function(
                            &runtime_func,
                            param_types,
                            result_type.clone(),
                        );
                        return self.builder.build_call_direct(
                            runtime_func_id,
                            vec![obj],
                            result_type,
                        );
                    }
                }
            }
        }

        // Check if this is a property with a custom getter
        if let Some(property_info) = self.property_access_map.get(&field) {
            match &property_info.getter {
                crate::tast::PropertyAccessor::Method(getter_method_name) => {
                    // Look up the getter method by name in function_map
                    let getter_func_id = self.function_map.iter()
                        .find(|(sym_id, _)| {
                            if let Some(symbol) = self.symbol_table.get_symbol(**sym_id) {
                                symbol.name == *getter_method_name
                            } else {
                                false
                            }
                        })
                        .map(|(_, func_id)| *func_id);

                    if let Some(func_id) = getter_func_id {
                        // Generate a call to the getter method
                        // Getters are called with the object as the first parameter (this)
                        let result_type = self.convert_type(field_ty);

                        return self.builder.build_call_direct(
                            func_id,
                            vec![obj],  // Pass object as 'this'
                            result_type,
                        );
                    } else {
                        // Getter method not found - this is an error
                        let method_name_str = self.string_interner.get(*getter_method_name)
                            .unwrap_or("<unknown>");
                        self.add_error(
                            &format!("Property getter method '{}' not found", method_name_str),
                            SourceLocation::unknown()
                        );
                        return None;
                    }
                }
                crate::tast::PropertyAccessor::Null | crate::tast::PropertyAccessor::Never => {
                    self.add_error(
                        "Cannot read from write-only property (Null or Never getter)",
                        SourceLocation::unknown()
                    );
                    return None;
                }
                crate::tast::PropertyAccessor::Default | crate::tast::PropertyAccessor::Dynamic => {
                    // Fall through to direct field access
                }
            }
        }

        // Look up the field index from our field_index_map
        let (class_type_id, field_index) = match self.field_index_map.get(&field) {
            Some(&mapping) => mapping,
            None => {
                // Fallback: Try to find field by name instead of SymbolId
                // This handles cases where the same field has different SymbolIds in different scopes
                let field_name = self
                    .symbol_table
                    .get_symbol(field)
                    .map(|s| s.name)
                    .or_else(|| {
                        // eprintln!("WARNING: Could not find symbol {:?} in symbol table", field);
                        None
                    })?;

                // eprintln!(
                //     "DEBUG: Field {:?} ({}) not found by SymbolId, trying lookup by name",
                //     field, field_name
                // );

                // Search for any field with the same name
                let mut found_mapping = None;
                for (sym, mapping) in &self.field_index_map {
                    if let Some(sym_info) = self.symbol_table.get_symbol(*sym) {
                        if sym_info.name == field_name {
                            // eprintln!(
                            //     "DEBUG: Found field '{}' via name match: {:?} -> {:?}",
                            //     field_name, sym, mapping
                            // );
                            found_mapping = Some(*mapping);
                            break;
                        }
                    }
                }

                match found_mapping {
                    Some(mapping) => mapping,
                    None => {
                        // Try typedef_field_map for anonymous struct fields (like FileStat)
                        // This handles cases where the typedef's anonymous struct fields
                        // are accessed with newly created symbols at the access site
                        //
                        // receiver_ty might be the typedef's TypeId OR the aliased anonymous struct TypeId
                        // Try both and also search all registered typedefs for this field name
                        let mut typedef_lookup = self.typedef_field_map.get(&(receiver_ty, field_name)).copied();

                        // If not found with receiver_ty, search all typedefs for this field
                        if typedef_lookup.is_none() {
                            for ((typedef_ty, fname), &idx) in &self.typedef_field_map {
                                if *fname == field_name {
                                    typedef_lookup = Some(idx);
                                    // Use the typedef's type id for the result
                                    return Some(self.lower_typedef_field_access(obj, *typedef_ty, idx, field_ty)?);
                                }
                            }
                        }

                        if let Some(typedef_field_idx) = typedef_lookup {
                            // Found in typedef_field_map - return (receiver_type, field_index)
                            (receiver_ty, typedef_field_idx)
                        } else {
                            // Last resort: look up the field by name in the type_table for anonymous structs
                            // This handles cross-module typedef field access where the typedef was
                            // registered in a different HIR->MIR pass
                            let type_table = self.type_table.borrow();

                            // Get the field name string for lookup
                            let field_name_str = self.string_interner.get(field_name)
                                .map(|s| s.to_string())
                                .unwrap_or_default();

                            // Search all types for an anonymous struct with this field name
                            let mut found_field = None;
                            for (type_id, type_info) in type_table.iter() {
                                if let TypeKind::Anonymous { fields } = &type_info.kind {
                                    for (idx, anon_field) in fields.iter().enumerate() {
                                        let anon_field_name = self.string_interner.get(anon_field.name)
                                            .map(|s| s.to_string())
                                            .unwrap_or_default();
                                        if anon_field_name == field_name_str {
                                            found_field = Some((type_id, idx as u32));
                                            break;
                                        }
                                    }
                                    if found_field.is_some() {
                                        break;
                                    }
                                }
                            }
                            drop(type_table);

                            if let Some((found_type_id, field_idx)) = found_field {
                                // Get the actual field type from the type_table
                                let actual_field_ty = {
                                    let type_table = self.type_table.borrow();
                                    if let Some(type_info) = type_table.get(found_type_id) {
                                        if let TypeKind::Anonymous { fields } = &type_info.kind {
                                            if let Some(field) = fields.get(field_idx as usize) {
                                                field.type_id
                                            } else {
                                                field_ty
                                            }
                                        } else {
                                            field_ty
                                        }
                                    } else {
                                        field_ty
                                    }
                                };
                                return Some(self.lower_typedef_field_access(obj, receiver_ty, field_idx, actual_field_ty)?);
                            }

                            self.add_error(
                                &format!(
                                    "Field '{}' ({:?}) index not found - class may not be registered",
                                    field_name, field
                                ),
                                SourceLocation::unknown(),
                            );
                            return None;
                        }
                    }
                }
            }
        };

        // eprintln!(
        //     "DEBUG: Field access - field={:?}, class_type={:?}, index={}",
        //     field, class_type_id, field_index
        // );

        // Create constant for field index
        let index_const = self.builder.build_const(IrValue::I32(field_index as i32))?;

        // Get the type of the field
        let field_ir_ty = self.convert_type(field_ty);

        // Use GetElementPtr to get pointer to the field
        // obj is a pointer to the struct, indices are [field_index]
        let field_ptr = self
            .builder
            .build_gep(obj, vec![index_const], field_ir_ty.clone())?;

        // Load the value from the field pointer
        let field_value = self.builder.build_load(field_ptr, field_ir_ty.clone())?;

        // Register the type of the loaded value for use in later instructions (e.g., Cmp)
        self.builder.set_register_type(field_value, field_ir_ty);

        // eprintln!("DEBUG: Field access successful - value={:?}", field_value);
        Some(field_value)
    }

    /// Direct field access for typedef'd anonymous structs (like FileStat)
    /// All fields are 8 bytes for consistent boxing/sizing
    fn lower_typedef_field_access(&mut self, obj: IrId, _typedef_ty: TypeId, field_index: u32, field_ty: TypeId) -> Option<IrId> {
        // Create constant for field index
        let index_const = self.builder.build_const(IrValue::I32(field_index as i32))?;

        // Get the type of the field
        let field_ir_ty = self.convert_type(field_ty);

        // Use GetElementPtr to get pointer to the field
        // All typedef anonymous struct fields are 8 bytes
        let field_ptr = self
            .builder
            .build_gep(obj, vec![index_const], field_ir_ty.clone())?;

        // Load the value from the field pointer
        let field_value = self.builder.build_load(field_ptr, field_ir_ty.clone())?;

        // Register the type of the loaded value
        self.builder.set_register_type(field_value, field_ir_ty);

        Some(field_value)
    }

    /// Direct field access for class objects without Dynamic unboxing.
    /// Used when we know the object is a raw pointer (e.g., from StringMap<Point>.get())
    /// but TAST thinks it's Dynamic because the type parameter wasn't resolved.
    fn lower_field_access_for_class(&mut self, obj: IrId, field: SymbolId, field_ty: TypeId) -> Option<IrId> {
        // CRITICAL: Check for stdlib runtime properties FIRST (e.g., Array.length, String.length)
        // These are properties that should call runtime functions, not do direct field access
        // When we have a Ptr(Void) from a stdlib function, we need to check if the field
        // being accessed is a stdlib property by trying all stdlib class types

        // Get field name for debugging
        let field_name = self.symbol_table.get_symbol(field)
            .and_then(|s| self.string_interner.get(s.name))
            .unwrap_or("<unknown>");

        eprintln!("DEBUG: [lower_field_access_for_class] field_name='{}', field={:?}", field_name, field);

        // Try common stdlib classes that might have properties
        let common_stdlib_types = [
            crate::tast::TypeKind::Array { element_type: TypeId::from_raw(0) },
            crate::tast::TypeKind::String,
        ];

        for ref_kind in &common_stdlib_types {
            // Find a type ID matching this kind
            let matching_type_id = {
                let type_table = self.type_table.borrow();
                let mut found = None;
                for (type_id, type_info) in type_table.iter() {
                    let matches = match (&type_info.kind, ref_kind) {
                        (crate::tast::TypeKind::Array { .. }, crate::tast::TypeKind::Array { .. }) => true,
                        (crate::tast::TypeKind::String, crate::tast::TypeKind::String) => true,
                        _ => false,
                    };
                    if matches {
                        found = Some(type_id);
                        break;
                    }
                }
                eprintln!("DEBUG: [lower_field_access_for_class] Checked for {:?}, found type_id: {:?}", ref_kind, found);
                found
            };

            // Check if this field is a stdlib property for this class
            if let Some(class_ty) = matching_type_id {
                eprintln!("DEBUG: [lower_field_access_for_class] Checking get_stdlib_runtime_info for field={:?}, class_ty={:?}", field, class_ty);
                if let Some((_class, _method, runtime_call)) = self.get_stdlib_runtime_info(field, class_ty) {
                    let runtime_func = runtime_call.runtime_name;
                    eprintln!("DEBUG: [lower_field_access_for_class] Found stdlib property! runtime_func={}", runtime_func);
                    // Use explicit pointer type for parameter (matches stdlib signatures)
                    let param_types = vec![IrType::Ptr(Box::new(IrType::Void))];

                    // Determine result type based on whether it returns a primitive or complex type
                    let result_type = if !runtime_call.needs_out_param && runtime_call.has_return {
                        // Returns a primitive - look up the MIR wrapper function to get its return type
                        // The stdlib property getter is registered in MIR (e.g., array_length)
                        if let Some(mir_func) = self.builder.module.functions.iter()
                            .find(|(_, f)| f.name == runtime_func)
                            .map(|(_, f)| f) {
                            // Use the MIR function's return type
                            eprintln!("DEBUG: [lower_field_access_for_class] Found MIR function {}, return_type={:?}",
                                runtime_func, mir_func.signature.return_type);
                            mir_func.signature.return_type.clone()
                        } else {
                            // Fallback: try to infer from field type in symbol table
                            let actual_field_type = self.symbol_table.get_symbol(field)
                                .map(|s| s.type_id)
                                .unwrap_or(field_ty);

                            let field_kind = {
                                let type_table = self.type_table.borrow();
                                type_table.get(actual_field_type).map(|t| t.kind.clone())
                            };

                            eprintln!("DEBUG: [lower_field_access_for_class] No MIR function found, actual_field_type={:?}, field_kind={:?}",
                                actual_field_type, field_kind);

                            // Map TAST primitive types to IR types correctly
                            match field_kind {
                                Some(crate::tast::TypeKind::Int) => IrType::I64,
                                Some(crate::tast::TypeKind::Float) => IrType::F64,
                                Some(crate::tast::TypeKind::Bool) => IrType::Bool,
                                _ => {
                                    eprintln!("WARNING: Unexpected field kind {:?} for primitive-returning function {}, defaulting to I64", field_kind, runtime_func);
                                    // Default to I64 for unknown primitive types (most stdlib properties return integers)
                                    IrType::I64
                                }
                            }
                        }
                    } else {
                        // Returns a complex type or void
                        self.convert_type(field_ty)
                    };

                    eprintln!("DEBUG: [lower_field_access_for_class] result_type for {} = {:?} (needs_out_param={}, has_return={})",
                        runtime_func, result_type, runtime_call.needs_out_param, runtime_call.has_return);

                    let runtime_func_id = self.get_or_register_extern_function(
                        &runtime_func,
                        param_types,
                        result_type.clone(),
                    );

                    let result_reg = self.builder.build_call_direct(
                        runtime_func_id,
                        vec![obj],
                        result_type.clone(),
                    );

                    // DEBUG: Check actual type of result register
                    if let Some(reg) = result_reg {
                        if let Some(reg_type) = self.builder.get_register_type(reg) {
                            eprintln!("DEBUG: [lower_field_access_for_class] result_reg={}, register_type={:?}", reg, reg_type);
                        }
                    }

                    return result_reg;
                } else {
                    eprintln!("DEBUG: [lower_field_access_for_class] get_stdlib_runtime_info returned None");
                }
            }
        }

        eprintln!("DEBUG: [lower_field_access_for_class] No stdlib property found, falling through to field_index_map");

        // Look up the field index from our field_index_map
        // Also get the actual field type from the symbol table, NOT from the passed-in field_ty
        // which may be Dynamic due to unresolved type parameters
        let (_, field_index, actual_field_type) = match self.field_index_map.get(&field) {
            Some(&(class_ty, idx)) => {
                // Get the actual field type from the symbol table
                let sym_type = self.symbol_table.get_symbol(field)
                    .map(|s| s.type_id)
                    .unwrap_or(field_ty);
                (class_ty, idx, sym_type)
            },
            None => {
                // Fallback: Try to find field by name instead of SymbolId
                let field_name = self
                    .symbol_table
                    .get_symbol(field)
                    .map(|s| s.name)?;

                // Search for any field with the same name
                let mut found = None;
                for (sym, &(class_ty, idx)) in &self.field_index_map {
                    if let Some(sym_info) = self.symbol_table.get_symbol(*sym) {
                        if sym_info.name == field_name {
                            // Get the actual field type from the matched symbol
                            found = Some((class_ty, idx, sym_info.type_id));
                            break;
                        }
                    }
                }

                match found {
                    Some(result) => result,
                    None => {
                        let name_str = self.string_interner.get(field_name).unwrap_or("<unknown>");
                        self.add_error(
                            &format!(
                                "Field '{}' ({:?}) index not found for raw pointer access",
                                name_str, field
                            ),
                            SourceLocation::unknown(),
                        );
                        return None;
                    }
                }
            }
        };

        // Create constant for field index
        let index_const = self.builder.build_const(IrValue::I32(field_index as i32))?;

        // Get the type of the field from the actual field symbol, NOT the Dynamic-typed field_ty
        let field_ir_ty = self.convert_type(actual_field_type);

        // Use GetElementPtr to get pointer to the field
        let field_ptr = self
            .builder
            .build_gep(obj, vec![index_const], field_ir_ty.clone())?;

        // Load the value from the field pointer
        let field_value = self.builder.build_load(field_ptr, field_ir_ty.clone())?;

        // Register the type of the loaded value
        self.builder.set_register_type(field_value, field_ir_ty);

        Some(field_value)
    }

    fn lower_index_access(&mut self, obj: IrId, idx: IrId, ty: TypeId) -> Option<IrId> {
        // Array index access - call haxe_array_get_ptr runtime function
        // For HaxeArray, we need to call the runtime function instead of using GEP
        // because array elements may be boxed and require proper dynamic type handling
        //
        // Signature: fn haxe_array_get_ptr(arr: *const HaxeArray, index: usize) -> *mut u8
        //
        // The runtime function returns a pointer to the boxed element

        // Get or declare the haxe_array_get_ptr extern function
        let func_id = self.get_or_register_extern_function(
            "haxe_array_get_ptr",
            vec![IrType::Ptr(Box::new(IrType::Void)), IrType::I64],
            IrType::Ptr(Box::new(IrType::U8)),
        );

        // Call haxe_array_get_ptr(array, index)
        // The function returns a pointer to the element (*mut u8)
        let elem_ptr = self.builder.build_call_direct(
            func_id,
            vec![obj, idx],
            IrType::Ptr(Box::new(IrType::U8)),
        )?;

        // Dereference the pointer to get the actual value
        // Array elements are stored as i64 values, so load as i64
        self.builder.build_load(elem_ptr, IrType::I64)
    }

    fn lower_logical_and(&mut self, lhs: &HirExpr, rhs: &HirExpr) -> Option<IrId> {
        // Short-circuit AND: if lhs is false, don't evaluate rhs
        // Create blocks: eval_rhs, merge
        let eval_rhs = self.builder.create_block()?;
        let merge = self.builder.create_block()?;

        // Evaluate LHS
        let lhs_val = self.lower_expression(lhs)?;

        // Create false_val BEFORE branching so it's in this block's scope
        let false_val = self.builder.build_bool(false)?;

        // Capture the current block BEFORE branching - this is where LHS was evaluated
        let lhs_block = self.builder.current_block()?;

        // Branch on LHS: if true, evaluate RHS; if false, skip to merge with false
        self.builder.build_cond_branch(lhs_val, eval_rhs, merge)?;

        // Block for evaluating RHS
        self.builder.switch_to_block(eval_rhs);
        let rhs_val = self.lower_expression(rhs)?;
        let rhs_block = self.builder.current_block()?;
        self.builder.build_branch(merge)?;

        // Merge block with phi node
        self.builder.switch_to_block(merge);
        let result = self.builder.build_phi(merge, IrType::Bool)?;
        // lhs_block is where we came from if LHS was false (short-circuit path)
        self.builder
            .add_phi_incoming(merge, result, lhs_block, false_val)?;
        self.builder
            .add_phi_incoming(merge, result, rhs_block, rhs_val)?;

        Some(result)
    }

    fn lower_logical_or(&mut self, lhs: &HirExpr, rhs: &HirExpr) -> Option<IrId> {
        // Short-circuit OR: if lhs is true, don't evaluate rhs
        // Create blocks: eval_rhs, merge
        let eval_rhs = self.builder.create_block()?;
        let merge = self.builder.create_block()?;

        // Evaluate LHS
        let lhs_val = self.lower_expression(lhs)?;

        // Create true_val BEFORE branching so it's in this block's scope
        let true_val = self.builder.build_bool(true)?;

        // Capture the current block BEFORE branching - this is where LHS was evaluated
        let lhs_block = self.builder.current_block()?;

        // Branch on LHS: if false, evaluate RHS; if true, skip to merge with true
        self.builder.build_cond_branch(lhs_val, merge, eval_rhs)?;

        // Block for evaluating RHS
        self.builder.switch_to_block(eval_rhs);
        let rhs_val = self.lower_expression(rhs)?;
        let rhs_block = self.builder.current_block()?;
        self.builder.build_branch(merge)?;

        // Merge block with phi node
        self.builder.switch_to_block(merge);
        let result = self.builder.build_phi(merge, IrType::Bool)?;
        // lhs_block is where we came from if LHS was true (short-circuit path)
        self.builder
            .add_phi_incoming(merge, result, lhs_block, true_val)?;
        self.builder
            .add_phi_incoming(merge, result, rhs_block, rhs_val)?;

        Some(result)
    }

    fn lower_conditional(
        &mut self,
        cond: &HirExpr,
        then_expr: &HirExpr,
        else_expr: &HirExpr,
    ) -> Option<IrId> {
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
        // eprintln!(
        //     "DEBUG lower_conditional: symbol_map has {} entries before condition",
        //     symbol_map_before.len()
        // );

        // Evaluate condition
        let cond_val = self.lower_expression(cond)?;
        // eprintln!(
        //     "DEBUG lower_conditional: After evaluating condition, in block {:?}",
        //     self.builder.current_block()
        // );

        // Branch based on condition
        self.builder
            .build_cond_branch(cond_val, then_block, else_block)?;

        // Then block
        self.builder.switch_to_block(then_block);
        let then_val = self.lower_expression(then_expr);
        let then_terminated = self.is_terminated();
        // eprintln!(
        //     "DEBUG lower_conditional: then_terminated = {}",
        //     then_terminated
        // );
        if !then_terminated {
            self.builder.build_branch(merge_block)?;
        }
        let then_end_block = self.builder.current_block()?;
        let symbol_map_after_then = self.symbol_map.clone();
        // eprintln!(
        //     "DEBUG lower_conditional: then_end_block = {:?}, symbol_map has {} entries",
        //     then_end_block,
        //     symbol_map_after_then.len()
        // );

        // Else block
        self.symbol_map = symbol_map_before.clone(); // Reset to before-branch state
        self.builder.switch_to_block(else_block);
        let else_val = self.lower_expression(else_expr);
        let else_terminated = self.is_terminated();
        // eprintln!(
        //     "DEBUG lower_conditional: else_terminated = {}",
        //     else_terminated
        // );
        if !else_terminated {
            self.builder.build_branch(merge_block)?;
        }
        let else_end_block = self.builder.current_block()?;
        let symbol_map_after_else = self.symbol_map.clone();
        // eprintln!(
        //     "DEBUG lower_conditional: else_end_block = {:?}, symbol_map has {} entries",
        //     else_end_block,
        //     symbol_map_after_else.len()
        // );

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
        // eprintln!("DEBUG: Checking for modified symbols");
        // eprintln!("  symbol_map_before: {} entries", symbol_map_before.len());
        // eprintln!(
        //     "  symbol_map_after_then: {} entries",
        //     symbol_map_after_then.len()
        // );
        // eprintln!(
        //     "  symbol_map_after_else: {} entries",
        //     symbol_map_after_else.len()
        // );

        for (sym, reg_after_then) in &symbol_map_after_then {
            if symbol_map_before.get(sym) != Some(reg_after_then) {
                // eprintln!(
                //     "  Modified in then branch: {:?} (before: {:?}, after: {:?})",
                //     sym,
                //     symbol_map_before.get(sym),
                //     reg_after_then
                // );
                modified_symbols.insert(*sym);
            }
        }
        for (sym, reg_after_else) in &symbol_map_after_else {
            if symbol_map_before.get(sym) != Some(reg_after_else) {
                // eprintln!(
                //     "  Modified in else branch: {:?} (before: {:?}, after: {:?})",
                //     sym,
                //     symbol_map_before.get(sym),
                //     reg_after_else
                // );
                modified_symbols.insert(*sym);
            }
        }
        // eprintln!("DEBUG: Found {} modified symbols", modified_symbols.len());

        // Create phi nodes for modified variables
        // eprintln!(
        //     "DEBUG: Creating phi nodes for {} symbols",
        //     modified_symbols.len()
        // );
        for symbol_id in &modified_symbols {
            // eprintln!("  Processing symbol {:?}", symbol_id);
            let before_reg = symbol_map_before.get(symbol_id).copied();
            let then_reg = symbol_map_after_then.get(symbol_id).copied();
            let else_reg = symbol_map_after_else.get(symbol_id).copied();

            // Get type from locals table using the "before" register (from variable declaration)
            // because new registers from assignments don't have local entries
            let type_lookup_reg = before_reg.or(then_reg).or(else_reg);
            let var_type = match type_lookup_reg.and_then(|r| {
                self.builder
                    .current_function()
                    .and_then(|f| f.locals.get(&r))
                    .map(|local| local.ty.clone())
            }) {
                Some(t) => {
                    // eprintln!("  Found type {:?} for symbol {:?}", t, symbol_id);
                    t
                }
                None => {
                    // eprintln!(
                    //     "  No type found for symbol {:?} (tried {:?}), skipping",
                    //     symbol_id, type_lookup_reg
                    // );
                    continue;
                }
            };

            // Only create phi nodes for variables that have values from all non-terminated branches
            // This prevents creating invalid phi nodes for branch-local variables
            let has_then_value = !then_terminated && (then_reg.is_some() || before_reg.is_some());
            let has_else_value = !else_terminated && (else_reg.is_some() || before_reg.is_some());

            // Skip if we can't provide values from all active branches
            if (!then_terminated && !has_then_value) || (!else_terminated && !has_else_value) {
                // eprintln!("  Skipping phi for {:?} - not in all branches", symbol_id);
                continue;
            }

            let sample_reg = then_reg.or(else_reg).or(before_reg).unwrap();

            // Create phi node
            // eprintln!(
            //     "  Creating phi for {:?} with type {:?}",
            //     symbol_id, var_type
            // );
            let phi_reg = match self.builder.build_phi(merge_block, var_type.clone()) {
                Some(r) => r,
                None => {
                    // eprintln!("  Failed to create phi node");
                    continue;
                }
            };
            // eprintln!("  Created phi node {:?}", phi_reg);

            // Add incoming edges for non-terminated branches
            // IMPORTANT: Only add phi incoming if the variable exists in that branch
            // Don't use variables from other branches (causes domination errors)
            // eprintln!(
            //     "  Adding phi incoming: then_terminated={}, else_terminated={}",
            //     then_terminated, else_terminated
            // );
            if !then_terminated {
                // Use then_reg if it exists, otherwise before_reg
                // Do NOT use else_reg here - it would violate SSA dominance
                if let Some(val) = then_reg.or(before_reg) {
                    // eprintln!(
                    //     "  Calling add_phi_incoming(merge={:?}, phi={:?}, from={:?}, val={:?})",
                    //     merge_block, phi_reg, then_end_block, val
                    // );
                    self.builder
                        .add_phi_incoming(merge_block, phi_reg, then_end_block, val);
                    // {
                    //     Some(()) => eprintln!("  Successfully added phi incoming from then"),
                    //     None => eprintln!(
                    //         "  WARNING: Failed to add phi incoming from then block {:?}",
                    //         then_end_block
                    //     ),
                    // }
                }
            }
            if !else_terminated {
                // Use else_reg if it exists, otherwise before_reg
                // Do NOT use then_reg here - it would violate SSA dominance
                if let Some(val) = else_reg.or(before_reg) {
                    // eprintln!(
                    //     "  Calling add_phi_incoming(merge={:?}, phi={:?}, from={:?}, val={:?})",
                    //     merge_block, phi_reg, else_end_block, val
                    // );
                    self.builder
                        .add_phi_incoming(merge_block, phi_reg, else_end_block, val);
                    // {
                    //     Some(()) => eprintln!("  Successfully added phi incoming from else"),
                    //     None => eprintln!(
                    //         "  WARNING: Failed to add phi incoming from else block {:?}",
                    //         else_end_block
                    //     ),
                    // }
                }
            }

            // Register phi as local
            if let Some(func) = self.builder.current_function_mut() {
                if let Some(local) = func.locals.get(&sample_reg).cloned() {
                    func.locals.insert(
                        phi_reg,
                        super::IrLocal {
                            name: format!("{}_phi", local.name),
                            ty: var_type.clone(),
                            mutable: true,
                            source_location: local.source_location,
                            allocation: super::AllocationHint::Register,
                        },
                    );
                }
            }

            // Update symbol map to use phi
            self.symbol_map.insert(*symbol_id, phi_reg);
        }

        // Create phi for expression result if both branches returned values
        let mut result_phi = None;
        // eprintln!(
        //     "DEBUG: Checking if need result phi: then_val={:?}, else_val={:?}",
        //     then_val.is_some(),
        //     else_val.is_some()
        // );
        // Only create result phi if BOTH branches return values (for expression-style ifs)
        // If only one returns a value, that's a type error - skip result phi
        if then_val.is_some() && else_val.is_some() {
            // Determine result type from then expression
            // TODO: Get actual type from HIR expression
            let result_type = IrType::I32; // Placeholder
            let result = match self.builder.build_phi(merge_block, result_type.clone()) {
                Some(r) => {
                    // eprintln!("DEBUG: Created result phi {:?}", r);
                    r
                }
                None => {
                    // eprintln!("DEBUG: Failed to create result phi");
                    return None;
                }
            };

            // eprintln!(
            //     "DEBUG: Adding result phi incoming: then_term={}, else_term={}",
            //     then_terminated, else_terminated
            // );
            // Both branches returned values, so add phi incoming from both
            if !then_terminated {
                let val = then_val.unwrap(); // Safe because we checked is_some() above
                // eprintln!(
                //     "DEBUG:   Adding from then: block={:?}, val={:?}",
                //     then_end_block, val
                // );
               self
                    .builder
                    .add_phi_incoming(merge_block, result, then_end_block, val);
                // {
                //     Some(()) => eprintln!("DEBUG:   Success"),
                //     None => eprintln!("DEBUG:   FAILED!"),
                // }
            }
            if !else_terminated {
                let val = else_val.unwrap(); // Safe because we checked is_some() above
                // eprintln!(
                //     "DEBUG:   Adding from else: block={:?}, val={:?}",
                //     else_end_block, val
                // );
                self
                    .builder
                    .add_phi_incoming(merge_block, result, else_end_block, val);
               
            }
            result_phi = Some(result);
        }

        result_phi
    }

    fn lower_do_while_loop(
        &mut self,
        body: &HirBlock,
        condition: &HirExpr,
        label: Option<&SymbolId>,
    ) {
        // Do-while loop structure:
        // do {
        //     body;
        // } while (condition);
        //
        // MIR structure with phi nodes:
        // entry_block:
        //     goto body_block(initial_values)
        // body_block(phi_nodes):
        //     <body statements>
        //     goto cond_block
        // cond_block:
        //     %cond = <evaluate condition>
        //     br %cond, body_block(updated_values), exit_block(final_values)
        // exit_block(exit_phi_nodes):
        //     <continue>

        // Create blocks
        let Some(body_block) = self.builder.create_block() else {
            return;
        };
        let Some(cond_block) = self.builder.create_block() else {
            return;
        };
        let Some(exit_block) = self.builder.create_block() else {
            return;
        };

        // Save the entry block (current block before loop)
        let entry_block = if let Some(block_id) = self.builder.current_block() {
            block_id
        } else {
            return;
        };

        // Find all variables that are referenced in the loop body and condition
        let mut referenced_vars = std::collections::HashSet::new();
        self.collect_referenced_variables_in_block(body, &mut referenced_vars);
        self.collect_referenced_variables_in_expr(condition, &mut referenced_vars);

        // Only include variables that were declared before the loop
        // (i.e., they're already in the symbol_map)
        // Exclude function parameters since they're immutable
        let modified_vars: std::collections::HashSet<SymbolId> = referenced_vars
            .into_iter()
            .filter(|sym| {
                let in_map = self.symbol_map.contains_key(sym);
                // Check if this is a function parameter by seeing if it's in the current function's parameters
                let is_param = if let Some(func) = self.builder.current_function() {
                    func.signature.parameters.iter().any(|p| {
                        self.symbol_map.get(sym) == Some(&p.reg)
                    })
                } else {
                    false
                };
                in_map && !is_param
            })
            .collect();

        // Save initial values of loop variables before jumping to body
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

        // Jump to body first (do-while always executes once)
        self.builder.build_branch(body_block);

        // Body block - create phi nodes for loop variables
        self.builder.switch_to_block(body_block);

        // Create phi nodes for all loop variables at the start of body block
        let mut phi_nodes: HashMap<SymbolId, IrId> = HashMap::new();
        for (symbol_id, (initial_reg, var_type)) in &loop_var_initial_values {
            if let Some(phi_reg) = self.builder.build_phi(body_block, var_type.clone()) {
                // Add incoming value from entry block (first iteration)
                self.builder
                    .add_phi_incoming(body_block, phi_reg, entry_block, *initial_reg);

                // Register the phi node as a local so Cranelift can find its type
                if let Some(func) = self.builder.current_function_mut() {
                    if let Some(local) = func.locals.get(initial_reg).cloned() {
                        func.locals.insert(
                            phi_reg,
                            super::IrLocal {
                                name: format!("{}_phi", local.name),
                                ty: var_type.clone(),
                                mutable: true,
                                source_location: local.source_location,
                                allocation: super::AllocationHint::Register,
                            },
                        );
                    }
                }

                // Update symbol map to use phi node
                phi_nodes.insert(*symbol_id, phi_reg);
                self.symbol_map.insert(*symbol_id, phi_reg);
            }
        }

        // Push loop context with empty exit_phi_nodes (will be populated later)
        self.loop_stack.push(LoopContext {
            continue_block: cond_block,
            break_block: exit_block,
            label: label.cloned(),
            exit_phi_nodes: HashMap::new(),
        });

        // Lower the body statements
        self.lower_block(body);

        // Get the block we're in after the body (might be different if there are nested blocks)
        let body_end_block = if let Some(block_id) = self.builder.current_block() {
            block_id
        } else {
            self.loop_stack.pop();
            return;
        };

        // Branch to condition block if not already terminated
        if !self.is_terminated() {
            self.builder.build_branch(cond_block);
        }

        // Build condition block
        self.builder.switch_to_block(cond_block);
        let cond_result = self.lower_expression(condition);

        // Capture the block we're actually in AFTER condition evaluation
        // This is where the conditional branch to body/exit will happen
        let cond_end_block = self.builder.current_block().unwrap_or(cond_block);

        // Now create exit block phi nodes with the correct predecessor block
        let mut exit_phi_nodes: HashMap<SymbolId, IrId> = HashMap::new();
        for (symbol_id, _phi_reg) in &phi_nodes {
            if let Some((_, var_type)) = loop_var_initial_values.get(symbol_id) {
                // Get the current value of the variable after the loop body
                let current_value = if let Some(&updated_reg) = self.symbol_map.get(symbol_id) {
                    updated_reg
                } else {
                    continue;
                };

                // Allocate a new register for the exit block parameter
                let exit_param_reg = self.builder.alloc_reg().unwrap();

                // Create the phi node in the exit block with incoming edge from cond_end_block
                if let Some(func) = self.builder.current_function_mut() {
                    if let Some(exit_block_data) = func.cfg.get_block_mut(exit_block) {
                        let exit_phi = super::IrPhiNode {
                            dest: exit_param_reg,
                            incoming: vec![(cond_end_block, current_value)],
                            ty: var_type.clone(),
                        };
                        exit_block_data.add_phi(exit_phi);

                        // Register as a local
                        func.locals.insert(
                            exit_param_reg,
                            super::IrLocal {
                                name: format!("loop_exit_{}", symbol_id.as_raw()),
                                ty: var_type.clone(),
                                mutable: false,
                                source_location: super::IrSourceLocation::unknown(),
                                allocation: super::AllocationHint::Register,
                            },
                        );
                    }
                }

                exit_phi_nodes.insert(*symbol_id, exit_param_reg);
            }
        }

        // Update loop context with the exit phi nodes
        if let Some(loop_ctx) = self.loop_stack.last_mut() {
            loop_ctx.exit_phi_nodes = exit_phi_nodes.clone();
        }

        // Add back-edge phi incoming values from body end to body block
        // These represent the updated values for the next iteration
        for (symbol_id, phi_reg) in &phi_nodes {
            // Get the current value of the variable after the loop body
            let back_edge_value = if let Some(&updated_reg) = self.symbol_map.get(symbol_id) {
                updated_reg
            } else {
                *phi_reg
            };

            // Add incoming value from cond block (back edge for next iteration)
            // The back edge comes from cond_end_block (after condition is evaluated)
            self.builder.add_phi_incoming(body_block, *phi_reg, cond_end_block, back_edge_value);
        }

        // Build conditional branch from the block we're actually in
        if let Some(cond_reg) = cond_result {
            self.builder
                .build_cond_branch(cond_reg, body_block, exit_block);
        }

        // Pop loop context
        self.loop_stack.pop();

        // Continue at exit block
        self.builder.switch_to_block(exit_block);

        // Update symbol map to use exit phi nodes after the loop
        for (symbol_id, exit_reg) in exit_phi_nodes {
            self.symbol_map.insert(symbol_id, exit_reg);
        }
    }

    fn lower_for_in_loop(
        &mut self,
        pattern: &HirPattern,
        iter_expr: &HirExpr,
        body: &HirBlock,
        label: Option<&SymbolId>,
    ) {
        eprintln!("DEBUG [for-in]: ENTERED lower_for_in_loop!");
        eprintln!("DEBUG [for-in]: pattern={:?}", pattern);
        eprintln!("DEBUG [for-in]: iter_expr.ty={:?}", iter_expr.ty);

        // For-in loops over Arrays desugar to index-based iteration:
        // for (x in arr) { body }
        //
        // Becomes:
        // {
        //     var _i = 0;
        //     var _len = haxe_array_length(arr);
        //     while (_i < _len) {
        //         var x = arr[_i];  // Using lower_index_access
        //         body;
        //         _i++;
        //     }
        // }

        // Step 1: Lower the collection expression (the array)
        eprintln!("DEBUG [for-in]: lowering collection expression...");
        let Some(collection) = self.lower_expression(iter_expr) else {
            eprintln!("DEBUG [for-in]: FAILED to lower collection expression!");
            return;
        };

        // DEBUG: Log collection info
        let collection_type = self.builder.get_register_type(collection);
        eprintln!("DEBUG [for-in]: collection reg={:?}, type={:?}", collection, collection_type);

        // Get element type from the iterator expression type
        // iter_expr.ty is the Array<T> type, we need to extract T
        let elem_type_id = self.get_array_element_type(iter_expr.ty).unwrap_or(iter_expr.ty);
        eprintln!("DEBUG [for-in]: array_type={:?}, elem_type={:?}", iter_expr.ty, elem_type_id);

        // Step 2: Get array length by directly reading the 'len' field from HaxeArray struct
        // HaxeArray layout: { ptr: *u8 (8 bytes), len: usize (8 bytes), cap: usize, elem_size: usize }
        // The 'len' field is at offset 8 bytes from the start of the struct
        //
        // We use pointer arithmetic: len_ptr = collection + 8
        let Some(offset_8) = self.builder.build_const(IrValue::I64(8)) else {
            return;
        };
        let Some(len_ptr) = self.builder.build_binop(
            crate::ir::instructions::BinaryOp::Add,
            collection,
            offset_8,
        ) else {
            return;
        };
        let Some(array_len) = self.builder.build_load(len_ptr, IrType::I64) else {
            eprintln!("DEBUG [for-in]: FAILED to load array length!");
            return;
        };
        eprintln!("DEBUG [for-in]: array_len reg={:?} (loaded from offset 8)", array_len);

        // Step 3: Initialize index to 0
        let Some(zero) = self.builder.build_const(IrValue::I64(0)) else {
            return;
        };
        let Some(index_ptr) = self.builder.build_alloc(IrType::I64, None) else {
            return;
        };
        self.builder.build_store(index_ptr, zero);

        // Step 4: Create loop blocks
        let Some(loop_cond_block) = self.builder.create_block() else {
            return;
        };
        let Some(loop_body_block) = self.builder.create_block() else {
            return;
        };
        let Some(loop_exit_block) = self.builder.create_block() else {
            return;
        };

        // Jump to condition check
        self.builder.build_branch(loop_cond_block);

        // Push loop context for break/continue
        self.loop_stack.push(LoopContext {
            continue_block: loop_cond_block,
            break_block: loop_exit_block,
            label: label.cloned(),
            exit_phi_nodes: HashMap::new(),
        });

        // Step 5: Build condition block - check if index < length
        self.builder.switch_to_block(loop_cond_block);
        let Some(current_index) = self.builder.build_load(index_ptr, IrType::I64) else {
            self.loop_stack.pop();
            return;
        };
        let Some(cmp_result) = self.builder.build_cmp(
            crate::ir::instructions::CompareOp::Lt,
            current_index,
            array_len,
        ) else {
            self.loop_stack.pop();
            return;
        };

        // Conditional branch based on comparison
        self.builder.build_cond_branch(cmp_result, loop_body_block, loop_exit_block);

        // Step 6: Build body block
        self.builder.switch_to_block(loop_body_block);

        // Reload current index for element access
        let Some(idx_for_access) = self.builder.build_load(index_ptr, IrType::I64) else {
            self.loop_stack.pop();
            return;
        };

        // Get element at current index using lower_index_access (same as arr[i])
        let Some(element_value) = self.lower_index_access(collection, idx_for_access, elem_type_id) else {
            self.loop_stack.pop();
            return;
        };

        // Bind the pattern to the element value
        match pattern {
            HirPattern::Variable { symbol, .. } => {
                self.symbol_map.insert(*symbol, element_value);
            }
            _ => {
                // Complex patterns need full pattern matching
                // For now, just use the element value
            }
        }

        // Lower the loop body
        self.lower_block(body);

        // Increment index: _i++
        if !self.is_terminated() {
            let Some(idx_to_inc) = self.builder.build_load(index_ptr, IrType::I64) else {
                self.loop_stack.pop();
                return;
            };
            let Some(one) = self.builder.build_const(IrValue::I64(1)) else {
                self.loop_stack.pop();
                return;
            };
            let Some(next_index) = self.builder.build_binop(
                crate::ir::instructions::BinaryOp::Add,
                idx_to_inc,
                one,
            ) else {
                self.loop_stack.pop();
                return;
            };
            self.builder.build_store(index_ptr, next_index);

            // Jump back to condition check
            self.builder.build_branch(loop_cond_block);
        }

        // Pop loop context
        self.loop_stack.pop();

        // Step 7: Continue at exit block
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
            if let (Some(test), Some(body)) =
                (self.builder.create_block(), self.builder.create_block())
            {
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
            let next_test = case_test_blocks
                .get(i + 1)
                .copied()
                .unwrap_or(default_block);

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
                self.builder
                    .build_cond_branch(pattern_matches, guard_block, next_test);

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
                self.builder
                    .build_cond_branch(guard_val, body_block, next_test);
            } else {
                // No guard, just test pattern
                self.builder
                    .build_cond_branch(pattern_matches, body_block, next_test);
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
                    _ => TypeId::from_raw(1),                  // Default to Int
                };
                let lit_val = self.lower_literal(lit, default_type)?;
                // TODO: Use proper comparison based on type
                self.builder.build_cmp(CompareOp::Eq, scrutinee, lit_val)
            }

            HirPattern::Constructor {
                enum_type,
                variant,
                fields,
            } => {
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
                    IrType::Ptr(Box::new(IrType::I32)),
                ) else {
                    return None;
                };

                let Some(tag_val) = self.builder.build_load(tag_ptr, IrType::I32) else {
                    return None;
                };

                // TODO: Look up variant discriminant from type metadata
                // For now, use a placeholder value (hash of variant name)
                let variant_discriminant = variant.to_string().len() as i64; // Placeholder

                let Some(expected_tag) = self.builder.build_int(variant_discriminant, IrType::I32)
                else {
                    return None;
                };

                // Compare tags
                let Some(tag_matches) =
                    self.builder.build_cmp(CompareOp::Eq, tag_val, expected_tag)
                else {
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
                    let Some(field_idx) = self.builder.build_int((i + 1) as i64, IrType::I64)
                    else {
                        return None;
                    };

                    let Some(field_ptr) = self.builder.build_gep(
                        scrutinee,
                        vec![field_idx],
                        IrType::Ptr(Box::new(IrType::Any)),
                    ) else {
                        return None;
                    };

                    let Some(field_val) = self.builder.build_load(field_ptr, IrType::Any) else {
                        return None;
                    };

                    // Recursively test field pattern
                    let Some(field_match) = self.lower_pattern_test(field_val, field_pattern)
                    else {
                        return None;
                    };

                    // Combine with AND
                    all_fields_match =
                        self.builder
                            .build_binop(BinaryOp::And, all_fields_match, field_match)?;
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
                        IrType::Ptr(Box::new(IrType::Any)),
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
                    all_match = self
                        .builder
                        .build_binop(BinaryOp::And, all_match, elem_match)?;
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
                    IrType::Ptr(Box::new(IrType::I64)),
                ) else {
                    return None;
                };

                let Some(array_length) = self.builder.build_load(length_ptr, IrType::I64) else {
                    return None;
                };

                let mut all_match = self.builder.build_bool(true)?;

                // If no rest pattern, check exact length
                if rest.is_none() {
                    let Some(expected_len) =
                        self.builder.build_int(elements.len() as i64, IrType::I64)
                    else {
                        return None;
                    };

                    let Some(length_matches) =
                        self.builder
                            .build_cmp(CompareOp::Eq, array_length, expected_len)
                    else {
                        return None;
                    };

                    all_match =
                        self.builder
                            .build_binop(BinaryOp::And, all_match, length_matches)?;
                } else {
                    // With rest pattern, check minimum length
                    let Some(min_len) = self.builder.build_int(elements.len() as i64, IrType::I64)
                    else {
                        return None;
                    };

                    let Some(length_sufficient) =
                        self.builder.build_cmp(CompareOp::Ge, array_length, min_len)
                    else {
                        return None;
                    };

                    all_match =
                        self.builder
                            .build_binop(BinaryOp::And, all_match, length_sufficient)?;
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
                        IrType::Ptr(Box::new(IrType::Any)),
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

                    all_match = self
                        .builder
                        .build_binop(BinaryOp::And, all_match, elem_match)?;
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
                        IrType::Ptr(Box::new(IrType::Any)),
                    ) else {
                        return None;
                    };

                    let Some(field_val) = self.builder.build_load(field_ptr, IrType::Any) else {
                        return None;
                    };

                    // Recursively test field pattern
                    let Some(field_match) = self.lower_pattern_test(field_val, field_pattern)
                    else {
                        return None;
                    };

                    all_match = self
                        .builder
                        .build_binop(BinaryOp::And, all_match, field_match)?;
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
                self.builder
                    .build_binop(BinaryOp::And, pattern_match, guard_val)
            }
        }
    }

    fn lower_try_catch(
        &mut self,
        try_block: &HirBlock,
        catches: &[HirCatchClause],
        finally: Option<&HirBlock>,
    ) {
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

    fn lower_lambda(
        &mut self,
        params: &[HirParam],
        body: &HirExpr,
        captures: &[HirCapture],
        lambda_type: TypeId,
    ) -> Option<IrId> {
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

        // Step 1: Collect captured values from current scope FIRST
        // (before generate_lambda_function which saves/restores state)
        let mut captured_values = Vec::new();
        eprintln!("DEBUG: Collecting {} captured values", captures.len());
        eprintln!("DEBUG: symbol_map has {} entries", self.symbol_map.len());
        for capture in captures {
            eprintln!("DEBUG: Looking for captured symbol {:?}", capture.symbol);
            if let Some(&captured_val) = self.symbol_map.get(&capture.symbol) {
                eprintln!("DEBUG:   Found! Register: {:?}", captured_val);
                captured_values.push(captured_val);
            } else {
                // Captured variable not found in current scope
                eprintln!("DEBUG:   NOT FOUND! Available symbols:");
                for (sym, reg) in &self.symbol_map {
                    eprintln!("DEBUG:     {:?} -> {:?}", sym, reg);
                }
                self.errors.push(LoweringError {
                    message: format!("Captured variable {:?} not found in scope", capture.symbol),
                    location: body.source_location.clone(),
                });
                return None;
            }
        }

        // Step 2: Generate the lambda function
        let lambda_func_id = self.generate_lambda_function(params, body, captures, lambda_type)?;

        // Step 3: Use MakeClosure instruction to create closure
        self.builder
            .build_make_closure(lambda_func_id, captured_values)
    }

    // ========================================================================
    // Two-Pass Lambda Generation (New Architecture) - Helper Methods
    // ========================================================================

    fn save_state(&self) -> SavedLoweringState {
        SavedLoweringState {
            current_function: self.builder.current_function,
            current_block: self.builder.current_block,
            symbol_map: self.symbol_map.clone(),
            current_env_layout: self.current_env_layout.clone(),
        }
    }

    fn restore_state(&mut self, state: SavedLoweringState) {
        self.builder.current_function = state.current_function;
        self.builder.current_block = state.current_block;
        self.symbol_map = state.symbol_map;
        self.current_env_layout = state.current_env_layout;
    }

    /// PASS 1: Create lambda skeleton with placeholder signature
    fn generate_lambda_skeleton(
        &mut self,
        params: &[HirParam],
        captures: &[HirCapture],
    ) -> LambdaContext {
        // Allocate function ID
        let func_id = self.builder.module.alloc_function_id();
        let lambda_name = format!("<lambda_{}>", self.lambda_counter);
        self.lambda_counter += 1;

        // Build environment layout if we have captures
        let env_layout = if !captures.is_empty() {
            Some(EnvironmentLayout::new(captures, |ty| self.convert_type(ty)))
        } else {
            None
        };

        // Build parameters: [env*,] lambda_params...
        let mut func_params = Vec::new();
        let mut next_reg_id = 0u32;

        if env_layout.is_some() {
            func_params.push(IrParameter {
                name: "env".to_string(),
                ty: IrType::Ptr(Box::new(IrType::Void)),
                reg: IrId::new(next_reg_id),
                by_ref: false,
            });
            next_reg_id += 1;
        }

        for param in params {
            let param_type = self.convert_type(param.ty);
            let param_name = self.string_interner
                .get(param.name)
                .unwrap_or("<param>")
                .to_string();

            func_params.push(IrParameter {
                name: param_name,
                ty: param_type,
                reg: IrId::new(next_reg_id),
                by_ref: false,
            });
            next_reg_id += 1;
        }

        // Create PLACEHOLDER signature
        let signature = IrFunctionSignature {
            parameters: func_params,
            return_type: IrType::Any,  // PLACEHOLDER - will be inferred
            calling_convention: CallingConvention::Haxe,
            can_throw: false,
            type_params: vec![],
            uses_sret: false,
        };

        // Create empty function
        let symbol_id = SymbolId::from_raw(1000000 + func_id.0);
        let lambda_function = IrFunction::new(func_id, symbol_id, lambda_name, signature);
        let entry_block = lambda_function.entry_block();

        // Add to module
        self.builder.module.add_function(lambda_function);

        LambdaContext {
            func_id,
            entry_block,
            param_offset: if env_layout.is_some() { 1 } else { 0 },
            env_layout,
        }
    }

    /// PASS 2: Lower lambda body and infer signature
    #[allow(dead_code)]  // Will be used once we switch to two-pass
    fn lower_lambda_body(
        &mut self,
        context: LambdaContext,
        params: &[HirParam],
        body: &HirExpr,
    ) -> Option<IrFunctionId> {
        let LambdaContext { func_id, entry_block, param_offset, env_layout } = context;

        // Save state
        let saved_state = self.save_state();

        // Switch to lambda context
        self.builder.current_function = Some(func_id);
        self.builder.current_block = Some(entry_block);
        self.symbol_map.clear();
        self.current_env_layout = env_layout.clone();

        // Map lambda parameters to registers
        for (i, param) in params.iter().enumerate() {
            let param_reg = IrId::new(param_offset + i as u32);
            self.symbol_map.insert(param.symbol_id, param_reg);
        }

        // Setup captured variables using environment layout
        if let Some(layout) = &env_layout {
            eprintln!("DEBUG: Lambda has {} captured variables in environment", layout.fields.len());
            for field in &layout.fields {
                eprintln!("DEBUG: Captured symbol: {:?}", field.symbol);
            }

            let env_ptr = IrId::new(0); // First parameter

            for field in &layout.fields {
                // Use layout to load field (handles casting automatically)
                let value_reg = layout.load_field(&mut self.builder, env_ptr, field.symbol)?;
                self.symbol_map.insert(field.symbol, value_reg);
            }
        }

        // Lower the body expression
        let body_result = self.lower_expression(body);

        // Infer return type from actual generated code (borrows function immutably)
        let return_type = {
            let lambda_func = self.builder.module.functions.get(&func_id)?;
            let rt = self.infer_lambda_return_type(lambda_func, entry_block, body_result);
            eprintln!("DEBUG: Lambda final return type: {:?}, body_result: {:?}", rt, body_result);
            rt
        };

        // Update signature and add terminator (borrows function mutably)
        {
            let lambda_func = self.builder.module.functions.get_mut(&func_id)?;
            eprintln!("DEBUG: Updating lambda signature from {:?} to {:?}", lambda_func.signature.return_type, return_type);
            eprintln!("DEBUG: Lambda has {} parameters: {:?}", lambda_func.signature.parameters.len(),
                     lambda_func.signature.parameters.iter().map(|p| &p.name).collect::<Vec<_>>());
            lambda_func.signature.return_type = return_type.clone();
            Self::finalize_lambda_terminator_static(lambda_func, entry_block, body_result, &return_type)?;
        }

        // Restore state
        self.current_env_layout = None;
        self.restore_state(saved_state);

        Some(func_id)
    }

    /// Infer the return type from generated MIR instructions
    fn infer_lambda_return_type(
        &self,
        function: &IrFunction,
        entry_block: IrBlockId,
        body_result: Option<IrId>,
    ) -> IrType {
        use crate::ir::IrInstruction;

        // Strategy 1: Search ALL blocks for Return terminators (not just entry block)
        // Lambdas with complex control flow (loops, conditionals) may have return in other blocks
        for (block_id, block) in &function.cfg.blocks {
            if let IrTerminator::Return { value: Some(ret_reg) } = &block.terminator {
                eprintln!("DEBUG: Found Return terminator in block {:?} with register {:?}", block_id, ret_reg);

                // First try locals table (for parameters and explicitly registered values)
                if let Some(local) = function.locals.get(ret_reg) {
                    eprintln!("DEBUG: Found type in locals table: {:?}", local.ty);
                    return local.ty.clone();
                }

                // If not in locals table, scan ALL blocks for the instruction that defines this register
                eprintln!("DEBUG: Register not in locals, scanning all blocks for defining instruction...");
                for (search_block_id, search_block) in &function.cfg.blocks {
                    for inst in &search_block.instructions {
                        match inst {
                            IrInstruction::Load { dest, ty, .. } if *dest == *ret_reg => {
                                eprintln!("DEBUG: Inferred type from Load instruction in block {:?}: {:?}", search_block_id, ty);
                                return ty.clone();
                            }
                            IrInstruction::Cast { dest, to_ty, .. } if *dest == *ret_reg => {
                                eprintln!("DEBUG: Inferred type from Cast instruction: {:?}", to_ty);
                                return to_ty.clone();
                            }
                            IrInstruction::Const { dest, value, .. } if *dest == *ret_reg => {
                                // Infer from constant value
                                let ty = match value {
                                    IrValue::I32(_) => IrType::I32,
                                    IrValue::I64(_) => IrType::I64,
                                    IrValue::F64(_) => IrType::F64,
                                    IrValue::Bool(_) => IrType::Bool,
                                    IrValue::Null => IrType::Ptr(Box::new(IrType::Void)),
                                    _ => IrType::I64,
                                };
                                eprintln!("DEBUG: Inferred type from Const instruction: {:?}", ty);
                                return ty;
                            }
                            IrInstruction::Cmp { dest, .. } if *dest == *ret_reg => {
                                eprintln!("DEBUG: Inferred type from Cmp instruction: Bool");
                                return IrType::Bool;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Strategy 2: Use body result register type
        if let Some(result_reg) = body_result {
            if let Some(local) = function.locals.get(&result_reg) {
                eprintln!("DEBUG: Inferred return type from body result: {:?}", local.ty);
                return local.ty.clone();
            }
        }

        // Fallback: Void return
        eprintln!("DEBUG: No return type found, using Void");
        IrType::Void
    }

    /// Add terminator to lambda if not already present (static version)
    fn finalize_lambda_terminator_static(
        function: &mut IrFunction,
        entry_block: IrBlockId,
        body_result: Option<IrId>,
        return_type: &IrType,
    ) -> Option<()> {
        // Check if terminator already exists
        {
            let block = function.cfg.get_block_mut(entry_block)?;
            if !matches!(block.terminator, IrTerminator::Unreachable) {
                return Some(()); // Already has terminator
            }
        }

        // Create appropriate terminator
        let terminator = match return_type {
            IrType::Void => IrTerminator::Return { value: None },
            _ => {
                if let Some(result_reg) = body_result {
                    IrTerminator::Return { value: Some(result_reg) }
                } else {
                    // Create default value - allocate register first
                    let default_reg = function.alloc_reg();
                    let default_value = match return_type {
                        IrType::I32 => IrValue::I32(0),
                        IrType::I64 => IrValue::I64(0),
                        IrType::Bool => IrValue::Bool(false),
                        _ => IrValue::I64(0),
                    };

                    // Now get block and add instruction
                    let block = function.cfg.get_block_mut(entry_block)?;
                    block.add_instruction(IrInstruction::Const {
                        dest: default_reg,
                        value: default_value,
                    });

                    IrTerminator::Return { value: Some(default_reg) }
                }
            }
        };

        let block = function.cfg.get_block_mut(entry_block)?;
        block.set_terminator(terminator);
        Some(())
    }

    // ========================================================================
    // Lambda Generation - Now Using Two-Pass Architecture
    // ========================================================================

    /// Generate a lambda function using two-pass architecture
    ///
    /// Creates a new function that takes (env*, params...) as arguments,
    /// where env* is a pointer to a struct containing captured variables.
    ///
    /// Two-Pass Architecture:
    /// - Pass 1: Create skeleton with placeholder signature
    /// - Pass 2: Lower body, infer types from actual MIR, update signature
    fn generate_lambda_function(
        &mut self,
        params: &[HirParam],
        body: &HirExpr,
        captures: &[HirCapture],
        _lambda_type: TypeId,  // No longer needed - type inferred from MIR
    ) -> Option<IrFunctionId> {
        // TWO-PASS LAMBDA LOWERING
        // Pass 1: Create skeleton with placeholder signature
        let context = self.generate_lambda_skeleton(params, captures);

        // Pass 2: Lower body and infer return type from actual MIR
        self.lower_lambda_body(context, params, body)
    }

    fn lower_array_literal(&mut self, elements: &[HirExpr]) -> Option<IrId> {
        // Array literal: [e1, e2, e3, ...]
        //
        // HaxeArray is a 32-byte struct (4 x 8-byte fields): ptr, len, cap, elem_size
        //
        // Strategy:
        // 1. Allocate HaxeArray struct on stack
        // 2. Zero-initialize it (like new Array<T>())
        // 3. For each element, call haxe_array_push_ptr to add it
        // 4. Return pointer to HaxeArray struct

        let element_count = elements.len();

        // Allocate HaxeArray struct on stack (4 x i64 = 32 bytes)
        let array_ptr = self.builder.build_alloc(IrType::I64, None)?;

        // Zero-initialize the HaxeArray struct fields
        if let Some(zero_i64) = self.builder.build_const(IrValue::I64(0)) {
            // Zero out ptr field (offset 0)
            if let Some(index_0) = self.builder.build_const(IrValue::I32(0)) {
                if let Some(ptr_field) = self.builder.build_gep(array_ptr, vec![index_0], IrType::I64) {
                    self.builder.build_store(ptr_field, zero_i64);
                }
            }
            // Zero out len field (offset 8)
            if let Some(index_1) = self.builder.build_const(IrValue::I32(1)) {
                if let Some(len_field) = self.builder.build_gep(array_ptr, vec![index_1], IrType::I64) {
                    self.builder.build_store(len_field, zero_i64);
                }
            }
            // Zero out cap field (offset 16)
            if let Some(index_2) = self.builder.build_const(IrValue::I32(2)) {
                if let Some(cap_field) = self.builder.build_gep(array_ptr, vec![index_2], IrType::I64) {
                    self.builder.build_store(cap_field, zero_i64);
                }
            }
            // Set elem_size field to 8 bytes (offset 24) - assume pointer size
            if let Some(elem_size_val) = self.builder.build_const(IrValue::I64(8)) {
                if let Some(index_3) = self.builder.build_const(IrValue::I32(3)) {
                    if let Some(elem_size_field) = self.builder.build_gep(array_ptr, vec![index_3], IrType::I64) {
                        self.builder.build_store(elem_size_field, elem_size_val);
                    }
                }
            }
        }

        // For non-empty arrays, push each element using haxe_array_push_i64
        // This is inefficient but works correctly with the HaxeArray runtime
        if element_count > 0 {
            // Register haxe_array_push_i64: fn(arr: *HaxeArray, val: i64) -> void
            let push_func_id = self.get_or_register_extern_function(
                "haxe_array_push_i64",
                vec![
                    IrType::Ptr(Box::new(IrType::I64)),  // arr pointer
                    IrType::I64,                         // value (i64 for pointer-sized values)
                ],
                IrType::Void,
            );

            for elem in elements.iter() {
                let elem_val = self.lower_expression(elem)?;

                // Call haxe_array_push_i64(arr, val) - this is a void function, so ignore the None return
                self.builder.build_call_direct(push_func_id, vec![array_ptr, elem_val], IrType::Void);
            }
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
        let map_ptr = self
            .builder
            .build_alloc(IrType::Ptr(Box::new(IrType::Any)), Some(count_val))?;

        // Store size in header (index 0)
        let size_field = self.builder.build_int(entry_count as i64, IrType::I64)?;
        self.builder.build_store(map_ptr, size_field)?;

        // Store capacity (index 1)
        let capacity_val = self.builder.build_int(entry_count as i64, IrType::I64)?;
        let capacity_idx = self.builder.build_int(1, IrType::I64)?;
        let capacity_ptr = self.builder.build_gep(
            map_ptr,
            vec![capacity_idx],
            IrType::Ptr(Box::new(IrType::Any)),
        )?;
        self.builder.build_store(capacity_ptr, capacity_val)?;

        // Store each key-value pair
        for (i, (key, value)) in entries.iter().enumerate() {
            let key_val = self.lower_expression(key)?;
            let value_val = self.lower_expression(value)?;

            // Store key at index: 2 + i * 2
            let key_index = 2 + (i * 2);
            let key_idx = self.builder.build_int(key_index as i64, IrType::I64)?;
            let key_ptr = self.builder.build_gep(
                map_ptr,
                vec![key_idx],
                IrType::Ptr(Box::new(IrType::Any)),
            )?;
            self.builder.build_store(key_ptr, key_val)?;

            // Store value at index: 2 + i * 2 + 1
            let val_index = 2 + (i * 2) + 1;
            let val_idx = self.builder.build_int(val_index as i64, IrType::I64)?;
            let val_ptr = self.builder.build_gep(
                map_ptr,
                vec![val_idx],
                IrType::Ptr(Box::new(IrType::Any)),
            )?;
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
        let object_ptr = self
            .builder
            .build_alloc(IrType::Ptr(Box::new(IrType::Any)), Some(count_val))?;

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
            let field_ptr = self.builder.build_gep(
                object_ptr,
                vec![index],
                IrType::Ptr(Box::new(IrType::Any)),
            )?;
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
            let fields: Vec<IrField> = variant
                .fields
                .iter()
                .map(|field| {
                    IrField {
                        name: field.name.to_string(),
                        ty: IrType::Any, // TODO: Convert TypeId to IrType
                        offset: None,
                    }
                })
                .collect();

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
          

            // Try direct lookup first
            if let Some(HirTypeDecl::Class(parent_class)) =
                self.current_hir_types.get(&parent_type_id)
            {
               
                self.add_parent_fields(parent_class, child_type, fields, field_index);
            } else {
                // TypeId mismatch - the extends field might use instance type while
                // hir_module.types uses declaration type. Search by matching class type.
               

                // Get the type definition to find the class symbol
                if let Some(parent_type_def) = self.type_table.borrow().get(parent_type_id) {
                    if let crate::tast::TypeKind::Class {
                        symbol_id: parent_symbol,
                        ..
                    } = &parent_type_def.kind
                    {
                       

                        // Find the HIR class by symbol_id
                        for (decl_type_id, type_decl) in self.current_hir_types.iter() {
                            if let HirTypeDecl::Class(class) = type_decl {
                                if class.symbol_id == *parent_symbol {
                                    
                                    self.add_parent_fields(class, child_type, fields, field_index);
                                    return;
                                }
                            }
                        }
                    }
                }

                // eprintln!(
                //     "WARNING: Could not find parent class for TypeId={:?}",
                //     parent_type_id
                // );
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
        // First, recursively collect grandparent fields
        self.collect_inherited_fields(parent_class.extends, child_type, fields, field_index);

        // Then add parent's own fields
        for parent_field in &parent_class.fields {
            // Map parent field symbol to child class's type with the correct index
            self.field_index_map
                .insert(parent_field.symbol_id, (child_type, *field_index));

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
        self.collect_inherited_fields(class.extends, type_id, &mut fields, &mut field_index);

        // Then add this class's own fields

        for field in &class.fields {
            // Store field index mapping for field access lowering
            self.field_index_map
                .insert(field.symbol_id, (type_id, field_index));

            // Store property accessor info if this is a property with custom getters/setters
            if let Some(ref property_info) = field.property_access {
                self.property_access_map
                    .insert(field.symbol_id, property_info.clone());
            }

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

        let fields: Vec<IrField> = interface
            .methods
            .iter()
            .map(|method| {
                IrField {
                    name: method.name.to_string(),
                    ty: IrType::Ptr(Box::new(IrType::Function {
                        params: vec![IrType::Any], // Placeholder
                        return_type: Box::new(IrType::Any),
                        varargs: false,
                    })),
                    offset: None,
                }
            })
            .collect();

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
        // Type aliases - check if it's an alias to an anonymous struct
        // If so, register the anonymous struct fields in typedef_field_map
        // This allows field access on typedef'd anonymous structs like FileStat

        let type_table = self.type_table.borrow();
        if let Some(aliased_info) = type_table.get(alias.aliased_type) {
            if let TypeKind::Anonymous { fields } = &aliased_info.kind {
                // Register each field of the anonymous struct by name
                // All fields are 8 bytes for consistent boxing/sizing
                for (index, field) in fields.iter().enumerate() {
                    // Register in typedef_field_map by (typedef_type_id, field_name) -> index
                    // This allows lookup when field access creates new symbols
                    self.typedef_field_map.insert((type_id, field.name), index as u32);

                    // Also try to register any existing symbols with this name
                    let field_symbol = self.symbol_table
                        .symbols_of_kind(crate::tast::symbols::SymbolKind::Field)
                        .into_iter()
                        .find(|s| s.name == field.name)
                        .map(|s| s.id);

                    if let Some(field_sym_id) = field_symbol {
                        // Register in field_index_map: (TypeId of typedef, field index)
                        self.field_index_map.insert(field_sym_id, (type_id, index as u32));
                    }
                }

                // Also create an IrTypeDef with struct fields for proper layout info
                let typedef_id = self.builder.module.alloc_typedef_id();

                let ir_fields: Vec<IrField> = fields.iter().enumerate().map(|(idx, f)| {
                    let field_name = self.string_interner.get(f.name)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("field_{}", idx));
                    IrField {
                        name: field_name,
                        ty: self.convert_type(f.type_id),
                        offset: Some((idx * 8) as u32), // 8 bytes per field
                    }
                }).collect();

                let typedef = IrTypeDef {
                    id: typedef_id,
                    name: alias.name.to_string(),
                    type_id,
                    definition: IrTypeDefinition::Struct {
                        fields: ir_fields,
                        packed: false,
                    },
                    source_location: IrSourceLocation::unknown(),
                };

                self.builder.module.add_type(typedef);
                drop(type_table);
                return;
            }
        }
        drop(type_table);

        // Not an anonymous struct - just register as simple alias
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
        let _init_func_id =
            self.builder
                .start_function(init_symbol, "__init__".to_string(), init_sig);

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
    lower_hir_to_mir_with_externals(
        hir_module,
        string_interner,
        type_table,
        symbol_table,
        HashMap::new(),
    )
}

/// Public API for HIR to MIR lowering with external function references
///
/// The `external_functions` map contains SymbolId -> IrFunctionId mappings for functions
/// defined in other modules (e.g., stdlib) that can be called from this module.
pub fn lower_hir_to_mir_with_externals(
    hir_module: &HirModule,
    string_interner: &StringInterner,
    type_table: &Rc<RefCell<TypeTable>>,
    symbol_table: &SymbolTable,
    external_functions: HashMap<SymbolId, IrFunctionId>,
) -> Result<IrModule, Vec<LoweringError>> {
    let mut context = HirToMirContext::new(
        hir_module.name.clone(),
        hir_module.metadata.source_file.clone(),
        string_interner,
        type_table,
        &hir_module.types,
        symbol_table,
    );

    // Set the external function map
    context.external_function_map = external_functions;

    context.lower_module(hir_module)
}

/// Result of MIR lowering that includes both the module and the function mappings
pub struct MirLoweringResult {
    /// The compiled MIR module
    pub module: IrModule,
    /// Mapping from HIR function symbols to MIR function IDs
    /// This can be used to build the external_functions map for other modules
    pub function_map: HashMap<SymbolId, IrFunctionId>,
}

/// Lower HIR to MIR and return both the module and function mappings
///
/// This is useful when you need to compile multiple modules and have later modules
/// call functions from earlier modules (e.g., user code calling stdlib functions).
pub fn lower_hir_to_mir_with_function_map(
    hir_module: &HirModule,
    string_interner: &StringInterner,
    type_table: &Rc<RefCell<TypeTable>>,
    symbol_table: &SymbolTable,
    external_functions: HashMap<SymbolId, IrFunctionId>,
    external_functions_by_name: HashMap<String, IrFunctionId>,
) -> Result<MirLoweringResult, Vec<LoweringError>> {
    let mut context = HirToMirContext::new(
        hir_module.name.clone(),
        hir_module.metadata.source_file.clone(),
        string_interner,
        type_table,
        &hir_module.types,
        symbol_table,
    );

    // Set the external function maps (by SymbolId and by qualified name)
    context.external_function_map = external_functions;
    context.external_function_name_map = external_functions_by_name;

    let module = context.lower_module(hir_module)?;

    Ok(MirLoweringResult {
        module,
        function_map: context.function_map,
    })
}
