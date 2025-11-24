//! Runtime Type System for Dynamic values
//!
//! This module implements runtime type information (RTTI) for Haxe Dynamic values.
//! Each Dynamic value is represented as a tagged union: (type_id, value_ptr)
//!
//! ## Architecture
//!
//! - TypeId: Unique identifier for each type (Int, Float, Bool, String, classes, etc.)
//! - TypeInfo: Metadata for each type (size, alignment, toString, etc.)
//! - Type Registry: Global registry mapping TypeId -> TypeInfo
//!
//! ## Usage
//!
//! 1. Boxing: Convert a concrete value to Dynamic
//!    ```
//!    let dynamic = box_int(42);  // Returns (TYPE_INT, ptr)
//!    ```
//!
//! 2. Unboxing: Extract concrete value from Dynamic
//!    ```
//!    let value = unbox_int(dynamic);  // Returns 42
//!    ```
//!
//! 3. toString: Convert any Dynamic value to String
//!    ```
//!    let s = dynamic_to_string(dynamic);  // Dispatches based on type_id
//!    ```

use std::sync::RwLock;
use std::collections::HashMap;

/// Runtime type identifier
///
/// Each type in the Haxe type system gets a unique TypeId.
/// Primitive types have fixed IDs, classes get dynamic IDs.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

// Fixed type IDs for primitive types
pub const TYPE_VOID: TypeId = TypeId(0);
pub const TYPE_NULL: TypeId = TypeId(1);
pub const TYPE_BOOL: TypeId = TypeId(2);
pub const TYPE_INT: TypeId = TypeId(3);
pub const TYPE_FLOAT: TypeId = TypeId(4);
pub const TYPE_STRING: TypeId = TypeId(5);

// Starting ID for user-defined types (classes, enums, etc.)
pub const TYPE_USER_START: u32 = 1000;

/// Dynamic value: tagged union of (type_id, value_ptr)
///
/// This is the runtime representation of Haxe's Dynamic type.
/// The value_ptr points to heap-allocated memory containing the actual value.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DynamicValue {
    pub type_id: TypeId,
    pub value_ptr: *mut u8,
}

/// Function pointer type for toString implementations
///
/// Takes a pointer to the value and returns a String pointer (ptr + len)
pub type ToStringFn = unsafe extern "C" fn(*const u8) -> StringPtr;

/// String representation: pointer + length
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StringPtr {
    pub ptr: *const u8,
    pub len: usize,
}

/// Type metadata
///
/// Contains all runtime information needed for a type:
/// - Size and alignment for memory allocation
/// - toString function for string conversion
/// - Type name for debugging
#[derive(Clone)]
pub struct TypeInfo {
    pub name: &'static str,
    pub size: usize,
    pub align: usize,
    pub to_string: ToStringFn,
}

/// Global type registry
///
/// Maps TypeId -> TypeInfo for runtime type dispatch
static TYPE_REGISTRY: RwLock<Option<HashMap<TypeId, TypeInfo>>> = RwLock::new(None);

/// Initialize the type registry with primitive types
pub fn init_type_system() {
    let mut registry = HashMap::new();

    // Register primitive types
    registry.insert(TYPE_VOID, TypeInfo {
        name: "Void",
        size: 0,
        align: 1,
        to_string: void_to_string,
    });

    registry.insert(TYPE_NULL, TypeInfo {
        name: "Null",
        size: 0,
        align: 1,
        to_string: null_to_string,
    });

    registry.insert(TYPE_BOOL, TypeInfo {
        name: "Bool",
        size: std::mem::size_of::<bool>(),
        align: std::mem::align_of::<bool>(),
        to_string: bool_to_string,
    });

    registry.insert(TYPE_INT, TypeInfo {
        name: "Int",
        size: std::mem::size_of::<i64>(),
        align: std::mem::align_of::<i64>(),
        to_string: int_to_string,
    });

    registry.insert(TYPE_FLOAT, TypeInfo {
        name: "Float",
        size: std::mem::size_of::<f64>(),
        align: std::mem::align_of::<f64>(),
        to_string: float_to_string,
    });

    registry.insert(TYPE_STRING, TypeInfo {
        name: "String",
        size: std::mem::size_of::<StringPtr>(),
        align: std::mem::align_of::<StringPtr>(),
        to_string: string_to_string,
    });

    *TYPE_REGISTRY.write().unwrap() = Some(registry);
}

/// Register a user-defined type (class, enum, etc.)
pub fn register_type(type_id: TypeId, info: TypeInfo) {
    let mut registry = TYPE_REGISTRY.write().unwrap();
    if let Some(ref mut map) = *registry {
        map.insert(type_id, info);
    }
}

/// Get type info for a TypeId
pub fn get_type_info(type_id: TypeId) -> Option<TypeInfo> {
    let registry = TYPE_REGISTRY.read().unwrap();
    registry.as_ref()?.get(&type_id).cloned()
}

// ============================================================================
// toString implementations for primitive types
// ============================================================================

