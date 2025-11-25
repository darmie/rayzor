//! # LLVM JIT Backend for Rayzor
//!
//! Implements Tier 3 (Maximum optimization) using LLVM's MCJIT for production-quality code generation.
//!
//! ## Architecture
//! - Uses LLVM 17.0 via inkwell bindings
//! - Translates Rayzor MIR (Mid-level IR) to LLVM IR
//! - Provides JIT compilation with aggressive optimization (-O3)
//! - Designed as drop-in replacement for Cranelift in hot paths
//!
//! ## Use Cases
//! 1. **Tier 3 in tiered JIT**: Optimize ultra-hot functions (>10k-100k calls)
//! 2. **AOT compilation**: Generate optimized native binaries
//! 3. **Profile-guided optimization**: Recompile based on runtime profiling
//!
//! ## Performance Target
//! - Compilation: 1-5s per function (slower than Cranelift)
//! - Runtime: 5-20x baseline (production C/C++ quality)
//! - Use only for truly hot code (<1% of functions)

#[cfg(feature = "llvm-backend")]
use inkwell::{
    OptimizationLevel,
    context::Context,
    execution_engine::{ExecutionEngine, JitFunction},
    module::Module,
    builder::Builder,
    types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum},
    values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue, PhiValue},
    basic_block::BasicBlock,
    IntPredicate, FloatPredicate,
    AddressSpace,
    targets::{InitializationConfig, Target, TargetMachine, RelocMode, CodeModel, FileType, TargetData},
};

use std::collections::HashMap;
use crate::ir::{
    IrModule, IrFunction, IrFunctionId, IrType, IrValue, IrInstruction, IrTerminator,
    IrBasicBlock, IrBlockId, IrId, IrPhiNode, BinaryOp, UnaryOp, CompareOp,
};

/// LLVM JIT backend using MCJIT
///
/// Compiles Rayzor MIR to native code using LLVM's aggressive optimizations.
/// Used as Tier 3 in the tiered compilation system for ultra-hot functions.
#[cfg(feature = "llvm-backend")]
pub struct LLVMJitBackend<'ctx> {
    /// LLVM context (lifetime-bound)
    context: &'ctx Context,

    /// LLVM module
    module: Module<'ctx>,

    /// LLVM IR builder
    builder: Builder<'ctx>,

    /// JIT execution engine
    execution_engine: Option<ExecutionEngine<'ctx>>,

    /// Maps MIR value IDs to LLVM values
    value_map: HashMap<IrId, BasicValueEnum<'ctx>>,

    /// Maps MIR function IDs to LLVM functions
    function_map: HashMap<IrFunctionId, FunctionValue<'ctx>>,

    /// Maps MIR block IDs to LLVM basic blocks
    block_map: HashMap<IrBlockId, BasicBlock<'ctx>>,

    /// Maps phi node destination IDs to LLVM phi instructions
    phi_map: HashMap<IrId, PhiValue<'ctx>>,

    /// Function pointers cache (usize for thread safety)
    function_pointers: HashMap<IrFunctionId, usize>,

    /// Optimization level
    opt_level: OptimizationLevel,

    /// Target data for architecture-specific type sizes/alignment
    target_data: Option<TargetData>,
}

#[cfg(feature = "llvm-backend")]
impl<'ctx> LLVMJitBackend<'ctx> {
    /// Create a new LLVM JIT backend with aggressive optimization (Tier 3)
    pub fn new(context: &'ctx Context) -> Result<Self, String> {
        Self::with_opt_level(context, OptimizationLevel::Aggressive)
    }

    /// Create with custom optimization level
    pub fn with_opt_level(context: &'ctx Context, opt_level: OptimizationLevel) -> Result<Self, String> {
        // Initialize LLVM native target
        Target::initialize_native(&InitializationConfig::default())
            .map_err(|e| format!("Failed to initialize LLVM target: {}", e))?;

        // Link in MCJIT
        ExecutionEngine::link_in_mc_jit();

        // Create module
        let module = context.create_module("rayzor_jit");
        let builder = context.create_builder();

        // Get target data for the native target
        let target_triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&target_triple)
            .map_err(|e| format!("Failed to get target from triple: {}", e))?;
        let target_machine = target.create_target_machine(
            &target_triple,
            TargetMachine::get_host_cpu_name().to_str().unwrap_or("generic"),
            TargetMachine::get_host_cpu_features().to_str().unwrap_or(""),
            opt_level,
            RelocMode::Default,
            CodeModel::Default,
        ).ok_or("Failed to create target machine")?;

        let target_data = target_machine.get_target_data();

