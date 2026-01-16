//! MIR Register-Based Interpreter
//!
//! Provides instant startup by directly interpreting MIR without compilation.
//! Performance: ~5-10x native speed (suitable for cold paths and development)
//!
//! ## Design
//! - **Register-based execution** (not stack-based) - matches MIR's SSA form
//! - Direct mapping from IrId to interpreter registers
//! - Support for all MIR instructions
//! - FFI calls to runtime functions
//! - GC-safe (no raw pointers in interpreter state)
//!
//! ## Why Register-Based?
//! 1. MIR is already in SSA form with explicit registers (IrId)
//! 2. ~30% faster than stack-based (see Lua 5.x vs 4.x benchmarks)
//! 3. Fewer instructions needed (no push/pop overhead)
//! 4. Direct 1:1 mapping from MIR to interpreter state

use std::collections::HashMap;
use crate::ir::{
    IrModule, IrFunction, IrFunctionId, IrInstruction, IrValue, IrType,
    IrBasicBlock, IrBlockId, IrTerminator, IrId,
    BinaryOp, UnaryOp, CompareOp, IrFunctionSignature, IrExternFunction,
    FunctionKind,
};

/// MIR interpreter value (boxed for GC safety)
#[derive(Clone, Debug)]
pub enum InterpValue {
    Void,
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Ptr(usize),        // Raw pointer (for FFI)
    Null,
    /// String value (owned)
    String(String),
    /// Array value
    Array(Vec<InterpValue>),
    /// Struct value (fields by index)
    Struct(Vec<InterpValue>),
    /// Function reference
    Function(IrFunctionId),
}

impl Default for InterpValue {
    fn default() -> Self {
        InterpValue::Void
    }
}

impl InterpValue {
    /// Convert to boolean (for conditionals)
    pub fn to_bool(&self) -> Result<bool, InterpError> {
        match self {
            InterpValue::Bool(b) => Ok(*b),
            InterpValue::I32(n) => Ok(*n != 0),
            InterpValue::I64(n) => Ok(*n != 0),
            InterpValue::Ptr(p) => Ok(*p != 0),
            InterpValue::Null => Ok(false),
            _ => Err(InterpError::TypeError(format!(
                "Cannot convert {:?} to bool",
                self
            ))),
        }
    }

    /// Convert to i64 (for integer operations)
    pub fn to_i64(&self) -> Result<i64, InterpError> {
        match self {
            InterpValue::I8(n) => Ok(*n as i64),
            InterpValue::I16(n) => Ok(*n as i64),
            InterpValue::I32(n) => Ok(*n as i64),
            InterpValue::I64(n) => Ok(*n),
            InterpValue::U8(n) => Ok(*n as i64),
            InterpValue::U16(n) => Ok(*n as i64),
            InterpValue::U32(n) => Ok(*n as i64),
            InterpValue::U64(n) => Ok(*n as i64),
            InterpValue::Bool(b) => Ok(if *b { 1 } else { 0 }),
            InterpValue::Ptr(p) => Ok(*p as i64),
            _ => Err(InterpError::TypeError(format!(
                "Cannot convert {:?} to i64",
                self
            ))),
        }
    }

    /// Convert to f64 (for floating point operations)
    pub fn to_f64(&self) -> Result<f64, InterpError> {
        match self {
            InterpValue::F32(n) => Ok(*n as f64),
            InterpValue::F64(n) => Ok(*n),
            InterpValue::I32(n) => Ok(*n as f64),
            InterpValue::I64(n) => Ok(*n as f64),
            _ => Err(InterpError::TypeError(format!(
                "Cannot convert {:?} to f64",
                self
            ))),
        }
    }

    /// Convert to usize (for pointer operations)
    pub fn to_usize(&self) -> Result<usize, InterpError> {
        match self {
            InterpValue::Ptr(p) => Ok(*p),
            InterpValue::I64(n) => Ok(*n as usize),
            InterpValue::U64(n) => Ok(*n as usize),
            InterpValue::I32(n) => Ok(*n as usize),
            InterpValue::U32(n) => Ok(*n as usize),
            InterpValue::Null => Ok(0),
            _ => Err(InterpError::TypeError(format!(
                "Cannot convert {:?} to usize",
                self
            ))),
        }
    }
}

/// Register file for a single function execution frame
/// Uses a Vec for O(1) register access (IrId.0 is the index)
#[derive(Debug)]
struct RegisterFile {
    /// Register values indexed by IrId.as_u32()
    /// Pre-allocated to function's max register count for speed
    registers: Vec<InterpValue>,
}

impl RegisterFile {
    fn new(register_count: usize) -> Self {
        Self {
            registers: vec![InterpValue::Void; register_count],
        }
    }

    #[inline(always)]
    fn get(&self, reg: IrId) -> &InterpValue {
        &self.registers[reg.as_u32() as usize]
    }

    #[inline(always)]
    fn set(&mut self, reg: IrId, value: InterpValue) {
        let idx = reg.as_u32() as usize;
        if idx >= self.registers.len() {
            self.registers.resize(idx + 1, InterpValue::Void);
        }
        self.registers[idx] = value;
    }
}

/// Interpreter execution frame (one per function call)
#[derive(Debug)]
struct InterpreterFrame {
    function_id: IrFunctionId,
    registers: RegisterFile,       // Register-based storage (fast O(1) access)
    current_block: IrBlockId,
    prev_block: Option<IrBlockId>, // For phi node resolution
}

/// Result of executing a terminator
enum TerminatorResult {
    Continue(IrBlockId),
    Return(InterpValue),
}

/// MIR Register-Based Interpreter
pub struct MirInterpreter {
    /// Runtime function pointers (for FFI calls)
    runtime_symbols: HashMap<String, *const u8>,

    /// Call stack (frames with register files)
    stack: Vec<InterpreterFrame>,

    /// Maximum stack depth (prevent stack overflow)
    max_stack_depth: usize,

    /// Heap memory for allocations (simple bump allocator)
    heap: Vec<u8>,

    /// Next heap allocation offset
    heap_offset: usize,
}

// Safety: MirInterpreter can be sent across threads
// The runtime_symbols are function pointers that remain valid
unsafe impl Send for MirInterpreter {}
unsafe impl Sync for MirInterpreter {}

impl MirInterpreter {
    /// Create a new interpreter
    pub fn new() -> Self {
        Self {
            runtime_symbols: HashMap::new(),
            stack: Vec::new(),
            max_stack_depth: 1000,
            heap: vec![0u8; 1024 * 1024], // 1MB heap
            heap_offset: 0,
        }
    }

    /// Create interpreter with runtime symbols for FFI calls
    pub fn with_symbols(symbols: &[(&str, *const u8)]) -> Self {
        let mut interp = Self::new();
        for (name, ptr) in symbols {
            interp.runtime_symbols.insert(name.to_string(), *ptr);
        }
        interp
    }

    /// Register a runtime symbol
    pub fn register_symbol(&mut self, name: &str, ptr: *const u8) {
        self.runtime_symbols.insert(name.to_string(), ptr);
    }

    /// Calculate the maximum register ID used in a function
    fn calculate_register_count(function: &IrFunction) -> usize {
        let mut max_reg = 0usize;

        // Check parameters
        for param in &function.signature.parameters {
            max_reg = max_reg.max(param.reg.as_u32() as usize + 1);
        }

        // Check all instructions for destination registers
        for block in function.cfg.blocks.values() {
            for instr in &block.instructions {
                if let Some(dest) = instr.dest() {
                    max_reg = max_reg.max(dest.as_u32() as usize + 1);
                }
            }
            // Check phi nodes
            for phi in &block.phi_nodes {
                max_reg = max_reg.max(phi.dest.as_u32() as usize + 1);
            }
        }

        // Add some headroom for temporaries
        max_reg + 16
    }

    /// Execute a function and return the result
    pub fn execute(
        &mut self,
        module: &IrModule,
        func_id: IrFunctionId,
        args: Vec<InterpValue>,
    ) -> Result<InterpValue, InterpError> {
        let function = module
            .functions
            .get(&func_id)
            .ok_or(InterpError::FunctionNotFound(func_id))?;

        // Check stack depth
        if self.stack.len() >= self.max_stack_depth {
            return Err(InterpError::StackOverflow);
        }

        // Pre-calculate register count for efficient allocation
        let register_count = Self::calculate_register_count(function);

        // Create new frame with pre-allocated register file
        let mut frame = InterpreterFrame {
            function_id: func_id,
            registers: RegisterFile::new(register_count),
            current_block: function.cfg.entry_block,
            prev_block: None,
        };

        // Bind arguments to parameter registers (direct register assignment)
        for (i, param) in function.signature.parameters.iter().enumerate() {
            if let Some(arg) = args.get(i) {
                frame.registers.set(param.reg, arg.clone());
            }
        }

        self.stack.push(frame);

        // Execute blocks until return
        let result = self.execute_function(module, function);

        self.stack.pop();
        result
    }

    /// Get the current frame (mutable)
    fn current_frame_mut(&mut self) -> &mut InterpreterFrame {
        self.stack.last_mut().expect("No active frame")
    }

