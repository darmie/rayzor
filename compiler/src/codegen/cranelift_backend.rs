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
use cranelift_codegen::ir::{ArgumentPurpose, Function};
use cranelift_codegen::settings;
use cranelift_frontend::Variable;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_native;
use std::collections::HashMap;

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
    fn with_symbols_and_opt(opt_level: &str, symbols: &[(&str, *const u8)]) -> Result<Self, String> {
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
            IrType::Ptr(_) | IrType::Ref(_) | IrType::Function { .. } => self.get_pointer_size() as u64,
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
                let max_variant_size = variants.iter()
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
                fields.iter()
                    .map(|f| self.get_type_alignment(&f.ty))
                    .max()
                    .unwrap_or(1)
            }
            IrType::Union { variants, .. } => {
                // Max alignment of variant fields
                let max_align = variants.iter()
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

    /// Compile an entire MIR module
    pub fn compile_module(&mut self, mir_module: &IrModule) -> Result<(), String> {
        // First pass: declare all functions
        for (func_id, function) in &mir_module.functions {
            self.declare_function(*func_id, function)?;
        }

        // Declare C standard library memory functions
        // These will be linked from libc (malloc, realloc, free)
        for (_, function) in &mir_module.functions {
            if function.name == "malloc" {
                eprintln!("DEBUG: Declaring libc function: malloc");
                self.declare_libc_function("malloc", 1, true)?;  // 1 param, has return
            } else if function.name == "realloc" {
                eprintln!("DEBUG: Declaring libc function: realloc");
                self.declare_libc_function("realloc", 2, true)?;  // 2 params (ptr, size), has return
            } else if function.name == "free" {
                eprintln!("DEBUG: Declaring libc function: free");
                self.declare_libc_function("free", 1, false)?;  // 1 param (ptr), no return
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
        // Build Cranelift signature
        let mut sig = self.module.make_signature();

        // Determine if this is an extern function (empty CFG)
        let is_extern = function.cfg.blocks.is_empty();

        // Check if we need sret (struct return by pointer)
        // MUST use sret for ALL functions (including extern) that return structs
        // because the C ABI on ARM64 uses sret for structs > 16 bytes
        let use_sret_in_signature = function.signature.uses_sret;

        if use_sret_in_signature {
            // Add sret parameter as first parameter
            sig.params.push(AbiParam::special(
                types::I64,  // pointer type
                ArgumentPurpose::StructReturn,
            ));
        }

        // Add parameters
        for param in &function.signature.parameters {
            let cranelift_type = self.mir_type_to_cranelift(&param.ty)?;
            sig.params.push(AbiParam::new(cranelift_type));
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
            // Regular functions get unique names and Export linkage
            if let Some(ref qualified_name) = function.qualified_name {
                // Use qualified name for better debugging/profiling
                (format!("{}__func_{}", qualified_name.replace(".", "_"), mir_func_id.0), Linkage::Export)
            } else {
                (format!("func_{}", mir_func_id.0), Linkage::Export)
            }
        };

        let func_id = self
            .module
            .declare_function(&func_name, linkage, &sig)
            .map_err(|e| format!("Failed to declare function: {}", e))?;

        // eprintln!("DEBUG Cranelift: Declared function '{}' - MIR={:?} -> Cranelift={:?}", func_name, mir_func_id, func_id);
        self.function_map.insert(mir_func_id, func_id);

        Ok(())
    }

    /// Declare a libc function (malloc, realloc, free)
    /// These are provided by the system C library
    fn declare_libc_function(&mut self, name: &str, param_count: usize, has_return: bool) -> Result<FuncId, String> {
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

        eprintln!("DEBUG: Declared libc {} as Cranelift func_id: {:?}", name, func_id);
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

        // Add parameters to signature
        for param in &function.signature.parameters {
            let cranelift_type = self.mir_type_to_cranelift(&param.ty)?;
            self.ctx.func.signature.params.push(AbiParam::new(cranelift_type));
        }

        // Add return type to signature (void for sret functions)
        if uses_sret {
            // sret functions return void - the value is written through the pointer
        } else {
            let return_type = self.mir_type_to_cranelift(&function.signature.return_type)?;
            if return_type != types::INVALID {
                self.ctx.func.signature.returns.push(AbiParam::new(return_type));
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

        // If using sret, first parameter is the return pointer
        let param_offset = if uses_sret { 1 } else { 0 };

        for (i, param) in function.signature.parameters.iter().enumerate() {
            self.value_map.insert(param.reg, param_values[i + param_offset]);
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
            let cl_block = *block_map.get(mir_block_id)
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
                Self::translate_phi_node_static(&mut self.value_map, &mut builder, phi_node, &block_map, &function.cfg)?;
                // eprintln!("    After translation, value_map has {:?}", self.value_map.keys().collect::<Vec<_>>());
            }

            // Translate instructions
            for instruction in &mir_block.instructions {
                Self::translate_instruction(&mut self.value_map, &mut builder, instruction, function, &self.function_map, &mut self.runtime_functions, mir_module, &mut self.module, &mut self.closure_environments)?;
            }

            // Translate terminator
            // eprintln!("DEBUG Cranelift: MIR terminator for block {:?}: {:?}", mir_block_id, mir_block.terminator);
            if let Err(e) = Self::translate_terminator_static(&mut self.value_map, &mut builder, &mir_block.terminator, &block_map, function, sret_ptr) {
                eprintln!("\n!!! Error translating terminator in block {:?}: {}", mir_block_id, e);
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

        // Clear the context for next function
        self.module.clear_context(&mut self.ctx);

        Ok(())
    }

    /// Collect phi node arguments when branching to a block
    fn collect_phi_args(
        value_map: &HashMap<IrId, Value>,
        function: &IrFunction,
        target_block: IrBlockId,
        from_block: IrBlockId,
    ) -> Result<Vec<Value>, String> {
        let target = function.cfg.blocks.get(&target_block)
            .ok_or_else(|| format!("Target block {:?} not found", target_block))?;

        let mut phi_args = Vec::new();

        // For each phi node in the target block, find the incoming value from our block
        for phi_node in &target.phi_nodes {
            // Find the incoming value for this phi from our current block
            let incoming_value = phi_node.incoming.iter()
                .find(|(block_id, _)| *block_id == from_block)
                .map(|(_, value_id)| value_id)
                .ok_or_else(|| format!(
                    "No incoming value for phi node {:?} from block {:?}",
                    phi_node.dest, from_block
                ))?;

            // Look up the Cranelift value for this MIR value
            let cl_value = *value_map.get(incoming_value)
                .ok_or_else(|| format!(
                    "Value {:?} not found in value_map for phi incoming",
                    incoming_value
                ))?;

            phi_args.push(cl_value);
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
            _ => cranelift_codegen::ir::types::I64, // Default
        };

        // Get the current block
        let current_block = builder.current_block()
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
    ) -> Result<(), String> {
        use crate::ir::IrInstruction;

        match instruction {
            IrInstruction::Const { dest, value } => {
                let cl_value = Self::translate_const_value(builder, value, function_map, module)?;
                value_map.insert(*dest, cl_value);
            }

            IrInstruction::Copy { dest, src } => {
                let src_value = *value_map.get(src)
                    .ok_or_else(|| format!("Source value {:?} not found", src))?;
                value_map.insert(*dest, src_value);
            }

            IrInstruction::BinOp { dest, op, left, right } => {
                // Get type from register_types map first, then fall back to locals
                let ty = function.register_types.get(dest)
                    .or_else(|| function.register_types.get(left))
                    .or_else(|| function.locals.get(dest).map(|local| &local.ty))
                    .ok_or_else(|| format!("Type not found for BinOp dest {:?}", dest))?;

                let value = Self::lower_binary_op_static(value_map, builder, op, ty, *left, *right)?;
                value_map.insert(*dest, value);
            }

            IrInstruction::UnOp { dest, op, operand } => {
                // Get type from register_types map first, then fall back to locals
                let ty = function.register_types.get(dest)
                    .or_else(|| function.register_types.get(operand))
                    .or_else(|| function.locals.get(dest).map(|local| &local.ty))
                    .ok_or_else(|| format!("Type not found for UnOp dest {:?}", dest))?;

                let value = Self::lower_unary_op_static(value_map, builder, op, ty, *operand)?;
                value_map.insert(*dest, value);
            }

            IrInstruction::Cmp { dest, op, left, right } => {
                // Get type from register_types map
                let ty = function.register_types.get(left)
                    .or_else(|| function.locals.get(left).map(|local| &local.ty))
                    .ok_or_else(|| format!("Type not found for Cmp operand {:?}", left))?;

                let value = Self::lower_compare_op_static(value_map, builder, op, ty, *left, *right)?;
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

            IrInstruction::CallDirect { dest, func_id, args } => {
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

                        // Add parameters (all F64 for Math functions)
                        for _ in 0..extern_func.signature.parameters.len() {
                            sig.params.push(AbiParam::new(types::F64));
                        }

                        // Add return type (F64 for Math functions)
                        if extern_func.signature.return_type != crate::ir::IrType::Void {
                            sig.returns.push(AbiParam::new(types::F64));
                        }

                        let id = module
                            .declare_function(&extern_func.name, Linkage::Import, &sig)
                            .map_err(|e| format!("Failed to declare runtime function {}: {}", extern_func.name, e))?;

                        eprintln!("INFO: Declared external runtime function {} as func_id: {:?}", extern_func.name, id);
                        runtime_functions.insert(extern_func.name.clone(), id);
                        id
                    };

                    let func_ref = module.declare_func_in_func(cl_func_id, builder.func);

                    // Lower arguments
                    let mut arg_values = Vec::new();
                    for &arg_reg in args {
                        let cl_value = *value_map.get(&arg_reg)
                            .ok_or_else(|| format!("Argument register {:?} not found in value_map", arg_reg))?;
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
                    let called_func = mir_module.functions.get(func_id)
                        .ok_or_else(|| format!("Called function {:?} not found in module", func_id))?;

                let (cl_func_id, func_ref) = if called_func.name == "malloc" ||
                                                called_func.name == "realloc" ||
                                                called_func.name == "free" {
                    // This is a memory management function - call the libc version
                    let libc_id = *runtime_functions.get(&called_func.name)
                        .ok_or_else(|| format!("libc function {} not declared", called_func.name))?;
                    eprintln!("DEBUG: Redirecting {} call to libc func_id: {:?}", called_func.name, libc_id);
                    let func_ref = module.declare_func_in_func(libc_id, builder.func);
                    (libc_id, func_ref)
                } else {
                    // Normal MIR function call
                    let cl_func_id = *function_map.get(func_id)
                        .ok_or_else(|| format!("Function {:?} not found in function_map", func_id))?;
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
                for arg_id in args {
                    let arg_val = *value_map.get(arg_id)
                        .ok_or_else(|| format!("Argument {:?} not found in value_map", arg_id))?;
                    call_args.push(arg_val);
                }

                // Emit the call instruction
                let call_inst = builder.ins().call(func_ref, &call_args);

                // Handle return value
                if let Some(dest_reg) = dest {
                    if uses_sret {
                        // For sret, the "return value" is the pointer to the sret slot
                        value_map.insert(*dest_reg, sret_slot.unwrap());
                    } else {
                        // Normal return value
                        let results = builder.inst_results(call_inst);
                        if !results.is_empty() {
                            value_map.insert(*dest_reg, results[0]);
                        } else {
                            return Err(format!("Function call expected to return value but got none (func_id={:?}, dest={:?})", func_id, dest_reg));
                        }
                    }
                }
                }
            }

            IrInstruction::CallIndirect { dest, func_ptr, args, signature } => {
                // Get the function pointer value
                let func_val = *value_map.get(func_ptr)
                    .ok_or_else(|| format!("Function pointer {:?} not found in value_map", func_ptr))?;

                // Check if this is a closure call (has an environment)
                let is_closure = closure_environments.contains_key(func_ptr);
                let env_ptr = if is_closure {
                    closure_environments.get(func_ptr).copied()
                } else {
                    None
                };

                // Build Cranelift signature from MIR signature
                let sig = match signature {
                    IrType::Function { params, return_type, .. } => {
                        let mut cl_sig = module.make_signature();

                        // If this is a closure, add environment pointer as first parameter
                        if is_closure {
                            cl_sig.params.push(AbiParam::new(types::I64)); // env pointer
                            eprintln!("Info: Adding environment parameter to closure call signature");
                        }

                        // Add parameter types
                        for param_ty in params {
                            let cl_param_ty = CraneliftBackend::mir_type_to_cranelift_static(param_ty)?;
                            cl_sig.params.push(AbiParam::new(cl_param_ty));
                        }

                        // Add return type
                        let cl_ret_ty = CraneliftBackend::mir_type_to_cranelift_static(return_type)?;
                        if cl_ret_ty != types::INVALID {
                            cl_sig.returns.push(AbiParam::new(cl_ret_ty));
                        }

                        cl_sig
                    }
                    _ => {
                        return Err(format!("CallIndirect signature must be Function type, got {:?}", signature));
                    }
                };

                // Import the signature into the module
                let sig_ref = builder.import_signature(sig);

                // Translate arguments
                let mut call_args = Vec::new();

                // If this is a closure, prepend environment pointer as first argument
                if let Some(env) = env_ptr {
                    call_args.push(env);
                    eprintln!("Info: Passing environment pointer to closure call");
                }

                for arg_id in args {
                    let arg_val = *value_map.get(arg_id)
                        .ok_or_else(|| format!("Argument {:?} not found in value_map", arg_id))?;
                    call_args.push(arg_val);
                }

                // Emit the indirect call instruction
                let call_inst = builder.ins().call_indirect(sig_ref, func_val, &call_args);

                // Get return value if the function returns something
                if let Some(dest_reg) = dest {
                    let results = builder.inst_results(call_inst);
                    if !results.is_empty() {
                        value_map.insert(*dest_reg, results[0]);
                    } else {
                        return Err(format!("Indirect call expected to return value but got none"));
                    }
                }
            }

            IrInstruction::MakeClosure { dest, func_id, captured_values } => {
                // Create a closure with environment for captured variables
                //
                // Strategy:
                // 1. Allocate environment struct on the stack
                // 2. Store captured values into environment
                // 3. Return function pointer and track environment separately

                // Get the Cranelift FuncId for the lambda
                let cl_func_id = function_map.get(func_id)
                    .ok_or_else(|| format!("Lambda function {:?} not found in function_map", func_id))?;

                // Import function and get its address
                let func_ref = module.declare_func_in_func(*cl_func_id, builder.func);
                let func_addr = builder.ins().func_addr(types::I64, func_ref);

                // If there are captured values, allocate environment and store them
                if !captured_values.is_empty() {
                    // Calculate environment size: 8 bytes per captured value
                    let env_size = (captured_values.len() * 8) as i32;

                    // Allocate environment on stack
                    let slot = builder.create_sized_stack_slot(StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        env_size as u32,
                        8, // 8-byte alignment
                    ));

                    // Get the address of the stack slot
                    let env_ptr = builder.ins().stack_addr(types::I64, slot, 0);

                    // Store each captured value into the environment
                    for (i, captured_id) in captured_values.iter().enumerate() {
                        let captured_val = *value_map.get(captured_id)
                            .ok_or_else(|| format!("Captured value {:?} not found in value_map", captured_id))?;

                        // Calculate offset for this field (i * 8 bytes)
                        let offset = (i * 8) as i32;

                        // Store the value at env_ptr + offset
                        // For now, assume all captured values are i32 (we'll improve this later)
                        builder.ins().store(MemFlags::new(), captured_val, env_ptr, offset);
                    }

                    // Track the environment pointer for this closure
                    closure_environments.insert(*dest, env_ptr);

                    eprintln!("Info: Allocated environment for {} captured variables", captured_values.len());
                }

                // Store the function pointer as the closure value
                // The environment is tracked separately in closure_environments map
                value_map.insert(*dest, func_addr);
            }

            IrInstruction::ClosureFunc { dest, closure } => {
                // Extract function pointer from closure
                // For now, closure is just the function pointer
                let closure_val = *value_map.get(closure)
                    .ok_or_else(|| format!("Closure {:?} not found in value_map", closure))?;
                value_map.insert(*dest, closure_val);
            }

            IrInstruction::ClosureEnv { dest, closure } => {
                // Extract environment pointer from closure
                // For now, return null since we haven't implemented environment yet
                let _ = value_map.get(closure)
                    .ok_or_else(|| format!("Closure {:?} not found in value_map", closure))?;
                let null_ptr = builder.ins().iconst(types::I64, 0);
                value_map.insert(*dest, null_ptr);
            }

            IrInstruction::Cast { dest, src, from_ty, to_ty } => {
                // Type casting (e.g., int to float, float to int)
                let src_val = *value_map.get(src)
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

                    _ => return Err(format!("Unsupported cast from {:?} to {:?}", from_ty, to_ty)),
                };

                value_map.insert(*dest, result);
            }

            IrInstruction::GetElementPtr { dest, ptr, indices, ty } => {
                // Get Element Pointer - compute address of field within struct
                // This is similar to LLVM's GEP instruction

                // eprintln!("DEBUG Cranelift: GetElementPtr - ptr={:?}, indices={:?}, ty={:?}", ptr, indices, ty);

                let ptr_val = *value_map.get(ptr)
                    .ok_or_else(|| format!("GEP ptr {:?} not found in value_map", ptr))?;

                // For now, we assume a single index (field index in struct)
                // More complex GEP operations (nested structs, arrays) need additional work
                if indices.len() != 1 {
                    return Err(format!("GEP with {} indices not yet supported (only single index supported)", indices.len()));
                }

                let index_id = indices[0];
                let index_val = *value_map.get(&index_id)
                    .ok_or_else(|| format!("GEP index {:?} not found in value_map", index_id))?;

                // Get the size of the element type
                let elem_size = Self::type_size(ty);
                let size_val = builder.ins().iconst(types::I64, elem_size as i64);

                // Convert index to i64 if needed
                let index_i64 = builder.ins().sextend(types::I64, index_val);

                // Compute offset: index * elem_size
                let offset = builder.ins().imul(index_i64, size_val);

                // Add offset to base pointer
                let result_ptr = builder.ins().iadd(ptr_val, offset);

                // eprintln!("DEBUG Cranelift: GEP result - dest={:?}", dest);
                value_map.insert(*dest, result_ptr);
            }

            IrInstruction::ExtractValue { dest, aggregate, indices } => {
                // For struct field extraction, we need to calculate the offset and load
                // Get the aggregate value (should be a pointer to struct on stack)
                let aggregate_val = *value_map.get(aggregate)
                    .ok_or_else(|| format!("Aggregate value {:?} not found", aggregate))?;

                // For now, handle simple single-index case (most common for structs)
                if indices.len() != 1 {
                    return Err(format!("ExtractValue with multiple indices not yet supported: {:?}", indices));
                }

                let field_index = indices[0] as usize;

                // Get the struct type from the aggregate - check both parameters and locals
                // If not found, try to find the Load instruction that produced this value
                let aggregate_ty = function.signature.parameters.iter()
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
                            return Err(format!("Field index {} out of bounds for struct with {} fields",
                                field_index, fields.len()));
                        }

                        // Calculate offset: sum of sizes of all previous fields
                        let offset: usize = fields.iter()
                            .take(field_index)
                            .map(|f| CraneliftBackend::type_size(&f.ty))
                            .sum();

                        let field = &fields[field_index];
                        (offset, &field.ty)
                    }
                    _ => {
                        return Err(format!("ExtractValue on non-struct type: {:?}", aggregate_ty));
                    }
                };

                // Add offset to base pointer
                let offset_val = builder.ins().iconst(types::I64, field_offset as i64);
                let field_ptr = builder.ins().iadd(aggregate_val, offset_val);

                // Load the field value
                let field_cl_ty = CraneliftBackend::mir_type_to_cranelift_static(field_ty)?;
                let field_value = builder.ins().load(field_cl_ty, MemFlags::new(), field_ptr, 0);

                value_map.insert(*dest, field_value);
            }

            IrInstruction::FunctionRef { dest, func_id } => {
                // Get function reference as a pointer
                let cl_func_id = *function_map.get(func_id)
                    .ok_or_else(|| format!("Function {:?} not found in function_map", func_id))?;

                // Import the function reference into the current function
                let func_ref = module.declare_func_in_func(cl_func_id, builder.func);

                // Convert function reference to an address (i64 pointer)
                let func_ptr = builder.ins().func_addr(types::I64, func_ref);

                value_map.insert(*dest, func_ptr);
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
                    IrType::Struct { fields: field_tys, .. } => {
                        field_tys.iter().map(|f| CraneliftBackend::type_size(&f.ty)).sum::<usize>()
                    }
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
                if let IrType::Struct { fields: field_tys, .. } = ty {
                    let mut offset = 0;
                    for (i, field_val_id) in fields.iter().enumerate() {
                        let field_val = *value_map.get(field_val_id)
                            .ok_or_else(|| format!("Struct field {:?} not found", field_val_id))?;

                        builder.ins().store(MemFlags::new(), field_val, slot_addr, offset as i32);

                        // Move offset forward by field size
                        offset += CraneliftBackend::type_size(&field_tys[i].ty);
                    }
                }

                // Return the stack address as the struct value
                value_map.insert(*dest, slot_addr);
            }

            IrInstruction::CreateUnion { dest, discriminant, value, ty: _ } => {
                // For now, represent union as a struct { tag: i32, value_ptr: i64 }
                // This is a simplified representation - proper implementation would use
                // tagged union with max variant size

                // Create tag value
                let tag_val = builder.ins().iconst(types::I32, *discriminant as i64);

                // Get the value (for now, just use the value as-is or convert to pointer)
                let value_val = *value_map.get(value)
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
                builder.ins().store(MemFlags::new(), value_val, slot_addr, value_offset);

                // Return the stack address as the union value
                value_map.insert(*dest, slot_addr);
            }

            IrInstruction::PtrAdd { dest, ptr, offset, ty } => {
                // Pointer arithmetic: ptr + offset
                // Get pointer value
                let ptr_val = *value_map.get(ptr)
                    .ok_or_else(|| format!("PtrAdd ptr {:?} not found", ptr))?;

                // Get offset value
                let offset_val = *value_map.get(offset)
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
                        let val = *value_map.get(val_id)
                            .ok_or_else(|| format!("Return value {:?} not found", val_id))?;

                        // Get the struct type to determine size
                        let struct_ty = function.register_types.get(val_id)
                            .or_else(|| function.locals.get(val_id).map(|l| &l.ty))
                            .ok_or_else(|| format!("Cannot find type for return value {:?}", val_id))?;

                        let struct_size = match struct_ty {
                            IrType::Struct { fields, .. } => {
                                fields.iter().map(|f| CraneliftBackend::type_size(&f.ty)).sum::<usize>()
                            }
                            _ => return Err(format!("sret with non-struct type: {:?}", struct_ty)),
                        };

                        // Copy struct from source (val is a pointer to stack) to sret destination
                        // We need to do a memcpy-style copy of each field
                        if let IrType::Struct { fields, .. } = struct_ty {
                            let mut offset = 0;
                            for field in fields {
                                let field_ty = CraneliftBackend::mir_type_to_cranelift_static(&field.ty)?;
                                // Load from source struct
                                let field_val = builder.ins().load(field_ty, MemFlags::new(), val, offset as i32);
                                // Store to sret destination
                                builder.ins().store(MemFlags::new(), field_val, sret, offset as i32);
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
                        let val = *value_map.get(val_id)
                            .ok_or_else(|| {
                                eprintln!("ERROR: Return value {:?} NOT FOUND in value_map!", val_id);
                                eprintln!("ERROR: Available values: {:?}", value_map.keys().collect::<Vec<_>>());
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
                let cl_block = *block_map.get(target)
                    .ok_or_else(|| format!("Branch target {:?} not found", target))?;

                // Get current block to find phi node arguments
                let current_block_id = function.cfg.blocks.iter()
                    .find(|(_, block)| std::ptr::eq(&block.terminator, terminator))
                    .map(|(id, _)| *id)
                    .ok_or_else(|| "Cannot find current block".to_string())?;

                // Collect phi node arguments for the target block
                let phi_args = Self::collect_phi_args(value_map, function, *target, current_block_id)?;

                builder.ins().jump(cl_block, &phi_args);
            }

            IrTerminator::CondBranch { condition, true_target, false_target } => {
                let cond_val = *value_map.get(condition)
                    .ok_or_else(|| format!("Condition value {:?} not found", condition))?;

                let true_block = *block_map.get(true_target)
                    .ok_or_else(|| format!("True target {:?} not found", true_target))?;
                let false_block = *block_map.get(false_target)
                    .ok_or_else(|| format!("False target {:?} not found", false_target))?;

                // Get current block to find phi node arguments
                let current_block_id = function.cfg.blocks.iter()
                    .find(|(_, block)| std::ptr::eq(&block.terminator, terminator))
                    .map(|(id, _)| *id)
                    .ok_or_else(|| "Cannot find current block".to_string())?;

                // Collect phi node arguments for both targets
                let true_phi_args = Self::collect_phi_args(value_map, function, *true_target, current_block_id)?;
                let false_phi_args = Self::collect_phi_args(value_map, function, *false_target, current_block_id)?;

                builder.ins().brif(cond_val, true_block, &true_phi_args, false_block, &false_phi_args);
            }

            IrTerminator::Unreachable => {
                builder.ins().trap(cranelift_codegen::ir::TrapCode::UnreachableCodeReached);
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
        module: &mut JITModule,
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
            IrValue::String(_s) => {
                // TODO: Properly allocate string constants in data section
                // For now, return null pointer (empty string)
                builder.ins().iconst(types::I64, 0)
            }
            IrValue::Function(mir_func_id) => {
                // Get the Cranelift FuncId for this MIR function
                let cl_func_id = *function_map.get(mir_func_id)
                    .ok_or_else(|| format!("Function {:?} not found in function_map", mir_func_id))?;

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
            _ => 8, // Default to pointer size
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
