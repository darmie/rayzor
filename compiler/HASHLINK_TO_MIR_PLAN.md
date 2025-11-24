# HashLink Bytecode → Rayzor MIR Compilation Plan

## Executive Summary

**Goal:** Enable Rayzor to execute existing HashLink `.hl` bytecode files, providing a drop-in replacement runtime that can leverage Rayzor's high-performance JIT compilation (Cranelift + LLVM).

**Benefits:**
- Instant compatibility with existing HashLink applications
- A/B testing: Compare Rayzor JIT performance vs HashLink VM
- Migration path for HashLink users to Rayzor
- Validation of Rayzor's runtime correctness
- Access to HashLink's extensive library ecosystem

**Status:** Planning Phase
**Estimated Timeline:** 3-4 weeks
**Priority:** High (enables testing and validation)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    HashLink Bytecode (.hl)                       │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│              HashLink Bytecode Parser/Decoder                    │
│  - Read .hl file format                                          │
│  - Parse bytecode structure (functions, types, constants)        │
│  - Build in-memory representation                                │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│              HashLink → Rayzor MIR Translator                    │
│  - Map HL types to MIR types                                     │
│  - Translate HL opcodes to MIR instructions                      │
│  - Build MIR CFG from HL bytecode                                │
│  - Handle HL calling conventions                                 │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                     Rayzor MIR Module                            │
│  (Standard MIR representation, same as Haxe→MIR)                │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│              Rayzor JIT Runtime (Existing)                       │
│  - Cranelift JIT (cold path)                                     │
│  - LLVM JIT (hot path tier-up)                                   │
│  - Native execution                                              │
└─────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: HashLink Bytecode Understanding (Week 1)

### 1.1 HashLink Bytecode Format (.hl)

**File Structure:**
```
.hl File Format:
┌──────────────────────┐
│ Magic Header "HLB"   │
│ Version (3 bytes)    │
├──────────────────────┤
│ Integers Pool        │
│ Floats Pool          │
│ Strings Pool         │
│ Bytes Pool           │
├──────────────────────┤
│ Types Table          │
│  - Basic types       │
│  - Composite types   │
│  - Function sigs     │
├──────────────────────┤
│ Globals Table        │
│ Natives Table        │
│ Functions Table      │
│  - Bytecode per fn   │
│  - Debug info        │
├──────────────────────┤
│ Entry Point          │
└──────────────────────┘
```

**Key Resources:**
- HashLink repository: https://github.com/HaxeFoundation/hashlink
- Bytecode format: `hashlink/src/code.h` and `hashlink/src/code.c`
- Opcode definitions: `hashlink/src/opcodes.h`
- Type system: `hashlink/src/hlmodule.h`

**Deliverables:**
- [ ] Document complete .hl format specification
- [ ] Create Rust struct definitions for .hl format
- [ ] List all HashLink opcodes with semantics
- [ ] Map HashLink type system to MIR types

### 1.2 HashLink Type System

**HashLink Types:**
```rust
// Basic types
HVoid       // void
HI8         // i8
HI16        // i16
HI32        // i32
HF32        // f32
HF64        // f64
HBool       // bool
HBytes      // byte buffer
HDyn        // dynamic/Any

// Composite types
HObj        // object/class instance
HArray      // array
HType       // type value
HRef        // reference
HVirtual    // virtual class
HDynObj     // dynamic object
HAbstract   // abstract type
HEnum       // enum
HNull       // nullable wrapper

// Functions
HFun { args: Vec<HType>, ret: HType }

// Advanced
HMethod     // method reference
HStruct     // struct
HPacked     // packed type
```

**Type Mapping Strategy:**
```rust
// HashLink Type → Rayzor MIR Type
HVoid       → IrType::Void
HI8         → IrType::I8
HI16        → IrType::I16
HI32        → IrType::I32
HF32        → IrType::F32
HF64        → IrType::F64
HBool       → IrType::Bool
HBytes      → IrType::Ptr(IrType::U8)  // byte pointer
HDyn        → IrType::Any              // boxed value
HObj        → IrType::Ptr(IrType::Struct { ... })
HArray      → IrType::Ptr(IrType::Array(...))
HFun        → IrType::Function { params, return_type }
HNull<T>    → IrType::Option(Box<T>)   // or tagged union
```

### 1.3 HashLink Opcodes

**Categories:**
1. **Stack Operations:** mov, push, pop
2. **Arithmetic:** add, sub, mul, div, mod, shl, shr, and, or, xor, neg, not
3. **Comparisons:** eq, neq, lt, lte, gt, gte
4. **Control Flow:** jmp, jtrue, jfalse, jnull, jnotnull, switch
5. **Function Calls:** call, vcall, ocall, fcall
6. **Memory:** getglobal, setglobal, field, setfield, getarray, setarray
7. **Object Operations:** new, type, instanceof, cast
8. **Type Conversions:** toint, tofloat, tostring
9. **Special:** ret, throw, rethrow, trap