    /// Get the current frame
    fn current_frame(&self) -> &InterpreterFrame {
        self.stack.last().expect("No active frame")
    }

    fn execute_function(
        &mut self,
        module: &IrModule,
        function: &IrFunction,
    ) -> Result<InterpValue, InterpError> {
        loop {
            let block_id = self.current_frame().current_block;
            let block = function
                .cfg
                .blocks
                .get(&block_id)
                .ok_or(InterpError::BlockNotFound(block_id))?;

            // Execute phi nodes first (using prev_block for value selection)
            self.execute_phi_nodes(block)?;

            // Execute instructions
            for instr in &block.instructions {
                self.execute_instruction(module, function, instr)?;
            }

            // Execute terminator
            match self.execute_terminator(module, function, &block.terminator)? {
                TerminatorResult::Continue(next_block) => {
                    let frame = self.current_frame_mut();
                    frame.prev_block = Some(frame.current_block);
                    frame.current_block = next_block;
                }
                TerminatorResult::Return(value) => {
                    return Ok(value);
                }
            }
        }
    }

    /// Execute phi nodes at the beginning of a block
    fn execute_phi_nodes(&mut self, block: &IrBasicBlock) -> Result<(), InterpError> {
        let prev_block = self.current_frame().prev_block;

        // Collect phi values first to avoid interference
        let mut phi_values: Vec<(IrId, InterpValue)> = Vec::new();

        for phi in &block.phi_nodes {
            // Find the value from the previous block
            if let Some(prev) = prev_block {
                for (pred_block, value_reg) in &phi.incoming {
                    if *pred_block == prev {
                        let value = self.current_frame().registers.get(*value_reg).clone();
                        phi_values.push((phi.dest, value));
                        break;
                    }
                }
            }
        }

        // Apply phi values
        for (dest, value) in phi_values {
            self.current_frame_mut().registers.set(dest, value);
        }

        Ok(())
    }

