# Generics Design for Rayzor

## Overview

Design for implementing generic types in Rayzor's MIR, based on Zyntax's proven approach but adapted for our SSA-based IR.

## Key Insights from Zyntax

### 1. Type Parameter Representation
- **Type parameters** (like `T` in `Vec<T>`) are represented as `IrType::Opaque(name)` where `name` is an interned string
- **Monomorphization** happens later - each concrete instantiation gets its own specialized function

### 2. Union Types for Sum Types
Zyntax uses tagged unions for `Option<T>` and `Result<T,E>`:

```rust
union Option<T> {
    None,      // discriminant = 0, type = void
    Some(T),   // discriminant = 1, type = T
}
```

Memory layout:
```
[discriminant: i32][value: T (if Some) or padding (if None)]
```

### 3. Vec<T> Memory Structure

```rust
struct Vec<T> {
    ptr: *T,      // Heap-allocated array
    len: usize,   // Number of elements
    cap: usize,   // Allocated capacity
}
```

Growth strategy:
- Initial: 4 elements (16 for Vec<u8>)
- Growth: Double capacity (4 → 8 → 16 → 32...)
- Uses C `realloc()` for resizing

### 4. String Composition

```rust
struct String {
    bytes: Vec<u8>,  // UTF-8 byte storage
}
```

This is a **wrapper** around Vec<u8>, not inheritance.

## Rayzor Implementation Plan

### Phase 1: Type System Extensions

#### Add Generic Type Support to IrType

```rust
// In compiler/src/ir/types.rs
pub enum IrType {
    // ... existing types ...

    /// Type parameter (e.g., "T" in Vec<T>)
    TypeParam(String),

    /// Generic instantiation (e.g., Vec<i32>)
    Generic {
        base: Box<IrType>,      // The generic type (Vec, Option, etc.)
        type_args: Vec<IrType>, // Concrete type arguments
    },

    /// Tagged union type
    Union {
        name: Option<String>,
        variants: Vec<UnionVariant>,
    },

    /// Array type [T; N] (for fixed-size arrays)
    Array {
        element_type: Box<IrType>,
        size: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnionVariant {
    pub name: String,
    pub ty: IrType,
    pub discriminant: u32,
}
```

#### Add Type Parameters to Function Signatures

```rust
// In compiler/src/ir/functions.rs
pub struct IrFunctionSignature {
    pub parameters: Vec<IrParameter>,
    pub return_type: IrType,
    pub calling_convention: CallingConvention,
    pub can_throw: bool,
    pub type_params: Vec<TypeParameter>,  // NEW: Generic parameters
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeParameter {
    pub name: String,
    pub constraints: Vec<String>,  // For future trait bounds
}
```

### Phase 2: MIR Builder Enhancements

#### Add Builder Methods for Generics