**Total Opcodes:** ~80-100 opcodes

**Example Opcode → MIR Mapping:**
```rust
// HL: OAdd r0 r1 r2  (r0 = r1 + r2)
// MIR: BinOp { dest: r0, op: Add, left: r1, right: r2 }

// HL: OJTrue r0 offset
// MIR: Cmp { dest: cond, op: Ne, left: r0, right: const(0) }
//      CondBranch { condition: cond, true_target: block_at(offset) }

// HL: OCall2 ret_reg func_reg arg1 arg2
// MIR: Call { dest: Some(ret_reg), func: func_reg, args: [arg1, arg2] }
```

**Deliverables:**
- [ ] Complete opcode reference table (HL opcode → MIR instruction)
- [ ] Document complex opcode semantics
- [ ] Identify opcodes requiring runtime support

---

## Phase 2: Bytecode Parser Implementation (Week 1-2)

### 2.1 Binary Format Parser

**File:** `compiler/src/hashlink/bytecode_parser.rs`

```rust
/// HashLink bytecode parser
pub struct HlBytecodeParser {
    data: Vec<u8>,
    offset: usize,
}

impl HlBytecodeParser {
    pub fn parse(data: Vec<u8>) -> Result<HlModule, HlParseError> {
        let mut parser = Self { data, offset: 0 };

        // Parse header
        parser.parse_header()?;

        // Parse constant pools
        let integers = parser.parse_int_pool()?;
        let floats = parser.parse_float_pool()?;
        let strings = parser.parse_string_pool()?;
        let bytes = parser.parse_bytes_pool()?;

        // Parse type table
        let types = parser.parse_types()?;

        // Parse globals and natives
        let globals = parser.parse_globals()?;
        let natives = parser.parse_natives()?;

        // Parse functions
        let functions = parser.parse_functions()?;

        // Parse debug info (optional)
        let debug_info = parser.parse_debug_info()?;

        Ok(HlModule {
            integers,
            floats,
            strings,
            bytes,
            types,
            globals,
            natives,
            functions,
            debug_info,
            entry_point: parser.parse_entry_point()?,
        })
    }

    // Helper methods
    fn read_u8(&mut self) -> Result<u8, HlParseError> { /* ... */ }
    fn read_i32(&mut self) -> Result<i32, HlParseError> { /* ... */ }
    fn read_f64(&mut self) -> Result<f64, HlParseError> { /* ... */ }
    fn read_string(&mut self) -> Result<String, HlParseError> { /* ... */ }
    fn read_index(&mut self) -> Result<usize, HlParseError> { /* ... */ }
}
```

**Data Structures:**
```rust
pub struct HlModule {
    pub integers: Vec<i32>,
    pub floats: Vec<f64>,
    pub strings: Vec<String>,
    pub bytes: Vec<Vec<u8>>,
    pub types: Vec<HlType>,
    pub globals: Vec<HlGlobal>,
    pub natives: Vec<HlNative>,
    pub functions: Vec<HlFunction>,
    pub debug_info: Option<HlDebugInfo>,
    pub entry_point: usize,
}

pub struct HlFunction {
    pub type_idx: usize,  // Index into types table
    pub regs: Vec<usize>,  // Register type indices
    pub ops: Vec<HlOp>,    // Bytecode operations
    pub debug_info: Option<HlFunctionDebug>,
}

pub struct HlOp {
    pub opcode: HlOpcode,
    pub args: Vec<i32>,  // Register indices or immediate values
}

pub enum HlOpcode {
    Mov,
    Add, Sub, Mul, Div,
    Call0, Call1, Call2, Call3, Call4, CallN,
    Jump, JTrue, JFalse,
    Ret,
    GetGlobal, SetGlobal,
    // ... all ~80-100 opcodes
}
```

**Deliverables:**
- [ ] Complete bytecode parser implementation
- [ ] Unit tests for each section parser
- [ ] Error handling with detailed diagnostics
- [ ] Benchmark: Parse common .hl files in < 10ms

### 2.2 Bytecode Validation

**File:** `compiler/src/hashlink/bytecode_validator.rs`

```rust
pub struct HlValidator {
    module: HlModule,
}

impl HlValidator {
    pub fn validate(module: &HlModule) -> Result<(), Vec<HlValidationError>> {
        let mut errors = Vec::new();

        // Validate types
        Self::validate_types(&module.types, &mut errors);

        // Validate functions
        for (idx, func) in module.functions.iter().enumerate() {
            Self::validate_function(func, &module.types, idx, &mut errors);
        }

        // Validate entry point
        Self::validate_entry_point(module, &mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_function(
        func: &HlFunction,
        types: &[HlType],
        func_idx: usize,
        errors: &mut Vec<HlValidationError>,
    ) {
        // Check register types are valid
        for (reg_idx, type_idx) in func.regs.iter().enumerate() {
            if *type_idx >= types.len() {
                errors.push(HlValidationError::InvalidTypeIndex {
                    func_idx,
                    reg_idx,
                    type_idx: *type_idx,
                });
            }
        }

        // Check opcodes are valid
        for (op_idx, op) in func.ops.iter().enumerate() {
            Self::validate_opcode(op, func, types, func_idx, op_idx, errors);
        }
    }

    fn validate_opcode(/* ... */) { /* ... */ }
}
```