    /// Execute a single instruction using register-based operations
    fn execute_instruction(
        &mut self,
        module: &IrModule,
        function: &IrFunction,
        instr: &IrInstruction,
    ) -> Result<(), InterpError> {
        match instr {
            // === Value Operations ===
            IrInstruction::Const { dest, value } => {
                let val = self.ir_value_to_interp(value)?;
                self.current_frame_mut().registers.set(*dest, val);
            }

            IrInstruction::Copy { dest, src } => {
                let val = self.current_frame().registers.get(*src).clone();
                self.current_frame_mut().registers.set(*dest, val);
            }

            IrInstruction::Move { dest, src } => {
                let val = self.current_frame().registers.get(*src).clone();
                self.current_frame_mut().registers.set(*dest, val);
            }

            // === Arithmetic Operations ===
            IrInstruction::BinOp {
                dest,
                op,
                left,
                right,
            } => {
                let l = self.current_frame().registers.get(*left).clone();
                let r = self.current_frame().registers.get(*right).clone();
                let result = self.eval_binary_op(*op, l, r)?;
                self.current_frame_mut().registers.set(*dest, result);
            }

            IrInstruction::UnOp { dest, op, operand } => {
                let val = self.current_frame().registers.get(*operand).clone();
                let result = self.eval_unary_op(*op, val)?;
                self.current_frame_mut().registers.set(*dest, result);
            }

            IrInstruction::Cmp {
                dest,
                op,
                left,
                right,
            } => {
                let l = self.current_frame().registers.get(*left).clone();
                let r = self.current_frame().registers.get(*right).clone();
                let result = self.eval_compare_op(*op, l, r)?;
                self.current_frame_mut()
                    .registers
                    .set(*dest, InterpValue::Bool(result));
            }

            // === Memory Operations ===
            IrInstruction::Load { dest, ptr, ty } => {
                let ptr_val = self.current_frame().registers.get(*ptr).clone();
                let result = self.load_from_ptr(ptr_val, ty)?;
                self.current_frame_mut().registers.set(*dest, result);
            }

            IrInstruction::Store { ptr, value } => {
                let ptr_val = self.current_frame().registers.get(*ptr).clone();
                let val = self.current_frame().registers.get(*value).clone();
                self.store_to_ptr(ptr_val, val)?;
            }

            IrInstruction::Alloc { dest, ty, count } => {
                let size = ty.size();
                let count_val = if let Some(c) = count {
                    self.current_frame().registers.get(*c).to_i64()? as usize
                } else {
                    1
                };
                let total_size = size * count_val;
                let ptr = self.alloc_heap(total_size)?;
                self.current_frame_mut()
                    .registers
                    .set(*dest, InterpValue::Ptr(ptr));
            }

            IrInstruction::Free { ptr: _ } => {
                // Simple bump allocator doesn't support free
                // In a full implementation, we'd track allocations
            }

            IrInstruction::GetElementPtr {
                dest,
                ptr,
                indices,
                ty,
            } => {
                let base_ptr = self.current_frame().registers.get(*ptr).to_usize()?;
                let mut offset = 0usize;

                // Calculate offset based on indices and type
                for idx in indices {
                    let idx_val = self.current_frame().registers.get(*idx).to_i64()? as usize;
                    offset += idx_val * ty.size();
                }

                self.current_frame_mut()
                    .registers
                    .set(*dest, InterpValue::Ptr(base_ptr + offset));
            }

            IrInstruction::PtrAdd {
                dest,
                ptr,
                offset,
                ty,
            } => {
                let base_ptr = self.current_frame().registers.get(*ptr).to_usize()?;
                let offset_val =
                    self.current_frame().registers.get(*offset).to_i64()? as usize;
                let elem_size = ty.size();
                self.current_frame_mut()
                    .registers
                    .set(*dest, InterpValue::Ptr(base_ptr + offset_val * elem_size));
            }

            // === Function Calls ===
            IrInstruction::CallDirect {
                dest,
                func_id,
                args,
                ..
            } => {
                // Collect argument values
                let arg_values: Vec<InterpValue> = args
                    .iter()
                    .map(|a| self.current_frame().registers.get(*a).clone())
                    .collect();

                // Check if it's a user function or extern
                let result = if let Some(func) = module.functions.get(func_id) {
                    // Check the function kind - ExternC functions need FFI
                    if func.kind == FunctionKind::ExternC {
                        // Extern function - use FFI with the function's signature
                        self.call_ffi_for_function(func, &arg_values)?
                    } else if func.cfg.blocks.is_empty() {
                        // Function with no blocks - try extern_functions or FFI
                        if let Some(extern_fn) = module.extern_functions.get(func_id) {
                            self.call_extern_with_signature(extern_fn, &arg_values)?
                        } else {
                            // Try runtime symbols as fallback
                            self.call_ffi_for_function(func, &arg_values)?
                        }
                    } else {
                        // Regular user function - execute recursively
                        self.execute(module, *func_id, arg_values)?
                    }
                } else if let Some(extern_fn) = module.extern_functions.get(func_id) {
                    // FFI call to extern function with full signature info
                    self.call_extern_with_signature(extern_fn, &arg_values)?
                } else {
                    return Err(InterpError::FunctionNotFound(*func_id));
                };

                if let Some(d) = dest {
                    self.current_frame_mut().registers.set(*d, result);
                }
            }

            IrInstruction::CallIndirect {
                dest,
                func_ptr,
                args,
                signature,
                ..
            } => {
                let ptr_val = self.current_frame().registers.get(*func_ptr).clone();

                // Collect argument values
                let arg_values: Vec<InterpValue> = args
                    .iter()
                    .map(|a| self.current_frame().registers.get(*a).clone())
                    .collect();

                let result = match ptr_val {
                    InterpValue::Function(func_id) => {
                        // Check if it's a user function or extern
                        if module.functions.contains_key(&func_id) {
                            self.execute(module, func_id, arg_values)?
                        } else if let Some(extern_fn) = module.extern_functions.get(&func_id) {
                            self.call_extern_with_signature(extern_fn, &arg_values)?
                        } else {
                            return Err(InterpError::FunctionNotFound(func_id));
                        }
                    }
                    InterpValue::Ptr(ptr) => {
                        // Call through function pointer with signature info
                        if let IrType::Function { params, return_type, .. } = signature {
                            // We have full signature info - use it for proper FFI
                            self.call_ffi_ptr_with_types(ptr, &arg_values, params, return_type)?
                        } else {
                            // Fallback to simple FFI without type info
                            self.call_ffi_ptr_simple(ptr, &arg_values)?
                        }
                    }
                    _ => {
                        return Err(InterpError::TypeError(format!(
                            "Cannot call non-function value: {:?}",
                            ptr_val
                        )));
                    }
                };

                if let Some(d) = dest {
                    self.current_frame_mut().registers.set(*d, result);
                }
            }

            // === Type Operations ===
            IrInstruction::Cast {
                dest,
                src,
                from_ty: _,
                to_ty,
            } => {
                let val = self.current_frame().registers.get(*src).clone();
                let result = self.cast_value(val, to_ty)?;
                self.current_frame_mut().registers.set(*dest, result);
            }

            IrInstruction::BitCast { dest, src, ty: _ } => {
                // Bitcast preserves the bits, just reinterprets the type
                let val = self.current_frame().registers.get(*src).clone();
                self.current_frame_mut().registers.set(*dest, val);
            }

            // === Struct Operations ===
            IrInstruction::CreateStruct { dest, ty: _, fields } => {
                let field_values: Vec<InterpValue> = fields
                    .iter()
                    .map(|f| self.current_frame().registers.get(*f).clone())
                    .collect();
                self.current_frame_mut()
                    .registers
                    .set(*dest, InterpValue::Struct(field_values));
            }

            IrInstruction::ExtractValue {
                dest,
                aggregate,
                indices,
            } => {
                let agg = self.current_frame().registers.get(*aggregate).clone();
                let result = self.extract_value(agg, indices)?;
                self.current_frame_mut().registers.set(*dest, result);
            }

            IrInstruction::InsertValue {
                dest,
                aggregate,
                value,
                indices,
            } => {
                let mut agg = self.current_frame().registers.get(*aggregate).clone();
                let val = self.current_frame().registers.get(*value).clone();
                self.insert_value(&mut agg, indices, val)?;
                self.current_frame_mut().registers.set(*dest, agg);
            }

            // === Union Operations ===
            IrInstruction::CreateUnion {
                dest,
                discriminant,
                value,
                ty: _,
            } => {
                let val = self.current_frame().registers.get(*value).clone();
                // Store as struct: [discriminant, value]
                self.current_frame_mut().registers.set(
                    *dest,
                    InterpValue::Struct(vec![InterpValue::U32(*discriminant), val]),
                );
            }

            IrInstruction::ExtractDiscriminant { dest, union_val } => {
                let union_v = self.current_frame().registers.get(*union_val).clone();
                match union_v {
                    InterpValue::Struct(fields) if !fields.is_empty() => {
                        self.current_frame_mut()
                            .registers
                            .set(*dest, fields[0].clone());
                    }
                    _ => {
                        return Err(InterpError::TypeError(
                            "Expected union value".to_string(),
                        ));
                    }
                }
            }

            IrInstruction::ExtractUnionValue {
                dest,
                union_val,
                discriminant: _,
                value_ty: _,
            } => {
                let union_v = self.current_frame().registers.get(*union_val).clone();
                match union_v {
                    InterpValue::Struct(fields) if fields.len() > 1 => {
                        self.current_frame_mut()
                            .registers
                            .set(*dest, fields[1].clone());
                    }
                    _ => {
                        return Err(InterpError::TypeError(
                            "Expected union value with data".to_string(),
                        ));
                    }
                }
            }

            // === Select Operation ===
            IrInstruction::Select {
                dest,
                condition,
                true_val,
                false_val,
            } => {
                let cond = self.current_frame().registers.get(*condition).to_bool()?;
                let result = if cond {
                    self.current_frame().registers.get(*true_val).clone()
                } else {
                    self.current_frame().registers.get(*false_val).clone()
                };
                self.current_frame_mut().registers.set(*dest, result);
            }

            // === Function Reference ===
            IrInstruction::FunctionRef { dest, func_id } => {
                self.current_frame_mut()
                    .registers
                    .set(*dest, InterpValue::Function(*func_id));
            }

            // === Closure Operations ===
            IrInstruction::MakeClosure {
                dest,
                func_id,
                captured_values,
            } => {
                let captured: Vec<InterpValue> = captured_values
                    .iter()
                    .map(|v| self.current_frame().registers.get(*v).clone())
                    .collect();
                // Store closure as struct: [func_id, captured_values...]
                let mut closure_data = vec![InterpValue::Function(*func_id)];
                closure_data.extend(captured);
                self.current_frame_mut()
                    .registers
                    .set(*dest, InterpValue::Struct(closure_data));
            }

            IrInstruction::ClosureFunc { dest, closure } => {
                let closure_val = self.current_frame().registers.get(*closure).clone();
                match closure_val {
                    InterpValue::Struct(fields) if !fields.is_empty() => {
                        self.current_frame_mut()
                            .registers
                            .set(*dest, fields[0].clone());
                    }
                    _ => {
                        return Err(InterpError::TypeError(
                            "Expected closure value".to_string(),
                        ));
                    }
                }
            }

            IrInstruction::ClosureEnv { dest, closure } => {
                let closure_val = self.current_frame().registers.get(*closure).clone();
                match closure_val {
                    InterpValue::Struct(fields) if fields.len() > 1 => {
                        // Return the environment as a struct (skip the function pointer)
                        let env: Vec<InterpValue> = fields[1..].to_vec();
                        self.current_frame_mut()
                            .registers
                            .set(*dest, InterpValue::Struct(env));
                    }
                    _ => {
                        self.current_frame_mut()
                            .registers
                            .set(*dest, InterpValue::Struct(vec![]));
                    }
                }
            }

            // === Borrowing (no-op in interpreter) ===
            IrInstruction::BorrowImmutable { dest, src, .. }
            | IrInstruction::BorrowMutable { dest, src, .. } => {
                let val = self.current_frame().registers.get(*src).clone();
                self.current_frame_mut().registers.set(*dest, val);
            }

            IrInstruction::Clone { dest, src } => {
                let val = self.current_frame().registers.get(*src).clone();
                self.current_frame_mut().registers.set(*dest, val);
            }

            IrInstruction::EndBorrow { .. } => {
                // No-op in interpreter
            }

            // === Memory Operations ===
            IrInstruction::MemCopy { dest, src, size } => {
                let dest_ptr = self.current_frame().registers.get(*dest).to_usize()?;
                let src_ptr = self.current_frame().registers.get(*src).to_usize()?;
                let size_val = self.current_frame().registers.get(*size).to_usize()?;

                // Copy bytes
                for i in 0..size_val {
                    if src_ptr + i < self.heap.len() && dest_ptr + i < self.heap.len() {
                        self.heap[dest_ptr + i] = self.heap[src_ptr + i];
                    }
                }
            }

            IrInstruction::MemSet { dest, value, size } => {
                let dest_ptr = self.current_frame().registers.get(*dest).to_usize()?;
                let val = self.current_frame().registers.get(*value).to_i64()? as u8;
                let size_val = self.current_frame().registers.get(*size).to_usize()?;

                // Set bytes
                for i in 0..size_val {
                    if dest_ptr + i < self.heap.len() {
                        self.heap[dest_ptr + i] = val;
                    }
                }
            }

            // === Undefined/Special ===
            IrInstruction::Undef { dest, .. } => {
                self.current_frame_mut()
                    .registers
                    .set(*dest, InterpValue::Void);
            }

            IrInstruction::Panic { message } => {
                return Err(InterpError::Panic(
                    message.clone().unwrap_or_else(|| "panic".to_string()),
                ));
            }

            IrInstruction::DebugLoc { .. } => {
                // No-op: debug info
            }

            // === Phi nodes are handled separately ===
            IrInstruction::Phi { .. } => {
                // Phi nodes are processed at block entry
            }

            // === Control flow (should be terminators) ===
            IrInstruction::Jump { .. }
            | IrInstruction::Branch { .. }
            | IrInstruction::Switch { .. }
            | IrInstruction::Return { .. } => {
                // These should be terminators, not instructions
                // They're handled by execute_terminator
            }

            // === Exception handling (simplified) ===
            IrInstruction::Throw { exception } => {
                let exc = self.current_frame().registers.get(*exception).clone();
                return Err(InterpError::Exception(format!("{:?}", exc)));
            }

            IrInstruction::LandingPad { dest, .. } => {
                // Simplified: just store a null value
                self.current_frame_mut()
                    .registers
                    .set(*dest, InterpValue::Null);
            }

            IrInstruction::Resume { .. } => {
                return Err(InterpError::Exception("resumed exception".to_string()));
            }

            // === Inline Assembly (not supported in interpreter) ===
            IrInstruction::InlineAsm { dest, .. } => {
                if let Some(d) = dest {
                    self.current_frame_mut()
                        .registers
                        .set(*d, InterpValue::Void);
                }
            }
        }
        Ok(())
    }

    /// Execute a block terminator
    fn execute_terminator(
        &mut self,
        _module: &IrModule,
        _function: &IrFunction,
        terminator: &IrTerminator,
    ) -> Result<TerminatorResult, InterpError> {
        match terminator {
            IrTerminator::Branch { target } => Ok(TerminatorResult::Continue(*target)),

            IrTerminator::CondBranch {
                condition,
                true_target,
                false_target,
            } => {
                let cond = self.current_frame().registers.get(*condition).to_bool()?;
                if cond {
                    Ok(TerminatorResult::Continue(*true_target))
                } else {
                    Ok(TerminatorResult::Continue(*false_target))
                }
            }

            IrTerminator::Switch {
                value,
                cases,
                default,
            } => {
                let val = self.current_frame().registers.get(*value).to_i64()?;
                for (case_val, target) in cases {
                    if *case_val == val {
                        return Ok(TerminatorResult::Continue(*target));
                    }
                }
                Ok(TerminatorResult::Continue(*default))
            }

            IrTerminator::Return { value } => {
                let result = if let Some(v) = value {
                    self.current_frame().registers.get(*v).clone()
                } else {
                    InterpValue::Void
                };
                Ok(TerminatorResult::Return(result))
            }

            IrTerminator::Unreachable => {
                Err(InterpError::RuntimeError("Reached unreachable code".to_string()))
            }

            IrTerminator::NoReturn { .. } => {
                Err(InterpError::RuntimeError("NoReturn terminator executed".to_string()))
            }
        }
    }