        Ok(Self {
            context,
            module,
            builder,
            execution_engine: None,
            value_map: HashMap::new(),
            function_map: HashMap::new(),
            block_map: HashMap::new(),
            phi_map: HashMap::new(),
            function_pointers: HashMap::new(),
            opt_level,
            target_data: Some(target_data),
        })
    }

    /// Get the size of a type in bytes according to the target architecture
    pub fn get_type_size(&self, ty: &IrType) -> Result<u64, String> {
        let llvm_ty = self.translate_type(ty)?;
        if let Some(ref target_data) = self.target_data {
            Ok(target_data.get_store_size(&llvm_ty))
        } else {
            Err("Target data not available".to_string())
        }
    }

    /// Get the alignment of a type in bytes according to the target architecture
    pub fn get_type_alignment(&self, ty: &IrType) -> Result<u32, String> {
        let llvm_ty = self.translate_type(ty)?;
        if let Some(ref target_data) = self.target_data {
            Ok(target_data.get_abi_alignment(&llvm_ty))
        } else {
            Err("Target data not available".to_string())
        }
    }

    /// Get pointer size in bytes for the target architecture
    pub fn get_pointer_size(&self) -> u32 {
        if let Some(ref target_data) = self.target_data {
            target_data.get_pointer_byte_size(None)
        } else {
            8 // Default to 64-bit
        }
    }

    /// Compile a single function (for tiered JIT)
    ///
    /// This is the main entry point for Tier 3 optimization.
    /// Compiles one function at maximum optimization level.
    pub fn compile_single_function(
        &mut self,
        func_id: IrFunctionId,
        function: &IrFunction,
    ) -> Result<(), String> {
        // Declare the function
        let llvm_func = self.declare_function(func_id, function)?;

        // Compile the function body
        self.compile_function_body(func_id, function, llvm_func)?;

        // Create execution engine if not exists
        if self.execution_engine.is_none() {
            let engine = self.module
                .create_jit_execution_engine(self.opt_level)
                .map_err(|e| format!("Failed to create JIT execution engine: {}", e))?;
            self.execution_engine = Some(engine);
        }

        // Get function pointer
        let func_name = Self::function_name(func_id);
        if let Some(ref engine) = self.execution_engine {
            let fn_ptr = engine
                .get_function_address(&func_name)
                .map_err(|e| format!("Failed to get function address for '{}': {:?}", func_name, e))?;

            self.function_pointers.insert(func_id, fn_ptr as usize);
        }

        Ok(())
    }

    /// Get a compiled function pointer
    pub fn get_function_ptr(&self, func_id: IrFunctionId) -> Result<*const u8, String> {
        self.function_pointers
            .get(&func_id)
            .map(|&addr| addr as *const u8)
            .ok_or_else(|| format!("Function {:?} not compiled", func_id))
    }

    /// Generate function name for LLVM
    fn function_name(func_id: IrFunctionId) -> String {
        format!("rayzor_func_{}", func_id.0)
    }

    /// Translate function type signature to LLVM function type
    fn translate_function_type(&self, ty: &IrType) -> Result<inkwell::types::FunctionType<'ctx>, String> {
        match ty {
            IrType::Function { params, return_type, .. } => {
                // Translate parameter types
                let param_types: Result<Vec<BasicMetadataTypeEnum>, _> = params.iter()
                    .map(|param_ty| self.translate_type(param_ty).map(|t| t.into()))
                    .collect();
                let param_types = param_types?;

                // Translate return type
                if **return_type == IrType::Void {
                    Ok(self.context.void_type().fn_type(&param_types, false))
                } else {
                    let ret_ty = self.translate_type(return_type)?;
                    Ok(ret_ty.fn_type(&param_types, false))
                }
            }
            _ => Err(format!("Expected function type, got {:?}", ty))
        }
    }

    /// Translate MIR type to LLVM type
    fn translate_type(&self, ty: &IrType) -> Result<BasicTypeEnum<'ctx>, String> {
        match ty {
            IrType::Void => Err("Void type cannot be used as BasicType".to_string()),
            IrType::Bool | IrType::I8 | IrType::U8 => Ok(self.context.i8_type().into()),
            IrType::I16 | IrType::U16 => Ok(self.context.i16_type().into()),
            IrType::I32 | IrType::U32 => Ok(self.context.i32_type().into()),
            IrType::I64 | IrType::U64 => Ok(self.context.i64_type().into()),
            IrType::F32 => Ok(self.context.f32_type().into()),
            IrType::F64 => Ok(self.context.f64_type().into()),

            // Pointers become i8* in LLVM (opaque pointers)
            IrType::Ptr(_) | IrType::Ref(_) => {
                Ok(self.context.i8_type().ptr_type(AddressSpace::default()).into())
            }

            // Arrays
            IrType::Array(elem_ty, count) => {
                let elem_llvm_ty = self.translate_type(elem_ty)?;
                Ok(elem_llvm_ty.array_type(*count as u32).into())
            }

            // Slices are represented as {ptr, len}
            IrType::Slice(_) | IrType::String => {
                let ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
                let len_ty = self.context.i64_type();
                Ok(self.context.struct_type(&[ptr_ty.into(), len_ty.into()], false).into())
            }

            // Functions become function pointers
            IrType::Function { .. } => {
                Ok(self.context.i8_type().ptr_type(AddressSpace::default()).into())
            }

            // Structs
            IrType::Struct { fields, .. } => {
                let field_types: Result<Vec<_>, _> = fields
                    .iter()
                    .map(|f| self.translate_type(&f.ty))
                    .collect();
                let field_types = field_types?;
                Ok(self.context.struct_type(&field_types, false).into())
            }

            // Unions are represented as a struct with a tag + largest variant
            IrType::Union { variants, .. } => {
                let tag_ty = self.context.i32_type();

                // Find largest variant size
                let mut max_size = 0usize;
                for variant in variants {
                    let size: usize = variant.fields.iter()
                        .map(|f| f.size())
                        .sum();
                    max_size = max_size.max(size);
                }

                // Create union as {i32 tag, [i8 x max_size]}
                let data_ty = self.context.i8_type().array_type(max_size as u32);
                Ok(self.context.struct_type(&[tag_ty.into(), data_ty.into()], false).into())
            }

            IrType::Opaque { size, .. } => {
                // Opaque types become byte arrays
                Ok(self.context.i8_type().array_type(*size as u32).into())
            }

            IrType::Any => {
                // Any type is {i64 type_id, i8* value_ptr}
                let type_id = self.context.i64_type();
                let value_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
                Ok(self.context.struct_type(&[type_id.into(), value_ptr.into()], false).into())
            }

            IrType::TypeVar(_) => Err("Type variables should be monomorphized before codegen".to_string()),
        }
    }

    /// Declare a function signature
    fn declare_function(
        &mut self,
        func_id: IrFunctionId,
        function: &IrFunction,
    ) -> Result<FunctionValue<'ctx>, String> {
        // Translate parameter types
        let param_types: Result<Vec<BasicMetadataTypeEnum>, _> = function
            .signature
            .parameters
            .iter()
            .map(|param| self.translate_type(&param.ty).map(|t| t.into()))
            .collect();
        let param_types = param_types?;

        // Translate return type
        let fn_type = if function.signature.return_type == IrType::Void {
            // Void function
            self.context.void_type().fn_type(&param_types, false)
        } else {
            // Function with return value
            let return_type = self.translate_type(&function.signature.return_type)?;
            return_type.fn_type(&param_types, false)
        };

        let func_name = Self::function_name(func_id);
        let llvm_func = self.module.add_function(&func_name, fn_type, None);

        self.function_map.insert(func_id, llvm_func);
        Ok(llvm_func)
    }

    /// Compile function body
    fn compile_function_body(
        &mut self,
        func_id: IrFunctionId,
        function: &IrFunction,
        llvm_func: FunctionValue<'ctx>,
    ) -> Result<(), String> {
        // Clear previous compilation state
        self.value_map.clear();
        self.block_map.clear();
        self.phi_map.clear();

        // Map function parameters to LLVM values
        for (i, param) in llvm_func.get_param_iter().enumerate() {
            if i < function.signature.parameters.len() {
                let param_id = IrId::new(i as u32);
                self.value_map.insert(param_id, param);
            }
        }

        // Create LLVM basic blocks for all MIR blocks
        for (block_id, _) in &function.cfg.blocks {
            let block_name = format!("bb{}", block_id.as_u32());
            let llvm_block = self.context.append_basic_block(llvm_func, &block_name);
            self.block_map.insert(*block_id, llvm_block);
        }

        // Pass 1: Create all phi nodes (without incoming values)
        for (block_id, mir_block) in &function.cfg.blocks {
            let llvm_block = self.block_map[block_id];
            self.builder.position_at_end(llvm_block);

            for phi in &mir_block.phi_nodes {
                self.create_phi_node(phi)?;
            }
        }

        // Pass 2: Compile all blocks (instructions and terminators)
        for (block_id, mir_block) in &function.cfg.blocks {
            let llvm_block = self.block_map[block_id];
            self.builder.position_at_end(llvm_block);

            // Compile instructions
            for instruction in &mir_block.instructions {
                self.compile_instruction(instruction)?;
            }

            // Compile terminator
            self.compile_terminator(&mir_block.terminator)?;
        }

        // Pass 3: Fill in phi node incoming values
        for (block_id, mir_block) in &function.cfg.blocks {
            for phi in &mir_block.phi_nodes {
                self.fill_phi_incoming(phi)?;
            }
        }

        Ok(())
    }

    /// Create a phi node (without incoming values)
    fn create_phi_node(&mut self, phi: &IrPhiNode) -> Result<(), String> {
        let phi_ty = self.translate_type(&phi.ty)?;
        let llvm_phi = self.builder.build_phi(phi_ty, &format!("phi_{}", phi.dest.as_u32()))
            .map_err(|e| format!("Failed to build phi: {}", e))?;

        // Store the phi node for later filling
        self.phi_map.insert(phi.dest, llvm_phi);

        // Also add to value map so it can be used by instructions
        self.value_map.insert(phi.dest, llvm_phi.as_basic_value());
        Ok(())
    }

    /// Fill in phi node incoming values (after all blocks are compiled)
    fn fill_phi_incoming(&mut self, phi: &IrPhiNode) -> Result<(), String> {
        let llvm_phi = self.phi_map.get(&phi.dest)
            .ok_or_else(|| format!("Phi node {:?} not found", phi.dest))?;

        // Add incoming values
        for (block_id, value_id) in &phi.incoming {
            let llvm_block = self.block_map.get(block_id)
                .ok_or_else(|| format!("Block {:?} not found for phi", block_id))?;
            let llvm_value = self.value_map.get(value_id)
                .ok_or_else(|| format!("Value {:?} not found in value map for phi incoming", value_id))?;

            llvm_phi.add_incoming(&[(llvm_value, *llvm_block)]);
        }

        Ok(())
    }

    /// Compile a single MIR instruction to LLVM IR
    fn compile_instruction(&mut self, inst: &IrInstruction) -> Result<(), String> {
        match inst {
            IrInstruction::Const { dest, value } => {
                let llvm_value = self.compile_constant(value)?;
                self.value_map.insert(*dest, llvm_value);
            }

            IrInstruction::Copy { dest, src } => {
                let src_value = self.get_value(*src)?;
                self.value_map.insert(*dest, src_value);
            }

            IrInstruction::Load { dest, ptr, ty } => {
                let ptr_value = self.get_value(*ptr)?;
                let ptr = ptr_value.into_pointer_value();
                let load_ty = self.translate_type(ty)?;

                let loaded = self.builder.build_load(load_ty, ptr, &format!("load_{}", dest.as_u32()))
                    .map_err(|e| format!("Failed to build load: {}", e))?;
                self.value_map.insert(*dest, loaded);
            }

            IrInstruction::Store { ptr, value } => {
                let ptr_val = self.get_value(*ptr)?.into_pointer_value();
                let value_val = self.get_value(*value)?;
                self.builder.build_store(ptr_val, value_val)
                    .map_err(|e| format!("Failed to build store: {}", e))?;
            }

            IrInstruction::BinOp { dest, op, left, right } => {
                let left_val = self.get_value(*left)?;
                let right_val = self.get_value(*right)?;
                let result = self.compile_binop(*op, left_val, right_val, *dest)?;
                self.value_map.insert(*dest, result);
            }

            IrInstruction::UnOp { dest, op, operand } => {
                let operand_val = self.get_value(*operand)?;
                let result = self.compile_unop(*op, operand_val, *dest)?;
                self.value_map.insert(*dest, result);
            }

            IrInstruction::Cmp { dest, op, left, right } => {
                let left_val = self.get_value(*left)?;
                let right_val = self.get_value(*right)?;
                let result = self.compile_compare(*op, left_val, right_val, *dest)?;
                self.value_map.insert(*dest, result);
            }

            IrInstruction::CallDirect { dest, func_id, args, arg_ownership: _, type_args: _ } => {
                // Note: type_args are handled by monomorphization pass before codegen
                let result = self.compile_direct_call(*func_id, args)?;
                if let Some(dest) = dest {
                    if let Some(result_val) = result {
                        self.value_map.insert(*dest, result_val);
                    }
                }
            }

            IrInstruction::Select { dest, condition, true_val, false_val } => {
                let cond = self.get_value(*condition)?.into_int_value();
                let true_v = self.get_value(*true_val)?;
                let false_v = self.get_value(*false_val)?;

                let result = self.builder.build_select(cond, true_v, false_v, &format!("select_{}", dest.as_u32()))
                    .map_err(|e| format!("Failed to build select: {}", e))?;
                self.value_map.insert(*dest, result);
            }

            IrInstruction::Alloc { dest, ty, count } => {
                let alloc_ty = self.translate_type(ty)?;
                let ptr = if let Some(count_id) = count {
                    let count_val = self.get_value(*count_id)?.into_int_value();
                    self.builder.build_array_alloca(alloc_ty, count_val, &format!("alloca_{}", dest.as_u32()))
                        .map_err(|e| format!("Failed to build array alloca: {}", e))?
                } else {
                    self.builder.build_alloca(alloc_ty, &format!("alloca_{}", dest.as_u32()))
                        .map_err(|e| format!("Failed to build alloca: {}", e))?
                };
                self.value_map.insert(*dest, ptr.into());
            }

            IrInstruction::GetElementPtr { dest, ptr, indices, .. } => {
                let ptr_val = self.get_value(*ptr)?.into_pointer_value();

                // Convert indices to LLVM values
                let index_vals: Result<Vec<_>, _> = indices.iter()
                    .map(|&id| self.get_value(id).map(|v| v.into_int_value()))
                    .collect();
                let index_vals = index_vals?;

                unsafe {
                    let gep = self.builder.build_gep(
                        self.context.i8_type(),
                        ptr_val,
                        &index_vals,
                        &format!("gep_{}", dest.as_u32())
                    ).map_err(|e| format!("Failed to build GEP: {}", e))?;
                    self.value_map.insert(*dest, gep.into());
                }
            }

            IrInstruction::Cast { dest, src, from_ty, to_ty } => {
                let src_val = self.get_value(*src)?;
                let result = self.compile_cast(src_val, from_ty, to_ty, *dest)?;
                self.value_map.insert(*dest, result);
            }

            IrInstruction::BitCast { dest, src, ty } => {
                let src_val = self.get_value(*src)?;
                let target_ty = self.translate_type(ty)?;

                let result = if src_val.is_int_value() {
                    self.builder.build_bitcast(src_val.into_int_value(), target_ty, &format!("bitcast_{}", dest.as_u32()))
                } else if src_val.is_float_value() {
                    self.builder.build_bitcast(src_val.into_float_value(), target_ty, &format!("bitcast_{}", dest.as_u32()))
                } else if src_val.is_pointer_value() {
                    self.builder.build_bitcast(src_val.into_pointer_value(), target_ty, &format!("bitcast_{}", dest.as_u32()))
                } else {
                    return Err("Unsupported bitcast type".to_string());
                }.map_err(|e| format!("Failed to build bitcast: {}", e))?;

                self.value_map.insert(*dest, result);
            }

            IrInstruction::CallIndirect { dest, func_ptr, args, signature } => {
                let func_ptr_val = self.get_value(*func_ptr)?.into_pointer_value();

                // Get argument values
                let arg_values: Result<Vec<_>, _> = args.iter()
                    .map(|&id| self.get_value(id).map(|v| v.into()))
                    .collect();
                let arg_values = arg_values?;

                // Build indirect call
                let call_site = self.builder.build_indirect_call(
                    self.translate_function_type(signature)?,
                    func_ptr_val,
                    &arg_values,
                    "indirect_call"
                ).map_err(|e| format!("Failed to build indirect call: {}", e))?;

                if let Some(dest) = dest {
                    if let Some(result_val) = call_site.try_as_basic_value().left() {
                        self.value_map.insert(*dest, result_val);
                    }
                }
            }

            IrInstruction::Free { ptr } => {
                let ptr_val = self.get_value(*ptr)?.into_pointer_value();

                // Call free function (requires libc linkage)
                // For now, we'll use LLVM's built-in free
                let free_fn_type = self.context.void_type().fn_type(
                    &[self.context.i8_type().ptr_type(AddressSpace::default()).into()],
                    false
                );
                let free_fn = self.module.add_function("free", free_fn_type, None);

                self.builder.build_call(free_fn, &[ptr_val.into()], "")
                    .map_err(|e| format!("Failed to build free call: {}", e))?;
            }

            IrInstruction::MemCopy { dest, src, size } => {
                let dest_ptr = self.get_value(*dest)?.into_pointer_value();
                let src_ptr = self.get_value(*src)?.into_pointer_value();
                let size_val = self.get_value(*size)?.into_int_value();

                // Use LLVM's memcpy intrinsic with default alignment (1 byte for i8*)
                self.builder.build_memcpy(
                    dest_ptr,
                    1, // alignment for i8* (can be optimized by LLVM)
                    src_ptr,
                    1, // alignment
                    size_val
                ).map_err(|e| format!("Failed to build memcpy: {}", e))?;
            }

            IrInstruction::MemSet { dest, value, size } => {
                let dest_ptr = self.get_value(*dest)?.into_pointer_value();
                let value_val = self.get_value(*value)?.into_int_value();
                let size_val = self.get_value(*size)?.into_int_value();

                // Use LLVM's memset intrinsic with default alignment
                self.builder.build_memset(
                    dest_ptr,
                    1, // alignment for i8* (can be optimized by LLVM)
                    value_val,
                    size_val
                ).map_err(|e| format!("Failed to build memset: {}", e))?;
            }
            IrInstruction::Throw { .. } => {
                return Err("Throw not yet implemented".to_string());
            }
            IrInstruction::LandingPad { .. } => {
                return Err("LandingPad not yet implemented".to_string());
            }
            IrInstruction::Resume { .. } => {
                return Err("Resume not yet implemented".to_string());
            }
            IrInstruction::ExtractValue { dest, aggregate, indices } => {
                let agg_val = self.get_value(*aggregate)?;

                let result = if agg_val.is_struct_value() {
                    self.builder.build_extract_value(
                        agg_val.into_struct_value(),
                        indices[0],
                        &format!("extract_{}", dest.as_u32())
                    ).map_err(|e| format!("Failed to build extract_value: {}", e))?
                } else if agg_val.is_array_value() {
                    self.builder.build_extract_value(
                        agg_val.into_array_value(),
                        indices[0],
                        &format!("extract_{}", dest.as_u32())
                    ).map_err(|e| format!("Failed to build extract_value: {}", e))?
                } else {
                    return Err("ExtractValue only works on struct or array values".to_string());
                };

                self.value_map.insert(*dest, result);
            }

            IrInstruction::InsertValue { dest, aggregate, value, indices } => {
                let agg_val = self.get_value(*aggregate)?;
                let insert_val = self.get_value(*value)?;

                let result = if agg_val.is_struct_value() {
                    let struct_val = self.builder.build_insert_value(
                        agg_val.into_struct_value(),
                        insert_val,
                        indices[0],
                        &format!("insert_{}", dest.as_u32())
                    ).map_err(|e| format!("Failed to build insert_value: {}", e))?;
                    struct_val.as_basic_value_enum()
                } else if agg_val.is_array_value() {
                    let array_val = self.builder.build_insert_value(
                        agg_val.into_array_value(),
                        insert_val,
                        indices[0],
                        &format!("insert_{}", dest.as_u32())
                    ).map_err(|e| format!("Failed to build insert_value: {}", e))?;
                    array_val.as_basic_value_enum()
                } else {
                    return Err("InsertValue only works on struct or array values".to_string());
                };

                self.value_map.insert(*dest, result);
            }
            IrInstruction::MakeClosure { .. } => {
                return Err("MakeClosure not yet implemented".to_string());
            }
            IrInstruction::ClosureFunc { .. } => {
                return Err("ClosureFunc not yet implemented".to_string());
            }
            IrInstruction::ClosureEnv { .. } => {
                return Err("ClosureEnv not yet implemented".to_string());
            }
            IrInstruction::DebugLoc { .. } => {
                // Debug locations are metadata, skip for now
            }
            IrInstruction::InlineAsm { .. } => {
                return Err("InlineAsm not yet implemented".to_string());
            }

            // Control flow is handled by terminators, not regular instructions
            IrInstruction::Jump { .. } |
            IrInstruction::Branch { .. } |
            IrInstruction::Switch { .. } |
            IrInstruction::Return { .. } => {
                return Err("Control flow instructions should be terminators".to_string());
            }

            IrInstruction::Phi { .. } => {
                return Err("Phi nodes should be in phi_nodes list".to_string());
            }
        }

        Ok(())
    }

    /// Compile a terminator instruction
    fn compile_terminator(&mut self, term: &IrTerminator) -> Result<(), String> {
        match term {
            IrTerminator::Return { value } => {
                if let Some(val_id) = value {
                    let return_val = self.get_value(*val_id)?;
                    self.builder.build_return(Some(&return_val))
                        .map_err(|e| format!("Failed to build return: {}", e))?;
                } else {
                    self.builder.build_return(None)
                        .map_err(|e| format!("Failed to build void return: {}", e))?;
                }
            }

            IrTerminator::Branch { target } => {
                let target_block = self.block_map.get(target)
                    .ok_or_else(|| format!("Target block {:?} not found", target))?;
                self.builder.build_unconditional_branch(*target_block)
                    .map_err(|e| format!("Failed to build branch: {}", e))?;
            }

            IrTerminator::CondBranch { condition, true_target, false_target } => {
                let cond_val = self.get_value(*condition)?.into_int_value();
                let true_block = self.block_map.get(true_target)
                    .ok_or_else(|| format!("True target block {:?} not found", true_target))?;
                let false_block = self.block_map.get(false_target)
                    .ok_or_else(|| format!("False target block {:?} not found", false_target))?;

                self.builder.build_conditional_branch(cond_val, *true_block, *false_block)
                    .map_err(|e| format!("Failed to build conditional branch: {}", e))?;
            }

            IrTerminator::Switch { value, cases, default } => {
                let switch_val = self.get_value(*value)?.into_int_value();
                let default_block = self.block_map.get(default)
                    .ok_or_else(|| format!("Default block {:?} not found", default))?;

                // Build the cases vector for LLVM
                let llvm_cases: Result<Vec<_>, String> = cases.iter()
                    .map(|(case_val, case_target)| -> Result<_, String> {
                        let case_block = self.block_map.get(case_target)
                            .ok_or_else(|| format!("Case target block {:?} not found", case_target))?;
                        let const_val = self.context.i64_type().const_int(*case_val as u64, false);
                        Ok((const_val, *case_block))
                    })
                    .collect();
                let llvm_cases = llvm_cases?;

                self.builder.build_switch(switch_val, *default_block, &llvm_cases)
                    .map_err(|e| format!("Failed to build switch: {}", e))?;
            }

            IrTerminator::Unreachable => {
                self.builder.build_unreachable()
                    .map_err(|e| format!("Failed to build unreachable: {}", e))?;
            }

            IrTerminator::NoReturn { .. } => {
                self.builder.build_unreachable()
                    .map_err(|e| format!("Failed to build unreachable (no return): {}", e))?;
            }
        }

        Ok(())
    }

    /// Get an LLVM value from the value map
    fn get_value(&self, id: IrId) -> Result<BasicValueEnum<'ctx>, String> {
        self.value_map.get(&id)
            .copied()
            .ok_or_else(|| format!("Value {:?} not found in value map", id))
    }

    /// Compile a constant value
    fn compile_constant(&self, value: &IrValue) -> Result<BasicValueEnum<'ctx>, String> {
        match value {
            IrValue::Void | IrValue::Undef => {
                Err("Cannot compile void/undef as value".to_string())
            }
            IrValue::Null => {
                Ok(self.context.i8_type().ptr_type(AddressSpace::default()).const_null().into())
            }
            IrValue::Bool(b) => {
                Ok(self.context.bool_type().const_int(*b as u64, false).into())
            }
            IrValue::I8(v) => Ok(self.context.i8_type().const_int(*v as u64, true).into()),
            IrValue::I16(v) => Ok(self.context.i16_type().const_int(*v as u64, true).into()),
            IrValue::I32(v) => Ok(self.context.i32_type().const_int(*v as u64, true).into()),
            IrValue::I64(v) => Ok(self.context.i64_type().const_int(*v as u64, true).into()),
            IrValue::U8(v) => Ok(self.context.i8_type().const_int(*v as u64, false).into()),
            IrValue::U16(v) => Ok(self.context.i16_type().const_int(*v as u64, false).into()),
            IrValue::U32(v) => Ok(self.context.i32_type().const_int(*v as u64, false).into()),
            IrValue::U64(v) => Ok(self.context.i64_type().const_int(*v, false).into()),
            IrValue::F32(v) => Ok(self.context.f32_type().const_float(*v as f64).into()),
            IrValue::F64(v) => Ok(self.context.f64_type().const_float(*v).into()),
            IrValue::String(s) => {
                let global_str = self.builder.build_global_string_ptr(s, "str")
                    .map_err(|e| format!("Failed to build global string: {}", e))?;
                Ok(global_str.as_pointer_value().into())
            }
            IrValue::Array(_) | IrValue::Struct(_) | IrValue::Function(_) | IrValue::Closure { .. } => {
                Err("Complex constant values not yet supported".to_string())
            }
        }
    }

    /// Compile binary operation
    fn compile_binop(
        &self,
        op: BinaryOp,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
        dest: IrId,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let name = format!("binop_{}", dest.as_u32());

        match op {
            // Integer arithmetic
            BinaryOp::Add => {
                let result = self.builder.build_int_add(
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build add: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::Sub => {
                let result = self.builder.build_int_sub(
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build sub: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::Mul => {
                let result = self.builder.build_int_mul(
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build mul: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::Div => {
                let result = self.builder.build_int_signed_div(
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build div: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::Rem => {
                let result = self.builder.build_int_signed_rem(
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build rem: {}", e))?;
                Ok(result.into())
            }

            // Bitwise operations
            BinaryOp::And => {
                let result = self.builder.build_and(
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build and: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::Or => {
                let result = self.builder.build_or(
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build or: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::Xor => {
                let result = self.builder.build_xor(
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build xor: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::Shl => {
                let result = self.builder.build_left_shift(
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build shl: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::Shr => {
                let result = self.builder.build_right_shift(
                    left.into_int_value(),
                    right.into_int_value(),
                    true, // arithmetic shift
                    &name
                ).map_err(|e| format!("Failed to build shr: {}", e))?;
                Ok(result.into())
            }

            // Float arithmetic
            BinaryOp::FAdd => {
                let result = self.builder.build_float_add(
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build fadd: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::FSub => {
                let result = self.builder.build_float_sub(
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build fsub: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::FMul => {
                let result = self.builder.build_float_mul(
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build fmul: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::FDiv => {
                let result = self.builder.build_float_div(
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build fdiv: {}", e))?;
                Ok(result.into())
            }
            BinaryOp::FRem => {
                let result = self.builder.build_float_rem(
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build frem: {}", e))?;
                Ok(result.into())
            }
        }
    }

    /// Compile unary operation
    fn compile_unop(
        &self,
        op: UnaryOp,
        operand: BasicValueEnum<'ctx>,
        dest: IrId,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let name = format!("unop_{}", dest.as_u32());

        match op {
            UnaryOp::Neg => {
                let result = self.builder.build_int_neg(operand.into_int_value(), &name)
                    .map_err(|e| format!("Failed to build neg: {}", e))?;
                Ok(result.into())
            }
            UnaryOp::Not => {
                let result = self.builder.build_not(operand.into_int_value(), &name)
                    .map_err(|e| format!("Failed to build not: {}", e))?;
                Ok(result.into())
            }
            UnaryOp::FNeg => {
                let result = self.builder.build_float_neg(operand.into_float_value(), &name)
                    .map_err(|e| format!("Failed to build fneg: {}", e))?;
                Ok(result.into())
            }
        }
    }

    /// Compile comparison operation
    fn compile_compare(
        &self,
        op: CompareOp,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
        dest: IrId,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let name = format!("cmp_{}", dest.as_u32());

        match op {
            // Integer comparisons
            CompareOp::Eq => {
                let result = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build eq: {}", e))?;
                Ok(result.into())
            }
            CompareOp::Ne => {
                let result = self.builder.build_int_compare(
                    IntPredicate::NE,
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build ne: {}", e))?;
                Ok(result.into())
            }
            CompareOp::Lt => {
                let result = self.builder.build_int_compare(
                    IntPredicate::SLT,
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build lt: {}", e))?;
                Ok(result.into())
            }
            CompareOp::Le => {
                let result = self.builder.build_int_compare(
                    IntPredicate::SLE,
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build le: {}", e))?;
                Ok(result.into())
            }
            CompareOp::Gt => {
                let result = self.builder.build_int_compare(
                    IntPredicate::SGT,
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build gt: {}", e))?;
                Ok(result.into())
            }
            CompareOp::Ge => {
                let result = self.builder.build_int_compare(
                    IntPredicate::SGE,
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build ge: {}", e))?;
                Ok(result.into())
            }

            // Unsigned comparisons
            CompareOp::ULt => {
                let result = self.builder.build_int_compare(
                    IntPredicate::ULT,
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build ult: {}", e))?;
                Ok(result.into())
            }
            CompareOp::ULe => {
                let result = self.builder.build_int_compare(
                    IntPredicate::ULE,
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build ule: {}", e))?;
                Ok(result.into())
            }
            CompareOp::UGt => {
                let result = self.builder.build_int_compare(
                    IntPredicate::UGT,
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build ugt: {}", e))?;
                Ok(result.into())
            }
            CompareOp::UGe => {
                let result = self.builder.build_int_compare(
                    IntPredicate::UGE,
                    left.into_int_value(),
                    right.into_int_value(),
                    &name
                ).map_err(|e| format!("Failed to build uge: {}", e))?;
                Ok(result.into())
            }

            // Float comparisons (ordered)
            CompareOp::FEq => {
                let result = self.builder.build_float_compare(
                    FloatPredicate::OEQ,
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build feq: {}", e))?;
                Ok(result.into())
            }
            CompareOp::FNe => {
                let result = self.builder.build_float_compare(
                    FloatPredicate::ONE,
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build fne: {}", e))?;
                Ok(result.into())
            }
            CompareOp::FLt => {
                let result = self.builder.build_float_compare(
                    FloatPredicate::OLT,
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build flt: {}", e))?;
                Ok(result.into())
            }
            CompareOp::FLe => {
                let result = self.builder.build_float_compare(
                    FloatPredicate::OLE,
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build fle: {}", e))?;
                Ok(result.into())
            }
            CompareOp::FGt => {
                let result = self.builder.build_float_compare(
                    FloatPredicate::OGT,
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build fgt: {}", e))?;
                Ok(result.into())
            }
            CompareOp::FGe => {
                let result = self.builder.build_float_compare(
                    FloatPredicate::OGE,
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build fge: {}", e))?;
                Ok(result.into())
            }

            // Ordered/Unordered comparisons
            CompareOp::FOrd => {
                let result = self.builder.build_float_compare(
                    FloatPredicate::ORD,
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build ford: {}", e))?;
                Ok(result.into())
            }
            CompareOp::FUno => {
                let result = self.builder.build_float_compare(
                    FloatPredicate::UNO,
                    left.into_float_value(),
                    right.into_float_value(),
                    &name
                ).map_err(|e| format!("Failed to build funo: {}", e))?;
                Ok(result.into())
            }
        }
    }

    /// Compile a direct function call
    fn compile_direct_call(
        &mut self,
        func_id: IrFunctionId,
        args: &[IrId],
    ) -> Result<Option<BasicValueEnum<'ctx>>, String> {
        let llvm_func = self.function_map.get(&func_id)
            .ok_or_else(|| format!("Function {:?} not found", func_id))?;

        // Get argument values
        let arg_values: Result<Vec<_>, _> = args.iter()
            .map(|&id| self.get_value(id).map(|v| v.into()))
            .collect();
        let arg_values = arg_values?;

        let call_site = self.builder.build_call(*llvm_func, &arg_values, "call")
            .map_err(|e| format!("Failed to build call: {}", e))?;

        Ok(call_site.try_as_basic_value().left())
    }

    /// Compile type cast
    fn compile_cast(
        &self,
        src: BasicValueEnum<'ctx>,
        from_ty: &IrType,
        to_ty: &IrType,
        dest: IrId,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let name = format!("cast_{}", dest.as_u32());
        let target_llvm_ty = self.translate_type(to_ty)?;

        // Integer to integer casts
        if from_ty.is_integer() && to_ty.is_integer() {
            let src_int = src.into_int_value();
            let target_int_ty = target_llvm_ty.into_int_type();

            let result = if from_ty.size() < to_ty.size() {
                // Extend
                if from_ty.is_signed_integer() {
                    self.builder.build_int_s_extend(src_int, target_int_ty, &name)
                } else {
                    self.builder.build_int_z_extend(src_int, target_int_ty, &name)
                }
            } else {
                // Truncate
                self.builder.build_int_truncate(src_int, target_int_ty, &name)
            }.map_err(|e| format!("Failed to build int cast: {}", e))?;

            return Ok(result.into());
        }

        // Float to float casts
        if from_ty.is_float() && to_ty.is_float() {
            let src_float = src.into_float_value();
            let target_float_ty = target_llvm_ty.into_float_type();

            let result = if from_ty.size() < to_ty.size() {
                self.builder.build_float_ext(src_float, target_float_ty, &name)
            } else {
                self.builder.build_float_trunc(src_float, target_float_ty, &name)
            }.map_err(|e| format!("Failed to build float cast: {}", e))?;

            return Ok(result.into());
        }

        // Int to float
        if from_ty.is_integer() && to_ty.is_float() {
            let src_int = src.into_int_value();
            let target_float_ty = target_llvm_ty.into_float_type();

            let result = if from_ty.is_signed_integer() {
                self.builder.build_signed_int_to_float(src_int, target_float_ty, &name)
            } else {
                self.builder.build_unsigned_int_to_float(src_int, target_float_ty, &name)
            }.map_err(|e| format!("Failed to build int to float: {}", e))?;

            return Ok(result.into());
        }

        // Float to int
        if from_ty.is_float() && to_ty.is_integer() {
            let src_float = src.into_float_value();
            let target_int_ty = target_llvm_ty.into_int_type();

            let result = if to_ty.is_signed_integer() {
                self.builder.build_float_to_signed_int(src_float, target_int_ty, &name)
            } else {
                self.builder.build_float_to_unsigned_int(src_float, target_int_ty, &name)
            }.map_err(|e| format!("Failed to build float to int: {}", e))?;

            return Ok(result.into());
        }

        // Pointer casts
        if from_ty.is_pointer() && to_ty.is_pointer() {
            let src_ptr = src.into_pointer_value();
            let target_ptr_ty = target_llvm_ty.into_pointer_type();

            let result = self.builder.build_pointer_cast(src_ptr, target_ptr_ty, &name)
                .map_err(|e| format!("Failed to build pointer cast: {}", e))?;

            return Ok(result.into());
        }

        Err(format!("Unsupported cast from {:?} to {:?}", from_ty, to_ty))
    }
}

// Stub implementation when LLVM backend is disabled
#[cfg(not(feature = "llvm-backend"))]
pub struct LLVMJitBackend {
    _phantom: std::marker::PhantomData<()>,
}

#[cfg(not(feature = "llvm-backend"))]
impl LLVMJitBackend {
    pub fn new(_context: &()) -> Result<Self, String> {
        Err("LLVM backend not enabled. Compile with --features llvm-backend".to_string())
    }

    pub fn compile_single_function(&mut self, _func_id: crate::ir::IrFunctionId, _function: &crate::ir::IrFunction) -> Result<(), String> {
        Err("LLVM backend not enabled".to_string())
    }

    pub fn get_function_ptr(&self, _func_id: crate::ir::IrFunctionId) -> Result<*const u8, String> {
        Err("LLVM backend not enabled".to_string())
    }
}