**Validation Checks:**
- [ ] All type indices are valid
- [ ] All register references are in bounds
- [ ] Jump targets are valid
- [ ] Function signatures match call sites
- [ ] Entry point exists and is valid

**Deliverables:**
- [ ] Complete validation implementation
- [ ] Comprehensive error messages
- [ ] Test suite with valid/invalid .hl files

---

## Phase 3: HashLink → MIR Translation (Week 2-3)

### 3.1 Type Translation

**File:** `compiler/src/hashlink/type_translator.rs`

```rust
pub struct HlTypeTranslator {
    hl_types: Vec<HlType>,
    mir_types: HashMap<usize, IrType>,  // HL type idx → MIR type
}

impl HlTypeTranslator {
    pub fn translate_type(&mut self, hl_type_idx: usize) -> Result<IrType, TranslateError> {
        // Check cache
        if let Some(mir_type) = self.mir_types.get(&hl_type_idx) {
            return Ok(mir_type.clone());
        }

        let hl_type = &self.hl_types[hl_type_idx];
        let mir_type = match hl_type {
            HlType::Void => IrType::Void,
            HlType::I32 => IrType::I32,
            HlType::F64 => IrType::F64,
            HlType::Bool => IrType::Bool,
            HlType::Bytes => IrType::Ptr(Box::new(IrType::U8)),
            HlType::Dyn => IrType::Any,

            HlType::Fun { args, ret } => {
                let params = args.iter()
                    .map(|idx| self.translate_type(*idx))
                    .collect::<Result<Vec<_>, _>>()?;
                let return_type = Box::new(self.translate_type(*ret)?);
                IrType::Function { params, return_type }
            }

            HlType::Obj { name, fields, .. } => {
                let field_types = fields.iter()
                    .map(|f| (f.name.clone(), self.translate_type(f.type_idx)))
                    .collect::<Result<Vec<_>, _>>()?;
                IrType::Struct {
                    name: name.clone(),
                    fields: field_types,
                }
            }

            HlType::Array(elem_type_idx) => {
                let elem_type = Box::new(self.translate_type(*elem_type_idx)?);
                IrType::Ptr(Box::new(IrType::Array(elem_type, 0))) // Dynamic size
            }

            HlType::Null(inner_idx) => {
                // Nullable type - represent as Option or tagged union
                let inner = self.translate_type(*inner_idx)?;
                // TODO: Decide on nullable representation
                // Option 1: IrType::Nullable(Box::new(inner))
                // Option 2: IrType::Union with null variant
                IrType::Any // Placeholder
            }

            // ... handle all HL types
        };

        self.mir_types.insert(hl_type_idx, mir_type.clone());
        Ok(mir_type)
    }
}
```

**Special Cases:**
- **Dynamic types (HDyn):** Map to `IrType::Any` with runtime boxing
- **Nullable types:** Need nullable representation in MIR
- **Virtual methods:** Require vtable support in runtime
- **Closures:** Capture environment in heap-allocated structure

**Deliverables:**
- [ ] Complete type translation for all HL types
- [ ] Handle recursive types correctly
- [ ] Test with complex HL type structures

### 3.2 Function Translation

**File:** `compiler/src/hashlink/function_translator.rs`