    /// Convert IrValue to InterpValue
    fn ir_value_to_interp(&self, value: &IrValue) -> Result<InterpValue, InterpError> {
        match value {
            IrValue::Void => Ok(InterpValue::Void),
            IrValue::Undef => Ok(InterpValue::Void),
            IrValue::Null => Ok(InterpValue::Null),
            IrValue::Bool(b) => Ok(InterpValue::Bool(*b)),
            IrValue::I8(n) => Ok(InterpValue::I8(*n)),
            IrValue::I16(n) => Ok(InterpValue::I16(*n)),
            IrValue::I32(n) => Ok(InterpValue::I32(*n)),
            IrValue::I64(n) => Ok(InterpValue::I64(*n)),
            IrValue::U8(n) => Ok(InterpValue::U8(*n)),
            IrValue::U16(n) => Ok(InterpValue::U16(*n)),
            IrValue::U32(n) => Ok(InterpValue::U32(*n)),
            IrValue::U64(n) => Ok(InterpValue::U64(*n)),
            IrValue::F32(n) => Ok(InterpValue::F32(*n)),
            IrValue::F64(n) => Ok(InterpValue::F64(*n)),
            IrValue::String(s) => Ok(InterpValue::String(s.clone())),
            IrValue::Array(arr) => {
                let values: Result<Vec<_>, _> =
                    arr.iter().map(|v| self.ir_value_to_interp(v)).collect();
                Ok(InterpValue::Array(values?))
            }
            IrValue::Struct(fields) => {
                let values: Result<Vec<_>, _> =
                    fields.iter().map(|v| self.ir_value_to_interp(v)).collect();
                Ok(InterpValue::Struct(values?))
            }
            IrValue::Function(func_id) => Ok(InterpValue::Function(*func_id)),
            IrValue::Closure { function, environment } => {
                let env = self.ir_value_to_interp(environment)?;
                Ok(InterpValue::Struct(vec![
                    InterpValue::Function(*function),
                    env,
                ]))
            }
        }
    }

    /// Evaluate a binary operation
    fn eval_binary_op(
        &self,
        op: BinaryOp,
        left: InterpValue,
        right: InterpValue,
    ) -> Result<InterpValue, InterpError> {
        match op {
            // Integer arithmetic
            BinaryOp::Add => {
                let l = left.to_i64()?;
                let r = right.to_i64()?;
                Ok(InterpValue::I64(l.wrapping_add(r)))
            }
            BinaryOp::Sub => {
                let l = left.to_i64()?;
                let r = right.to_i64()?;
                Ok(InterpValue::I64(l.wrapping_sub(r)))
            }
            BinaryOp::Mul => {
                let l = left.to_i64()?;
                let r = right.to_i64()?;
                Ok(InterpValue::I64(l.wrapping_mul(r)))
            }
            BinaryOp::Div => {
                let l = left.to_i64()?;
                let r = right.to_i64()?;
                if r == 0 {
                    return Err(InterpError::RuntimeError("Division by zero".to_string()));
                }
                Ok(InterpValue::I64(l / r))
            }
            BinaryOp::Rem => {
                let l = left.to_i64()?;
                let r = right.to_i64()?;
                if r == 0 {
                    return Err(InterpError::RuntimeError("Modulo by zero".to_string()));
                }
                Ok(InterpValue::I64(l % r))
            }

            // Bitwise operations
            BinaryOp::And => {
                let l = left.to_i64()?;
                let r = right.to_i64()?;
                Ok(InterpValue::I64(l & r))
            }
            BinaryOp::Or => {
                let l = left.to_i64()?;
                let r = right.to_i64()?;
                Ok(InterpValue::I64(l | r))
            }
            BinaryOp::Xor => {
                let l = left.to_i64()?;
                let r = right.to_i64()?;
                Ok(InterpValue::I64(l ^ r))
            }
            BinaryOp::Shl => {
                let l = left.to_i64()?;
                let r = right.to_i64()?;
                Ok(InterpValue::I64(l << (r & 63)))
            }
            BinaryOp::Shr => {
                let l = left.to_i64()?;
                let r = right.to_i64()?;
                Ok(InterpValue::I64(l >> (r & 63)))
            }

            // Floating point arithmetic
            BinaryOp::FAdd => {
                let l = left.to_f64()?;
                let r = right.to_f64()?;
                Ok(InterpValue::F64(l + r))
            }
            BinaryOp::FSub => {
                let l = left.to_f64()?;
                let r = right.to_f64()?;
                Ok(InterpValue::F64(l - r))
            }
            BinaryOp::FMul => {
                let l = left.to_f64()?;
                let r = right.to_f64()?;
                Ok(InterpValue::F64(l * r))
            }
            BinaryOp::FDiv => {
                let l = left.to_f64()?;
                let r = right.to_f64()?;
                Ok(InterpValue::F64(l / r))
            }
            BinaryOp::FRem => {
                let l = left.to_f64()?;
                let r = right.to_f64()?;
                Ok(InterpValue::F64(l % r))
            }
        }
    }

    /// Evaluate a unary operation
    fn eval_unary_op(
        &self,
        op: UnaryOp,
        operand: InterpValue,
    ) -> Result<InterpValue, InterpError> {
        match op {
            UnaryOp::Neg => {
                let val = operand.to_i64()?;
                Ok(InterpValue::I64(-val))
            }
            UnaryOp::Not => {
                let val = operand.to_i64()?;
                Ok(InterpValue::I64(!val))
            }
            UnaryOp::FNeg => {
                let val = operand.to_f64()?;
                Ok(InterpValue::F64(-val))
            }
        }
    }

    /// Evaluate a comparison operation
    fn eval_compare_op(
        &self,
        op: CompareOp,
        left: InterpValue,
        right: InterpValue,
    ) -> Result<bool, InterpError> {
        match op {
            // Integer comparisons (signed)
            CompareOp::Eq => Ok(left.to_i64()? == right.to_i64()?),
            CompareOp::Ne => Ok(left.to_i64()? != right.to_i64()?),
            CompareOp::Lt => Ok(left.to_i64()? < right.to_i64()?),
            CompareOp::Le => Ok(left.to_i64()? <= right.to_i64()?),
            CompareOp::Gt => Ok(left.to_i64()? > right.to_i64()?),
            CompareOp::Ge => Ok(left.to_i64()? >= right.to_i64()?),

            // Unsigned comparisons
            CompareOp::ULt => Ok((left.to_i64()? as u64) < (right.to_i64()? as u64)),
            CompareOp::ULe => Ok((left.to_i64()? as u64) <= (right.to_i64()? as u64)),
            CompareOp::UGt => Ok((left.to_i64()? as u64) > (right.to_i64()? as u64)),
            CompareOp::UGe => Ok((left.to_i64()? as u64) >= (right.to_i64()? as u64)),

            // Floating point comparisons
            CompareOp::FEq => Ok(left.to_f64()? == right.to_f64()?),
            CompareOp::FNe => Ok(left.to_f64()? != right.to_f64()?),
            CompareOp::FLt => Ok(left.to_f64()? < right.to_f64()?),
            CompareOp::FLe => Ok(left.to_f64()? <= right.to_f64()?),
            CompareOp::FGt => Ok(left.to_f64()? > right.to_f64()?),
            CompareOp::FGe => Ok(left.to_f64()? >= right.to_f64()?),

            // Floating point ordered/unordered
            CompareOp::FOrd => {
                let l = left.to_f64()?;
                let r = right.to_f64()?;
                Ok(!l.is_nan() && !r.is_nan())
            }
            CompareOp::FUno => {
                let l = left.to_f64()?;
                let r = right.to_f64()?;
                Ok(l.is_nan() || r.is_nan())
            }
        }
    }

    /// Cast a value to a target type
    fn cast_value(
        &self,
        val: InterpValue,
        to_ty: &IrType,
    ) -> Result<InterpValue, InterpError> {
        match to_ty {
            IrType::Bool => Ok(InterpValue::Bool(val.to_bool()?)),
            IrType::I8 => Ok(InterpValue::I8(val.to_i64()? as i8)),
            IrType::I16 => Ok(InterpValue::I16(val.to_i64()? as i16)),
            IrType::I32 => Ok(InterpValue::I32(val.to_i64()? as i32)),
            IrType::I64 => Ok(InterpValue::I64(val.to_i64()?)),
            IrType::U8 => Ok(InterpValue::U8(val.to_i64()? as u8)),
            IrType::U16 => Ok(InterpValue::U16(val.to_i64()? as u16)),
            IrType::U32 => Ok(InterpValue::U32(val.to_i64()? as u32)),
            IrType::U64 => Ok(InterpValue::U64(val.to_i64()? as u64)),
            IrType::F32 => Ok(InterpValue::F32(val.to_f64()? as f32)),
            IrType::F64 => Ok(InterpValue::F64(val.to_f64()?)),
            IrType::Ptr(_) => Ok(InterpValue::Ptr(val.to_usize()?)),
            _ => Ok(val), // For other types, pass through
        }
    }

