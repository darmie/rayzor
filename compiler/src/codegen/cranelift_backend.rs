/// Cranelift JIT Backend
///
/// This backend translates MIR to Cranelift IR and performs JIT compilation.
/// Used for:
/// - Cold path execution (first call of functions)
/// - Development mode (fast iteration)
/// - Testing
///
/// Performance targets:
/// - Compilation: 50-200ms per function
/// - Runtime: 15-25x interpreter speed
use cranelift::prelude::*;
use cranelift_codegen::ir::{ArgumentPurpose, BlockArg, Function};
use cranelift_codegen::settings;
use cranelift_frontend::Variable;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_native;
use std::collections::{HashMap, HashSet};

use crate::ir::{
    IrBasicBlock, IrBlockId, IrControlFlowGraph, IrFunction, IrFunctionId, IrId, IrInstruction,
    IrModule, IrTerminator, IrType, IrValue,
};

/// Cranelift JIT backend for compiling MIR to native code
pub struct CraneliftBackend {
    /// Cranelift JIT module
    module: JITModule,

    /// Cranelift codegen context
    ctx: codegen::Context,

    /// Map from MIR function IDs to Cranelift function IDs
    function_map: HashMap<IrFunctionId, FuncId>,

    /// Map from MIR value IDs to Cranelift values (per function)
    pub(super) value_map: HashMap<IrId, Value>,

    /// Map from closure registers (function pointers) to their environment pointers
    /// This is populated during MakeClosure and used during CallIndirect
    closure_environments: HashMap<IrId, Value>,

    /// Map from runtime function names to Cranelift function IDs
    /// Used for rayzor_malloc, rayzor_realloc, rayzor_free
    runtime_functions: HashMap<String, FuncId>,

    /// Target pointer size (32-bit or 64-bit) from ISA
    pointer_type: types::Type,

    /// Module counter for unique function naming across multiple MIR modules
    /// Each MIR module starts function IDs from 0, so we need to disambiguate
    module_counter: usize,

    /// Set of Cranelift FuncIds that have already been defined (had their bodies compiled)
    /// Used to prevent duplicate definition errors when functions are shared across modules
    /// We track by FuncId (not name) because different modules can have functions with the
    /// same MIR name (e.g., 'new') but different Cranelift symbols (e.g., m1_func_0 vs m3_func_0)
    defined_functions: HashSet<FuncId>,

    /// The environment parameter for the current function being compiled
    /// This is used by ClosureEnv to access the environment
    current_env_param: Option<Value>,

    /// Map from string content to its DataId in the module
    /// Used to reuse string constants across functions
    string_data: HashMap<String, DataId>,

    /// Counter for unique string data names
    string_counter: usize,
}

impl CraneliftBackend {
    /// Create a new Cranelift backend with default optimization level (speed)
    pub fn new() -> Result<Self, String> {
        Self::with_symbols(&[])
    }

    /// Create a new Cranelift backend with custom runtime symbols from plugins
    pub fn with_symbols(symbols: &[(&str, *const u8)]) -> Result<Self, String> {
        Self::with_symbols_and_opt("speed", symbols)
    }

    /// Create a new Cranelift backend with specified optimization level
    ///
    /// Optimization levels:
    /// - "none": No optimization (Tier 0 - Baseline)
    /// - "speed": Moderate optimization (Tier 1 - Standard)
    /// - "speed_and_size": Aggressive optimization (Tier 2 - Optimized)
    pub fn with_optimization_level(opt_level: &str) -> Result<Self, String> {
        Self::with_symbols_and_opt(opt_level, &[])
    }

    /// Internal: Create backend with symbols and optimization level
    fn with_symbols_and_opt(
        opt_level: &str,
        symbols: &[(&str, *const u8)],
    ) -> Result<Self, String> {
        // Configure Cranelift for the current platform
        let mut flag_builder = settings::builder();

        // Disable colocated libcalls for compatibility with ARM64 (Apple Silicon)
        // This prevents PLT usage which is only supported on x86_64
        flag_builder
            .set("use_colocated_libcalls", "false")
            .map_err(|e| format!("Failed to set use_colocated_libcalls: {}", e))?;

        // Disable PIC (Position Independent Code) for simpler code generation
        flag_builder
            .set("is_pic", "false")
            .map_err(|e| format!("Failed to set is_pic: {}", e))?;

        // Set optimization level (configurable for tiered compilation)
        flag_builder
            .set("opt_level", opt_level)
            .map_err(|e| format!("Failed to set opt_level: {}", e))?;

        // Enable verifier for detailed error messages during development
        flag_builder
            .set("enable_verifier", "true")
            .map_err(|e| format!("Failed to set enable_verifier: {}", e))?;

        // Create ISA for the current platform
        let isa_builder = cranelift_native::builder()
            .map_err(|e| format!("Failed to create ISA builder: {}", e))?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| format!("Failed to create ISA: {}", e))?;

        // Create JIT builder with ISA
        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        // Register runtime symbols from plugins
        for (name, ptr) in symbols {
            builder.symbol(*name, *ptr);
        }

        // Create JIT module
        let mut module = JITModule::new(builder);
        let ctx = module.make_context();

        // Get target pointer type from ISA
        let pointer_type = module.target_config().pointer_type();