```rust
// In compiler/src/ir/mir_builder.rs
impl MirBuilder {
    /// Create a type parameter reference
    pub fn type_param(&mut self, name: impl Into<String>) -> IrType {
        IrType::TypeParam(name.into())
    }

    /// Begin a generic function
    pub fn begin_generic_function(
        &mut self,
        name: impl Into<String>,
        type_params: Vec<String>,
    ) -> FunctionBuilder {
        // Store type params in FunctionBuilder
        FunctionBuilder {
            // ... existing fields ...
            type_params,
        }
    }

    /// Create a union type
    pub fn union_type(
        &mut self,
        name: Option<impl Into<String>>,
        variants: Vec<UnionVariant>,
    ) -> IrType {
        IrType::Union {
            name: name.map(|n| n.into()),
            variants,
        }
    }

    /// Create union value with discriminant
    pub fn create_union(
        &mut self,
        discriminant: u32,
        value: IrId,
        union_ty: IrType,
    ) -> IrId {
        let dest = self.alloc_reg();
        self.insert_inst(IrInstruction::CreateUnion {
            dest,
            discriminant,
            value,
            ty: union_ty,
        });
        dest
    }

    /// Extract discriminant from union
    pub fn extract_discriminant(&mut self, union_val: IrId) -> IrId {
        let dest = self.alloc_reg();
        self.insert_inst(IrInstruction::ExtractDiscriminant {
            dest,
            union_val,
        });
        dest
    }

    /// Extract value from union variant
    pub fn extract_union_value(
        &mut self,
        union_val: IrId,
        discriminant: u32,
        value_ty: IrType,
    ) -> IrId {
        let dest = self.alloc_reg();
        self.insert_inst(IrInstruction::ExtractUnionValue {
            dest,
            union_val,
            discriminant,
            value_ty,
        });
        dest
    }

    /// Create struct from fields
    pub fn create_struct_value(
        &mut self,
        ty: IrType,
        fields: Vec<IrId>,
    ) -> IrId {
        let dest = self.alloc_reg();
        self.insert_inst(IrInstruction::CreateStruct {
            dest,
            ty,
            fields,
        });
        dest
    }

    /// Pointer arithmetic: ptr + offset
    pub fn ptr_add(
        &mut self,
        ptr: IrId,
        offset: IrId,
        result_ty: IrType,
    ) -> IrId {
        let dest = self.alloc_reg();
        self.insert_inst(IrInstruction::PtrAdd {
            dest,
            ptr,
            offset,
            ty: result_ty,
        });
        dest
    }

    /// Integer comparison
    pub fn icmp(
        &mut self,
        op: CompareOp,
        left: IrId,
        right: IrId,
        result_ty: IrType,
    ) -> IrId {
        let dest = self.alloc_reg();
        self.insert_inst(IrInstruction::Cmp {
            dest,
            op,
            left,
            right,
        });
        dest
    }

    /// Arithmetic operations
    pub fn add(&mut self, left: IrId, right: IrId, ty: IrType) -> IrId {
        self.bin_op(BinaryOp::Add, left, right)
    }

    pub fn sub(&mut self, left: IrId, right: IrId, ty: IrType) -> IrId {
        self.bin_op(BinaryOp::Sub, left, right)
    }

    pub fn mul(&mut self, left: IrId, right: IrId, ty: IrType) -> IrId {
        self.bin_op(BinaryOp::Mul, left, right)
    }

    /// Undefined value (for None variant)
    pub fn undef(&mut self, ty: IrType) -> IrId {
        let dest = self.alloc_reg();
        self.insert_inst(IrInstruction::Undef { dest, ty });
        dest
    }

    /// Unit/void value
    pub fn unit_value(&mut self) -> IrId {
        self.undef(IrType::Void)
    }

    /// Panic/abort
    pub fn panic(&mut self) {
        self.insert_inst(IrInstruction::Panic);
    }

    /// Unreachable terminator
    pub fn unreachable(&mut self) {
        self.set_terminator(IrTerminator::Unreachable);
    }

    /// Get function by name for calling
    pub fn get_function_by_name(&self, name: &str) -> Option<IrFunctionId> {
        self.module.functions.iter()
            .find(|(_, f)| f.name == name)
            .map(|(id, _)| *id)
    }

    /// Create function reference for indirect calls
    pub fn function_ref(&mut self, func_id: IrFunctionId) -> IrId {
        let dest = self.alloc_reg();
        self.insert_inst(IrInstruction::FunctionRef { dest, func_id });
        dest
    }
}
```

### Phase 3: New MIR Instructions

Add to `IrInstruction` enum:

```rust
// Union operations
CreateUnion {
    dest: IrId,
    discriminant: u32,
    value: IrId,
    ty: IrType,
},

ExtractDiscriminant {
    dest: IrId,
    union_val: IrId,
},

ExtractUnionValue {
    dest: IrId,
    union_val: IrId,
    discriminant: u32,
    value_ty: IrType,
},

// Struct operations
CreateStruct {
    dest: IrId,
    ty: IrType,
    fields: Vec<IrId>,
},

// Pointer operations
PtrAdd {
    dest: IrId,
    ptr: IrId,
    offset: IrId,
    ty: IrType,
},

// Special values
Undef {
    dest: IrId,
    ty: IrType,
},

FunctionRef {
    dest: IrId,
    func_id: IrFunctionId,
},

// Control flow
Panic,
```

Add to `IrTerminator` enum:

```rust
Unreachable,
```

### Phase 4: Standard Library Implementation

#### 4.1 Memory Management (`compiler/src/stdlib/memory.rs`)

```rust
/// Declare C memory management functions
pub fn build_memory_functions(builder: &mut MirBuilder) {
    declare_malloc(builder);
    declare_realloc(builder);
    declare_free(builder);
}

fn declare_malloc(builder: &mut MirBuilder) {
    builder.begin_function("malloc")
        .param("size", IrType::U64)
        .returns(IrType::Ptr(Box::new(IrType::U8)))
        .calling_convention(CallingConvention::C)
        .extern_func()
        .build();
}

fn declare_realloc(builder: &mut MirBuilder) {
    builder.begin_function("realloc")
        .param("ptr", IrType::Ptr(Box::new(IrType::U8)))
        .param("new_size", IrType::U64)
        .returns(IrType::Ptr(Box::new(IrType::U8)))
        .calling_convention(CallingConvention::C)
        .extern_func()
        .build();
}

fn declare_free(builder: &mut MirBuilder) {
    builder.begin_function("free")
        .param("ptr", IrType::Ptr(Box::new(IrType::U8)))
        .returns(IrType::Void)
        .calling_convention(CallingConvention::C)
        .extern_func()
        .build();
}
```

#### 4.2 Vec<u8> Implementation (`compiler/src/stdlib/vec_u8.rs`)