    /// Load a value from a pointer
    fn load_from_ptr(
        &self,
        ptr: InterpValue,
        ty: &IrType,
    ) -> Result<InterpValue, InterpError> {
        let addr = ptr.to_usize()?;

        // Simple implementation: read from heap
        match ty {
            IrType::Bool => {
                if addr < self.heap.len() {
                    Ok(InterpValue::Bool(self.heap[addr] != 0))
                } else {
                    Err(InterpError::RuntimeError("Invalid memory access".to_string()))
                }
            }
            IrType::I8 => {
                if addr < self.heap.len() {
                    Ok(InterpValue::I8(self.heap[addr] as i8))
                } else {
                    Err(InterpError::RuntimeError("Invalid memory access".to_string()))
                }
            }
            IrType::I32 => {
                if addr + 4 <= self.heap.len() {
                    let bytes: [u8; 4] = self.heap[addr..addr + 4].try_into().unwrap();
                    Ok(InterpValue::I32(i32::from_le_bytes(bytes)))
                } else {
                    Err(InterpError::RuntimeError("Invalid memory access".to_string()))
                }
            }
            IrType::I64 => {
                if addr + 8 <= self.heap.len() {
                    let bytes: [u8; 8] = self.heap[addr..addr + 8].try_into().unwrap();
                    Ok(InterpValue::I64(i64::from_le_bytes(bytes)))
                } else {
                    Err(InterpError::RuntimeError("Invalid memory access".to_string()))
                }
            }
            IrType::F32 => {
                if addr + 4 <= self.heap.len() {
                    let bytes: [u8; 4] = self.heap[addr..addr + 4].try_into().unwrap();
                    Ok(InterpValue::F32(f32::from_le_bytes(bytes)))
                } else {
                    Err(InterpError::RuntimeError("Invalid memory access".to_string()))
                }
            }
            IrType::F64 => {
                if addr + 8 <= self.heap.len() {
                    let bytes: [u8; 8] = self.heap[addr..addr + 8].try_into().unwrap();
                    Ok(InterpValue::F64(f64::from_le_bytes(bytes)))
                } else {
                    Err(InterpError::RuntimeError("Invalid memory access".to_string()))
                }
            }
            IrType::Ptr(_) => {
                if addr + 8 <= self.heap.len() {
                    let bytes: [u8; 8] = self.heap[addr..addr + 8].try_into().unwrap();
                    Ok(InterpValue::Ptr(usize::from_le_bytes(bytes)))
                } else {
                    Err(InterpError::RuntimeError("Invalid memory access".to_string()))
                }
            }
            _ => {
                // For other types, return a placeholder
                Ok(InterpValue::Void)
            }
        }
    }