```rust
pub struct HlFunctionTranslator<'a> {
    hl_module: &'a HlModule,
    type_translator: &'a HlTypeTranslator,
    current_function: &'a HlFunction,

    // Translation state
    mir_function: IrFunction,
    block_map: HashMap<usize, IrBlockId>,  // HL offset → MIR block
    register_map: HashMap<usize, IrId>,    // HL reg → MIR value
}

impl<'a> HlFunctionTranslator<'a> {
    pub fn translate_function(
        hl_func: &HlFunction,
        func_idx: usize,
        hl_module: &HlModule,
        type_translator: &HlTypeTranslator,
    ) -> Result<IrFunction, TranslateError> {
        let mut translator = Self::new(hl_func, func_idx, hl_module, type_translator);

        // Step 1: Create MIR function signature
        translator.translate_signature()?;

        // Step 2: Create MIR registers for HL registers
        translator.allocate_registers()?;

        // Step 3: Build CFG structure (identify basic blocks)
        translator.build_cfg()?;

        // Step 4: Translate opcodes to MIR instructions
        translator.translate_opcodes()?;

        Ok(translator.mir_function)
    }

    fn translate_signature(&mut self) -> Result<(), TranslateError> {
        let hl_func_type = &self.hl_module.types[self.current_function.type_idx];

        if let HlType::Fun { args, ret } = hl_func_type {
            let parameters = args.iter().enumerate()
                .map(|(idx, type_idx)| {
                    let ty = self.type_translator.translate_type(*type_idx)?;
                    let reg = IrId::new(idx as u32);
                    Ok(IrParameter {
                        name: format!("arg{}", idx),
                        ty,
                        reg,
                        by_ref: false,
                    })
                })
                .collect::<Result<Vec<_>, TranslateError>>()?;

            let return_type = self.type_translator.translate_type(*ret)?;

            self.mir_function.signature = IrFunctionSignature {
                parameters,
                return_type,
                calling_convention: CallingConvention::Haxe,
                can_throw: true,  // HL functions can throw
                type_params: vec![],
            };
        }

        Ok(())
    }

    fn build_cfg(&mut self) -> Result<(), TranslateError> {
        // Identify basic block boundaries
        let mut block_starts = std::collections::BTreeSet::new();
        block_starts.insert(0);  // Entry block

        // Scan for jump targets and control flow
        for (offset, op) in self.current_function.ops.iter().enumerate() {
            match op.opcode {
                HlOpcode::Jump => {
                    let target = op.args[0] as usize;
                    block_starts.insert(target);
                    block_starts.insert(offset + 1);  // Block after jump
                }
                HlOpcode::JTrue | HlOpcode::JFalse => {
                    let target = op.args[1] as usize;
                    block_starts.insert(target);
                    block_starts.insert(offset + 1);  // Fallthrough block
                }
                HlOpcode::Ret | HlOpcode::Throw => {
                    block_starts.insert(offset + 1);  // Block after terminator
                }
                HlOpcode::Switch => {
                    // Multiple jump targets
                    let num_cases = op.args[1] as usize;
                    for i in 0..num_cases {
                        let target = op.args[2 + i] as usize;
                        block_starts.insert(target);
                    }
                    block_starts.insert(offset + 1);
                }
                _ => {}
            }
        }

        // Create MIR blocks for each boundary
        for start_offset in block_starts {
            if start_offset == 0 {
                self.block_map.insert(0, self.mir_function.cfg.entry_block);
            } else {
                let block_id = self.mir_function.cfg.create_block();
                self.block_map.insert(start_offset, block_id);
            }
        }

        Ok(())
    }

    fn translate_opcodes(&mut self) -> Result<(), TranslateError> {
        let mut current_block = self.mir_function.cfg.entry_block;

        for (offset, op) in self.current_function.ops.iter().enumerate() {
            // Check if we need to switch blocks
            if let Some(&new_block) = self.block_map.get(&offset) {
                if new_block != current_block {
                    current_block = new_block;
                }
            }

            // Translate opcode to MIR instruction(s)
            self.translate_opcode(op, current_block)?;
        }

        Ok(())
    }

    fn translate_opcode(
        &mut self,
        op: &HlOp,
        block: IrBlockId,
    ) -> Result<(), TranslateError> {
        let block_mut = self.mir_function.cfg.blocks.get_mut(&block).unwrap();

        match op.opcode {
            // Arithmetic operations
            HlOpcode::Add => {
                // HL: Add dst src1 src2
                let dest = self.get_register(op.args[0] as usize);
                let left = self.get_register(op.args[1] as usize);
                let right = self.get_register(op.args[2] as usize);

                block_mut.instructions.push(IrInstruction::BinOp {
                    dest,
                    op: BinaryOp::Add,
                    left,
                    right,
                });
            }

            HlOpcode::Sub => {
                let dest = self.get_register(op.args[0] as usize);
                let left = self.get_register(op.args[1] as usize);
                let right = self.get_register(op.args[2] as usize);

                block_mut.instructions.push(IrInstruction::BinOp {
                    dest,
                    op: BinaryOp::Sub,
                    left,
                    right,
                });
            }

            // ... similar for Mul, Div, Mod, etc.

            // Comparisons
            HlOpcode::Eq => {
                let dest = self.get_register(op.args[0] as usize);
                let left = self.get_register(op.args[1] as usize);
                let right = self.get_register(op.args[2] as usize);

                block_mut.instructions.push(IrInstruction::Cmp {
                    dest,
                    op: CompareOp::Eq,
                    left,
                    right,
                });
            }

            // Control flow
            HlOpcode::Jump => {
                let target_offset = op.args[0] as usize;
                let target_block = *self.block_map.get(&target_offset).unwrap();

                block_mut.terminator = IrTerminator::Branch {
                    target: target_block,
                };
            }

            HlOpcode::JTrue => {
                let condition = self.get_register(op.args[0] as usize);
                let target_offset = op.args[1] as usize;
                let target_block = *self.block_map.get(&target_offset).unwrap();

                // Find fallthrough block (next offset)
                let fallthrough_offset = offset + 1;
                let fallthrough_block = *self.block_map.get(&fallthrough_offset).unwrap();

                block_mut.terminator = IrTerminator::CondBranch {
                    condition,
                    true_target: target_block,
                    false_target: fallthrough_block,
                };
            }

            HlOpcode::Ret => {
                let value = if op.args.is_empty() {
                    None
                } else {
                    Some(self.get_register(op.args[0] as usize))
                };

                block_mut.terminator = IrTerminator::Return { value };
            }

            // Function calls
            HlOpcode::Call0 | HlOpcode::Call1 | HlOpcode::Call2 |
            HlOpcode::Call3 | HlOpcode::Call4 | HlOpcode::CallN => {
                let dest = if op.args[0] >= 0 {
                    Some(self.get_register(op.args[0] as usize))
                } else {
                    None
                };

                let func = self.get_register(op.args[1] as usize);

                let num_args = match op.opcode {
                    HlOpcode::Call0 => 0,
                    HlOpcode::Call1 => 1,
                    HlOpcode::Call2 => 2,
                    HlOpcode::Call3 => 3,
                    HlOpcode::Call4 => 4,
                    HlOpcode::CallN => op.args[2] as usize,
                    _ => unreachable!(),
                };

                let args = (0..num_args)
                    .map(|i| self.get_register(op.args[2 + i] as usize))
                    .collect();

                block_mut.instructions.push(IrInstruction::Call {
                    dest,
                    func,
                    args,
                });
            }

            // Memory operations
            HlOpcode::GetGlobal => {
                let dest = self.get_register(op.args[0] as usize);
                let global_idx = op.args[1] as usize;

                // Translate to load from global address
                // (requires runtime support for global table)
                block_mut.instructions.push(IrInstruction::Load {
                    dest,
                    ptr: self.get_global_ptr(global_idx),
                    ty: self.get_register_type(dest),
                });
            }

            HlOpcode::SetGlobal => {
                let value = self.get_register(op.args[0] as usize);
                let global_idx = op.args[1] as usize;

                block_mut.instructions.push(IrInstruction::Store {
                    ptr: self.get_global_ptr(global_idx),
                    value,
                });
            }

            // TODO: Implement all remaining opcodes
            _ => {
                return Err(TranslateError::UnsupportedOpcode {
                    opcode: op.opcode,
                    offset,
                });
            }
        }

        Ok(())
    }

    fn get_register(&mut self, hl_reg: usize) -> IrId {
        *self.register_map.get(&hl_reg).unwrap()
    }

    fn get_global_ptr(&self, global_idx: usize) -> IrId {
        // TODO: Create global pointer lookup
        IrId::new(1000 + global_idx as u32)
    }
}
```