        Ok(Self {
            module,
            ctx,
            function_map: HashMap::new(),
            value_map: HashMap::new(),
            closure_environments: HashMap::new(),
            runtime_functions: HashMap::new(),
            pointer_type,
            module_counter: 0,
            defined_functions: HashSet::new(),
            current_env_param: None,
            string_data: HashMap::new(),
            string_counter: 0,
        })
    }

    /// Get the pointer size in bytes for the target architecture
    pub fn get_pointer_size(&self) -> u32 {
        match self.pointer_type {
            types::I32 => 4,
            types::I64 => 8,
            _ => 8, // Default to 64-bit
        }
    }

    /// Get the size of a type in bytes according to the target architecture
    pub fn get_type_size(&self, ty: &IrType) -> u64 {
        match ty {
            IrType::Void => 0,
            IrType::Bool | IrType::I8 | IrType::U8 => 1,
            IrType::I16 | IrType::U16 => 2,
            IrType::I32 | IrType::U32 | IrType::F32 => 4,
            IrType::I64 | IrType::U64 | IrType::F64 => 8,
            IrType::Ptr(_) | IrType::Ref(_) | IrType::Function { .. } => {
                self.get_pointer_size() as u64
            }
            IrType::Array(elem_ty, count) => self.get_type_size(elem_ty) * (*count as u64),
            IrType::Slice(_) | IrType::String => {
                // Slice is {ptr, len} = pointer + i64
                self.get_pointer_size() as u64 + 8
            }
            IrType::Struct { fields, .. } => {
                // Sum of field sizes (simplified, doesn't account for padding)
                fields.iter().map(|f| self.get_type_size(&f.ty)).sum()
            }
            IrType::Union { variants, .. } => {
                // Tag (i32) + max variant size
                let max_variant_size = variants
                    .iter()
                    .map(|v| v.fields.iter().map(|f| f.size()).sum::<usize>())
                    .max()
                    .unwrap_or(0);
                4 + max_variant_size as u64
            }
            IrType::Opaque { size, .. } => *size as u64,
            IrType::Any => {
                // {i64 type_id, ptr value_ptr}
                8 + self.get_pointer_size() as u64
            }
            IrType::TypeVar(_) => 0, // Should be monomorphized
        }
    }

    /// Get the alignment of a type in bytes (simplified)
    pub fn get_type_alignment(&self, ty: &IrType) -> u32 {
        match ty {
            IrType::Void => 1,
            IrType::Bool | IrType::I8 | IrType::U8 => 1,
            IrType::I16 | IrType::U16 => 2,
            IrType::I32 | IrType::U32 | IrType::F32 => 4,
            IrType::I64 | IrType::U64 | IrType::F64 => 8,
            IrType::Ptr(_) | IrType::Ref(_) | IrType::Function { .. } => self.get_pointer_size(),
            IrType::Array(elem_ty, _) => self.get_type_alignment(elem_ty),
            IrType::Slice(_) | IrType::String => self.get_pointer_size(), // Aligned to pointer
            IrType::Struct { fields, .. } => {
                // Max alignment of fields
                fields
                    .iter()
                    .map(|f| self.get_type_alignment(&f.ty))
                    .max()
                    .unwrap_or(1)
            }
            IrType::Union { variants, .. } => {
                // Max alignment of variant fields
                let max_align = variants
                    .iter()
                    .flat_map(|v| v.fields.iter())
                    .map(|f| self.get_type_alignment(f))
                    .max()
                    .unwrap_or(1);
                max_align.max(4) // At least 4 for the tag
            }
            IrType::Opaque { align, .. } => *align as u32,
            IrType::Any => 8, // Aligned to i64
            IrType::TypeVar(_) => 1,
        }
    }

    /// Find the source function ID for a function pointer register
    /// This scans the MIR instructions to find if this register comes from FunctionRef or MakeClosure
    fn find_function_ref_source(func_ptr: IrId, function: &IrFunction) -> Option<IrFunctionId> {
        // First, try to find the direct source
        for (_, block) in &function.cfg.blocks {
            for inst in &block.instructions {
                match inst {
                    IrInstruction::FunctionRef { dest, func_id } if *dest == func_ptr => {
                        return Some(*func_id);
                    }
                    IrInstruction::MakeClosure { dest, func_id, .. } if *dest == func_ptr => {
                        return Some(*func_id);
                    }
                    _ => {}
                }
            }
        }

        // If not found, check if func_ptr comes from a Load of a closure object
        // This handles the case where closure fn_ptr is extracted via Load
        for (_, block) in &function.cfg.blocks {
            for inst in &block.instructions {
                if let IrInstruction::Load { dest, ptr, .. } = inst {
                    if *dest == func_ptr {
                        // Check if ptr comes from a MakeClosure or PtrAdd of a MakeClosure
                        // First try direct MakeClosure
                        for (_, inner_block) in &function.cfg.blocks {
                            for inner_inst in &inner_block.instructions {
                                match inner_inst {
                                    IrInstruction::MakeClosure {
                                        dest: closure_dest,
                                        func_id,
                                        ..
                                    } if closure_dest == ptr => {
                                        eprintln!("DEBUG: Traced func_ptr through Load from MakeClosure to lambda {:?}", func_id);
                                        return Some(*func_id);
                                    }
                                    // Also check PtrAdd (for field access at offset)
                                    IrInstruction::PtrAdd {
                                        dest: ptr_add_dest,
                                        ptr: base_ptr,
                                        ..
                                    } if ptr_add_dest == ptr => {
                                        // Check if base_ptr is from MakeClosure
                                        for (_, deepest_block) in &function.cfg.blocks {
                                            for deepest_inst in &deepest_block.instructions {
                                                if let IrInstruction::MakeClosure {
                                                    dest: closure_dest,
                                                    func_id,
                                                    ..
                                                } = deepest_inst
                                                {
                                                    if closure_dest == base_ptr {
                                                        eprintln!("DEBUG: Traced func_ptr through Load->PtrAdd->MakeClosure to lambda {:?}", func_id);
                                                        return Some(*func_id);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Compile an entire MIR module
    pub fn compile_module(&mut self, mir_module: &IrModule) -> Result<(), String> {
        // Skip modules that only have extern functions (no implementations)
        // These are typically stdlib Haxe wrapper files (Thread.hx, Channel.hx, etc.)
        // that only declare externs. The actual implementations come from build_stdlib()
        // which gets merged into the user module.
        let has_implementations = mir_module
            .functions
            .values()
            .any(|f| !f.cfg.blocks.is_empty());

        if !has_implementations {
            eprintln!(
                "DEBUG: Skipping module '{}' - no implementations (only {} extern declarations)",
                mir_module.name,
                mir_module.functions.len()
            );
            return Ok(());
        }

        // Increment module counter for unique function naming across modules
        // This prevents collisions when multiple MIR modules (each starting IDs from 0) are compiled
        let current_module = self.module_counter;
        self.module_counter += 1;

        // IMPORTANT: Don't clear function_map between modules!
        //
        // Each MIR module starts function IDs from 0, which would normally cause collisions.
        // However, we handle this with:
        // 1. Renumbered stdlib function IDs (done in compilation.rs) - ensures no ID collisions
        // 2. Unique function naming for regular functions (module_counter prefix)
        // 3. Extern function reuse (runtime_functions tracking) - prevents duplicate declarations
        //
        // We MUST NOT clear function_map because:
        // - Extern functions are shared across modules and need persistent MIR ID -> Cranelift ID mappings
        // - Without persistent mappings, Module 1's code can't call externs after Module 2 clears the map
        //
        // Previously we cleared function_map, which broke cross-module extern function references.
        eprintln!(
            "DEBUG: Compiling module '{}' #{} (function_map has {} entries)",
            mir_module.name,
            current_module,
            self.function_map.len()
        );

        // First pass: declare all functions (except malloc/realloc/free which we handle separately)
        for (func_id, function) in &mir_module.functions {
            // Skip libc memory management functions - we'll declare them separately and map MIR IDs to libc
            if function.name == "malloc" || function.name == "realloc" || function.name == "free" {
                continue;
            }
            self.declare_function(*func_id, function)?;
        }

        // Declare C standard library memory functions ONCE (across ALL modules)
        // These are always available and will be linked from libc
        // Declare them unconditionally if not already declared
        if !self.runtime_functions.contains_key("malloc") {
            eprintln!("DEBUG: Declaring libc function: malloc");
            self.declare_libc_function("malloc", 1, true)?; // 1 param (size), has return value (ptr)
        }
        if !self.runtime_functions.contains_key("realloc") {
            eprintln!("DEBUG: Declaring libc function: realloc");
            self.declare_libc_function("realloc", 2, true)?; // 2 params (ptr, size), has return value (new ptr)
        }
        if !self.runtime_functions.contains_key("free") {
            eprintln!("DEBUG: Declaring libc function: free");
            self.declare_libc_function("free", 1, false)?; // 1 param (ptr), no return value
        }

        // Map MIR function IDs for malloc/realloc/free to their libc Cranelift IDs
        // This ensures that when MIR code calls these functions, they resolve to the libc versions
        for (func_id, function) in &mir_module.functions {
            if function.name == "malloc" {
                let libc_id = *self.runtime_functions.get("malloc").unwrap();
                eprintln!(
                    "DEBUG: Mapping MIR malloc {:?} -> Cranelift {:?}",
                    func_id, libc_id
                );
                self.function_map.insert(*func_id, libc_id);
            } else if function.name == "realloc" {
                let libc_id = *self.runtime_functions.get("realloc").unwrap();
                eprintln!(
                    "DEBUG: Mapping MIR realloc {:?} -> Cranelift {:?}",
                    func_id, libc_id
                );
                self.function_map.insert(*func_id, libc_id);
            } else if function.name == "free" {
                let libc_id = *self.runtime_functions.get("free").unwrap();
                eprintln!(
                    "DEBUG: Mapping MIR free {:?} -> Cranelift {:?}",
                    func_id, libc_id
                );
                self.function_map.insert(*func_id, libc_id);
            }
        }

        // Second pass: compile function bodies (skip extern functions with empty CFGs)
        for (func_id, function) in &mir_module.functions {
            // Skip extern functions (empty CFG means extern declaration)
            if function.cfg.blocks.is_empty() {
                eprintln!("DEBUG: Skipping extern function: {}", function.name);
                continue;
            }
            self.compile_function(*func_id, mir_module, function)?;
        }

        // Finalize the module
        self.module
            .finalize_definitions()
            .map_err(|e| format!("Failed to finalize definitions: {}", e))?;

        Ok(())
    }

    /// Compile a single function (for tiered compilation)
    ///
    /// This method declares, compiles, and finalizes a single function.
    /// Used by the tiered backend to recompile hot functions at higher optimization levels.
    pub fn compile_single_function(
        &mut self,
        mir_func_id: IrFunctionId,
        mir_module: &IrModule,
        function: &IrFunction,
    ) -> Result<(), String> {
        // Declare the function (if not already declared)
        if !self.function_map.contains_key(&mir_func_id) {
            self.declare_function(mir_func_id, function)?;
        }

        // Compile the function body
        self.compile_function(mir_func_id, mir_module, function)?;

        // Finalize this function
        self.module
            .finalize_definitions()
            .map_err(|e| format!("Failed to finalize function: {}", e))?;

        Ok(())
    }

    /// Declare a function signature (first pass)
    fn declare_function(
        &mut self,
        mir_func_id: IrFunctionId,
        function: &IrFunction,
    ) -> Result<(), String> {
        // Determine if this is an extern function (empty CFG)
        let is_extern = function.cfg.blocks.is_empty();

        // CRITICAL: Check if this function was already declared (by name)
        // Both extern functions AND stdlib wrapper functions are shared across modules,
        // so we must not declare them twice!
        // We use runtime_functions to track all such functions (not just libc ones)
        if let Some(&existing_func_id) = self.runtime_functions.get(&function.name) {
            eprintln!(
                "DEBUG: Reusing existing function '{}' - MIR {:?} -> Cranelift {:?}",
                function.name, mir_func_id, existing_func_id
            );
            self.function_map.insert(mir_func_id, existing_func_id);
            return Ok(());
        }

        // Build Cranelift signature
        let mut sig = self.module.make_signature();

        // Check if we need sret (struct return by pointer)
        // MUST use sret for ALL functions (including extern) that return structs
        // because the C ABI on ARM64 uses sret for structs > 16 bytes
        let use_sret_in_signature = function.signature.uses_sret;

        if use_sret_in_signature {
            // Add sret parameter as first parameter
            sig.params.push(AbiParam::special(
                types::I64, // pointer type
                ArgumentPurpose::StructReturn,
            ));
        }

        // Add environment parameter (hidden first/second parameter) for non-extern functions
        // All non-extern functions now accept an environment pointer (null for static functions)
        // Extern functions do NOT get this parameter since they're C ABI
        // IMPORTANT: Lambda functions already have an 'env' parameter added during HIR/MIR lowering,
        // so we must NOT add another environment parameter or we'll have a double-env-param bug
        let already_has_env_param = !function.signature.parameters.is_empty()
            && function.signature.parameters[0].name == "env";

        if !is_extern && !already_has_env_param {
            sig.params.push(AbiParam::new(types::I64));
        }

        // Add parameters
        for param in &function.signature.parameters {
            // For C calling convention extern functions on non-Windows platforms,
            // the ABI requires integer types smaller than 64 bits to be extended to i64.
            // On ARM64/AArch64 (Apple Silicon), i32 parameters are passed as i64.
            let will_extend = is_extern
                && function.signature.calling_convention == crate::ir::CallingConvention::C
                && !cfg!(target_os = "windows")
                && matches!(param.ty, crate::ir::IrType::I32 | crate::ir::IrType::U32);

            if will_extend {
                eprintln!("!!! EXTENDING {} param '{}' from {:?} to i64 (is_extern={}, calling_conv={:?})",
                         function.name, param.name, param.ty, is_extern, function.signature.calling_convention);
            }

            let cranelift_type = if will_extend {
                types::I64
            } else {
                self.mir_type_to_cranelift(&param.ty)?
            };

            sig.params.push(AbiParam::new(cranelift_type));
        }

        // Debug: log Thread_spawn and Thread_join and channel extern signatures
        if function.name == "Thread_spawn"
            || function.name == "Thread_join"
            || function.name.starts_with("<lambda_")
            || function.name.starts_with("rayzor_channel")
            || function.name == "Channel_init"
        {
            eprintln!(
                "DEBUG: Declaring '{}' (MIR {:?}) with {} params, is_extern={}, calling_conv={:?}",
                function.name,
                mir_func_id,
                function.signature.parameters.len(),
                is_extern,
                function.signature.calling_convention
            );
            for (i, param) in function.signature.parameters.iter().enumerate() {
                let cranelift_ty = self
                    .mir_type_to_cranelift(&param.ty)
                    .unwrap_or(types::INVALID);
                let actual_ty = if is_extern
                    && function.signature.calling_convention == crate::ir::CallingConvention::C
                    && !cfg!(target_os = "windows")
                {
                    match &param.ty {
                        crate::ir::IrType::I32 | crate::ir::IrType::U32 => types::I64,
                        _ => cranelift_ty,
                    }
                } else {
                    cranelift_ty
                };
                eprintln!(
                    "  param[{}]: {} (MIR {:?} -> Cranelift {:?} -> actual {:?})",
                    i, param.name, param.ty, cranelift_ty, actual_ty
                );
            }
            eprintln!(
                "  return_type: {:?}, uses_sret: {}",
                function.signature.return_type, use_sret_in_signature
            );
        }

        // Debug: log lambda function signatures
        if function.name.starts_with("<lambda_") {
            eprintln!(
                "DEBUG Lambda signature for {}: {} params",
                function.name,
                function.signature.parameters.len()
            );
            for (i, param) in function.signature.parameters.iter().enumerate() {
                eprintln!("  param{}: {} ({:?})", i, param.name, param.ty);
            }
        }

        // Add return type (unless using sret)
        if !use_sret_in_signature {
            let return_type = self.mir_type_to_cranelift(&function.signature.return_type)?;
            if return_type != types::INVALID {
                sig.returns.push(AbiParam::new(return_type));
            }
        }

        // Determine linkage and name based on whether this is an extern function
        let is_extern = function.cfg.blocks.is_empty();
        let (func_name, linkage) = if is_extern {
            // Extern functions use their actual name and Import linkage
            (function.name.clone(), Linkage::Import)
        } else {
            // Check if this is a stdlib MIR wrapper function by looking it up in the runtime mapping
            // Stdlib wrappers are functions registered in the runtime mapping system
            let stdlib_mapping = crate::stdlib::runtime_mapping::StdlibMapping::new();
            let is_stdlib_mir_wrapper = stdlib_mapping
                .find_by_runtime_name(&function.name)
                .is_some();

            if is_stdlib_mir_wrapper {
                // Stdlib MIR wrappers use their actual names with Export linkage
                // so that forward references can resolve to them
                (function.name.clone(), Linkage::Export)
            } else {
                // Regular functions get unique names and Export linkage
                // Include module_counter to avoid collisions when compiling multiple MIR modules
                if let Some(ref qualified_name) = function.qualified_name {
                    // Use qualified name for better debugging/profiling
                    (
                        format!(
                            "m{}__{}__func_{}",
                            self.module_counter,
                            qualified_name.replace(".", "_"),
                            mir_func_id.0
                        ),
                        Linkage::Export,
                    )
                } else {
                    (
                        format!("m{}_func_{}", self.module_counter, mir_func_id.0),
                        Linkage::Export,
                    )
                }
            }
        };

        let func_id = self
            .module
            .declare_function(&func_name, linkage, &sig)
            .map_err(|e| format!("Failed to declare function: {}", e))?;

        eprintln!(
            "DEBUG Cranelift: Declared '{}' - MIR={:?} -> Cranelift={:?}, {} params",
            func_name,
            mir_func_id,
            func_id,
            function.signature.parameters.len()
        );
        self.function_map.insert(mir_func_id, func_id);

        // Track extern functions and stdlib wrapper functions in runtime_functions so we don't declare them twice
        if is_extern {
            eprintln!(
                "DEBUG: Declared new extern '{}' - MIR {:?} -> Cranelift {:?}",
                func_name, mir_func_id, func_id
            );
            self.runtime_functions.insert(func_name, func_id);
        } else {
            // Check if this is a stdlib wrapper function - track it to prevent duplicates
            let stdlib_mapping = crate::stdlib::runtime_mapping::StdlibMapping::new();
            let is_stdlib_mir_wrapper = stdlib_mapping
                .find_by_runtime_name(&function.name)
                .is_some();
            if is_stdlib_mir_wrapper {
                eprintln!(
                    "DEBUG: Declared new stdlib wrapper '{}' - MIR {:?} -> Cranelift {:?}",
                    func_name, mir_func_id, func_id
                );
                self.runtime_functions.insert(func_name, func_id);
            }
        }

        Ok(())
    }

    /// Declare a libc function (malloc, realloc, free)
    /// These are provided by the system C library
    fn declare_libc_function(
        &mut self,
        name: &str,
        param_count: usize,
        has_return: bool,
    ) -> Result<FuncId, String> {
        // Check if already declared
        if let Some(&func_id) = self.runtime_functions.get(name) {
            return Ok(func_id);
        }

        // Create signature for standard libc memory functions
        let mut sig = self.module.make_signature();

        match name {
            "malloc" => {
                // fn malloc(size: size_t) -> *void
                // size_t is pointer-sized (i64 on 64-bit, i32 on 32-bit)
                sig.params.push(AbiParam::new(self.pointer_type)); // size
                sig.returns.push(AbiParam::new(self.pointer_type)); // *void
            }
            "realloc" => {
                // fn realloc(ptr: *void, size: size_t) -> *void
                sig.params.push(AbiParam::new(self.pointer_type)); // ptr
                sig.params.push(AbiParam::new(self.pointer_type)); // size
                sig.returns.push(AbiParam::new(self.pointer_type)); // *void
            }
            "free" => {
                // fn free(ptr: *void)
                sig.params.push(AbiParam::new(self.pointer_type)); // ptr
                                                                   // no return value
            }
            _ => return Err(format!("Unknown libc function: {}", name)),
        }

        // Declare the function with Import linkage (external symbol from libc)
        let func_id = self
            .module
            .declare_function(name, Linkage::Import, &sig)
            .map_err(|e| format!("Failed to declare libc function {}: {}", name, e))?;

        eprintln!(
            "DEBUG: Declared libc {} as Cranelift func_id: {:?}",
            name, func_id
        );
        self.runtime_functions.insert(name.to_string(), func_id);
        Ok(func_id)
    }

    /// Compile a function body (second pass)
    fn compile_function(
        &mut self,
        mir_func_id: IrFunctionId,
        mir_module: &IrModule,
        function: &IrFunction,
    ) -> Result<(), String> {
        // Get the Cranelift function ID
        let func_id = *self
            .function_map
            .get(&mir_func_id)
            .ok_or("Function not declared")?;

        // CRITICAL: Check if this function's body was already defined
        // This can happen when functions are shared across modules (e.g., stdlib wrappers)
        // and the same function appears in multiple MIR modules
        // We track by Cranelift FuncId (not MIR name) because different modules can have
        // functions with the same MIR name (e.g., 'new') but different Cranelift symbols
        if self.defined_functions.contains(&func_id) {
            eprintln!(
                "DEBUG: Skipping already-defined function '{}' (MIR {:?}, Cranelift {:?})",
                function.name, mir_func_id, func_id
            );
            return Ok(());
        }

        // Clear context for new function
        self.ctx.func.clear();
        self.ctx.func.signature = self.module.make_signature();

        // Check if we need sret (struct return convention)
        let uses_sret = function.signature.uses_sret;

        // If using sret, add hidden first parameter for return value pointer
        if uses_sret {
            self.ctx.func.signature.params.push(AbiParam::special(
                self.pointer_type,
                ArgumentPurpose::StructReturn,
            ));
        }

        // Add environment parameter (but not for lambdas that already have one)
        // Lambda functions already have an explicit 'env' parameter in their MIR signature
        let already_has_env_param = !function.signature.parameters.is_empty()
            && function.signature.parameters[0].name == "env";

        if !already_has_env_param {
            self.ctx
                .func
                .signature
                .params
                .push(AbiParam::new(types::I64));
        }

        // Add parameters to signature
        eprintln!(
            "DEBUG Cranelift: Function '{}' has {} parameters",
            function.name,
            function.signature.parameters.len()
        );
        for (i, param) in function.signature.parameters.iter().enumerate() {
            eprintln!(
                "DEBUG Cranelift:   param[{}]: {} ({})",
                i, param.name, param.ty
            );
            let cranelift_type = self.mir_type_to_cranelift(&param.ty)?;
            self.ctx
                .func
                .signature
                .params
                .push(AbiParam::new(cranelift_type));
        }

        // Add return type to signature (void for sret functions)
        if uses_sret {
            // sret functions return void - the value is written through the pointer
        } else {
            let return_type = self.mir_type_to_cranelift(&function.signature.return_type)?;
            if return_type != types::INVALID {
                self.ctx
                    .func
                    .signature
                    .returns
                    .push(AbiParam::new(return_type));
            }
        }

        // Build the function body using FunctionBuilder
        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut builder_ctx);

        // Clear value map for new function
        self.value_map.clear();

        // Create entry block
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);

        // Map function parameters to their Cranelift values
        let param_values = builder.block_params(entry_block).to_vec();

        // Determine if this function already has an explicit 'env' parameter (e.g., lambdas)
        let already_has_env_param = !function.signature.parameters.is_empty()
            && function.signature.parameters[0].name == "env";

        // If using sret, first parameter is the return pointer
        // Environment parameter is next (if added implicitly, not for lambdas with explicit env)
        let sret_offset = if uses_sret { 1 } else { 0 };

        if already_has_env_param {
            // Lambda with explicit env parameter: parameters map directly
            // No hidden environment parameter was added in declare_function
            // param_values[sret_offset] is the first user parameter (which is 'env')
            for (i, param) in function.signature.parameters.iter().enumerate() {
                self.value_map
                    .insert(param.reg, param_values[i + sret_offset]);
            }
            // For lambdas, the env parameter is the first user parameter
            // current_env_param should point to it
            self.current_env_param = Some(param_values[sret_offset]);
        } else {
            // Regular function with implicit hidden environment parameter
            let env_offset = sret_offset; // env param is at this index
            let param_offset = env_offset + 1; // user params start after env

            // Store environment parameter for ClosureEnv
            self.current_env_param = Some(param_values[env_offset]);

            for (i, param) in function.signature.parameters.iter().enumerate() {
                self.value_map
                    .insert(param.reg, param_values[i + param_offset]);
            }
        }

        // Store sret pointer for use in Return terminator
        let sret_ptr = if uses_sret {
            Some(param_values[0])
        } else {
            None
        };

        // Note: Don't seal entry block yet, we need to add instructions first

        // First pass: Create all Cranelift blocks for MIR blocks
        let mut block_map = std::collections::HashMap::new();
        // eprintln!("DEBUG Cranelift: Function {} has {} blocks in CFG", function.name, function.cfg.blocks.len());
        for (mir_block_id, mir_block) in &function.cfg.blocks {
            // eprintln!("DEBUG Cranelift:   Block {:?} has {} phi nodes, {} instructions",
            //  mir_block_id, mir_block.phi_nodes.len(), mir_block.instructions.len());
            // Skip entry block as we already created it
            if mir_block_id.is_entry() {
                block_map.insert(*mir_block_id, entry_block);
            } else {
                let cl_block = builder.create_block();
                block_map.insert(*mir_block_id, cl_block);
            }
        }

        // Second pass: Translate instructions for each block
        // Process entry block first, then others in ID order for determinism
        let mut blocks_to_process: Vec<_> = function.cfg.blocks.iter().collect();
        blocks_to_process.sort_by_key(|(id, _)| (if id.is_entry() { 0 } else { 1 }, id.0));

        // Track which blocks have been translated
        let mut translated_blocks = std::collections::HashSet::new();

        for (mir_block_id, mir_block) in blocks_to_process {
            let cl_block = *block_map
                .get(mir_block_id)
                .ok_or_else(|| format!("Block {:?} not found in block_map", mir_block_id))?;

            // Switch to this block (entry block is already active, but switch anyway for clarity)
            builder.switch_to_block(cl_block);

            // Translate phi nodes first
            // eprintln!("DEBUG Cranelift: Block {:?} has {} phi nodes", mir_block_id, mir_block.phi_nodes.len());
            for phi_node in &mir_block.phi_nodes {
                // eprintln!("  Phi node: dest={:?}, ty={:?}", phi_node.dest, phi_node.ty);
                // eprintln!("  Incoming edges ({}):", phi_node.incoming.len());
                // for (from_block, value_id) in &phi_node.incoming {
                //     eprintln!("    from block {:?}: value {:?}", from_block, value_id);
                // }
                Self::translate_phi_node_static(
                    &mut self.value_map,
                    &mut builder,
                    phi_node,
                    &block_map,
                    &function.cfg,
                )?;
                // eprintln!("    After translation, value_map has {:?}", self.value_map.keys().collect::<Vec<_>>());
            }

            // Translate instructions
            for instruction in &mir_block.instructions {
                Self::translate_instruction(
                    &mut self.value_map,
                    &mut builder,
                    instruction,
                    function,
                    &self.function_map,
                    &mut self.runtime_functions,
                    mir_module,
                    &mut self.module,
                    &mut self.closure_environments,
                    self.current_env_param,
                    &mut self.string_data,
                    &mut self.string_counter,
                )?;
            }

            // Translate terminator
            // eprintln!("DEBUG Cranelift: MIR terminator for block {:?}: {:?}", mir_block_id, mir_block.terminator);
            if let Err(e) = Self::translate_terminator_static(
                &mut self.value_map,
                &mut builder,
                &mir_block.terminator,
                &block_map,
                function,
                sret_ptr,
            ) {
                eprintln!(
                    "\n!!! Error translating terminator in block {:?}: {}",
                    mir_block_id, e
                );
                eprintln!("=== Cranelift IR so far ===");
                eprintln!("{}", self.ctx.func.display());
                eprintln!("=== End IR ===\n");
                return Err(e);
            }

            translated_blocks.insert(*mir_block_id);
        }

        // Third pass: Seal all blocks after all have been translated
        // This is crucial for loops - a block can only be sealed after all predecessors
        // have been processed (including back edges)
        for mir_block_id in translated_blocks {
            let cl_block = *block_map.get(&mir_block_id).unwrap();
            builder.seal_block(cl_block);
        }

        // Finalize the function
        builder.finalize();

        // Print Cranelift IR if debug mode
        if cfg!(debug_assertions) {
            eprintln!("\n=== Cranelift IR for {} ===", function.name);
            eprintln!("{}", self.ctx.func.display());
            eprintln!("=== End Cranelift IR ===\n");
        }

        // Verify the function before defining
        if let Err(errors) = cranelift_codegen::verify_function(&self.ctx.func, self.module.isa()) {
            eprintln!("!!! Cranelift Verifier Errors for {} !!!", function.name);
            eprintln!("{}", errors);
            return Err(format!("Verifier errors in {}: {}", function.name, errors));
        }

        // Define the function in the module
        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| format!("Failed to define function: {}", e))?;

        // Track that this function has been defined to prevent duplicate definitions
        self.defined_functions.insert(func_id);
        eprintln!(
            "DEBUG: Successfully defined function '{}' (MIR {:?}, Cranelift {:?})",
            function.name, mir_func_id, func_id
        );

        // Clear the context for next function
        self.module.clear_context(&mut self.ctx);

        Ok(())
    }

    /// Collect phi node arguments when branching to a block
    /// This function also coerces value types if they don't match the expected phi parameter type
    fn collect_phi_args_with_coercion(
        value_map: &HashMap<IrId, Value>,
        function: &IrFunction,
        target_block: IrBlockId,
        from_block: IrBlockId,
        builder: &mut FunctionBuilder,
    ) -> Result<Vec<BlockArg>, String> {
        let target = function
            .cfg
            .blocks
            .get(&target_block)
            .ok_or_else(|| format!("Target block {:?} not found", target_block))?;

        let mut phi_args = Vec::new();

        // For each phi node in the target block, find the incoming value from our block
        for phi_node in &target.phi_nodes {
            // Find the incoming value for this phi from our current block
            let incoming_value = phi_node
                .incoming
                .iter()
                .find(|(block_id, _)| *block_id == from_block)
                .map(|(_, value_id)| value_id)
                .ok_or_else(|| {
                    format!(
                        "No incoming value for phi node {:?} from block {:?}",
                        phi_node.dest, from_block
                    )
                })?;

            // Look up the Cranelift value for this MIR value
            let cl_value = *value_map.get(incoming_value).ok_or_else(|| {
                format!(
                    "Value {:?} not found in value_map for phi incoming",
                    incoming_value
                )
            })?;

            // Get the expected Cranelift type from the phi node's MIR type
            let expected_cl_type = match &phi_node.ty {
                crate::ir::IrType::I8 => types::I8,
                crate::ir::IrType::I16 => types::I16,
                crate::ir::IrType::I32 => types::I32,
                crate::ir::IrType::I64 => types::I64,
                crate::ir::IrType::F32 => types::F32,
                crate::ir::IrType::F64 => types::F64,
                crate::ir::IrType::Bool => types::I8,
                crate::ir::IrType::Ptr(_) => types::I64,
                crate::ir::IrType::Ref(_) => types::I64,
                _ => types::I64,
            };

            // Check if the actual value type matches expected; coerce if not
            let actual_type = builder.func.dfg.value_type(cl_value);
            let final_value = if actual_type != expected_cl_type {
                eprintln!(
                    "DEBUG: Phi arg type mismatch for {:?}: actual {:?}, expected {:?}, coercing",
                    phi_node.dest, actual_type, expected_cl_type
                );
                // Coerce the value
                match (actual_type, expected_cl_type) {
                    // i64 -> i32 truncation
                    (types::I64, types::I32) => builder.ins().ireduce(types::I32, cl_value),
                    // i32 -> i64 extension
                    (types::I32, types::I64) => builder.ins().sextend(types::I64, cl_value),
                    // i8 -> i32/i64
                    (types::I8, types::I32) => builder.ins().sextend(types::I32, cl_value),
                    (types::I8, types::I64) => builder.ins().sextend(types::I64, cl_value),
                    // Same type - no conversion needed
                    (from, to) if from == to => cl_value,
                    // Fallback: log warning and use as-is (may cause verifier error)
                    _ => {
                        eprintln!(
                            "WARNING: Cannot coerce phi arg from {:?} to {:?}",
                            actual_type, expected_cl_type
                        );
                        cl_value
                    }
                }
            } else {
                cl_value
            };

            // Wrap in BlockArg for the fork's phi node API
            phi_args.push(BlockArg::Value(final_value));
        }

        Ok(phi_args)
    }

    /// Translate a phi node to Cranelift IR
    fn translate_phi_node_static(
        value_map: &mut HashMap<IrId, Value>,
        builder: &mut FunctionBuilder,
        phi_node: &crate::ir::IrPhiNode,
        block_map: &HashMap<IrBlockId, Block>,
        _cfg: &crate::ir::IrControlFlowGraph,
    ) -> Result<(), String> {
        // Create block parameters for the phi node
        // In Cranelift, phi nodes are represented as block parameters

        // Get the Cranelift type for the phi node
        // For static methods, we need to use a simple type mapping
        let cl_type = match &phi_node.ty {
            crate::ir::IrType::I8 => cranelift_codegen::ir::types::I8,
            crate::ir::IrType::I16 => cranelift_codegen::ir::types::I16,
            crate::ir::IrType::I32 => cranelift_codegen::ir::types::I32,
            crate::ir::IrType::I64 => cranelift_codegen::ir::types::I64,
            crate::ir::IrType::F32 => cranelift_codegen::ir::types::F32,
            crate::ir::IrType::F64 => cranelift_codegen::ir::types::F64,
            crate::ir::IrType::Bool => cranelift_codegen::ir::types::I8,
            crate::ir::IrType::Ptr(_) => cranelift_codegen::ir::types::I64, // Assume 64-bit pointers
            crate::ir::IrType::Ref(_) => cranelift_codegen::ir::types::I64, // Assume 64-bit refs
            _ => cranelift_codegen::ir::types::I64,                         // Default
        };

        // Get the current block
        let current_block = builder
            .current_block()
            .ok_or_else(|| "No current block for phi node".to_string())?;

        // Append a block parameter
        let block_param = builder.append_block_param(current_block, cl_type);

        // Map the phi node's destination register to the block parameter
        value_map.insert(phi_node.dest, block_param);

        Ok(())
    }

    /// Translate a single MIR instruction to Cranelift IR (static method)
    fn translate_instruction(
        value_map: &mut HashMap<IrId, Value>,
        builder: &mut FunctionBuilder,
        instruction: &IrInstruction,
        function: &IrFunction,
        function_map: &HashMap<IrFunctionId, FuncId>,
        runtime_functions: &mut HashMap<String, FuncId>,
        mir_module: &IrModule,
        module: &mut JITModule,
        closure_environments: &mut HashMap<IrId, Value>,
        current_env_param: Option<Value>,
        string_data: &mut HashMap<String, DataId>,
        string_counter: &mut usize,
    ) -> Result<(), String> {
        use crate::ir::IrInstruction;

        match instruction {
            IrInstruction::Const { dest, value } => {
                let cl_value = Self::translate_const_value(
                    builder,
                    value,
                    function_map,
                    runtime_functions,
                    module,
                    string_data,
                    string_counter,
                )?;
                value_map.insert(*dest, cl_value);
            }

            IrInstruction::Copy { dest, src } => {
                // Copy: For Copy types (Int, Bool, etc.) - just copy the value
                let src_value = *value_map
                    .get(src)
                    .ok_or_else(|| format!("Source value {:?} not found", src))?;
                value_map.insert(*dest, src_value);
                // Note: src remains valid after copy
            }

            IrInstruction::Move { dest, src } => {
                // Move: Transfer ownership - move the value and invalidate source
                let src_value = *value_map
                    .get(src)
                    .ok_or_else(|| format!("Source value {:?} not found for move", src))?;
                value_map.insert(*dest, src_value);
                // Invalidate source - any future use is a compile error (caught by MIR validation)
                // In codegen, we just don't remove it from value_map to keep the value alive
                // The MIR validator ensures src isn't used after the move
            }

            IrInstruction::BorrowImmutable {
                dest,
                src,
                lifetime: _,
            } => {
                // Borrow immutable: Create a pointer to the value
                // In Cranelift, this is just the address of the value
                let src_value = *value_map
                    .get(src)
                    .ok_or_else(|| format!("Source value {:?} not found for borrow", src))?;

                // For heap-allocated objects, the value is already a pointer - just use it
                // For stack values, we'd need to take their address
                // TODO: Distinguish between stack and heap values
                value_map.insert(*dest, src_value);

                // Note: src remains valid - borrows don't invalidate
                // Multiple immutable borrows allowed (enforced by MIR validation)
            }

            IrInstruction::BorrowMutable {
                dest,
                src,
                lifetime: _,
            } => {
                // Borrow mutable: Create an exclusive pointer to the value
                let src_value = *value_map
                    .get(src)
                    .ok_or_else(|| format!("Source value {:?} not found for mut borrow", src))?;

                // Like immutable borrow, but exclusive
                value_map.insert(*dest, src_value);

                // Note: MIR validation ensures:
                // 1. Only ONE mutable borrow exists at a time
                // 2. No immutable borrows exist while mutable borrow is active
                // 3. src is not accessed while borrowed mutably
            }

            IrInstruction::Clone { dest, src } => {
                // Clone: Call the clone function for this type
                let src_value = *value_map
                    .get(src)
                    .ok_or_else(|| format!("Source value {:?} not found for clone", src))?;

                // Look up the clone function for this type
                // For now, we'll use memcpy for simple objects
                // TODO: Call actual clone() method if type has one

                // Get the size of the object to clone
                // For heap objects, we need to allocate new memory and deep copy
                let ptr_type = module.target_config().pointer_type();

                // Allocate new memory (same size as source)
                // This is simplified - real implementation needs type info
                let size_val = builder.ins().iconst(ptr_type, 64); // Placeholder size

                // Call rayzor_malloc - need to convert FuncId to FuncRef
                let malloc_func_id = *runtime_functions
                    .get("malloc") // Use libc malloc directly
                    .ok_or_else(|| "malloc not found".to_string())?;
                let malloc_func_ref = module.declare_func_in_func(malloc_func_id, builder.func);

                // Malloc is a libc function, it does NOT take an environment parameter.
                // The CallDirect logic below handles this distinction.
                let inst = builder.ins().call(malloc_func_ref, &[size_val]);
                let new_ptr = builder.inst_results(inst)[0];

                // Copy data from source to destination
                // TODO: Use actual memcpy or call clone() method
                builder.emit_small_memory_copy(
                    module.target_config(),
                    new_ptr,
                    src_value,
                    64, // size
                    8,  // alignment
                    8,  // alignment
                    true,
                    cranelift_codegen::ir::MemFlags::new(),
                );

                value_map.insert(*dest, new_ptr);
                // Both src and dest now own separate objects
            }

            IrInstruction::EndBorrow { borrow } => {
                // EndBorrow: Explicitly end a borrow's lifetime
                // In Cranelift, this is mostly a no-op since borrows are just pointers
                // The main purpose is to mark the end of the borrow scope for validation

                // We could optionally remove from value_map to catch use-after-end-borrow
                // but that's already enforced by MIR validation

                // In a more sophisticated implementation, we might:
                // 1. Insert debug assertions to check borrow validity
                // 2. Update borrow tracking metadata
                // 3. Enable optimizations (value can be moved after borrow ends)

                // For now, it's just a marker for the validator
            }

            IrInstruction::BinOp {
                dest,
                op,
                left,
                right,
            } => {
                // Get type from register_types map first, then fall back to locals
                let ty = function
                    .register_types
                    .get(dest)
                    .or_else(|| function.register_types.get(left))
                    .or_else(|| function.locals.get(dest).map(|local| &local.ty))
                    .ok_or_else(|| format!("Type not found for BinOp dest {:?}", dest))?;

                let value =
                    Self::lower_binary_op_static(value_map, builder, op, ty, *left, *right)?;
                value_map.insert(*dest, value);
            }

            IrInstruction::UnOp { dest, op, operand } => {
                // Get type from register_types map first, then fall back to locals
                let ty = function
                    .register_types
                    .get(dest)
                    .or_else(|| function.register_types.get(operand))
                    .or_else(|| function.locals.get(dest).map(|local| &local.ty))
                    .ok_or_else(|| format!("Type not found for UnOp dest {:?}", dest))?;

                let value = Self::lower_unary_op_static(value_map, builder, op, ty, *operand)?;
                value_map.insert(*dest, value);
            }

            IrInstruction::Cmp {
                dest,
                op,
                left,
                right,
            } => {
                // Get type from register_types map
                let ty = function
                    .register_types
                    .get(left)
                    .or_else(|| function.locals.get(left).map(|local| &local.ty))
                    .ok_or_else(|| format!("Type not found for Cmp operand {:?}", left))?;

                let value =
                    Self::lower_compare_op_static(value_map, builder, op, ty, *left, *right)?;
                value_map.insert(*dest, value);
            }

            IrInstruction::Load { dest, ptr, ty } => {
                let value = Self::lower_load_static(value_map, builder, ty, *ptr)?;
                value_map.insert(*dest, value);
            }

            IrInstruction::Store { ptr, value } => {
                Self::lower_store_static(value_map, builder, *ptr, *value)?;
            }

            IrInstruction::Alloc { dest, ty, count } => {
                // For now, allocate with fixed size (need to handle dynamic count)
                let count_val = match count {
                    Some(_id) => {
                        // TODO: Get runtime value
                        Some(1)
                    }
                    None => None,
                };
                let value = Self::lower_alloca_static(builder, ty, count_val)?;
                value_map.insert(*dest, value);
            }

            IrInstruction::CallDirect {
                dest,
                func_id,
                args,
                arg_ownership: _,
            } => {
                // TODO: Use arg_ownership to generate proper move/borrow/clone code
                // Check if this is an extern function call
                if let Some(extern_func) = mir_module.extern_functions.get(func_id) {
                    // This is an external runtime function call
                    eprintln!("INFO: Calling external function: {}", extern_func.name);

                    // Declare the external function if not already declared
                    let cl_func_id = if let Some(&id) = runtime_functions.get(&extern_func.name) {
                        id
                    } else {
                        // Declare the external runtime function dynamically
                        let mut sig = module.make_signature();

                        // Add parameters using actual types from the extern function signature
                        // Apply C ABI integer promotion for non-Windows platforms
                        for param in &extern_func.signature.parameters {
                            let mut cranelift_type = Self::mir_type_to_cranelift_static(&param.ty)?;

                            // For C calling convention externs on non-Windows platforms, extend i32/u32 to i64
                            if !cfg!(target_os = "windows")
                                && extern_func.signature.calling_convention
                                    == crate::ir::CallingConvention::C
                            {
                                match param.ty {
                                    crate::ir::IrType::I32 | crate::ir::IrType::U32 => {
                                        eprintln!("!!! [DYNAMIC DECL] Extending {} param '{}' from {:?} to i64", extern_func.name, param.name, param.ty);
                                        cranelift_type = types::I64;
                                    }
                                    _ => {}
                                }
                            }

                            sig.params.push(AbiParam::new(cranelift_type));
                        }

                        // Add return type using actual type from the extern function signature
                        if extern_func.signature.return_type != crate::ir::IrType::Void {
                            let return_type = Self::mir_type_to_cranelift_static(
                                &extern_func.signature.return_type,
                            )?;
                            if return_type != types::INVALID {
                                sig.returns.push(AbiParam::new(return_type));
                            }
                        }

                        let id = module
                            .declare_function(&extern_func.name, Linkage::Import, &sig)
                            .map_err(|e| {
                                format!(
                                    "Failed to declare runtime function {}: {}",
                                    extern_func.name, e
                                )
                            })?;

                        eprintln!(
                            "INFO: Declared external runtime function {} as func_id: {:?}",
                            extern_func.name, id
                        );
                        runtime_functions.insert(extern_func.name.clone(), id);
                        id
                    };

                    let func_ref = module.declare_func_in_func(cl_func_id, builder.func);

                    // Lower arguments
                    // For C extern functions on non-Windows platforms, extend i32/u32 to i64
                    let mut arg_values = Vec::new();
                    for (i, &arg_reg) in args.iter().enumerate() {
                        let mut cl_value = *value_map.get(&arg_reg).ok_or_else(|| {
                            format!("Argument register {:?} not found in value_map", arg_reg)
                        })?;

                        // Check if this C extern function parameter needs extension
                        if !cfg!(target_os = "windows")
                            && extern_func.signature.calling_convention
                                == crate::ir::CallingConvention::C
                        {
                            if let Some(param) = extern_func.signature.parameters.get(i) {
                                match param.ty {
                                    crate::ir::IrType::I32 => {
                                        eprintln!("!!! [EXTERN BRANCH] Extending arg {} for {} from i32 to i64", i, extern_func.name);
                                        // Sign-extend i32 to i64
                                        cl_value = builder.ins().sextend(types::I64, cl_value);
                                    }
                                    crate::ir::IrType::U32 => {
                                        eprintln!("!!! [EXTERN BRANCH] Extending arg {} for {} from u32 to i64", i, extern_func.name);
                                        // Zero-extend u32 to i64
                                        cl_value = builder.ins().uextend(types::I64, cl_value);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        arg_values.push(cl_value);
                    }

                    // Make the call
                    let call_inst = builder.ins().call(func_ref, &arg_values);

                    // Get return value if any
                    if let Some(dest_reg) = dest {
                        let results = builder.inst_results(call_inst);
                        if !results.is_empty() {
                            value_map.insert(*dest_reg, results[0]);
                        }
                    }
                } else {
                    // Check if this is a call to malloc/realloc/free
                    let called_func = mir_module.functions.get(func_id).ok_or_else(|| {
                        format!("Called function {:?} not found in module", func_id)
                    })?;

                    let (cl_func_id, func_ref) = if called_func.name == "malloc"
                        || called_func.name == "realloc"
                        || called_func.name == "free"
                    {
                        // This is a memory management function - call the libc version
                        let libc_id =
                            *runtime_functions.get(&called_func.name).ok_or_else(|| {
                                format!("libc function {} not declared", called_func.name)
                            })?;
                        eprintln!("DEBUG: [In {}] Redirecting {} call (MIR func_id={:?}) to libc func_id: {:?}", function.name, called_func.name, func_id, libc_id);
                        let func_ref = module.declare_func_in_func(libc_id, builder.func);
                        eprintln!(
                            "DEBUG: [In {}] Got func_ref for {} (libc_id={:?}): {:?}",
                            function.name, called_func.name, libc_id, func_ref
                        );
                        (libc_id, func_ref)
                    } else {
                        // Normal MIR function call
                        let cl_func_id = *function_map.get(func_id).ok_or_else(|| {
                            format!("Function {:?} not found in function_map", func_id)
                        })?;
                        eprintln!("DEBUG: [In {}] Regular function call to {:?} (MIR {:?} -> Cranelift {:?})", function.name, called_func.name, func_id, cl_func_id);
                        let func_ref = module.declare_func_in_func(cl_func_id, builder.func);
                        (cl_func_id, func_ref)
                    };

                    // Check if function uses sret (and is not extern)
                    let is_extern_func = called_func.cfg.blocks.is_empty();
                    let uses_sret = called_func.signature.uses_sret && !is_extern_func;

                    // Allocate stack space for sret if needed
                    let sret_slot = if uses_sret {
                        let ret_ty = &called_func.signature.return_type;
                        Some(Self::lower_alloca_static(builder, ret_ty, None)?)
                    } else {
                        None
                    };

                    // Translate arguments (prepend sret pointer if needed)
                    let mut call_args = Vec::new();
                    if let Some(sret_ptr) = sret_slot {
                        call_args.push(sret_ptr);
                    }

                    // Add environment argument (null for direct calls to non-extern functions)
                    // Extern functions (including libc functions) do NOT take an environment parameter.
                    // Lambdas already have an explicit 'env' parameter in their signature, so we shouldn't add a null one.
                    let is_lambda = called_func.name.starts_with("<lambda");
                    if !is_extern_func
                        && !is_lambda
                        && !(called_func.name == "malloc"
                            || called_func.name == "realloc"
                            || called_func.name == "free")
                    {
                        // For direct calls to regular functions, we pass a null environment pointer
                        call_args.push(builder.ins().iconst(types::I64, 0));
                    }

                    // For C extern functions on non-Windows platforms, extend i32/u32 arguments to i64
                    // A function is C extern if:
                    // 1. It has C calling convention AND
                    // 2. Either (a) it has no blocks (true extern) OR (b) it has External linkage (wrapper around extern)
                    let is_c_extern = called_func.signature.calling_convention
                        == crate::ir::CallingConvention::C
                        && (is_extern_func
                            || called_func.attributes.linkage == crate::ir::Linkage::External)
                        && !cfg!(target_os = "windows");

                    if called_func.name.starts_with("rayzor_channel")
                        || called_func.name == "Channel_init"
                    {
                        eprintln!("DEBUG: [In {}] Calling {} - is_c_extern={}, calling_conv={:?}, is_extern={}, linkage={:?}",
                             function.name, called_func.name, is_c_extern, called_func.signature.calling_convention,
                             is_extern_func, called_func.attributes.linkage);
                    }

                    for (i, arg_id) in args.iter().enumerate() {
                        let mut arg_val = *value_map.get(arg_id).ok_or_else(|| {
                            format!("Argument {:?} not found in value_map", arg_id)
                        })?;

                        // Get the expected Cranelift type for this parameter
                        let expected_cl_ty =
                            if let Some(param) = called_func.signature.parameters.get(i) {
                                Self::mir_type_to_cranelift_static(&param.ty)?
                            } else {
                                types::I64 // Default fallback
                            };

                        // Get the actual Cranelift type of the argument
                        let actual_cl_ty = builder.func.dfg.value_type(arg_val);

                        // Insert type conversion if needed (i32 -> i64 or i64 -> i32)
                        if actual_cl_ty != expected_cl_ty {
                            if actual_cl_ty == types::I32 && expected_cl_ty == types::I64 {
                                // Sign-extend i32 to i64
                                eprintln!("DEBUG: [CallDirect arg extension] {} param {} extending i32 to i64", called_func.name, i);
                                arg_val = builder.ins().sextend(types::I64, arg_val);
                            } else if actual_cl_ty == types::I64 && expected_cl_ty == types::I32 {
                                // Reduce i64 to i32
                                eprintln!("DEBUG: [CallDirect arg reduction] {} param {} reducing i64 to i32", called_func.name, i);
                                arg_val = builder.ins().ireduce(types::I32, arg_val);
                            }
                            // Other type mismatches are not handled here
                        }

                        // For C extern functions, also apply integer promotion rules
                        if is_c_extern {
                            if let Some(param) = called_func.signature.parameters.get(i) {
                                match &param.ty {
                                    crate::ir::IrType::I32 => {
                                        // C ABI: promote i32 to i64 on non-Windows
                                        if builder.func.dfg.value_type(arg_val) == types::I32 {
                                            eprintln!("!!! [C ABI] Extending arg {} for {} from i32 to i64", i, called_func.name);
                                            arg_val = builder.ins().sextend(types::I64, arg_val);
                                        }
                                    }
                                    crate::ir::IrType::U32 => {
                                        // C ABI: promote u32 to i64 on non-Windows
                                        if builder.func.dfg.value_type(arg_val) == types::I32 {
                                            eprintln!("!!! [C ABI] Extending arg {} for {} from u32 to i64", i, called_func.name);
                                            arg_val = builder.ins().uextend(types::I64, arg_val);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        call_args.push(arg_val);
                    }

                    // Emit the call instruction
                    let call_inst = builder.ins().call(func_ref, &call_args);

                    // Handle return value
                    // Special case: If the function returns Void but MIR has a dest register,
                    // just ignore the dest. This can happen with lambdas where the MIR was
                    // generated before the function signature was fully resolved.
                    if called_func.signature.return_type == crate::ir::IrType::Void {
                        // Void function - ignore dest register if present
                        // (MIR may have allocated one before signature was known)
                    } else if let Some(dest_reg) = dest {
                        if uses_sret {
                            // For sret, the "return value" is the pointer to the sret slot
                            value_map.insert(*dest_reg, sret_slot.unwrap());
                        } else {
                            // Normal return value
                            let results = builder.inst_results(call_inst);
                            if !results.is_empty() {
                                let result_val = results[0];

                                // Coerce return value to match expected MIR type
                                // IMPORTANT: Only coerce if both the function signature return type AND
                                // the MIR dest register type are primitive integers (not pointers/refs).
                                // Truncating pointer values would cause runtime crashes.
                                let actual_ret_ty = builder.func.dfg.value_type(result_val);

                                // Check if the function signature says this is a primitive integer return
                                let sig_return_is_primitive_int = matches!(
                                    &called_func.signature.return_type,
                                    crate::ir::IrType::I8
                                        | crate::ir::IrType::I16
                                        | crate::ir::IrType::I32
                                        | crate::ir::IrType::I64
                                        | crate::ir::IrType::U8
                                        | crate::ir::IrType::U16
                                        | crate::ir::IrType::U32
                                        | crate::ir::IrType::U64
                                        | crate::ir::IrType::Bool
                                );

                                // Check if MIR dest register type is also a primitive integer
                                let mir_dest_is_primitive_int = function
                                    .register_types
                                    .get(dest_reg)
                                    .map(|ty| {
                                        matches!(
                                            ty,
                                            crate::ir::IrType::I8
                                                | crate::ir::IrType::I16
                                                | crate::ir::IrType::I32
                                                | crate::ir::IrType::I64
                                                | crate::ir::IrType::U8
                                                | crate::ir::IrType::U16
                                                | crate::ir::IrType::U32
                                                | crate::ir::IrType::U64
                                                | crate::ir::IrType::Bool
                                        )
                                    })
                                    .unwrap_or(false);

                                let mir_expected_ty = function
                                    .register_types
                                    .get(dest_reg)
                                    .map(|ty| match ty {
                                        crate::ir::IrType::I8 => types::I8,
                                        crate::ir::IrType::I16 => types::I16,
                                        crate::ir::IrType::I32 => types::I32,
                                        crate::ir::IrType::I64 => types::I64,
                                        crate::ir::IrType::U8 => types::I8,
                                        crate::ir::IrType::U16 => types::I16,
                                        crate::ir::IrType::U32 => types::I32,
                                        crate::ir::IrType::U64 => types::I64,
                                        crate::ir::IrType::Bool => types::I8,
                                        _ => types::I64,
                                    })
                                    .unwrap_or(types::I64);

                                // Only coerce if BOTH signature and MIR say it's a primitive int
                                let final_val = if sig_return_is_primitive_int
                                    && mir_dest_is_primitive_int
                                    && actual_ret_ty != mir_expected_ty
                                    && actual_ret_ty.is_int()
                                    && mir_expected_ty.is_int()
                                {
                                    eprintln!("DEBUG Call return type coercion: actual={:?}, mir_expected={:?}, func={}",
                                    actual_ret_ty, mir_expected_ty, called_func.name);
                                    if actual_ret_ty.bits() > mir_expected_ty.bits() {
                                        // Truncate i64 -> i32
                                        builder.ins().ireduce(mir_expected_ty, result_val)
                                    } else {
                                        // Extend i32 -> i64
                                        builder.ins().sextend(mir_expected_ty, result_val)
                                    }
                                } else {
                                    result_val
                                };

                                value_map.insert(*dest_reg, final_val);
                            } else {
                                return Err(format!("Function call expected to return value but got none (func_id={:?}, dest={:?})", func_id, dest_reg));
                            }
                        }
                    }
                }
            }

            IrInstruction::CallIndirect {
                dest,
                func_ptr,
                args,
                signature,
                arg_ownership: _,
            } => {
                // Indirect function call (virtual call or closure call)
                // In our unified representation, func_ptr is ALWAYS a pointer to a Closure struct
                // { fn_ptr: i64, env_ptr: i64 }

                // Prepare arguments
                let mut call_args = Vec::new();
                for arg in args {
                    let arg_val = *value_map
                        .get(arg)
                        .ok_or_else(|| format!("Argument {:?} not found in value_map", arg))?;
                    call_args.push(arg_val);
                }

                // Get the closure object pointer
                let closure_ptr = *value_map.get(func_ptr).ok_or_else(|| {
                    format!("Function pointer {:?} not found in value_map", func_ptr)
                })?;

                // Load function pointer from offset 0
                let func_code_ptr = builder
                    .ins()
                    .load(types::I64, MemFlags::new(), closure_ptr, 0);

                // Load environment pointer from offset 8
                let env_ptr = builder
                    .ins()
                    .load(types::I64, MemFlags::new(), closure_ptr, 8);

                // Add environment pointer as first argument
                call_args.insert(0, env_ptr);

                // Determine function signature
                // We need to add the environment parameter to the signature
                let mut sig = module.make_signature();

                // Helper to add params to signature
                let add_params_to_sig = |sig: &mut Signature,
                                         param_types: &[IrType],
                                         ret_type: &IrType|
                 -> Result<(), String> {
                    // Check return type size to determine sret
                    let uses_sret = matches!(ret_type, IrType::Struct { .. });

                    if uses_sret {
                        sig.params
                            .push(AbiParam::special(types::I64, ArgumentPurpose::StructReturn));
                    }

                    // Add environment parameter
                    sig.params.push(AbiParam::new(types::I64));

                    // Add user parameters
                    for param_ty in param_types {
                        let cl_ty = Self::mir_type_to_cranelift_static(param_ty)?;
                        sig.params.push(AbiParam::new(cl_ty));
                    }
                    Ok(())
                };

                match signature {
                    IrType::Function {
                        params,
                        return_type,
                        varargs: _,
                    } => {
                        add_params_to_sig(&mut sig, params, return_type)?;

                        // Add return type
                        let cl_ret_ty = Self::mir_type_to_cranelift_static(return_type)?;
                        if cl_ret_ty != types::INVALID {
                            // Check for sret
                            if !matches!(return_type.as_ref(), IrType::Struct { .. }) {
                                sig.returns.push(AbiParam::new(cl_ret_ty));
                            }
                        }
                    }
                    _ => {
                        return Err(format!(
                            "Invalid signature type for CallIndirect: {:?}",
                            signature
                        ))
                    }
                }

                let sig_ref = builder.import_signature(sig);

                // Emit the indirect call instruction
                let call_inst = builder
                    .ins()
                    .call_indirect(sig_ref, func_code_ptr, &call_args);
                let results = builder.inst_results(call_inst);

                // Map return value
                if let Some(dest_id) = dest {
                    if !results.is_empty() {
                        value_map.insert(*dest_id, results[0]);
                    }
                }
            }

            IrInstruction::MakeClosure {
                dest,
                func_id,
                captured_values,
            } => {
                // Create a closure object as a struct { fn_ptr: *u8, env_ptr: *u8 }
                //
                // Strategy:
                // 1. Allocate environment struct for captured values (if any)
                // 2. Allocate closure object struct (16 bytes: fn_ptr + env_ptr)
                // 3. Store function pointer and environment pointer into closure object
                // 4. Return pointer to closure object

                // Get the Cranelift FuncId for the lambda
                let cl_func_id = function_map.get(func_id).ok_or_else(|| {
                    format!("Lambda function {:?} not found in function_map", func_id)
                })?;

                // Import function and get its address
                let func_ref = module.declare_func_in_func(*cl_func_id, builder.func);
                let func_addr = builder.ins().func_addr(types::I64, func_ref);

                // Allocate environment for captured values (if any)
                let env_ptr = if !captured_values.is_empty() {
                    // Calculate environment size: 8 bytes per captured value
                    let env_size = (captured_values.len() * 8) as i64;

                    // Heap-allocate environment using malloc
                    // This is necessary because the closure may outlive the current stack frame
                    // (e.g., when passed to Thread.spawn())
                    let malloc_func_id = *runtime_functions
                        .get("malloc")
                        .ok_or_else(|| "malloc not found in runtime_functions".to_string())?;
                    let malloc_func_ref = module.declare_func_in_func(malloc_func_id, builder.func);

                    let size_arg = builder.ins().iconst(types::I64, env_size);
                    let inst = builder.ins().call(malloc_func_ref, &[size_arg]);
                    let env_addr = builder.inst_results(inst)[0];

                    // Store each captured value into the environment
                    for (i, captured_id) in captured_values.iter().enumerate() {
                        let captured_val = *value_map.get(captured_id).ok_or_else(|| {
                            format!("Captured value {:?} not found in value_map", captured_id)
                        })?;

                        // Calculate offset for this field (i * 8 bytes)
                        let offset = (i * 8) as i32;

                        // All environment slots are i64 (8 bytes) for uniformity
                        // If the value is smaller, extend it to i64
                        let val_type = builder.func.dfg.value_type(captured_val);
                        let value_to_store = match val_type {
                            types::I32 => {
                                // Sign-extend i32 to i64
                                builder.ins().sextend(types::I64, captured_val)
                            }
                            types::I8 => {
                                // Sign-extend i8 to i64
                                builder.ins().sextend(types::I64, captured_val)
                            }
                            types::I64 => {
                                // Already i64, use as-is
                                captured_val
                            }
                            _ => {
                                // For other types (pointers, floats, etc.), assume they're already pointer-sized
                                captured_val
                            }
                        };

                        // Store the i64 value at env_ptr + offset
                        builder
                            .ins()
                            .store(MemFlags::new(), value_to_store, env_addr, offset);
                    }

                    eprintln!(
                        "Info: Allocated environment for {} captured variables",
                        captured_values.len()
                    );
                    env_addr
                } else {
                    // No captures - null environment pointer
                    builder.ins().iconst(types::I64, 0)
                };

                // Heap-allocate closure object struct: { fn_ptr: i64, env_ptr: i64 }
                // This is necessary because closures may outlive the current stack frame
                let malloc_func_id = *runtime_functions
                    .get("malloc")
                    .ok_or_else(|| "malloc not found in runtime_functions".to_string())?;
                let malloc_func_ref = module.declare_func_in_func(malloc_func_id, builder.func);

                let closure_size = builder.ins().iconst(types::I64, 16); // 2 pointers
                let inst = builder.ins().call(malloc_func_ref, &[closure_size]);
                let closure_obj_ptr = builder.inst_results(inst)[0];

                // Store function pointer at offset 0
                builder
                    .ins()
                    .store(MemFlags::new(), func_addr, closure_obj_ptr, 0);

                // Store environment pointer at offset 8
                builder
                    .ins()
                    .store(MemFlags::new(), env_ptr, closure_obj_ptr, 8);

                // Track the environment pointer for ClosureEnv instruction
                closure_environments.insert(*dest, env_ptr);

                // Return pointer to closure object struct
                value_map.insert(*dest, closure_obj_ptr);
            }

            IrInstruction::ClosureFunc { dest, closure } => {
                // Extract function pointer from closure
                // For now, closure is just the function pointer
                let closure_val = *value_map
                    .get(closure)
                    .ok_or_else(|| format!("Closure {:?} not found in value_map", closure))?;
                value_map.insert(*dest, closure_val);
            }

            IrInstruction::ClosureEnv { dest, closure: _ } => {
                // Extract environment pointer from the CURRENT function's environment parameter
                // The 'closure' argument to this instruction is usually the closure object itself,
                // but in our unified model, the environment is passed as a hidden parameter.
                // The MIR might still pass the closure object, but we just need the env param.

                if let Some(env_val) = current_env_param {
                    value_map.insert(*dest, env_val);
                } else {
                    // Should not happen if we set up current_env_param correctly
                    // But for safety, return null
                    let null_ptr = builder.ins().iconst(types::I64, 0);
                    value_map.insert(*dest, null_ptr);
                }
            }

            IrInstruction::Cast {
                dest,
                src,
                from_ty,
                to_ty,
            } => {
                // Type casting (e.g., int to float, float to int)
                let src_val = *value_map
                    .get(src)
                    .ok_or_else(|| format!("Cast source {:?} not found in value_map", src))?;

                let from_cl_ty = Self::mir_type_to_cranelift_static(from_ty)?;
                let to_cl_ty = Self::mir_type_to_cranelift_static(to_ty)?;

                let result = match (from_cl_ty, to_cl_ty) {
                    // Int to Float conversions
                    (types::I32, types::F64) => builder.ins().fcvt_from_sint(types::F64, src_val),
                    (types::I64, types::F64) => builder.ins().fcvt_from_sint(types::F64, src_val),
                    (types::I32, types::F32) => builder.ins().fcvt_from_sint(types::F32, src_val),
                    (types::I64, types::F32) => builder.ins().fcvt_from_sint(types::F32, src_val),

                    // Float to Int conversions
                    (types::F64, types::I32) => builder.ins().fcvt_to_sint(types::I32, src_val),
                    (types::F64, types::I64) => builder.ins().fcvt_to_sint(types::I64, src_val),
                    (types::F32, types::I32) => builder.ins().fcvt_to_sint(types::I32, src_val),
                    (types::F32, types::I64) => builder.ins().fcvt_to_sint(types::I64, src_val),

                    // Float to Float conversions
                    (types::F32, types::F64) => builder.ins().fpromote(types::F64, src_val),
                    (types::F64, types::F32) => builder.ins().fdemote(types::F32, src_val),

                    // Int to Int conversions (sign extension or truncation)
                    (types::I32, types::I64) => builder.ins().sextend(types::I64, src_val),
                    (types::I64, types::I32) => builder.ins().ireduce(types::I32, src_val),

                    // Same type - just copy
                    (from, to) if from == to => src_val,

                    _ => {
                        return Err(format!(
                            "Unsupported cast from {:?} to {:?}",
                            from_ty, to_ty
                        ))
                    }
                };

                value_map.insert(*dest, result);
            }

            IrInstruction::GetElementPtr {
                dest,
                ptr,
                indices,
                ty,
            } => {
                // Get Element Pointer - compute address of field within struct
                // This is similar to LLVM's GEP instruction

                // eprintln!("DEBUG Cranelift: GetElementPtr - ptr={:?}, indices={:?}, ty={:?}", ptr, indices, ty);

                let ptr_val = *value_map
                    .get(ptr)
                    .ok_or_else(|| format!("GEP ptr {:?} not found in value_map", ptr))?;

                // For now, we assume a single index (field index in struct)
                // More complex GEP operations (nested structs, arrays) need additional work
                if indices.len() != 1 {
                    return Err(format!(
                        "GEP with {} indices not yet supported (only single index supported)",
                        indices.len()
                    ));
                }

                let index_id = indices[0];
                let index_val = *value_map
                    .get(&index_id)
                    .ok_or_else(|| format!("GEP index {:?} not found in value_map", index_id))?;

                // Get the size of the element type
                let elem_size = Self::type_size(ty);
                let size_val = builder.ins().iconst(types::I64, elem_size as i64);

                // Convert index to i64 if needed (only if not already i64)
                let index_ty = builder.func.dfg.value_type(index_val);
                let index_i64 = if index_ty == types::I64 {
                    index_val
                } else if index_ty.bits() < 64 {
                    builder.ins().sextend(types::I64, index_val)
                } else {
                    // Shouldn't happen, but handle gracefully
                    return Err(format!("GEP index has unsupported type {:?}", index_ty));
                };

                // Compute offset: index * elem_size
                let offset = builder.ins().imul(index_i64, size_val);

                // Add offset to base pointer
                let result_ptr = builder.ins().iadd(ptr_val, offset);

                // eprintln!("DEBUG Cranelift: GEP result - dest={:?}", dest);
                value_map.insert(*dest, result_ptr);
            }

            IrInstruction::ExtractValue {
                dest,
                aggregate,
                indices,
            } => {
                // For struct field extraction, we need to calculate the offset and load
                // Get the aggregate value (should be a pointer to struct on stack)
                let aggregate_val = *value_map
                    .get(aggregate)
                    .ok_or_else(|| format!("Aggregate value {:?} not found", aggregate))?;

                // For now, handle simple single-index case (most common for structs)
                if indices.len() != 1 {
                    return Err(format!(
                        "ExtractValue with multiple indices not yet supported: {:?}",
                        indices
                    ));
                }

                let field_index = indices[0] as usize;

                // Get the struct type from the aggregate - check both parameters and locals
                // If not found, try to find the Load instruction that produced this value
                let aggregate_ty = function
                    .signature
                    .parameters
                    .iter()
                    .find(|p| p.reg == *aggregate)
                    .map(|p| &p.ty)
                    .or_else(|| function.locals.get(aggregate).map(|local| &local.ty))
                    .or_else(|| {
                        // Search for the Load instruction that produced this aggregate
                        for block in function.cfg.blocks.values() {
                            for inst in &block.instructions {
                                if let IrInstruction::Load { dest, ty, .. } = inst {
                                    if dest == aggregate {
                                        return Some(ty);
                                    }
                                }
                            }
                        }
                        None
                    })
                    .ok_or_else(|| format!("Type not found for aggregate {:?}", aggregate))?;

                // Calculate field offset based on struct layout
                let (field_offset, field_ty) = match aggregate_ty {
                    IrType::Struct { fields, .. } => {
                        if field_index >= fields.len() {
                            return Err(format!(
                                "Field index {} out of bounds for struct with {} fields",
                                field_index,
                                fields.len()
                            ));
                        }

                        // Calculate offset: sum of sizes of all previous fields
                        let offset: usize = fields
                            .iter()
                            .take(field_index)
                            .map(|f| CraneliftBackend::type_size(&f.ty))
                            .sum();

                        let field = &fields[field_index];
                        (offset, &field.ty)
                    }
                    _ => {
                        return Err(format!(
                            "ExtractValue on non-struct type: {:?}",
                            aggregate_ty
                        ));
                    }
                };

                // Add offset to base pointer
                let offset_val = builder.ins().iconst(types::I64, field_offset as i64);
                let field_ptr = builder.ins().iadd(aggregate_val, offset_val);

                // Load the field value
                let field_cl_ty = CraneliftBackend::mir_type_to_cranelift_static(field_ty)?;
                let field_value = builder
                    .ins()
                    .load(field_cl_ty, MemFlags::new(), field_ptr, 0);

                value_map.insert(*dest, field_value);
            }

            IrInstruction::FunctionRef { dest, func_id } => {
                // Get function reference as a pointer
                let cl_func_id = *function_map
                    .get(func_id)
                    .ok_or_else(|| format!("Function {:?} not found in function_map", func_id))?;

                // Import the function reference into the current function
                let func_ref = module.declare_func_in_func(cl_func_id, builder.func);

                // Convert function reference to an address (i64 pointer)
                let func_code_ptr = builder.ins().func_addr(types::I64, func_ref);

                // Create a Closure object { fn_ptr, env_ptr }
                // Even for static functions, we represent them as closures with null environment
                // This unifies the representation for CallIndirect

                // Allocate closure object (16 bytes)
                // We use malloc for consistency, though we could potentially optimize this
                let malloc_func_id = *runtime_functions
                    .get("malloc")
                    .ok_or_else(|| "malloc not found in runtime_functions".to_string())?;
                let malloc_func_ref = module.declare_func_in_func(malloc_func_id, builder.func);

                let closure_size = builder.ins().iconst(types::I64, 16); // 2 pointers
                let inst = builder.ins().call(malloc_func_ref, &[closure_size]);
                let closure_obj_ptr = builder.inst_results(inst)[0];

                // Store function pointer at offset 0
                builder
                    .ins()
                    .store(MemFlags::new(), func_code_ptr, closure_obj_ptr, 0);

                // Store null environment at offset 8
                let null_ptr = builder.ins().iconst(types::I64, 0);
                builder
                    .ins()
                    .store(MemFlags::new(), null_ptr, closure_obj_ptr, 8);

                value_map.insert(*dest, closure_obj_ptr);
            }

            IrInstruction::Undef { dest, ty } => {
                // Undefined value - use zero/null for simplicity
                let cl_ty = CraneliftBackend::mir_type_to_cranelift_static(ty)?;
                let undef_val = if cl_ty == types::INVALID {
                    // Void type - no value needed, but instruction expects one
                    // Use a dummy i64(0)
                    builder.ins().iconst(types::I64, 0)
                } else if cl_ty.is_int() {
                    builder.ins().iconst(cl_ty, 0)
                } else if cl_ty.is_float() {
                    if cl_ty == types::F32 {
                        builder.ins().f32const(0.0)
                    } else {
                        builder.ins().f64const(0.0)
                    }
                } else {
                    // Pointer or other type - use null (0)
                    builder.ins().iconst(types::I64, 0)
                };

                value_map.insert(*dest, undef_val);
            }

            IrInstruction::CreateStruct { dest, ty, fields } => {
                // Allocate stack space for the struct
                let struct_size = match ty {
                    IrType::Struct {
                        fields: field_tys, ..
                    } => field_tys
                        .iter()
                        .map(|f| CraneliftBackend::type_size(&f.ty))
                        .sum::<usize>(),
                    _ => return Err(format!("CreateStruct with non-struct type: {:?}", ty)),
                };

                // Create stack slot for struct
                let struct_slot = builder.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    struct_size as u32,
                    8, // 8-byte alignment
                ));

                let slot_addr = builder.ins().stack_addr(types::I64, struct_slot, 0);

                // Store each field at its offset
                if let IrType::Struct {
                    fields: field_tys, ..
                } = ty
                {
                    let mut offset = 0;
                    for (i, field_val_id) in fields.iter().enumerate() {
                        let field_val = *value_map
                            .get(field_val_id)
                            .ok_or_else(|| format!("Struct field {:?} not found", field_val_id))?;

                        builder
                            .ins()
                            .store(MemFlags::new(), field_val, slot_addr, offset as i32);

                        // Move offset forward by field size
                        offset += CraneliftBackend::type_size(&field_tys[i].ty);
                    }
                }

                // Return the stack address as the struct value
                value_map.insert(*dest, slot_addr);
            }

            IrInstruction::CreateUnion {
                dest,
                discriminant,
                value,
                ty: _,
            } => {
                // For now, represent union as a struct { tag: i32, value_ptr: i64 }
                // This is a simplified representation - proper implementation would use
                // tagged union with max variant size

                // Create tag value
                let tag_val = builder.ins().iconst(types::I32, *discriminant as i64);

                // Get the value (for now, just use the value as-is or convert to pointer)
                let value_val = *value_map
                    .get(value)
                    .ok_or_else(|| format!("Union value {:?} not found", value))?;

                // For simplicity, store tag and value separately in a struct-like layout
                // Allocate space for the union on stack
                let union_slot = builder.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    16, // tag (4 bytes) + value (8 bytes) + padding
                    8,  // 8-byte alignment
                ));

                let slot_addr = builder.ins().stack_addr(types::I64, union_slot, 0);

                // Store tag at offset 0
                builder.ins().store(MemFlags::new(), tag_val, slot_addr, 0);

                // Store value at offset 8 (after padding)
                let value_offset = 8i32;
                builder
                    .ins()
                    .store(MemFlags::new(), value_val, slot_addr, value_offset);

                // Return the stack address as the union value
                value_map.insert(*dest, slot_addr);
            }

            IrInstruction::PtrAdd {
                dest,
                ptr,
                offset,
                ty,
            } => {
                // Pointer arithmetic: ptr + offset
                // Get pointer value
                let ptr_val = *value_map
                    .get(ptr)
                    .ok_or_else(|| format!("PtrAdd ptr {:?} not found", ptr))?;

                // Get offset value
                let offset_val = *value_map
                    .get(offset)
                    .ok_or_else(|| format!("PtrAdd offset {:?} not found", offset))?;

                // Get the size of the pointee type
                let pointee_ty = match ty {
                    IrType::Ptr(inner) => inner.as_ref(),
                    _ => return Err(format!("PtrAdd on non-pointer type: {:?}", ty)),
                };
                let elem_size = CraneliftBackend::type_size(pointee_ty);

                // Calculate byte offset: offset * elem_size
                // Assume offset is already i64 (or convert if needed in the future)
                let size_val = builder.ins().iconst(types::I64, elem_size as i64);
                let byte_offset = builder.ins().imul(offset_val, size_val);

                // Add to pointer
                let result_ptr = builder.ins().iadd(ptr_val, byte_offset);
                value_map.insert(*dest, result_ptr);
            }

            // TODO: Implement remaining instructions
            _ => {
                return Err(format!("Unsupported instruction: {:?}", instruction));
            }
        }

        Ok(())
    }

    /// Translate a terminator instruction (static method)
    fn translate_terminator_static(
        value_map: &HashMap<IrId, Value>,
        builder: &mut FunctionBuilder,
        terminator: &IrTerminator,
        block_map: &HashMap<IrBlockId, Block>,
        function: &IrFunction,
        sret_ptr: Option<Value>,
    ) -> Result<(), String> {
        use crate::ir::IrTerminator;

        match terminator {
            IrTerminator::Return { value } => {
                // eprintln!("DEBUG Cranelift: Translating Return terminator, value={:?}", value);
                // eprintln!("DEBUG Cranelift: value_map has {} entries", value_map.len());

                // If using sret, write the return value through the pointer and return void
                if let Some(sret) = sret_ptr {
                    if let Some(val_id) = value {
                        let val = *value_map
                            .get(val_id)
                            .ok_or_else(|| format!("Return value {:?} not found", val_id))?;

                        // Get the struct type to determine size
                        let struct_ty = function
                            .register_types
                            .get(val_id)
                            .or_else(|| function.locals.get(val_id).map(|l| &l.ty))
                            .ok_or_else(|| {
                                format!("Cannot find type for return value {:?}", val_id)
                            })?;

                        let struct_size = match struct_ty {
                            IrType::Struct { fields, .. } => fields
                                .iter()
                                .map(|f| CraneliftBackend::type_size(&f.ty))
                                .sum::<usize>(),
                            _ => return Err(format!("sret with non-struct type: {:?}", struct_ty)),
                        };

                        // Copy struct from source (val is a pointer to stack) to sret destination
                        // We need to do a memcpy-style copy of each field
                        if let IrType::Struct { fields, .. } = struct_ty {
                            let mut offset = 0;
                            for field in fields {
                                let field_ty =
                                    CraneliftBackend::mir_type_to_cranelift_static(&field.ty)?;
                                // Load from source struct
                                let field_val = builder.ins().load(
                                    field_ty,
                                    MemFlags::new(),
                                    val,
                                    offset as i32,
                                );
                                // Store to sret destination
                                builder.ins().store(
                                    MemFlags::new(),
                                    field_val,
                                    sret,
                                    offset as i32,
                                );
                                // Move offset forward
                                offset += CraneliftBackend::type_size(&field.ty);
                            }
                        }
                    }
                    // Return void for sret functions
                    builder.ins().return_(&[]);
                } else {
                    // Normal return path
                    if let Some(val_id) = value {
                        // eprintln!("DEBUG Cranelift: Looking up return value {:?} in value_map", val_id);
                        let val = *value_map.get(val_id).ok_or_else(|| {
                            eprintln!("ERROR: Return value {:?} NOT FOUND in value_map!", val_id);
                            eprintln!(
                                "ERROR: Available values: {:?}",
                                value_map.keys().collect::<Vec<_>>()
                            );
                            format!("Return value {:?} not found", val_id)
                        })?;
                        // eprintln!("DEBUG Cranelift: Found value, emitting return instruction");
                        builder.ins().return_(&[val]);
                    } else {
                        // eprintln!("DEBUG Cranelift: Void return, no value");
                        builder.ins().return_(&[]);
                    }
                }
            }

            IrTerminator::Branch { target } => {
                let cl_block = *block_map
                    .get(target)
                    .ok_or_else(|| format!("Branch target {:?} not found", target))?;

                // Get current block to find phi node arguments
                let current_block_id = function
                    .cfg
                    .blocks
                    .iter()
                    .find(|(_, block)| std::ptr::eq(&block.terminator, terminator))
                    .map(|(id, _)| *id)
                    .ok_or_else(|| "Cannot find current block".to_string())?;

                // Collect phi node arguments for the target block (with type coercion if needed)
                let phi_args = Self::collect_phi_args_with_coercion(
                    value_map,
                    function,
                    *target,
                    current_block_id,
                    builder,
                )?;

                builder.ins().jump(cl_block, &phi_args);
            }

            IrTerminator::CondBranch {
                condition,
                true_target,
                false_target,
            } => {
                let cond_val = *value_map
                    .get(condition)
                    .ok_or_else(|| format!("Condition value {:?} not found", condition))?;

                let true_block = *block_map
                    .get(true_target)
                    .ok_or_else(|| format!("True target {:?} not found", true_target))?;
                let false_block = *block_map
                    .get(false_target)
                    .ok_or_else(|| format!("False target {:?} not found", false_target))?;

                // Get current block to find phi node arguments
                let current_block_id = function
                    .cfg
                    .blocks
                    .iter()
                    .find(|(_, block)| std::ptr::eq(&block.terminator, terminator))
                    .map(|(id, _)| *id)
                    .ok_or_else(|| "Cannot find current block".to_string())?;

                // Collect phi node arguments for both targets (with type coercion if needed)
                let true_phi_args = Self::collect_phi_args_with_coercion(
                    value_map,
                    function,
                    *true_target,
                    current_block_id,
                    builder,
                )?;
                let false_phi_args = Self::collect_phi_args_with_coercion(
                    value_map,
                    function,
                    *false_target,
                    current_block_id,
                    builder,
                )?;

                builder.ins().brif(
                    cond_val,
                    true_block,
                    &true_phi_args,
                    false_block,
                    &false_phi_args,
                );
            }

            IrTerminator::Unreachable => {
                // Use a user trap code for unreachable (100 = unreachable)
                builder
                    .ins()
                    .trap(cranelift_codegen::ir::TrapCode::unwrap_user(100));
            }

            // TODO: Implement Switch and NoReturn
            _ => {
                return Err(format!("Unsupported terminator: {:?}", terminator));
            }
        }

        Ok(())
    }

    /// Translate a constant value to Cranelift IR (static method)
    fn translate_const_value(
        builder: &mut FunctionBuilder,
        value: &IrValue,
        function_map: &HashMap<IrFunctionId, FuncId>,
        runtime_functions: &mut HashMap<String, FuncId>,
        module: &mut JITModule,
        string_data: &mut HashMap<String, DataId>,
        string_counter: &mut usize,
    ) -> Result<Value, String> {
        use crate::ir::IrValue;

        let cl_value = match value {
            IrValue::I8(v) => builder.ins().iconst(types::I8, i64::from(*v)),
            IrValue::I16(v) => builder.ins().iconst(types::I16, i64::from(*v)),
            IrValue::I32(v) => {
                // For I32, need to handle negative values by treating as u32 first
                let as_u32 = *v as u32;
                builder.ins().iconst(types::I32, i64::from(as_u32))
            }
            IrValue::I64(v) => builder.ins().iconst(types::I64, *v),
            IrValue::U8(v) => builder.ins().iconst(types::I8, i64::from(*v)),
            IrValue::U16(v) => builder.ins().iconst(types::I16, i64::from(*v)),
            IrValue::U32(v) => builder.ins().iconst(types::I32, i64::from(*v)),
            IrValue::U64(v) => builder.ins().iconst(types::I64, *v as i64),
            IrValue::F32(v) => builder.ins().f32const(*v),
            IrValue::F64(v) => builder.ins().f64const(*v),
            IrValue::Bool(v) => builder.ins().iconst(types::I8, if *v { 1 } else { 0 }),
            IrValue::Null => builder.ins().iconst(types::I64, 0),
            IrValue::String(s) => {
                // Allocate string data in data section and call runtime to create HaxeString
                let data_id = if let Some(&existing) = string_data.get(s) {
                    existing
                } else {
                    // Create new data section entry for this string
                    let name = format!("str_{}", *string_counter);
                    *string_counter += 1;

                    let data_id = module
                        .declare_data(&name, Linkage::Local, false, false)
                        .map_err(|e| format!("Failed to declare string data: {}", e))?;

                    let mut data_desc = DataDescription::new();
                    data_desc.define(s.as_bytes().to_vec().into_boxed_slice());

                    module
                        .define_data(data_id, &data_desc)
                        .map_err(|e| format!("Failed to define string data: {}", e))?;

                    string_data.insert(s.clone(), data_id);
                    data_id
                };

                // Get pointer to the string data
                let gv = module.declare_data_in_func(data_id, builder.func);
                let str_ptr = builder.ins().global_value(types::I64, gv);
                let str_len = builder.ins().iconst(types::I64, s.len() as i64);

                // Get or declare haxe_string_literal runtime function
                let string_literal_func = if let Some(&func_id) = runtime_functions.get("haxe_string_literal") {
                    func_id
                } else {
                    // Declare haxe_string_literal(ptr: *const u8, len: usize) -> *mut HaxeString
                    let mut sig = module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // ptr
                    sig.params.push(AbiParam::new(types::I64)); // len
                    sig.returns.push(AbiParam::new(types::I64)); // returns *mut HaxeString

                    let func_id = module
                        .declare_function("haxe_string_literal", Linkage::Import, &sig)
                        .map_err(|e| format!("Failed to declare haxe_string_literal: {}", e))?;

                    runtime_functions.insert("haxe_string_literal".to_string(), func_id);
                    func_id
                };

                // Call haxe_string_literal(ptr, len) -> *mut HaxeString
                let func_ref = module.declare_func_in_func(string_literal_func, builder.func);
                let call = builder.ins().call(func_ref, &[str_ptr, str_len]);
                builder.inst_results(call)[0]
            }
            IrValue::Function(mir_func_id) => {
                // Get the Cranelift FuncId for this MIR function
                let cl_func_id = *function_map.get(mir_func_id).ok_or_else(|| {
                    format!("Function {:?} not found in function_map", mir_func_id)
                })?;

                // Import the function reference into the current function
                let func_ref = module.declare_func_in_func(cl_func_id, builder.func);

                // Convert function reference to an address (i64 pointer)
                builder.ins().func_addr(types::I64, func_ref)
            }
            _ => {
                return Err(format!("Unsupported constant value: {:?}", value));
            }
        };

        Ok(cl_value)
    }

    /// Convert MIR type to Cranelift type (static version for use without self)
    pub(super) fn mir_type_to_cranelift_static(ty: &IrType) -> Result<Type, String> {
        match ty {
            IrType::Void => Ok(types::INVALID), // Void functions have no return value
            IrType::I8 => Ok(types::I8),
            IrType::I16 => Ok(types::I16),
            IrType::I32 => Ok(types::I32),
            IrType::I64 => Ok(types::I64),
            IrType::U8 => Ok(types::I8),
            IrType::U16 => Ok(types::I16),
            IrType::U32 => Ok(types::I32),
            IrType::U64 => Ok(types::I64),
            IrType::F32 => Ok(types::F32),
            IrType::F64 => Ok(types::F64),
            IrType::Bool => Ok(types::I8),
            IrType::Ptr(_) => Ok(types::I64),
            IrType::Ref(_) => Ok(types::I64),
            IrType::Array(..) => Ok(types::I64),
            IrType::Slice(_) => Ok(types::I64),
            IrType::String => Ok(types::I64),
            IrType::Struct { .. } => Ok(types::I64),
            IrType::Union { .. } => Ok(types::I64),
            IrType::Any => Ok(types::I64),
            IrType::Function { .. } => Ok(types::I64),
            IrType::Opaque { .. } => Ok(types::I64),
            IrType::TypeVar(_) => Ok(types::I64),
        }
    }

    pub(super) fn mir_type_to_cranelift(&self, ty: &IrType) -> Result<Type, String> {
        Self::mir_type_to_cranelift_static(ty)
    }

    /// Get a pointer to the compiled function
    pub fn get_function_ptr(&mut self, mir_func_id: IrFunctionId) -> Result<*const u8, String> {
        let func_id = *self
            .function_map
            .get(&mir_func_id)
            .ok_or("Function not found")?;

        let code_ptr = self.module.get_finalized_function(func_id);

        Ok(code_ptr)
    }

    /// Call the main function (assuming it's void main() -> void)
    ///
    /// This function also waits for all spawned threads to complete before returning,
    /// ensuring that JIT code memory remains valid while threads are executing.
    pub fn call_main(&mut self, module: &crate::ir::IrModule) -> Result<(), String> {
        // Find the main function in the MIR module
        // Try various naming conventions: main, Main_main, Main.main, etc.
        let main_func = module
            .functions
            .values()
            .find(|f| {
                f.name == "main"
                    || f.name == "Main_main"
                    || f.name == "Main.main"
                    || f.name.ends_with("_main")
                    || f.name.ends_with(".main")
            })
            .ok_or_else(|| {
                // List available functions for debugging
                let func_names: Vec<_> = module
                    .functions
                    .values()
                    .filter(|f| !f.cfg.blocks.is_empty()) // Skip externs
                    .map(|f| &f.name)
                    .take(10)
                    .collect();
                format!(
                    "No main function found in module. Available functions (first 10): {:?}",
                    func_names
                )
            })?;

        // Get the function pointer
        let func_ptr = self.get_function_ptr(main_func.id)?;

        println!("  🚀 Executing {}()...", main_func.name);

        // Call the main function (assuming it's void main() -> void)
        // This is unsafe because we're calling JIT-compiled code
        unsafe {
            let main_fn: extern "C" fn() = std::mem::transmute(func_ptr);
            main_fn();
        }

        // CRITICAL: Wait for all spawned threads to complete before returning
        // This prevents use-after-free when threads are still executing JIT code
        // and the JIT module is dropped
        eprintln!("  🔄 Waiting for spawned threads to complete...");
        rayzor_runtime::concurrency::rayzor_wait_all_threads();

        println!("  ✅ Execution completed successfully!");

        Ok(())
    }
}

impl Default for CraneliftBackend {
    fn default() -> Self {
        Self::new().expect("Failed to create Cranelift backend")
    }
}

impl CraneliftBackend {
    /// Get the size in bytes of an IR type
    fn type_size(ty: &crate::ir::IrType) -> usize {
        use crate::ir::IrType;
        match ty {
            IrType::I8 | IrType::U8 | IrType::Bool => 1,
            IrType::I16 | IrType::U16 => 2,
            IrType::I32 | IrType::U32 | IrType::F32 => 4,
            IrType::I64 | IrType::U64 | IrType::F64 => 8,
            IrType::Ptr(_) | IrType::Ref(_) => 8, // Assume 64-bit pointers
            IrType::Void => 0,
            IrType::Any => 8, // Boxed value pointer
            _ => 8,           // Default to pointer size
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cranelift_backend_creation() {
        let backend = CraneliftBackend::new().unwrap();
        assert!(backend.function_map.is_empty());
    }
}