Full concrete implementation following Zyntax pattern:
- `vec_u8_new()` - Create with capacity 16
- `vec_u8_push()` - Append with dynamic growth
- `vec_u8_pop()` - Remove last element
- `vec_u8_get()` - Bounds-checked access
- `vec_u8_set()` - Bounds-checked write
- `vec_u8_len()` - Get length
- `vec_u8_capacity()` - Get capacity
- `vec_u8_clear()` - Reset length to 0
- `vec_u8_free()` - Deallocate memory

#### 4.3 String Implementation (`compiler/src/stdlib/string_proper.rs`)

Replace placeholder string functions with proper Vec<u8>-backed implementation:

```rust
/// String wraps Vec<u8>
struct String {
    bytes: Vec<u8>,  // UTF-8 encoded bytes
}

// Functions:
// - string_new() -> create empty string via vec_u8_new()
// - string_from_bytes(Vec<u8>) -> wrap vec
// - string_length() -> get vec len
// - string_concat() -> copy both vecs into new vec
// - string_char_at() -> UTF-8 decode at index
// - etc.
```

#### 4.4 Array<T> Generic (`compiler/src/stdlib/array_generic.rs`)

Haxe's dynamic Array<T> type:

```rust
/// Haxe Array<T> (dynamic, growable)
struct Array<T> {
    ptr: *T,
    len: usize,
    cap: usize,
}

// Will be monomorphized for each concrete T used
```

### Phase 5: Monomorphization Strategy

**Approach**: Lazy monomorphization (generate specialized versions on-demand)

```rust
// In compiler/src/ir/monomorphize.rs

pub struct Monomorphizer {
    /// Cache of generated specializations
    instances: HashMap<MonoKey, IrFunctionId>,
    next_instance_id: u32,
}

#[derive(Hash, Eq, PartialEq)]
struct MonoKey {
    generic_func: IrFunctionId,
    type_args: Vec<IrType>,
}

impl Monomorphizer {
    /// Monomorphize a generic function call
    pub fn instantiate(
        &mut self,
        generic_func: &IrFunction,
        type_args: Vec<IrType>,
    ) -> IrFunctionId {
        let key = MonoKey {
            generic_func: generic_func.id,
            type_args: type_args.clone(),
        };

        // Check cache
        if let Some(&existing) = self.instances.get(&key) {
            return existing;
        }

        // Generate new specialization
        let new_func = self.specialize(generic_func, &type_args);
        let new_id = new_func.id;

        self.instances.insert(key, new_id);
        new_id
    }

    fn specialize(&self, func: &IrFunction, type_args: &[IrType]) -> IrFunction {
        // 1. Create substitution map: T -> concrete_type
        let mut subst_map = HashMap::new();
        for (param, arg) in func.signature.type_params.iter().zip(type_args) {
            subst_map.insert(param.name.clone(), arg.clone());
        }

        // 2. Clone function and substitute all type references
        let mut specialized = func.clone();
        specialized.id = IrFunctionId(self.next_instance_id);
        specialized.name = format!("{}__mono_{}", func.name, self.next_instance_id);

        // 3. Walk all instructions and substitute types
        self.substitute_types_in_function(&mut specialized, &subst_map);

        specialized
    }
}
```

## Implementation Order

1. ✅ **Add type system extensions** (IrType variants, function signature)
2. ✅ **Add MIR instructions** (CreateUnion, ExtractDiscriminant, etc.)
3. ✅ **Enhance MIR builder** (type_param(), union_type(), etc.)
4. ✅ **Implement memory functions** (malloc/realloc/free declarations)
5. ✅ **Implement Vec<u8>** (complete concrete implementation)
6. ✅ **Reimplement String** (using Vec<u8> backing)
7. ✅ **Implement Array<T>** (generic Haxe array)
8. ⏳ **Add monomorphization pass** (lazy instantiation)
9. ⏳ **Test and validate**

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_vec_u8_push_pop() {
    let stdlib = build_stdlib();
    // Verify vec_u8_push and vec_u8_pop exist and validate
}

#[test]
fn test_string_concat() {
    // Build two strings, concat, verify result
}
```

### Integration Tests
```haxe
class Main {
    static function main() {
        var vec = new Array<Int>();
        vec.push(1);
        vec.push(2);
        trace(vec.length); // Should print 2

        var s = "Hello" + " World";
        trace(s); // Should print "Hello World"
    }
}
```

## Benefits of This Approach

1. **Type Safety**: Generic types are checked at compile time
2. **Performance**: Monomorphization eliminates runtime overhead
3. **Flexibility**: Same code works for any type T
4. **Proven**: Based on Zyntax's working implementation
5. **Incremental**: Can implement Vec<u8> first, then generics

## Next Steps

1. Start with Phase 1: Extend type system
2. Implement Vec<u8> as proof-of-concept
3. Test with actual string operations
4. Expand to full generic support

This design provides a solid foundation for Haxe's type system in Rayzor!