**Complex Cases:**

1. **Dynamic dispatch (OCallMethod, OCallThis):**
   ```rust
   // Requires vtable lookup at runtime
   // 1. Get object pointer
   // 2. Load vtable pointer from object
   // 3. Load method pointer from vtable[method_idx]
   // 4. Call method pointer with this + args
   ```

2. **Closures (OCallClosure):**
   ```rust
   // Closure = { func_ptr, captured_env }
   // 1. Load func_ptr from closure
   // 2. Load env from closure
   // 3. Call func_ptr(env, args...)
   ```

3. **Exception handling (OTrap, OEndTrap, OThrow):**
   ```rust
   // Requires landing pad infrastructure
   // Map to MIR exception handling constructs
   ```

**Deliverables:**
- [ ] Complete opcode translation for all 80-100 opcodes
- [ ] Handle all control flow patterns
- [ ] Support for function calls (all variants)
- [ ] Exception handling translation
- [ ] Test suite with real .hl bytecode

### 3.3 Module Translation

**File:** `compiler/src/hashlink/module_translator.rs`

```rust
pub struct HlModuleTranslator {
    hl_module: HlModule,
    type_translator: HlTypeTranslator,
}

impl HlModuleTranslator {
    pub fn translate(hl_module: HlModule) -> Result<IrModule, TranslateError> {
        let mut translator = Self {
            type_translator: HlTypeTranslator::new(&hl_module.types),
            hl_module,
        };

        let mut mir_module = IrModule::new(
            "hashlink_module".to_string(),
            "bytecode.hl".to_string(),
        );

        // Translate global variables
        translator.translate_globals(&mut mir_module)?;

        // Translate functions
        for (idx, hl_func) in translator.hl_module.functions.iter().enumerate() {
            let mir_func = HlFunctionTranslator::translate_function(
                hl_func,
                idx,
                &translator.hl_module,
                &translator.type_translator,
            )?;
            mir_module.add_function(mir_func);
        }

        // Register native functions
        translator.register_natives(&mut mir_module)?;

        Ok(mir_module)
    }

    fn translate_globals(&mut self, mir_module: &mut IrModule) -> Result<(), TranslateError> {
        for (idx, hl_global) in self.hl_module.globals.iter().enumerate() {
            let ty = self.type_translator.translate_type(hl_global.type_idx)?;

            mir_module.globals.insert(
                IrGlobalId(idx as u32),
                IrGlobal {
                    name: format!("global_{}", idx),
                    ty,
                    mutable: true,
                    linkage: Linkage::Internal,
                    initializer: None,  // Initialized at runtime
                },
            );
        }
        Ok(())
    }

    fn register_natives(&mut self, mir_module: &mut IrModule) -> Result<(), TranslateError> {
        // Register native function stubs
        for (idx, native) in self.hl_module.natives.iter().enumerate() {
            let func_type = &self.hl_module.types[native.findex];
            // Create extern function declaration
            // TODO: Map HL native name to actual native implementation
        }
        Ok(())
    }
}
```