unsafe extern "C" fn void_to_string(_value_ptr: *const u8) -> StringPtr {
    let s = "void";
    StringPtr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

unsafe extern "C" fn null_to_string(_value_ptr: *const u8) -> StringPtr {
    let s = "null";
    StringPtr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

unsafe extern "C" fn bool_to_string(value_ptr: *const u8) -> StringPtr {
    let value = *(value_ptr as *const bool);
    let s = if value { "true" } else { "false" };
    StringPtr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

unsafe extern "C" fn int_to_string(value_ptr: *const u8) -> StringPtr {
    let value = *(value_ptr as *const i64);
    let s = value.to_string();
    // UNSAFE: Leaking memory! Need proper string management
    // TODO: Use a string pool or return owned strings
    let s_static = Box::leak(s.into_boxed_str());
    StringPtr {
        ptr: s_static.as_ptr(),
        len: s_static.len(),
    }
}

unsafe extern "C" fn float_to_string(value_ptr: *const u8) -> StringPtr {
    let value = *(value_ptr as *const f64);
    let s = value.to_string();
    // UNSAFE: Leaking memory! Need proper string management
    let s_static = Box::leak(s.into_boxed_str());
    StringPtr {
        ptr: s_static.as_ptr(),
        len: s_static.len(),
    }
}

unsafe extern "C" fn string_to_string(value_ptr: *const u8) -> StringPtr {
    // String is already a StringPtr, just return it
    *(value_ptr as *const StringPtr)
}

// ============================================================================
// Boxing functions: Convert concrete values to Dynamic
// ============================================================================

/// Box an Int as Dynamic
#[no_mangle]
pub extern "C" fn haxe_box_int(value: i64) -> DynamicValue {
    unsafe {
        let ptr = libc::malloc(std::mem::size_of::<i64>()) as *mut i64;
        *ptr = value;
        DynamicValue {
            type_id: TYPE_INT,
            value_ptr: ptr as *mut u8,
        }
    }
}

/// Box a Float as Dynamic
#[no_mangle]
pub extern "C" fn haxe_box_float(value: f64) -> DynamicValue {
    unsafe {
        let ptr = libc::malloc(std::mem::size_of::<f64>()) as *mut f64;
        *ptr = value;
        DynamicValue {
            type_id: TYPE_FLOAT,
            value_ptr: ptr as *mut u8,
        }
    }
}

/// Box a Bool as Dynamic
#[no_mangle]
pub extern "C" fn haxe_box_bool(value: bool) -> DynamicValue {
    unsafe {
        let ptr = libc::malloc(std::mem::size_of::<bool>()) as *mut bool;
        *ptr = value;
        DynamicValue {
            type_id: TYPE_BOOL,
            value_ptr: ptr as *mut u8,
        }
    }
}

/// Box a String as Dynamic
#[no_mangle]
pub extern "C" fn haxe_box_string(str_ptr: *const u8, len: usize) -> DynamicValue {
    unsafe {
        let ptr = libc::malloc(std::mem::size_of::<StringPtr>()) as *mut StringPtr;
        *ptr = StringPtr { ptr: str_ptr, len };
        DynamicValue {
            type_id: TYPE_STRING,
            value_ptr: ptr as *mut u8,
        }
    }
}

/// Box null as Dynamic
#[no_mangle]
pub extern "C" fn haxe_box_null() -> DynamicValue {
    DynamicValue {
        type_id: TYPE_NULL,
        value_ptr: std::ptr::null_mut(),
    }
}

// ============================================================================
// Unboxing functions: Extract concrete values from Dynamic
// ============================================================================

/// Unbox a Dynamic as Int (returns 0 if wrong type)
#[no_mangle]
pub extern "C" fn haxe_unbox_int(dynamic: DynamicValue) -> i64 {
    if dynamic.type_id == TYPE_INT {
        unsafe { *(dynamic.value_ptr as *const i64) }
    } else {
        0
    }
}

/// Unbox a Dynamic as Float (returns 0.0 if wrong type)
#[no_mangle]
pub extern "C" fn haxe_unbox_float(dynamic: DynamicValue) -> f64 {
    if dynamic.type_id == TYPE_FLOAT {
        unsafe { *(dynamic.value_ptr as *const f64) }
    } else {
        0.0
    }
}

/// Unbox a Dynamic as Bool (returns false if wrong type)
#[no_mangle]
pub extern "C" fn haxe_unbox_bool(dynamic: DynamicValue) -> bool {
    if dynamic.type_id == TYPE_BOOL {
        unsafe { *(dynamic.value_ptr as *const bool) }
    } else {
        false
    }
}

/// Unbox a Dynamic as String (returns empty string if wrong type)
#[no_mangle]
pub extern "C" fn haxe_unbox_string(dynamic: DynamicValue) -> StringPtr {
    if dynamic.type_id == TYPE_STRING {
        unsafe { *(dynamic.value_ptr as *const StringPtr) }
    } else {
        StringPtr {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

// ============================================================================
// Std.string() implementation with runtime type dispatch
// ============================================================================

/// Convert a Dynamic value to String using runtime type dispatch
///
/// This is the implementation of Std.string(Dynamic)
#[no_mangle]
pub extern "C" fn haxe_std_string(dynamic: DynamicValue) -> StringPtr {
    // Handle null specially
    if dynamic.type_id == TYPE_NULL || dynamic.value_ptr.is_null() {
        return unsafe { null_to_string(std::ptr::null()) };
    }

    // Look up type info and call toString
    if let Some(type_info) = get_type_info(dynamic.type_id) {
        unsafe { (type_info.to_string)(dynamic.value_ptr) }
    } else {
        // Unknown type, return type name or error
        let s = format!("<unknown type {}>", dynamic.type_id.0);
        let s_static = Box::leak(s.into_boxed_str());
        StringPtr {
            ptr: s_static.as_ptr(),
            len: s_static.len(),
        }
    }
}

/// Free a Dynamic value
#[no_mangle]
pub extern "C" fn haxe_free_dynamic(dynamic: DynamicValue) {
    if !dynamic.value_ptr.is_null() {
        unsafe {
            libc::free(dynamic.value_ptr as *mut libc::c_void);
        }
    }
}