    /// Store a value to a pointer
    fn store_to_ptr(&mut self, ptr: InterpValue, val: InterpValue) -> Result<(), InterpError> {
        let addr = ptr.to_usize()?;

        match val {
            InterpValue::Bool(b) => {
                if addr < self.heap.len() {
                    self.heap[addr] = if b { 1 } else { 0 };
                }
            }
            InterpValue::I8(n) => {
                if addr < self.heap.len() {
                    self.heap[addr] = n as u8;
                }
            }
            InterpValue::I32(n) => {
                if addr + 4 <= self.heap.len() {
                    self.heap[addr..addr + 4].copy_from_slice(&n.to_le_bytes());
                }
            }
            InterpValue::I64(n) => {
                if addr + 8 <= self.heap.len() {
                    self.heap[addr..addr + 8].copy_from_slice(&n.to_le_bytes());
                }
            }
            InterpValue::F32(n) => {
                if addr + 4 <= self.heap.len() {
                    self.heap[addr..addr + 4].copy_from_slice(&n.to_le_bytes());
                }
            }
            InterpValue::F64(n) => {
                if addr + 8 <= self.heap.len() {
                    self.heap[addr..addr + 8].copy_from_slice(&n.to_le_bytes());
                }
            }
            InterpValue::Ptr(p) => {
                if addr + 8 <= self.heap.len() {
                    self.heap[addr..addr + 8].copy_from_slice(&p.to_le_bytes());
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Allocate memory on the heap
    fn alloc_heap(&mut self, size: usize) -> Result<usize, InterpError> {
        // Align to 8 bytes
        let aligned_size = (size + 7) & !7;
        let ptr = self.heap_offset;

        if ptr + aligned_size > self.heap.len() {
            // Grow heap
            self.heap.resize(self.heap.len() * 2, 0);
        }

        self.heap_offset += aligned_size;
        Ok(ptr)
    }

    /// Extract a value from an aggregate using indices
    fn extract_value(
        &self,
        agg: InterpValue,
        indices: &[u32],
    ) -> Result<InterpValue, InterpError> {
        let mut current = agg;
        for &idx in indices {
            match current {
                InterpValue::Struct(fields) | InterpValue::Array(fields) => {
                    if (idx as usize) < fields.len() {
                        current = fields[idx as usize].clone();
                    } else {
                        return Err(InterpError::RuntimeError(format!(
                            "Index {} out of bounds",
                            idx
                        )));
                    }
                }
                _ => {
                    return Err(InterpError::TypeError(
                        "Cannot extract from non-aggregate".to_string(),
                    ));
                }
            }
        }
        Ok(current)
    }

    /// Insert a value into an aggregate using indices
    fn insert_value(
        &self,
        agg: &mut InterpValue,
        indices: &[u32],
        val: InterpValue,
    ) -> Result<(), InterpError> {
        if indices.is_empty() {
            *agg = val;
            return Ok(());
        }

        let idx = indices[0] as usize;
        match agg {
            InterpValue::Struct(fields) | InterpValue::Array(fields) => {
                if idx < fields.len() {
                    if indices.len() == 1 {
                        fields[idx] = val;
                    } else {
                        self.insert_value(&mut fields[idx], &indices[1..], val)?;
                    }
                } else {
                    return Err(InterpError::RuntimeError(format!(
                        "Index {} out of bounds",
                        idx
                    )));
                }
            }
            _ => {
                return Err(InterpError::TypeError(
                    "Cannot insert into non-aggregate".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Call an extern function by name (without signature - uses built-in handlers)
    fn call_extern(
        &mut self,
        name: &str,
        args: &[InterpValue],
    ) -> Result<InterpValue, InterpError> {
        // Built-in functions (simple implementations for common operations)
        match name {
            "trace" | "haxe_print" | "print" => {
                // Print function
                if let Some(arg) = args.first() {
                    match arg {
                        InterpValue::String(s) => println!("{}", s),
                        InterpValue::I32(n) => println!("{}", n),
                        InterpValue::I64(n) => println!("{}", n),
                        InterpValue::F64(n) => println!("{}", n),
                        InterpValue::Bool(b) => println!("{}", b),
                        other => println!("{:?}", other),
                    }
                }
                Ok(InterpValue::Void)
            }
            // Handle haxe_trace_string_struct which takes a struct containing the string
            "haxe_trace_string_struct" => {
                if let Some(arg) = args.first() {
                    match arg {
                        InterpValue::String(s) => println!("{}", s),
                        InterpValue::Struct(fields) => {
                            // The struct typically has (ptr, len) or (ptr, len, capacity)
                            // Try to extract and print the string
                            if let Some(first) = fields.first() {
                                match first {
                                    InterpValue::String(s) => println!("{}", s),
                                    InterpValue::Ptr(ptr) => {
                                        // Try to read string from pointer with length from second field
                                        if let Some(InterpValue::I64(len)) = fields.get(1) {
                                            if *ptr != 0 && *len > 0 {
                                                unsafe {
                                                    let slice = std::slice::from_raw_parts(*ptr as *const u8, *len as usize);
                                                    if let Ok(s) = std::str::from_utf8(slice) {
                                                        println!("{}", s);
                                                    } else {
                                                        println!("<non-utf8 string>");
                                                    }
                                                }
                                            } else {
                                                println!("");
                                            }
                                        } else {
                                            println!("<ptr:{:#x}>", ptr);
                                        }
                                    }
                                    _ => println!("{:?}", first),
                                }
                            } else {
                                println!("");
                            }
                        }
                        InterpValue::Ptr(ptr) => {
                            // It might be a pointer to the string struct - try to read it
                            if *ptr != 0 {
                                // In production, we'd dereference the struct. For now, just show address.
                                println!("<string at {:#x}>", ptr);
                            } else {
                                println!("null");
                            }
                        }
                        other => println!("{:?}", other),
                    }
                }
                Ok(InterpValue::Void)
            }
            // Handle haxe_trace_int for tracing integers
            "haxe_trace_int" => {
                if let Some(arg) = args.first() {
                    match arg {
                        InterpValue::I32(n) => println!("{}", n),
                        InterpValue::I64(n) => println!("{}", n),
                        InterpValue::I8(n) => println!("{}", n),
                        InterpValue::I16(n) => println!("{}", n),
                        InterpValue::U8(n) => println!("{}", n),
                        InterpValue::U16(n) => println!("{}", n),
                        InterpValue::U32(n) => println!("{}", n),
                        InterpValue::U64(n) => println!("{}", n),
                        other => println!("{:?}", other),
                    }
                }
                Ok(InterpValue::Void)
            }
            "haxe_string_length" => {
                if let Some(InterpValue::String(s)) = args.first() {
                    Ok(InterpValue::I32(s.len() as i32))
                } else {
                    Ok(InterpValue::I32(0))
                }
            }
            "haxe_string_concat" => {
                if args.len() >= 2 {
                    if let (InterpValue::String(a), InterpValue::String(b)) =
                        (&args[0], &args[1])
                    {
                        return Ok(InterpValue::String(format!("{}{}", a, b)));
                    }
                }
                Ok(InterpValue::String(String::new()))
            }
            _ => {
                // Check if we have a registered symbol (call without signature info)
                if let Some(&ptr) = self.runtime_symbols.get(name) {
                    // Without signature info, we can only handle simple cases
                    return self.call_ffi_ptr_simple(ptr as usize, args);
                }
                // Unknown extern - return void
                tracing::warn!("Unknown extern function: {}", name);
                Ok(InterpValue::Void)
            }
        }
    }

    /// Call an IrFunction as FFI (for extern functions with empty blocks)
    fn call_ffi_for_function(
        &mut self,
        func: &IrFunction,
        args: &[InterpValue],
    ) -> Result<InterpValue, InterpError> {
        // First check built-ins
        let builtin_result = self.call_extern(&func.name, args);
        if let Ok(ref val) = builtin_result {
            if !matches!(val, InterpValue::Void) || func.signature.return_type == IrType::Void {
                // If we got a non-void result, or the function is supposed to return void,
                // use the builtin result
                if !func.name.starts_with("Unknown") {
                    return builtin_result;
                }
            }
        }

        // Check if we have a registered symbol
        if let Some(&ptr) = self.runtime_symbols.get(&func.name) {
            return self.call_ffi_with_signature(ptr as usize, args, &func.signature);
        }

        // No symbol found - return default value (warning already logged by builtin check)
        tracing::warn!("Function symbol not found for FFI: {}", func.name);
        Ok(self.default_value_for_type(&func.signature.return_type))
    }

    /// Call an extern function with its full signature for proper FFI
    fn call_extern_with_signature(
        &mut self,
        extern_fn: &IrExternFunction,
        args: &[InterpValue],
    ) -> Result<InterpValue, InterpError> {
        // First check built-ins
        let builtin_result = self.call_extern(&extern_fn.name, args);
        if let Ok(ref val) = builtin_result {
            if !matches!(val, InterpValue::Void) || extern_fn.signature.return_type == IrType::Void {
                // If we got a non-void result, or the function is supposed to return void,
                // use the builtin result
                if !extern_fn.name.starts_with("Unknown") {
                    return builtin_result;
                }
            }
        }

        // Check if we have a registered symbol
        if let Some(&ptr) = self.runtime_symbols.get(&extern_fn.name) {
            return self.call_ffi_with_signature(ptr as usize, args, &extern_fn.signature);
        }

        // No symbol found
        tracing::warn!("Extern function not found: {}", extern_fn.name);
        Ok(self.default_value_for_type(&extern_fn.signature.return_type))
    }

    /// Call a function through a raw pointer with signature (proper FFI)
    ///
    /// This uses unsafe Rust to call native function pointers with the correct
    /// calling convention based on the IrFunctionSignature.
    fn call_ffi_with_signature(
        &self,
        ptr: usize,
        args: &[InterpValue],
        signature: &IrFunctionSignature,
    ) -> Result<InterpValue, InterpError> {
        // Convert arguments to native representation
        let native_args: Vec<NativeValue> = args
            .iter()
            .zip(signature.parameters.iter())
            .map(|(arg, param)| self.interp_to_native(arg, &param.ty))
            .collect::<Result<_, _>>()?;

        // Call the function based on arity and return type
        let result = unsafe {
            self.call_native_fn(ptr, &native_args, &signature.return_type)?
        };

        // Convert result back to InterpValue
        self.native_to_interp(result, &signature.return_type)
    }

    /// Simple FFI call without signature (for backward compatibility)
    fn call_ffi_ptr_simple(
        &self,
        ptr: usize,
        args: &[InterpValue],
    ) -> Result<InterpValue, InterpError> {
        // Without signature info, we infer types from arguments and assume i64 return
        let native_args: Vec<NativeValue> = args
            .iter()
            .map(|arg| self.interp_to_native_inferred(arg))
            .collect::<Result<_, _>>()?;

        let result = unsafe {
            self.call_native_fn(ptr, &native_args, &IrType::I64)?
        };

        self.native_to_interp(result, &IrType::I64)
    }

    /// FFI call with explicit parameter and return types (from IrType::Function)
    fn call_ffi_ptr_with_types(
        &self,
        ptr: usize,
        args: &[InterpValue],
        param_types: &[IrType],
        return_type: &IrType,
    ) -> Result<InterpValue, InterpError> {
        // Convert arguments to native representation using the explicit types
        let native_args: Vec<NativeValue> = args
            .iter()
            .enumerate()
            .map(|(i, arg)| {
                let ty = param_types.get(i).unwrap_or(&IrType::I64);
                self.interp_to_native(arg, ty)
            })
            .collect::<Result<_, _>>()?;

        let result = unsafe {
            self.call_native_fn(ptr, &native_args, return_type)?
        };

        self.native_to_interp(result, return_type)
    }

    /// Convert InterpValue to native representation for FFI
    fn interp_to_native(&self, val: &InterpValue, ty: &IrType) -> Result<NativeValue, InterpError> {
        match ty {
            IrType::Void => Ok(NativeValue::Void),
            IrType::Bool => Ok(NativeValue::U8(if val.to_bool()? { 1 } else { 0 })),
            IrType::I8 => Ok(NativeValue::I8(val.to_i64()? as i8)),
            IrType::I16 => Ok(NativeValue::I16(val.to_i64()? as i16)),
            IrType::I32 => Ok(NativeValue::I32(val.to_i64()? as i32)),
            IrType::I64 => Ok(NativeValue::I64(val.to_i64()?)),
            IrType::U8 => Ok(NativeValue::U8(val.to_i64()? as u8)),
            IrType::U16 => Ok(NativeValue::U16(val.to_i64()? as u16)),
            IrType::U32 => Ok(NativeValue::U32(val.to_i64()? as u32)),
            IrType::U64 => Ok(NativeValue::U64(val.to_i64()? as u64)),
            IrType::F32 => Ok(NativeValue::F32(val.to_f64()? as f32)),
            IrType::F64 => Ok(NativeValue::F64(val.to_f64()?)),
            IrType::Ptr(_) | IrType::Ref(_) => Ok(NativeValue::Ptr(val.to_usize()?)),
            IrType::String => {
                // For string FFI, we pass a pointer to the string data
                match val {
                    InterpValue::String(s) => Ok(NativeValue::Ptr(s.as_ptr() as usize)),
                    InterpValue::Ptr(p) => Ok(NativeValue::Ptr(*p)),
                    _ => Ok(NativeValue::Ptr(0)),
                }
            }
            _ => {
                // For other types, try to pass as pointer
                Ok(NativeValue::Ptr(val.to_usize().unwrap_or(0)))
            }
        }
    }

    /// Convert InterpValue to native, inferring type from the value
    fn interp_to_native_inferred(&self, val: &InterpValue) -> Result<NativeValue, InterpError> {
        match val {
            InterpValue::Void => Ok(NativeValue::Void),
            InterpValue::Bool(b) => Ok(NativeValue::U8(if *b { 1 } else { 0 })),
            InterpValue::I8(n) => Ok(NativeValue::I8(*n)),
            InterpValue::I16(n) => Ok(NativeValue::I16(*n)),
            InterpValue::I32(n) => Ok(NativeValue::I32(*n)),
            InterpValue::I64(n) => Ok(NativeValue::I64(*n)),
            InterpValue::U8(n) => Ok(NativeValue::U8(*n)),
            InterpValue::U16(n) => Ok(NativeValue::U16(*n)),
            InterpValue::U32(n) => Ok(NativeValue::U32(*n)),
            InterpValue::U64(n) => Ok(NativeValue::U64(*n)),
            InterpValue::F32(n) => Ok(NativeValue::F32(*n)),
            InterpValue::F64(n) => Ok(NativeValue::F64(*n)),
            InterpValue::Ptr(p) => Ok(NativeValue::Ptr(*p)),
            InterpValue::Null => Ok(NativeValue::Ptr(0)),
            InterpValue::String(s) => Ok(NativeValue::Ptr(s.as_ptr() as usize)),
            InterpValue::Array(_) | InterpValue::Struct(_) | InterpValue::Function(_) => {
                // Pass these as pointers (though this is a fallback)
                Ok(NativeValue::Ptr(0))
            }
        }
    }

    /// Convert native value back to InterpValue
    fn native_to_interp(&self, val: NativeValue, ty: &IrType) -> Result<InterpValue, InterpError> {
        match ty {
            IrType::Void => Ok(InterpValue::Void),
            IrType::Bool => match val {
                NativeValue::U8(n) => Ok(InterpValue::Bool(n != 0)),
                NativeValue::I32(n) => Ok(InterpValue::Bool(n != 0)),
                NativeValue::I64(n) => Ok(InterpValue::Bool(n != 0)),
                _ => Ok(InterpValue::Bool(false)),
            },
            IrType::I8 => Ok(InterpValue::I8(val.to_i64() as i8)),
            IrType::I16 => Ok(InterpValue::I16(val.to_i64() as i16)),
            IrType::I32 => Ok(InterpValue::I32(val.to_i64() as i32)),
            IrType::I64 => Ok(InterpValue::I64(val.to_i64())),
            IrType::U8 => Ok(InterpValue::U8(val.to_i64() as u8)),
            IrType::U16 => Ok(InterpValue::U16(val.to_i64() as u16)),
            IrType::U32 => Ok(InterpValue::U32(val.to_i64() as u32)),
            IrType::U64 => Ok(InterpValue::U64(val.to_i64() as u64)),
            IrType::F32 => Ok(InterpValue::F32(val.to_f64() as f32)),
            IrType::F64 => Ok(InterpValue::F64(val.to_f64())),
            IrType::Ptr(_) | IrType::Ref(_) => match val {
                NativeValue::Ptr(p) => Ok(InterpValue::Ptr(p)),
                _ => Ok(InterpValue::Ptr(val.to_i64() as usize)),
            },
            _ => Ok(InterpValue::I64(val.to_i64())),
        }
    }

    /// Get default value for a type
    fn default_value_for_type(&self, ty: &IrType) -> InterpValue {
        match ty {
            IrType::Void => InterpValue::Void,
            IrType::Bool => InterpValue::Bool(false),
            IrType::I8 => InterpValue::I8(0),
            IrType::I16 => InterpValue::I16(0),
            IrType::I32 => InterpValue::I32(0),
            IrType::I64 => InterpValue::I64(0),
            IrType::U8 => InterpValue::U8(0),
            IrType::U16 => InterpValue::U16(0),
            IrType::U32 => InterpValue::U32(0),
            IrType::U64 => InterpValue::U64(0),
            IrType::F32 => InterpValue::F32(0.0),
            IrType::F64 => InterpValue::F64(0.0),
            IrType::Ptr(_) | IrType::Ref(_) => InterpValue::Ptr(0),
            IrType::String => InterpValue::String(String::new()),
            _ => InterpValue::Void,
        }
    }

    /// Call a native function pointer with given arguments
    ///
    /// # Safety
    /// The caller must ensure:
    /// - `ptr` points to a valid function
    /// - `args` match the function's expected signature
    /// - The return type matches the function's actual return type
    unsafe fn call_native_fn(
        &self,
        ptr: usize,
        args: &[NativeValue],
        return_type: &IrType,
    ) -> Result<NativeValue, InterpError> {
        // Convert arguments to u64 for the trampoline
        let arg_values: Vec<u64> = args.iter().map(|a| a.to_u64()).collect();

        // Dispatch based on arity (0-8 arguments supported)
        let result = match arg_values.len() {
            0 => self.call_fn_0(ptr, return_type),
            1 => self.call_fn_1(ptr, arg_values[0], return_type),
            2 => self.call_fn_2(ptr, arg_values[0], arg_values[1], return_type),
            3 => self.call_fn_3(ptr, arg_values[0], arg_values[1], arg_values[2], return_type),
            4 => self.call_fn_4(ptr, arg_values[0], arg_values[1], arg_values[2], arg_values[3], return_type),
            5 => self.call_fn_5(ptr, arg_values[0], arg_values[1], arg_values[2], arg_values[3], arg_values[4], return_type),
            6 => self.call_fn_6(ptr, arg_values[0], arg_values[1], arg_values[2], arg_values[3], arg_values[4], arg_values[5], return_type),
            7 => self.call_fn_7(ptr, arg_values[0], arg_values[1], arg_values[2], arg_values[3], arg_values[4], arg_values[5], arg_values[6], return_type),
            8 => self.call_fn_8(ptr, arg_values[0], arg_values[1], arg_values[2], arg_values[3], arg_values[4], arg_values[5], arg_values[6], arg_values[7], return_type),
            n => {
                return Err(InterpError::RuntimeError(format!(
                    "FFI calls with {} arguments not supported (max 8)",
                    n
                )));
            }
        };

        result
    }

    // FFI trampoline functions for different arities
    // These use the system C calling convention (extern "C")

    unsafe fn call_fn_0(&self, ptr: usize, ret_ty: &IrType) -> Result<NativeValue, InterpError> {
        if ret_ty.is_float() {
            let f: extern "C" fn() -> f64 = std::mem::transmute(ptr);
            Ok(NativeValue::F64(f()))
        } else {
            let f: extern "C" fn() -> u64 = std::mem::transmute(ptr);
            Ok(NativeValue::U64(f()))
        }
    }

    unsafe fn call_fn_1(&self, ptr: usize, a0: u64, ret_ty: &IrType) -> Result<NativeValue, InterpError> {
        if ret_ty.is_float() {
            let f: extern "C" fn(u64) -> f64 = std::mem::transmute(ptr);
            Ok(NativeValue::F64(f(a0)))
        } else {
            let f: extern "C" fn(u64) -> u64 = std::mem::transmute(ptr);
            Ok(NativeValue::U64(f(a0)))
        }
    }

    unsafe fn call_fn_2(&self, ptr: usize, a0: u64, a1: u64, ret_ty: &IrType) -> Result<NativeValue, InterpError> {
        if ret_ty.is_float() {
            let f: extern "C" fn(u64, u64) -> f64 = std::mem::transmute(ptr);
            Ok(NativeValue::F64(f(a0, a1)))
        } else {
            let f: extern "C" fn(u64, u64) -> u64 = std::mem::transmute(ptr);
            Ok(NativeValue::U64(f(a0, a1)))
        }
    }

    unsafe fn call_fn_3(&self, ptr: usize, a0: u64, a1: u64, a2: u64, ret_ty: &IrType) -> Result<NativeValue, InterpError> {
        if ret_ty.is_float() {
            let f: extern "C" fn(u64, u64, u64) -> f64 = std::mem::transmute(ptr);
            Ok(NativeValue::F64(f(a0, a1, a2)))
        } else {
            let f: extern "C" fn(u64, u64, u64) -> u64 = std::mem::transmute(ptr);
            Ok(NativeValue::U64(f(a0, a1, a2)))
        }
    }

    unsafe fn call_fn_4(&self, ptr: usize, a0: u64, a1: u64, a2: u64, a3: u64, ret_ty: &IrType) -> Result<NativeValue, InterpError> {
        if ret_ty.is_float() {
            let f: extern "C" fn(u64, u64, u64, u64) -> f64 = std::mem::transmute(ptr);
            Ok(NativeValue::F64(f(a0, a1, a2, a3)))
        } else {
            let f: extern "C" fn(u64, u64, u64, u64) -> u64 = std::mem::transmute(ptr);
            Ok(NativeValue::U64(f(a0, a1, a2, a3)))
        }
    }

    unsafe fn call_fn_5(&self, ptr: usize, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, ret_ty: &IrType) -> Result<NativeValue, InterpError> {
        if ret_ty.is_float() {
            let f: extern "C" fn(u64, u64, u64, u64, u64) -> f64 = std::mem::transmute(ptr);
            Ok(NativeValue::F64(f(a0, a1, a2, a3, a4)))
        } else {
            let f: extern "C" fn(u64, u64, u64, u64, u64) -> u64 = std::mem::transmute(ptr);
            Ok(NativeValue::U64(f(a0, a1, a2, a3, a4)))
        }
    }

    unsafe fn call_fn_6(&self, ptr: usize, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, ret_ty: &IrType) -> Result<NativeValue, InterpError> {
        if ret_ty.is_float() {
            let f: extern "C" fn(u64, u64, u64, u64, u64, u64) -> f64 = std::mem::transmute(ptr);
            Ok(NativeValue::F64(f(a0, a1, a2, a3, a4, a5)))
        } else {
            let f: extern "C" fn(u64, u64, u64, u64, u64, u64) -> u64 = std::mem::transmute(ptr);
            Ok(NativeValue::U64(f(a0, a1, a2, a3, a4, a5)))
        }
    }

    unsafe fn call_fn_7(&self, ptr: usize, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64, ret_ty: &IrType) -> Result<NativeValue, InterpError> {
        if ret_ty.is_float() {
            let f: extern "C" fn(u64, u64, u64, u64, u64, u64, u64) -> f64 = std::mem::transmute(ptr);
            Ok(NativeValue::F64(f(a0, a1, a2, a3, a4, a5, a6)))
        } else {
            let f: extern "C" fn(u64, u64, u64, u64, u64, u64, u64) -> u64 = std::mem::transmute(ptr);
            Ok(NativeValue::U64(f(a0, a1, a2, a3, a4, a5, a6)))
        }
    }

    unsafe fn call_fn_8(&self, ptr: usize, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64, a7: u64, ret_ty: &IrType) -> Result<NativeValue, InterpError> {
        if ret_ty.is_float() {
            let f: extern "C" fn(u64, u64, u64, u64, u64, u64, u64, u64) -> f64 = std::mem::transmute(ptr);
            Ok(NativeValue::F64(f(a0, a1, a2, a3, a4, a5, a6, a7)))
        } else {
            let f: extern "C" fn(u64, u64, u64, u64, u64, u64, u64, u64) -> u64 = std::mem::transmute(ptr);
            Ok(NativeValue::U64(f(a0, a1, a2, a3, a4, a5, a6, a7)))
        }
    }
}

/// Native value representation for FFI calls
///
/// Uses a flat representation that can be easily converted to/from u64
/// for passing through the FFI trampoline.
#[derive(Debug, Clone, Copy)]
enum NativeValue {
    Void,
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Ptr(usize),
}

impl NativeValue {
    /// Convert to u64 for FFI (all integer/pointer types fit in u64)
    fn to_u64(&self) -> u64 {
        match self {
            NativeValue::Void => 0,
            NativeValue::I8(n) => *n as i64 as u64,
            NativeValue::I16(n) => *n as i64 as u64,
            NativeValue::I32(n) => *n as i64 as u64,
            NativeValue::I64(n) => *n as u64,
            NativeValue::U8(n) => *n as u64,
            NativeValue::U16(n) => *n as u64,
            NativeValue::U32(n) => *n as u64,
            NativeValue::U64(n) => *n,
            NativeValue::F32(n) => (*n as f64).to_bits(),
            NativeValue::F64(n) => n.to_bits(),
            NativeValue::Ptr(p) => *p as u64,
        }
    }

    /// Convert to i64
    fn to_i64(&self) -> i64 {
        match self {
            NativeValue::Void => 0,
            NativeValue::I8(n) => *n as i64,
            NativeValue::I16(n) => *n as i64,
            NativeValue::I32(n) => *n as i64,
            NativeValue::I64(n) => *n,
            NativeValue::U8(n) => *n as i64,
            NativeValue::U16(n) => *n as i64,
            NativeValue::U32(n) => *n as i64,
            NativeValue::U64(n) => *n as i64,
            NativeValue::F32(n) => *n as i64,
            NativeValue::F64(n) => *n as i64,
            NativeValue::Ptr(p) => *p as i64,
        }
    }

    /// Convert to f64
    fn to_f64(&self) -> f64 {
        match self {
            NativeValue::Void => 0.0,
            NativeValue::I8(n) => *n as f64,
            NativeValue::I16(n) => *n as f64,
            NativeValue::I32(n) => *n as f64,
            NativeValue::I64(n) => *n as f64,
            NativeValue::U8(n) => *n as f64,
            NativeValue::U16(n) => *n as f64,
            NativeValue::U32(n) => *n as f64,
            NativeValue::U64(n) => *n as f64,
            NativeValue::F32(n) => *n as f64,
            NativeValue::F64(n) => *n,
            NativeValue::Ptr(p) => *p as f64,
        }
    }
}

impl Default for MirInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

/// Interpreter error types
#[derive(Debug)]
pub enum InterpError {
    FunctionNotFound(IrFunctionId),
    BlockNotFound(IrBlockId),
    StackOverflow,
    TypeError(String),
    RuntimeError(String),
    Panic(String),
    Exception(String),
}

impl std::fmt::Display for InterpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpError::FunctionNotFound(id) => write!(f, "Function not found: {:?}", id),
            InterpError::BlockNotFound(id) => write!(f, "Block not found: {:?}", id),
            InterpError::StackOverflow => write!(f, "Stack overflow"),
            InterpError::TypeError(msg) => write!(f, "Type error: {}", msg),
            InterpError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
            InterpError::Panic(msg) => write!(f, "Panic: {}", msg),
            InterpError::Exception(msg) => write!(f, "Exception: {}", msg),
        }
    }
}

impl std::error::Error for InterpError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interp_value_conversions() {
        let i32_val = InterpValue::I32(42);
        assert_eq!(i32_val.to_i64().unwrap(), 42);
        assert_eq!(i32_val.to_bool().unwrap(), true);

        let zero_val = InterpValue::I32(0);
        assert_eq!(zero_val.to_bool().unwrap(), false);

        let f64_val = InterpValue::F64(3.14);
        assert!((f64_val.to_f64().unwrap() - 3.14).abs() < 0.001);
    }

    #[test]
    fn test_register_file() {
        let mut regs = RegisterFile::new(10);
        regs.set(IrId::new(0), InterpValue::I32(100));
        regs.set(IrId::new(5), InterpValue::Bool(true));

        assert!(matches!(regs.get(IrId::new(0)), InterpValue::I32(100)));
        assert!(matches!(regs.get(IrId::new(5)), InterpValue::Bool(true)));
        assert!(matches!(regs.get(IrId::new(3)), InterpValue::Void));
    }

    #[test]
    fn test_binary_ops() {
        let interp = MirInterpreter::new();

        // Integer operations
        let result = interp
            .eval_binary_op(BinaryOp::Add, InterpValue::I64(10), InterpValue::I64(20))
            .unwrap();
        assert!(matches!(result, InterpValue::I64(30)));

        let result = interp
            .eval_binary_op(BinaryOp::Mul, InterpValue::I64(5), InterpValue::I64(6))
            .unwrap();
        assert!(matches!(result, InterpValue::I64(30)));

        // Floating point operations
        let result = interp
            .eval_binary_op(BinaryOp::FAdd, InterpValue::F64(1.5), InterpValue::F64(2.5))
            .unwrap();
        if let InterpValue::F64(v) = result {
            assert!((v - 4.0).abs() < 0.001);
        } else {
            panic!("Expected F64");
        }
    }

    #[test]
    fn test_compare_ops() {
        let interp = MirInterpreter::new();

        assert!(interp
            .eval_compare_op(CompareOp::Lt, InterpValue::I64(5), InterpValue::I64(10))
            .unwrap());
        assert!(!interp
            .eval_compare_op(CompareOp::Lt, InterpValue::I64(10), InterpValue::I64(5))
            .unwrap());
        assert!(interp
            .eval_compare_op(CompareOp::Eq, InterpValue::I64(5), InterpValue::I64(5))
            .unwrap());
    }

    // FFI test functions (extern "C" for proper ABI)
    extern "C" fn test_add(a: u64, b: u64) -> u64 {
        a + b
    }

    extern "C" fn test_mul(a: u64, b: u64, c: u64) -> u64 {
        a * b * c
    }

    extern "C" fn test_f64_add(a: u64, b: u64) -> f64 {
        let a = f64::from_bits(a);
        let b = f64::from_bits(b);
        a + b
    }

    #[test]
    fn test_ffi_call_simple() {
        let mut interp = MirInterpreter::new();

        // Register the test_add function
        interp.register_symbol("test_add", test_add as *const u8);

        // Call through simple FFI (without signature)
        let result = interp.call_ffi_ptr_simple(
            test_add as usize,
            &[InterpValue::I64(5), InterpValue::I64(3)],
        ).unwrap();

        match result {
            InterpValue::I64(n) => assert_eq!(n, 8),
            _ => panic!("Expected I64 result, got {:?}", result),
        }
    }

    #[test]
    fn test_ffi_call_with_types() {
        let interp = MirInterpreter::new();

        // Call with explicit type information
        let param_types = vec![IrType::I64, IrType::I64, IrType::I64];
        let return_type = IrType::I64;

        let result = interp.call_ffi_ptr_with_types(
            test_mul as usize,
            &[InterpValue::I64(2), InterpValue::I64(3), InterpValue::I64(4)],
            &param_types,
            &return_type,
        ).unwrap();

        match result {
            InterpValue::I64(n) => assert_eq!(n, 24), // 2 * 3 * 4 = 24
            _ => panic!("Expected I64 result, got {:?}", result),
        }
    }

    #[test]
    fn test_ffi_call_float_return() {
        let interp = MirInterpreter::new();

        // Call function that returns f64
        let param_types = vec![IrType::F64, IrType::F64];
        let return_type = IrType::F64;

        let result = interp.call_ffi_ptr_with_types(
            test_f64_add as usize,
            &[InterpValue::F64(1.5), InterpValue::F64(2.5)],
            &param_types,
            &return_type,
        ).unwrap();

        match result {
            InterpValue::F64(n) => assert!((n - 4.0).abs() < 0.001),
            _ => panic!("Expected F64 result, got {:?}", result),
        }
    }

    #[test]
    fn test_native_value_conversions() {
        // Test u64 conversion for various types
        assert_eq!(NativeValue::I32(42).to_u64(), 42);
        assert_eq!(NativeValue::I64(-1).to_u64(), u64::MAX);
        assert_eq!(NativeValue::U8(255).to_u64(), 255);
        assert_eq!(NativeValue::Ptr(0x12345678).to_u64(), 0x12345678);

        // Test i64 conversion
        assert_eq!(NativeValue::I32(-5).to_i64(), -5);
        assert_eq!(NativeValue::U32(100).to_i64(), 100);

        // Test f64 conversion
        assert!((NativeValue::F32(3.14f32).to_f64() - 3.14f64).abs() < 0.01);
        assert_eq!(NativeValue::I64(42).to_f64(), 42.0);
    }
}