**Deliverables:**
- [ ] Complete module translation
- [ ] Global variable handling
- [ ] Native function registration
- [ ] Integration test with complete .hl files

---

## Phase 4: Runtime Support (Week 3)

### 4.1 HashLink Runtime Library

**File:** `runtime/src/hashlink_compat.rs`

HashLink bytecode relies on runtime support for:

1. **Memory Management:**
   - Garbage collector (or reference counting)
   - Object allocation
   - Array allocation

2. **Dynamic Types:**
   - Boxing/unboxing for HDyn
   - Type checking at runtime
   - Dynamic dispatch

3. **Built-in Functions:**
   - String operations
   - Array operations
   - Math functions
   - I/O functions

4. **Exception Handling:**
   - Exception objects
   - Stack unwinding
   - Try/catch support

**Implementation Strategy:**
```rust
// runtime/src/hashlink_compat.rs

/// HashLink-compatible runtime functions
pub mod hl_runtime {
    use std::any::Any;
    use std::sync::Arc;

    /// Dynamic value (HDyn) representation
    pub struct HlDyn {
        value: Arc<dyn Any>,
        type_info: HlTypeInfo,
    }

    /// Object representation
    pub struct HlObj {
        type_id: usize,
        fields: Vec<HlDyn>,
    }

    /// Array representation
    pub struct HlArray {
        elem_type: HlTypeInfo,
        data: Vec<HlDyn>,
    }

    // Runtime functions called from generated code
    #[no_mangle]
    pub extern "C" fn hl_alloc_obj(type_id: usize, num_fields: usize) -> *mut HlObj {
        // Allocate object on heap
        // TODO: Integrate with GC
        unimplemented!()
    }

    #[no_mangle]
    pub extern "C" fn hl_alloc_array(elem_type: usize, size: usize) -> *mut HlArray {
        unimplemented!()
    }

    #[no_mangle]
    pub extern "C" fn hl_dyn_cast(value: *const HlDyn, target_type: usize) -> *const HlDyn {
        // Dynamic cast with runtime type checking
        unimplemented!()
    }

    #[no_mangle]
    pub extern "C" fn hl_throw(exception: *const HlDyn) -> ! {
        // Throw exception and unwind stack
        unimplemented!()
    }

    // String operations
    #[no_mangle]
    pub extern "C" fn hl_string_concat(s1: *const HlString, s2: *const HlString) -> *mut HlString {
        unimplemented!()
    }

    // ... many more runtime functions
}
```

**Native Function Mapping:**
```rust
// Map HashLink native functions to Rust implementations
pub fn get_native_function(lib: &str, name: &str) -> Option<*const u8> {
    match (lib, name) {
        ("std", "print") => Some(hl_print as *const u8),
        ("std", "throw") => Some(hl_throw as *const u8),
        ("std", "alloc_array") => Some(hl_alloc_array as *const u8),
        // ... hundreds of native functions
        _ => None,
    }
}
```

**Deliverables:**
- [ ] Core runtime functions (alloc, cast, type checking)
- [ ] String operations
- [ ] Array operations
- [ ] Exception handling
- [ ] Native function mapping table
- [ ] GC integration (or RC for MVP)

### 4.2 Testing Infrastructure

**File:** `compiler/tests/hashlink_integration_tests.rs`

```rust
#[test]
fn test_simple_arithmetic() {
    // Compile simple.hl to MIR
    let hl_bytes = std::fs::read("tests/fixtures/simple.hl").unwrap();
    let hl_module = HlBytecodeParser::parse(hl_bytes).unwrap();
    let mir_module = HlModuleTranslator::translate(hl_module).unwrap();

    // Compile to native with Cranelift
    let mut backend = CraneliftBackend::new().unwrap();
    backend.compile_module(&mir_module).unwrap();

    // Execute and verify
    let main_fn = backend.get_function_ptr(IrFunctionId(0)).unwrap();
    let result: i32 = unsafe {
        let f: fn() -> i32 = std::mem::transmute(main_fn);
        f()
    };

    assert_eq!(result, 42);
}

#[test]
fn test_control_flow() {
    // Test if/else, loops, switch
}

#[test]
fn test_function_calls() {
    // Test various call types
}

#[test]
fn test_object_operations() {
    // Test object creation, field access, method calls
}
```

**Test Fixtures:**
Create simple .hl files for testing:
```haxe
// tests/fixtures/simple.hx (compile to simple.hl)
class Simple {
    static function main() {
        return 40 + 2;
    }
}

// tests/fixtures/control_flow.hx
class ControlFlow {
    static function max(a: Int, b: Int): Int {
        return a > b ? a : b;
    }
}

// tests/fixtures/objects.hx
class Point {
    public var x: Int;
    public var y: Int;

    public function new(x: Int, y: Int) {
        this.x = x;
        this.y = y;
    }

    public function distance(): Float {
        return Math.sqrt(x * x + y * y);
    }
}
```

**Deliverables:**
- [ ] Integration test suite
- [ ] Test fixtures (.hl files + expected outputs)
- [ ] Benchmark comparisons (Rayzor JIT vs HashLink VM)
- [ ] Compatibility test matrix

---

## Phase 5: CLI Tool and Integration (Week 4)

### 5.1 CLI Tool

**File:** `rayzor/src/bin/rayzor_hl.rs`

```rust
/// Rayzor HashLink runtime
///
/// Usage:
///   rayzor-hl program.hl                 # Run with Cranelift JIT
///   rayzor-hl --llvm program.hl          # Run with LLVM JIT (tier-up)
///   rayzor-hl --aot program.hl -o prog   # AOT compile
///   rayzor-hl --dump-mir program.hl      # Dump MIR for debugging

use clap::Parser;
use rayzor_compiler::hashlink::*;
use rayzor_compiler::codegen::CraneliftBackend;

#[derive(Parser)]
struct Args {
    /// HashLink bytecode file (.hl)
    #[arg(value_name = "FILE")]
    input: String,

    /// Use LLVM JIT instead of Cranelift
    #[arg(long)]
    llvm: bool,

    /// AOT compile to native executable
    #[arg(long)]
    aot: bool,

    /// Output file for AOT compilation
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// Dump MIR IR to stderr
    #[arg(long)]
    dump_mir: bool,

    /// Dump Cranelift IR to stderr
    #[arg(long)]
    dump_cranelift: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Parse .hl file
    if args.verbose {
        eprintln!("Parsing {}...", args.input);
    }
    let hl_bytes = std::fs::read(&args.input)?;
    let hl_module = HlBytecodeParser::parse(hl_bytes)?;

    // Validate
    if args.verbose {
        eprintln!("Validating bytecode...");
    }
    HlValidator::validate(&hl_module)?;

    // Translate to MIR
    if args.verbose {
        eprintln!("Translating to MIR...");
    }
    let mir_module = HlModuleTranslator::translate(hl_module)?;

    if args.dump_mir {
        eprintln!("=== MIR ===");
        eprintln!("{:#?}", mir_module);
        eprintln!("=== End MIR ===");
    }

    // Compile and execute
    if args.aot {
        // AOT compilation
        todo!("AOT compilation not yet implemented");
    } else if args.llvm {
        // LLVM JIT
        todo!("LLVM backend not yet implemented");
    } else {
        // Cranelift JIT (default)
        if args.verbose {
            eprintln!("Compiling with Cranelift JIT...");
        }

        let mut backend = CraneliftBackend::new()?;
        backend.compile_module(&mir_module)?;

        // Get entry point
        let entry_point_id = IrFunctionId(mir_module.entry_point as u32);
        let entry_fn = backend.get_function_ptr(entry_point_id)?;

        // Execute
        if args.verbose {
            eprintln!("Executing...");
        }

        unsafe {
            let main: fn() -> i32 = std::mem::transmute(entry_fn);
            let exit_code = main();
            std::process::exit(exit_code);
        }
    }
}
```

**Deliverables:**
- [ ] CLI tool implementation
- [ ] Command-line argument parsing
- [ ] Error reporting
- [ ] Usage documentation

### 5.2 Documentation

**File:** `docs/HASHLINK_COMPATIBILITY.md`

```markdown
# HashLink Compatibility Guide

## Overview
Rayzor can execute HashLink bytecode (.hl files) using its high-performance
JIT runtime. This provides a drop-in replacement for the HashLink VM.

## Quick Start
```bash
# Compile Haxe to HashLink bytecode
haxe -hl program.hl -main Main

# Run with Rayzor JIT
rayzor-hl program.hl
```

## Performance Comparison
| Benchmark | HashLink VM | Rayzor JIT (Cranelift) | Rayzor JIT (LLVM) |
|-----------|-------------|------------------------|-------------------|
| mandelbrot | 100ms | 80ms (1.25x) | 45ms (2.2x) |
| nbody | 150ms | 120ms (1.25x) | 65ms (2.3x) |
| ... | | | |

## Compatibility Status
- [x] Basic arithmetic operations
- [x] Control flow (if/else, loops, switch)
- [x] Function calls (static)
- [x] Local variables and registers
- [ ] Dynamic types (HDyn) - In Progress
- [ ] Object-oriented features - Partial
- [ ] Exception handling - Planned
- [ ] Closures - Planned
- [ ] Native functions - Partial (std lib only)

## Known Limitations
1. Some native functions not yet implemented
2. GC is simplified (may use more memory)
3. Debugging info not fully preserved
4. Some edge cases in dynamic dispatch

## Testing Your Application
... [guide for testing HL apps with Rayzor]
```

**Deliverables:**
- [ ] User documentation
- [ ] Compatibility matrix
- [ ] Migration guide
- [ ] Performance tuning guide

---

## Testing and Validation Strategy

### Test Corpus

1. **Unit Tests:**
   - Each opcode translated correctly
   - Type translation accuracy
   - CFG construction correctness

2. **Integration Tests:**
   - Simple programs (arithmetic, loops)
   - Standard library usage
   - Object-oriented code
   - Generic code

3. **Real-World Applications:**
   - Heaps (game framework)
   - Dead Cells game logic
   - Web applications
   - Command-line tools

4. **Benchmark Suite:**
   - Computer Language Benchmarks Game
   - HashLink benchmark suite
   - Custom micro-benchmarks

### Success Criteria

- [ ] 100% opcode coverage
- [ ] 95%+ compatibility with HL standard library
- [ ] Pass 90%+ of HashLink's own test suite
- [ ] Performance: 1.2x-2.5x faster than HashLink VM
- [ ] Memory usage: Within 150% of HashLink VM

---

## Performance Optimization Opportunities

Once the basic translation works, optimize:

1. **Type Specialization:**
   - Specialize HDyn operations when type is known
   - Remove unnecessary boxing/unboxing

2. **Inline Natives:**
   - Inline common native functions
   - Avoid FFI overhead for builtins

3. **Escape Analysis:**
   - Stack-allocate objects that don't escape
   - Reduce GC pressure

4. **Tier-Up to LLVM:**
   - Profile hot functions
   - Recompile with LLVM for 2-3x additional speedup

5. **Devirtualization:**
   - Resolve virtual calls when type is known
   - Inline monomorphic call sites

---

## Timeline and Milestones

### Week 1: Foundation
- [ ] Complete HashLink bytecode format documentation
- [ ] Implement bytecode parser
- [ ] Implement validator
- [ ] Create basic test fixtures

### Week 2: Core Translation
- [ ] Type translation (all HL types → MIR types)
- [ ] Function translation (basic opcodes)
- [ ] CFG construction
- [ ] Simple arithmetic tests passing

### Week 3: Advanced Features
- [ ] All opcodes implemented
- [ ] Function calls working
- [ ] Control flow working
- [ ] Basic runtime support

### Week 4: Integration and Testing
- [ ] CLI tool complete
- [ ] Integration tests passing
- [ ] Performance benchmarks
- [ ] Documentation complete

---

## Risk Assessment

### High Risk
- **Complex Opcodes:** Some opcodes (dynamic dispatch, closures) are complex
  - *Mitigation:* Start with simple opcodes, add complexity incrementally
- **Runtime Dependencies:** Need significant runtime support
  - *Mitigation:* Minimal runtime for MVP, expand later

### Medium Risk
- **Type System Mismatch:** HL types may not map cleanly to MIR
  - *Mitigation:* Use Any/boxed values where needed
- **Performance:** Initial version may be slower than HL VM
  - *Mitigation:* Optimize after correctness achieved

### Low Risk
- **Parsing:** File format is well-documented
- **Testing:** Can use existing HL test suite

---

## Future Enhancements

1. **Partial Evaluation:**
   - Specialize functions based on constant arguments
   - Eliminate dynamic checks when safe

2. **Profile-Guided Optimization:**
   - Collect runtime type profiles
   - Specialize based on actual usage patterns

3. **Incremental Compilation:**
   - Compile functions lazily on first call
   - Faster startup time

4. **Bytecode Optimization:**
   - Optimize MIR before JIT compilation
   - Dead code elimination
   - Constant propagation

---

## References

- HashLink Repository: https://github.com/HaxeFoundation/hashlink
- HashLink Bytecode Spec: `hashlink/src/code.h`
- HashLink Opcodes: `hashlink/src/opcodes.h`
- Haxe Manual: https://haxe.org/manual/
- Rayzor Architecture: `RAYZOR_ARCHITECTURE.md`

---

## Team and Resources

**Required Skills:**
- Rust programming
- Understanding of bytecode VMs
- Knowledge of JIT compilation
- Familiarity with Haxe/HashLink

**Estimated Effort:**
- 1 engineer: 3-4 weeks
- 2 engineers: 2-3 weeks (parallelizable work)

**Dependencies:**
- Cranelift JIT backend (✅ complete)
- MIR definition (✅ complete)
- Basic runtime infrastructure (⏳ needed)

---

## Conclusion

This plan provides a comprehensive roadmap for adding HashLink bytecode support to Rayzor. The work is well-scoped and achievable in 3-4 weeks. Success will provide:

1. **Validation:** Prove Rayzor's runtime correctness
2. **Adoption:** Lower barrier to entry for HashLink users
3. **Performance:** Demonstrate JIT performance advantages
4. **Ecosystem:** Access to HashLink's libraries and tools

The phased approach allows for incremental progress with working prototypes at each stage. Testing against real HashLink applications will ensure compatibility and drive optimization efforts.
