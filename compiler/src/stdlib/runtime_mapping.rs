//! Runtime Function Mapping
//!
//! Maps Haxe standard library method calls to rayzor-runtime function implementations.
//! This provides the bridge between high-level Haxe stdlib API and low-level runtime.
//!
//! # Architecture
//!
//! When the compiler encounters a method call like `str.charAt(5)`, it:
//! 1. Checks if it's a stdlib method using `is_stdlib_method()`
//! 2. Looks up the mapping using `get_runtime_mapping()`
//! 3. Generates a call to the runtime function (e.g., `haxe_string_char_at`)
//!
//! # Example
//!
//! ```haxe
//! var s:String = "hello";
//! var ch = s.charAt(0);  // Calls haxe_string_char_at(s, 0)
//! ```

use std::collections::HashMap;

/// Describes how to call a runtime function
#[derive(Debug, Clone)]
pub struct RuntimeFunctionCall {
    /// Name of the runtime function (e.g., "haxe_string_char_at")
    pub runtime_name: &'static str,

    /// Whether the function needs an output pointer as first argument
    /// True for functions that return complex types (String, Array)
    pub needs_out_param: bool,

    /// Whether the instance is passed as first argument (after out param if present)
    /// True for instance methods, false for static methods
    pub has_self_param: bool,

    /// Number of additional parameters (not counting self or out)
    pub param_count: usize,

    /// Whether this method returns a value
    pub has_return: bool,

    /// Which parameters need to be converted from values to boxed Dynamic pointers
    /// This is a bitmask where bit N indicates parameter N needs Dynamic boxing.
    /// DEPRECATED: Use raw_value_params for high-performance inline storage.
    pub params_need_ptr_conversion: u32,

    /// Which parameters should be passed as raw u64 bits (no boxing).
    /// This is a bitmask where bit N indicates parameter N should be cast to u64.
    /// Used for high-performance collections (StringMap, IntMap) that store values inline.
    /// The compiler casts Int/Float/Bool/Ptr to raw u64 bits at the call site.
    pub raw_value_params: u32,

    /// Whether the return value is raw u64 bits that should be cast to the type parameter.
    /// Used for StringMap<T>.get() and IntMap<T>.get() which return T as raw u64.
    /// The compiler will cast the u64 return value to the resolved type parameter.
    pub returns_raw_value: bool,

    /// Which parameters should be sign-extended from i32 to i64.
    /// This is a bitmask where bit N indicates parameter N should be extended.
    /// Used for IntMap key parameters which are Haxe Int (i32) but runtime expects i64.
    pub extend_to_i64_params: u32,
}

/// Method signature in Haxe stdlib
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodSignature {
    /// Class name (e.g., "String", "Array")
    pub class: &'static str,

    /// Method name (e.g., "charAt", "push")
    pub method: &'static str,

    /// Whether this is a static method
    pub is_static: bool,

    /// Whether this is a constructor (new method on extern class)
    pub is_constructor: bool,
}

/// Standard library runtime mapping
pub struct StdlibMapping {
    mappings: HashMap<MethodSignature, RuntimeFunctionCall>,
}

impl StdlibMapping {
    /// Create a new stdlib mapping with all built-in mappings
    pub fn new() -> Self {
        let mut mapping = StdlibMapping {
            mappings: HashMap::new(),
        };

        mapping.register_string_methods();
        mapping.register_array_methods();
        mapping.register_math_methods();
        mapping.register_sys_methods();
        mapping.register_std_methods();
        mapping.register_file_methods();
        mapping.register_fileinput_methods();
        mapping.register_fileoutput_methods();
        mapping.register_filesystem_methods();
        mapping.register_thread_methods();
        mapping.register_channel_methods();
        mapping.register_arc_methods();
        mapping.register_mutex_methods();
        mapping.register_vec_methods();
        mapping.register_stringmap_methods();
        mapping.register_intmap_methods();
        mapping.register_date_methods();
        mapping.register_bytes_methods();

        mapping
    }

    /// Look up the runtime function for a stdlib method call
    pub fn get(&self, sig: &MethodSignature) -> Option<&RuntimeFunctionCall> {
        self.mappings.get(sig)
    }

    /// Check if a method is a stdlib method with runtime mapping
    pub fn has_mapping(&self, class: &str, method: &str, is_static: bool) -> bool {
        self.mappings.keys().any(|sig| {
            sig.class == class && sig.method == method && sig.is_static == is_static
        })
    }

    /// Find a stdlib method mapping by class and method name
    /// Returns the signature and runtime function call if found
    pub fn find_by_name(&self, class: &str, method: &str) -> Option<(&MethodSignature, &RuntimeFunctionCall)> {
        self.mappings.iter().find(|(sig, _)| {
            sig.class == class && sig.method == method
        })
    }

    /// Get all unique stdlib class names that have registered methods
    pub fn get_all_classes(&self) -> Vec<&'static str> {
        let mut classes: Vec<&'static str> = self.mappings.keys()
            .map(|sig| sig.class)
            .collect();
        classes.sort_unstable();
        classes.dedup();
        classes
    }

    /// Check if a class name is a registered stdlib class
    pub fn is_stdlib_class(&self, class_name: &str) -> bool {
        self.mappings.keys().any(|sig| sig.class == class_name)
    }

    /// Check if methods of this class are typically static
    /// Used to determine the default method type for a class
    pub fn class_has_static_methods(&self, class_name: &str) -> bool {
        self.mappings.keys()
            .filter(|sig| sig.class == class_name)
            .any(|sig| sig.is_static)
    }

    /// Get the class name as a 'static str if it exists in the mapping
    /// This is useful for converting owned/borrowed strings to 'static references
    pub fn get_class_static_str(&self, class_name: &str) -> Option<&'static str> {
        self.mappings.keys()
            .find(|sig| sig.class == class_name)
            .map(|sig| sig.class)
    }

    /// Get all classes that have registered constructors (method="new", is_constructor=true)
    /// Returns a deduplicated, sorted list of class names with constructors
    pub fn get_constructor_classes(&self) -> Vec<&'static str> {
        let mut classes: Vec<&'static str> = self.mappings.keys()
            .filter(|sig| sig.is_constructor && sig.method == "new")
            .map(|sig| sig.class)
            .collect();
        classes.sort_unstable();
        classes.dedup();
        classes
    }

    /// Find a constructor mapping for a class (method="new", is_constructor=true)
    /// Returns the MethodSignature and RuntimeFunctionCall if found
    pub fn find_constructor(&self, class: &str) -> Option<(&MethodSignature, &RuntimeFunctionCall)> {
        self.mappings.iter().find(|(sig, _)| {
            sig.class == class && sig.method == "new" && sig.is_constructor
        })
    }

    /// Find a runtime function call by runtime function name
    /// Returns the RuntimeFunctionCall metadata if found
    pub fn find_by_runtime_name(&self, runtime_name: &str) -> Option<&RuntimeFunctionCall> {
        self.mappings.values().find(|call| call.runtime_name == runtime_name)
    }

    /// Check if a class name is a generic stdlib class that requires monomorphization
    /// For example, "Vec" is generic and maps to VecI32, VecI64, VecF64, etc.
    /// Returns the base name if it's a generic stdlib class, None otherwise
    pub fn is_generic_stdlib_class(&self, class_name: &str) -> bool {
        // Generic stdlib classes have monomorphized variants registered
        // Vec -> VecI32, VecI64, VecF64, VecPtr, VecBool
        match class_name {
            "Vec" => self.is_stdlib_class("VecI32"),
            _ => false,
        }
    }

    /// Get the monomorphized class name for a generic stdlib class
    /// E.g., Vec<Int> -> VecI32, Vec<Float> -> VecF64
    /// Returns None if the class is not a generic stdlib class or the type param is unknown
    pub fn get_monomorphized_class(&self, class_name: &str, type_param: &str) -> Option<&'static str> {
        match class_name {
            "Vec" => {
                let mono_class = match type_param {
                    "Int" => "VecI32",
                    "Int64" => "VecI64",
                    "Float" => "VecF64",
                    "Bool" => "VecBool",
                    _ => "VecPtr", // Pointer/reference types
                };
                // Verify the monomorphized class exists in our mapping
                if self.is_stdlib_class(mono_class) {
                    Some(mono_class)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get all monomorphized variants for a generic stdlib class
    /// E.g., "Vec" -> ["VecI32", "VecI64", "VecF64", "VecPtr", "VecBool"]
    pub fn get_monomorphized_variants(&self, class_name: &str) -> Vec<&'static str> {
        match class_name {
            "Vec" => {
                vec!["VecI32", "VecI64", "VecF64", "VecPtr", "VecBool"]
                    .into_iter()
                    .filter(|v| self.is_stdlib_class(v))
                    .collect()
            }
            _ => vec![],
        }
    }

    /// Check if a stdlib class uses MIR wrapper functions instead of direct extern calls.
    /// MIR wrapper classes have functions defined in stdlib/thread.rs, stdlib/channel.rs, etc.
    /// that need to be called as regular MIR functions (not extern C functions).
    ///
    /// Detection: MIR wrapper functions have runtime names in the format `{Class}_{method}`
    /// (e.g., Thread_spawn, VecI32_push) rather than prefixed names like `rayzor_thread_spawn`
    /// or `haxe_string_char_at`.
    pub fn is_mir_wrapper_class(&self, class_name: &str) -> bool {
        // Check if any method of this class has a MIR wrapper-style runtime name
        self.mappings.iter().any(|(sig, call)| {
            if sig.class != class_name {
                return false;
            }
            // MIR wrapper names: {Class}_{method} (e.g., Thread_spawn, VecI32_push)
            // Extern names use prefixes: rayzor_*, haxe_*
            let expected_mir_name = format!("{}_{}", sig.class, sig.method);
            call.runtime_name == expected_mir_name
        })
    }

    /// Register a stdlib method -> runtime function mapping
    fn register(&mut self, sig: MethodSignature, call: RuntimeFunctionCall) {
        self.mappings.insert(sig, call);
    }
}

/// Macro to register stdlib methods more concisely
macro_rules! map_method {
    // Constructor - returns complex type via out param (opaque pointer to extern class)
    (constructor $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: complex) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: true,  // Constructors are called like static methods
                is_constructor: true,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: true,
                has_self_param: false,
                param_count: $params,
                has_return: false,
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };

    // Constructor - returns pointer directly (no out param)
    // Use this for extern class constructors that return ptr directly (e.g., haxe_stringmap_new)
    (constructor $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: primitive) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: true,  // Constructors are called like static methods
                is_constructor: true,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: false,
                param_count: $params,
                has_return: true,  // Returns pointer directly
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };

    // Instance method returning primitive
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: primitive) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: true,
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };

    // Instance method returning primitive with i64 extension for int params
    // Used for IntMap methods where Haxe Int (i32) must be extended to runtime i64
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: primitive, extend_i64: $extend_mask:expr) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: true,
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: $extend_mask,
            }
        )
    };

    // Instance method returning raw value (u64 that needs cast to type param T)
    // Used for StringMap<T>.get() and IntMap<T>.get()
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: raw_value) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: true,
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: true,
                extend_to_i64_params: 0,
            }
        )
    };

    // Instance method returning raw value with i64 extension for int params
    // Used for IntMap<T>.get() where key is Haxe Int (i32) but runtime expects i64
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: raw_value, extend_i64: $extend_mask:expr) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: true,
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: true,
                extend_to_i64_params: $extend_mask,
            }
        )
    };

    // Instance method returning primitive with pointer conversion metadata (DEPRECATED - use raw_value_params)
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: primitive, ptr_params: $ptr_mask:expr) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: true,
                params_need_ptr_conversion: $ptr_mask,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };

    // Instance method returning complex type (String, Array)
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: complex) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: true,
                has_self_param: true,
                param_count: $params,
                has_return: false,
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };

    // Instance method returning void
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: void) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: false,
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };

    // Instance method returning void with pointer conversion metadata (DEPRECATED - use raw_value_params)
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: void, ptr_params: $ptr_mask:expr) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: false,
                params_need_ptr_conversion: $ptr_mask,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };

    // Instance method returning void with raw value params (high-performance, no boxing)
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: void, raw_value_params: $raw_mask:expr) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: false,
                params_need_ptr_conversion: 0,
                raw_value_params: $raw_mask,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };

    // Instance method returning void with raw value params AND i64 extension
    // Used for IntMap<T>.set(key: Int, value: T) where key needs i32->i64 and value needs raw u64
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: void, raw_value_params: $raw_mask:expr, extend_i64: $extend_mask:expr) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: false,
                params_need_ptr_conversion: 0,
                raw_value_params: $raw_mask,
                returns_raw_value: false,
                extend_to_i64_params: $extend_mask,
            }
        )
    };

    // Static method returning primitive
    (static $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: primitive) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: true,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: false,
                param_count: $params,
                has_return: true,
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };

    // Static method returning complex type
    (static $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: complex) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: true,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: true,
                has_self_param: false,
                param_count: $params,
                has_return: false,
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };

    // Static method returning void
    (static $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: void) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: true,
                is_constructor: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: false,
                param_count: $params,
                has_return: false,
                params_need_ptr_conversion: 0,
                raw_value_params: 0,
                returns_raw_value: false,
                extend_to_i64_params: 0,
            }
        )
    };
}

impl StdlibMapping {
    fn register_from_tuples(&mut self, mappings: Vec<(MethodSignature, RuntimeFunctionCall)>) {
        for (sig, call) in mappings {
            self.register(sig, call);
        }
    }

    // ============================================================================
    // String Methods
    // ============================================================================

    fn register_string_methods(&mut self) {
        let mappings = vec![
            // Static methods
            map_method!(static "String", "fromCharCode" => "haxe_string_from_char_code", params: 1, returns: primitive),

            // Properties (treated as getters with 0 params)
            map_method!(instance "String", "length" => "haxe_string_len", params: 0, returns: primitive),

            // Instance methods - character access
            // charAt returns String pointer (empty string for out of bounds)
            map_method!(instance "String", "charAt" => "haxe_string_char_at_ptr", params: 1, returns: primitive),
            // charCodeAt returns Null<Int> (-1 for out of bounds, which we represent as i64)
            map_method!(instance "String", "charCodeAt" => "haxe_string_char_code_at_ptr", params: 1, returns: primitive),

            // Search operations
            map_method!(instance "String", "indexOf" => "haxe_string_index_of_ptr", params: 2, returns: primitive),
            map_method!(instance "String", "lastIndexOf" => "haxe_string_last_index_of_ptr", params: 2, returns: primitive),

            // String transformations
            map_method!(instance "String", "split" => "haxe_string_split_ptr", params: 1, returns: complex),
            map_method!(instance "String", "substr" => "haxe_string_substr_ptr", params: 2, returns: primitive),
            map_method!(instance "String", "substring" => "haxe_string_substring_ptr", params: 2, returns: primitive),
            // toLowerCase/toUpperCase use pointer-returning wrapper functions (not out-param style)
            map_method!(instance "String", "toLowerCase" => "haxe_string_lower", params: 0, returns: primitive),
            map_method!(instance "String", "toUpperCase" => "haxe_string_upper", params: 0, returns: primitive),
            map_method!(instance "String", "toString" => "haxe_string_copy", params: 0, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Array Methods
    // ============================================================================

    fn register_array_methods(&mut self) {
        let mappings = vec![
            // Properties (treated as getters with 0 params)
            map_method!(instance "Array", "length" => "haxe_array_length", params: 0, returns: primitive),

            // Modification methods
            // push(x:T): arg[0]=array (no conversion), arg[1]=value (needs ptr conversion)
            // Bitmask: 0b10 = bit 1 set (param index 1)
            map_method!(instance "Array", "push" => "haxe_array_push", params: 1, returns: primitive, ptr_params: 0b10),
            map_method!(instance "Array", "pop" => "haxe_array_pop_ptr", params: 0, returns: primitive),
            map_method!(instance "Array", "reverse" => "haxe_array_reverse", params: 0, returns: void),
            // insert(pos:Int, x:T): arg[0]=array, arg[1]=pos (no conversion), arg[2]=value (needs ptr conversion)
            // Bitmask: 0b100 = bit 2 set (param index 2)
            map_method!(instance "Array", "insert" => "haxe_array_insert", params: 2, returns: void, ptr_params: 0b100),
            // remove(x:T): arg[0]=array, arg[1]=value (needs ptr conversion)
            // Bitmask: 0b10 = bit 1 set
            map_method!(instance "Array", "remove" => "haxe_array_remove", params: 1, returns: primitive, ptr_params: 0b10),

            // Extraction methods
            map_method!(instance "Array", "slice" => "haxe_array_slice", params: 2, returns: complex),
            map_method!(instance "Array", "copy" => "haxe_array_copy", params: 0, returns: complex),

            // Search methods
            // indexOf(x:T, fromIndex:Int): arg[0]=array, arg[1]=value (needs ptr), arg[2]=fromIndex
            // Bitmask: 0b10 = bit 1 set
            map_method!(instance "Array", "indexOf" => "haxe_array_index_of", params: 2, returns: primitive, ptr_params: 0b10),
            map_method!(instance "Array", "lastIndexOf" => "haxe_array_last_index_of", params: 2, returns: primitive, ptr_params: 0b10),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Math Methods
    // ============================================================================

    fn register_math_methods(&mut self) {
        let mappings = vec![
            // Basic operations
            map_method!(static "Math", "abs" => "haxe_math_abs", params: 1, returns: primitive),
            map_method!(static "Math", "min" => "haxe_math_min", params: 2, returns: primitive),
            map_method!(static "Math", "max" => "haxe_math_max", params: 2, returns: primitive),
            map_method!(static "Math", "floor" => "haxe_math_floor", params: 1, returns: primitive),
            map_method!(static "Math", "ceil" => "haxe_math_ceil", params: 1, returns: primitive),
            map_method!(static "Math", "round" => "haxe_math_round", params: 1, returns: primitive),

            // Trigonometric
            map_method!(static "Math", "sin" => "haxe_math_sin", params: 1, returns: primitive),
            map_method!(static "Math", "cos" => "haxe_math_cos", params: 1, returns: primitive),
            map_method!(static "Math", "tan" => "haxe_math_tan", params: 1, returns: primitive),
            map_method!(static "Math", "asin" => "haxe_math_asin", params: 1, returns: primitive),
            map_method!(static "Math", "acos" => "haxe_math_acos", params: 1, returns: primitive),
            map_method!(static "Math", "atan" => "haxe_math_atan", params: 1, returns: primitive),
            map_method!(static "Math", "atan2" => "haxe_math_atan2", params: 2, returns: primitive),

            // Exponential and logarithmic
            map_method!(static "Math", "exp" => "haxe_math_exp", params: 1, returns: primitive),
            map_method!(static "Math", "log" => "haxe_math_log", params: 1, returns: primitive),
            map_method!(static "Math", "pow" => "haxe_math_pow", params: 2, returns: primitive),
            map_method!(static "Math", "sqrt" => "haxe_math_sqrt", params: 1, returns: primitive),

            // Special
            map_method!(static "Math", "isNaN" => "haxe_math_is_nan", params: 1, returns: primitive),
            map_method!(static "Math", "isFinite" => "haxe_math_is_finite", params: 1, returns: primitive),
            map_method!(static "Math", "random" => "haxe_math_random", params: 0, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Sys Methods
    // ============================================================================

    fn register_sys_methods(&mut self) {
        let mappings = vec![
            // I/O
            map_method!(static "Sys", "print" => "haxe_string_print", params: 1, returns: void),
            map_method!(static "Sys", "println" => "haxe_sys_println", params: 0, returns: void),
            // Program control
            map_method!(static "Sys", "exit" => "haxe_sys_exit", params: 1, returns: void),
            map_method!(static "Sys", "time" => "haxe_sys_time", params: 0, returns: primitive),
            map_method!(static "Sys", "cpuTime" => "haxe_sys_cpu_time", params: 0, returns: primitive),
            // Environment
            map_method!(static "Sys", "getEnv" => "haxe_sys_get_env", params: 1, returns: complex),
            map_method!(static "Sys", "putEnv" => "haxe_sys_put_env", params: 2, returns: void),
            // Working directory
            map_method!(static "Sys", "getCwd" => "haxe_sys_get_cwd", params: 0, returns: complex),
            map_method!(static "Sys", "setCwd" => "haxe_sys_set_cwd", params: 1, returns: void),
            // Sleep
            map_method!(static "Sys", "sleep" => "haxe_sys_sleep", params: 1, returns: void),
            // System info
            map_method!(static "Sys", "systemName" => "haxe_sys_system_name", params: 0, returns: complex),
            map_method!(static "Sys", "programPath" => "haxe_sys_program_path", params: 0, returns: complex),
            map_method!(static "Sys", "executablePath" => "haxe_sys_program_path", params: 0, returns: complex),
            // Command execution
            map_method!(static "Sys", "command" => "haxe_sys_command", params: 1, returns: primitive),
            map_method!(static "Sys", "getChar" => "haxe_sys_get_char", params: 1, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Std Class Methods
    // ============================================================================

    fn register_std_methods(&mut self) {
        let mappings = vec![
            // Std.string(v: Dynamic) -> String
            map_method!(static "Std", "string" => "haxe_std_string_ptr", params: 1, returns: complex),
            // Std.int(x: Float) -> Int
            map_method!(static "Std", "int" => "haxe_std_int", params: 1, returns: primitive),
            // Std.parseInt(x: String) -> Null<Int>
            map_method!(static "Std", "parseInt" => "haxe_std_parse_int", params: 1, returns: primitive),
            // Std.parseFloat(x: String) -> Float
            map_method!(static "Std", "parseFloat" => "haxe_std_parse_float", params: 1, returns: primitive),
            // Std.random(x: Int) -> Int
            map_method!(static "Std", "random" => "haxe_std_random", params: 1, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // File I/O Methods (sys.io.File)
    // ============================================================================

    fn register_file_methods(&mut self) {
        let mappings = vec![
            // File.getContent(path: String) -> String
            map_method!(static "File", "getContent" => "haxe_file_get_content", params: 1, returns: complex),
            // File.saveContent(path: String, content: String) -> Void
            map_method!(static "File", "saveContent" => "haxe_file_save_content", params: 2, returns: void),
            // File.copy(srcPath: String, dstPath: String) -> Void
            map_method!(static "File", "copy" => "haxe_file_copy", params: 2, returns: void),
            // File.read(path: String, binary: Bool) -> FileInput
            map_method!(static "File", "read" => "haxe_file_read", params: 2, returns: primitive),
            // File.write(path: String, binary: Bool) -> FileOutput
            map_method!(static "File", "write" => "haxe_file_write", params: 2, returns: primitive),
            // File.append(path: String, binary: Bool) -> FileOutput
            map_method!(static "File", "append" => "haxe_file_append", params: 2, returns: primitive),
            // File.update(path: String, binary: Bool) -> FileOutput
            map_method!(static "File", "update" => "haxe_file_update", params: 2, returns: primitive),
            // File.getBytes(path: String) -> haxe.io.Bytes
            map_method!(static "File", "getBytes" => "haxe_file_get_bytes", params: 1, returns: primitive),
            // File.saveBytes(path: String, bytes: haxe.io.Bytes) -> Void
            map_method!(static "File", "saveBytes" => "haxe_file_save_bytes", params: 2, returns: void),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // FileInput Methods (sys.io.FileInput)
    // ============================================================================

    fn register_fileinput_methods(&mut self) {
        let mappings = vec![
            // FileInput.readByte() -> Int
            map_method!(instance "FileInput", "readByte" => "haxe_fileinput_read_byte", params: 0, returns: primitive),
            // FileInput.seek(p: Int, pos: FileSeek) -> Void
            map_method!(instance "FileInput", "seek" => "haxe_fileinput_seek", params: 2, returns: void),
            // FileInput.tell() -> Int
            map_method!(instance "FileInput", "tell" => "haxe_fileinput_tell", params: 0, returns: primitive),
            // FileInput.eof() -> Bool
            map_method!(instance "FileInput", "eof" => "haxe_fileinput_eof", params: 0, returns: primitive),
            // FileInput.close() -> Void
            map_method!(instance "FileInput", "close" => "haxe_fileinput_close", params: 0, returns: void),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // FileOutput Methods (sys.io.FileOutput)
    // ============================================================================

    fn register_fileoutput_methods(&mut self) {
        let mappings = vec![
            // FileOutput.writeByte(c: Int) -> Void
            map_method!(instance "FileOutput", "writeByte" => "haxe_fileoutput_write_byte", params: 1, returns: void),
            // FileOutput.seek(p: Int, pos: FileSeek) -> Void
            map_method!(instance "FileOutput", "seek" => "haxe_fileoutput_seek", params: 2, returns: void),
            // FileOutput.tell() -> Int
            map_method!(instance "FileOutput", "tell" => "haxe_fileoutput_tell", params: 0, returns: primitive),
            // FileOutput.flush() -> Void
            map_method!(instance "FileOutput", "flush" => "haxe_fileoutput_flush", params: 0, returns: void),
            // FileOutput.close() -> Void
            map_method!(instance "FileOutput", "close" => "haxe_fileoutput_close", params: 0, returns: void),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // FileSystem Methods (sys.FileSystem)
    // ============================================================================

    fn register_filesystem_methods(&mut self) {
        let mappings = vec![
            // FileSystem.exists(path: String) -> Bool
            map_method!(static "FileSystem", "exists" => "haxe_filesystem_exists", params: 1, returns: primitive),
            // FileSystem.isDirectory(path: String) -> Bool
            map_method!(static "FileSystem", "isDirectory" => "haxe_filesystem_is_directory", params: 1, returns: primitive),
            // FileSystem.isFile(path: String) -> Bool (extension - not in standard Haxe)
            map_method!(static "FileSystem", "isFile" => "haxe_filesystem_is_file", params: 1, returns: primitive),
            // FileSystem.createDirectory(path: String) -> Void
            map_method!(static "FileSystem", "createDirectory" => "haxe_filesystem_create_directory", params: 1, returns: void),
            // FileSystem.deleteFile(path: String) -> Void
            map_method!(static "FileSystem", "deleteFile" => "haxe_filesystem_delete_file", params: 1, returns: void),
            // FileSystem.deleteDirectory(path: String) -> Void
            map_method!(static "FileSystem", "deleteDirectory" => "haxe_filesystem_delete_directory", params: 1, returns: void),
            // FileSystem.rename(path: String, newPath: String) -> Void
            map_method!(static "FileSystem", "rename" => "haxe_filesystem_rename", params: 2, returns: void),
            // FileSystem.fullPath(relPath: String) -> String (returns pointer directly)
            map_method!(static "FileSystem", "fullPath" => "haxe_filesystem_full_path", params: 1, returns: primitive),
            // FileSystem.absolutePath(relPath: String) -> String (returns pointer directly)
            map_method!(static "FileSystem", "absolutePath" => "haxe_filesystem_absolute_path", params: 1, returns: primitive),
            // FileSystem.stat(path: String) -> FileStat (returns pointer directly)
            map_method!(static "FileSystem", "stat" => "haxe_filesystem_stat", params: 1, returns: primitive),
            // FileSystem.readDirectory(path: String) -> Array<String> (returns pointer directly)
            map_method!(static "FileSystem", "readDirectory" => "haxe_filesystem_read_directory", params: 1, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Thread Methods (rayzor.concurrent.Thread)
    // ============================================================================
    //
    // NOTE: Thread methods are implemented as MIR wrappers in compiler/src/stdlib/thread.rs
    // These are NOT extern functions - they are MIR functions that get merged into the module.
    // We register them here so the compiler knows they exist and can generate forward references.

    fn register_thread_methods(&mut self) {
        let mappings = vec![
            // Thread::spawn<T>(f: Void -> T) -> Thread<T>
            map_method!(static "Thread", "spawn" => "Thread_spawn", params: 1, returns: complex),
            // Thread<T>::join() -> T
            map_method!(instance "Thread", "join" => "Thread_join", params: 0, returns: complex),
            // Thread<T>::isFinished() -> Bool
            map_method!(instance "Thread", "isFinished" => "Thread_isFinished", params: 0, returns: primitive),
            // Thread::sleep(millis: Int) -> Void
            map_method!(static "Thread", "sleep" => "Thread_sleep", params: 1, returns: void),
            // Thread::yieldNow() -> Void
            map_method!(static "Thread", "yieldNow" => "Thread_yieldNow", params: 0, returns: void),
            // Thread::currentId() -> Int
            map_method!(static "Thread", "currentId" => "Thread_currentId", params: 0, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Channel Methods (rayzor.concurrent.Channel)
    // ============================================================================
    //
    // NOTE: Channel methods are implemented as MIR wrappers in compiler/src/stdlib/channel.rs
    // These are NOT extern functions - they are MIR functions that get merged into the module.
    // The MIR wrappers call the extern runtime functions (rayzor_channel_*).

    fn register_channel_methods(&mut self) {
        let mappings = vec![
            // Constructor: new Channel<T>(capacity: Int) -> Channel<T>
            map_method!(constructor "Channel", "new" => "Channel_init", params: 1, returns: complex),
            // Channel::init<T>(capacity: Int) -> Channel<T> (for backwards compatibility)
            map_method!(static "Channel", "init" => "Channel_init", params: 1, returns: complex),
            // Channel<T>::send(value: T) -> Void
            map_method!(instance "Channel", "send" => "Channel_send", params: 1, returns: void),
            // Channel<T>::trySend(value: T) -> Bool
            map_method!(instance "Channel", "trySend" => "Channel_trySend", params: 1, returns: primitive),
            // Channel<T>::receive() -> T
            map_method!(instance "Channel", "receive" => "Channel_receive", params: 0, returns: complex),
            // Channel<T>::tryReceive() -> Null<T>
            map_method!(instance "Channel", "tryReceive" => "Channel_tryReceive", params: 0, returns: complex),
            // Channel<T>::close() -> Void
            map_method!(instance "Channel", "close" => "Channel_close", params: 0, returns: void),
            // Channel<T>::isClosed() -> Bool
            map_method!(instance "Channel", "isClosed" => "Channel_isClosed", params: 0, returns: primitive),
            // Channel<T>::len() -> Int
            map_method!(instance "Channel", "len" => "Channel_len", params: 0, returns: primitive),
            // Channel<T>::capacity() -> Int
            map_method!(instance "Channel", "capacity" => "Channel_capacity", params: 0, returns: primitive),
            // Channel<T>::isEmpty() -> Bool
            map_method!(instance "Channel", "isEmpty" => "Channel_isEmpty", params: 0, returns: primitive),
            // Channel<T>::isFull() -> Bool
            map_method!(instance "Channel", "isFull" => "Channel_isFull", params: 0, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Arc Methods (rayzor.concurrent.Arc)
    // ============================================================================

    fn register_arc_methods(&mut self) {
        let mappings = vec![
            // Constructor: new Arc<T>(value: T) -> Arc<T>
            map_method!(constructor "Arc", "new" => "Arc_init", params: 1, returns: complex),
            // Arc::init<T>(value: T) -> Arc<T> (for backwards compatibility)
            map_method!(static "Arc", "init" => "Arc_init", params: 1, returns: complex),
            // Arc<T>::clone() -> Arc<T>
            map_method!(instance "Arc", "clone" => "Arc_clone", params: 0, returns: complex),
            // Arc<T>::get() -> T
            map_method!(instance "Arc", "get" => "Arc_get", params: 0, returns: complex),
            // Arc<T>::strongCount() -> Int
            map_method!(instance "Arc", "strongCount" => "Arc_strongCount", params: 0, returns: primitive),
            // Arc<T>::tryUnwrap() -> Null<T>
            map_method!(instance "Arc", "tryUnwrap" => "Arc_tryUnwrap", params: 0, returns: complex),
            // Arc<T>::asPtr() -> Int
            map_method!(instance "Arc", "asPtr" => "Arc_asPtr", params: 0, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Mutex Methods (rayzor.concurrent.Mutex)
    // ============================================================================

    fn register_mutex_methods(&mut self) {
        let mappings = vec![
            // Constructor: new Mutex<T>(value: T) -> Mutex<T>
            map_method!(constructor "Mutex", "new" => "Mutex_init", params: 1, returns: complex),
            // Mutex::init<T>(value: T) -> Mutex<T> (for backwards compatibility)
            map_method!(static "Mutex", "init" => "Mutex_init", params: 1, returns: complex),
            // Mutex<T>::lock() -> MutexGuard<T>
            map_method!(instance "Mutex", "lock" => "Mutex_lock", params: 0, returns: complex),
            // Mutex<T>::tryLock() -> Null<MutexGuard<T>>
            map_method!(instance "Mutex", "tryLock" => "Mutex_tryLock", params: 0, returns: complex),
            // Mutex<T>::isLocked() -> Bool
            map_method!(instance "Mutex", "isLocked" => "Mutex_isLocked", params: 0, returns: primitive),
            // MutexGuard<T>::get() -> T
            map_method!(instance "MutexGuard", "get" => "MutexGuard_get", params: 0, returns: complex),
            // MutexGuard<T>::unlock() -> Void
            map_method!(instance "MutexGuard", "unlock" => "MutexGuard_unlock", params: 0, returns: void),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Vec<T> Methods (rayzor.Vec - monomorphized generic vectors)
    // ============================================================================
    //
    // These are type-specialized vector methods for monomorphization.
    // When the compiler sees Vec<Int>, it maps to VecI32 runtime functions.
    // When it sees Vec<Float>, it maps to VecF64 runtime functions.
    //
    // The class names use the monomorphized naming convention:
    // - "VecI32" for Vec<Int>
    // - "VecI64" for Vec<Int64>
    // - "VecF64" for Vec<Float>
    // - "VecPtr" for Vec<T> where T is a reference type
    // - "VecBool" for Vec<Bool>

    fn register_vec_methods(&mut self) {
        // Vec<Int> -> VecI32
        // These map to MIR wrapper functions (VecI32_*) NOT directly to runtime functions
        let vec_i32_mappings = vec![
            map_method!(constructor "VecI32", "new" => "VecI32_new", params: 0, returns: complex),
            map_method!(instance "VecI32", "push" => "VecI32_push", params: 1, returns: void),
            map_method!(instance "VecI32", "pop" => "VecI32_pop", params: 0, returns: primitive),
            map_method!(instance "VecI32", "get" => "VecI32_get", params: 1, returns: primitive),
            map_method!(instance "VecI32", "set" => "VecI32_set", params: 2, returns: void),
            map_method!(instance "VecI32", "length" => "VecI32_length", params: 0, returns: primitive),
            map_method!(instance "VecI32", "capacity" => "VecI32_capacity", params: 0, returns: primitive),
            map_method!(instance "VecI32", "isEmpty" => "VecI32_isEmpty", params: 0, returns: primitive),
            map_method!(instance "VecI32", "clear" => "VecI32_clear", params: 0, returns: void),
            map_method!(instance "VecI32", "first" => "VecI32_first", params: 0, returns: primitive),
            map_method!(instance "VecI32", "last" => "VecI32_last", params: 0, returns: primitive),
            map_method!(instance "VecI32", "sort" => "VecI32_sort", params: 0, returns: void),
            map_method!(instance "VecI32", "sortBy" => "VecI32_sortBy", params: 2, returns: void),
        ];
        self.register_from_tuples(vec_i32_mappings);

        // Vec<Int64> -> VecI64
        let vec_i64_mappings = vec![
            map_method!(constructor "VecI64", "new" => "VecI64_new", params: 0, returns: complex),
            map_method!(instance "VecI64", "push" => "VecI64_push", params: 1, returns: void),
            map_method!(instance "VecI64", "pop" => "VecI64_pop", params: 0, returns: primitive),
            map_method!(instance "VecI64", "get" => "VecI64_get", params: 1, returns: primitive),
            map_method!(instance "VecI64", "set" => "VecI64_set", params: 2, returns: void),
            map_method!(instance "VecI64", "length" => "VecI64_length", params: 0, returns: primitive),
            map_method!(instance "VecI64", "isEmpty" => "VecI64_isEmpty", params: 0, returns: primitive),
            map_method!(instance "VecI64", "clear" => "VecI64_clear", params: 0, returns: void),
            map_method!(instance "VecI64", "first" => "VecI64_first", params: 0, returns: primitive),
            map_method!(instance "VecI64", "last" => "VecI64_last", params: 0, returns: primitive),
        ];
        self.register_from_tuples(vec_i64_mappings);

        // Vec<Float> -> VecF64
        let vec_f64_mappings = vec![
            map_method!(constructor "VecF64", "new" => "VecF64_new", params: 0, returns: complex),
            map_method!(instance "VecF64", "push" => "VecF64_push", params: 1, returns: void),
            map_method!(instance "VecF64", "pop" => "VecF64_pop", params: 0, returns: primitive),
            map_method!(instance "VecF64", "get" => "VecF64_get", params: 1, returns: primitive),
            map_method!(instance "VecF64", "set" => "VecF64_set", params: 2, returns: void),
            map_method!(instance "VecF64", "length" => "VecF64_length", params: 0, returns: primitive),
            map_method!(instance "VecF64", "isEmpty" => "VecF64_isEmpty", params: 0, returns: primitive),
            map_method!(instance "VecF64", "clear" => "VecF64_clear", params: 0, returns: void),
            map_method!(instance "VecF64", "first" => "VecF64_first", params: 0, returns: primitive),
            map_method!(instance "VecF64", "last" => "VecF64_last", params: 0, returns: primitive),
            map_method!(instance "VecF64", "sort" => "VecF64_sort", params: 0, returns: void),
            map_method!(instance "VecF64", "sortBy" => "VecF64_sortBy", params: 2, returns: void),
        ];
        self.register_from_tuples(vec_f64_mappings);

        // Vec<T> where T is reference type -> VecPtr
        let vec_ptr_mappings = vec![
            map_method!(constructor "VecPtr", "new" => "VecPtr_new", params: 0, returns: complex),
            map_method!(instance "VecPtr", "push" => "VecPtr_push", params: 1, returns: void),
            map_method!(instance "VecPtr", "pop" => "VecPtr_pop", params: 0, returns: complex),
            map_method!(instance "VecPtr", "get" => "VecPtr_get", params: 1, returns: complex),
            map_method!(instance "VecPtr", "set" => "VecPtr_set", params: 2, returns: void),
            map_method!(instance "VecPtr", "length" => "VecPtr_length", params: 0, returns: primitive),
            map_method!(instance "VecPtr", "isEmpty" => "VecPtr_isEmpty", params: 0, returns: primitive),
            map_method!(instance "VecPtr", "clear" => "VecPtr_clear", params: 0, returns: void),
            map_method!(instance "VecPtr", "first" => "VecPtr_first", params: 0, returns: complex),
            map_method!(instance "VecPtr", "last" => "VecPtr_last", params: 0, returns: complex),
            map_method!(instance "VecPtr", "sortBy" => "VecPtr_sortBy", params: 2, returns: void),
        ];
        self.register_from_tuples(vec_ptr_mappings);

        // Vec<Bool> -> VecBool
        let vec_bool_mappings = vec![
            map_method!(constructor "VecBool", "new" => "VecBool_new", params: 0, returns: complex),
            map_method!(instance "VecBool", "push" => "VecBool_push", params: 1, returns: void),
            map_method!(instance "VecBool", "pop" => "VecBool_pop", params: 0, returns: primitive),
            map_method!(instance "VecBool", "get" => "VecBool_get", params: 1, returns: primitive),
            map_method!(instance "VecBool", "set" => "VecBool_set", params: 2, returns: void),
            map_method!(instance "VecBool", "length" => "VecBool_length", params: 0, returns: primitive),
            map_method!(instance "VecBool", "isEmpty" => "VecBool_isEmpty", params: 0, returns: primitive),
            map_method!(instance "VecBool", "clear" => "VecBool_clear", params: 0, returns: void),
        ];
        self.register_from_tuples(vec_bool_mappings);
    }

    // ============================================================================
    // StringMap<T> Methods (haxe.ds.StringMap)
    // ============================================================================
    //
    // StringMap is an extern class that provides a hash map with String keys.
    // Values are type-erased at runtime (stored as pointers).

    fn register_stringmap_methods(&mut self) {
        let mappings = vec![
            // Constructor: new StringMap<T>() -> StringMap<T>
            // Returns pointer directly (primitive return style)
            map_method!(constructor "StringMap", "new" => "haxe_stringmap_new", params: 0, returns: primitive),
            // StringMap<T>::set(key: String, value: T) -> Void
            // Args: [self=map_ptr, key=String, value=u64]
            // Value is passed as raw u64 bits (no boxing) - high-performance inline storage
            // The compiler will cast the value to u64 at the call site
            map_method!(instance "StringMap", "set" => "haxe_stringmap_set", params: 2, returns: void, raw_value_params: 0b100),
            // StringMap<T>::get(key: String) -> T (as u64)
            // Returns raw u64 bits, compiler casts back to resolved type parameter T
            map_method!(instance "StringMap", "get" => "haxe_stringmap_get", params: 1, returns: raw_value),
            // StringMap<T>::exists(key: String) -> Bool
            map_method!(instance "StringMap", "exists" => "haxe_stringmap_exists", params: 1, returns: primitive),
            // StringMap<T>::remove(key: String) -> Bool
            map_method!(instance "StringMap", "remove" => "haxe_stringmap_remove", params: 1, returns: primitive),
            // StringMap<T>::clear() -> Void
            map_method!(instance "StringMap", "clear" => "haxe_stringmap_clear", params: 0, returns: void),
            // StringMap<T>::toString() -> String
            // Returns pointer directly
            map_method!(instance "StringMap", "toString" => "haxe_stringmap_to_string", params: 0, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // IntMap<T> Methods (haxe.ds.IntMap)
    // ============================================================================
    //
    // IntMap is an extern class that provides a hash map with Int keys.
    // Values are type-erased at runtime (stored as pointers).

    fn register_intmap_methods(&mut self) {
        // Parameter indices (0-indexed, including self):
        // - 0: self (map_ptr)
        // - 1: key (Int, needs i32->i64 extension)
        // - 2: value (T, needs raw u64 conversion)
        let mappings = vec![
            // Constructor: new IntMap<T>() -> IntMap<T>
            // Returns pointer directly (primitive return style)
            map_method!(constructor "IntMap", "new" => "haxe_intmap_new", params: 0, returns: primitive),
            // IntMap<T>::set(key: Int, value: T) -> Void
            // Args: [self=map_ptr, key=i64(extended), value=u64(raw)]
            // Key is extended from i32 to i64, value is passed as raw u64 bits
            map_method!(instance "IntMap", "set" => "haxe_intmap_set", params: 2, returns: void, raw_value_params: 0b100, extend_i64: 0b010),
            // IntMap<T>::get(key: Int) -> T (as u64)
            // Key is extended from i32 to i64, returns raw u64 bits for type parameter T
            map_method!(instance "IntMap", "get" => "haxe_intmap_get", params: 1, returns: raw_value, extend_i64: 0b010),
            // IntMap<T>::exists(key: Int) -> Bool
            // Key is extended from i32 to i64
            map_method!(instance "IntMap", "exists" => "haxe_intmap_exists", params: 1, returns: primitive, extend_i64: 0b010),
            // IntMap<T>::remove(key: Int) -> Bool
            // Key is extended from i32 to i64
            map_method!(instance "IntMap", "remove" => "haxe_intmap_remove", params: 1, returns: primitive, extend_i64: 0b010),
            // IntMap<T>::clear() -> Void
            map_method!(instance "IntMap", "clear" => "haxe_intmap_clear", params: 0, returns: void),
            // IntMap<T>::toString() -> String
            // Returns pointer directly
            map_method!(instance "IntMap", "toString" => "haxe_intmap_to_string", params: 0, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Date Methods
    // ============================================================================

    fn register_date_methods(&mut self) {
        let mappings = vec![
            // Date.new(year, month, day, hour, min, sec): Date (constructor)
            map_method!(static "Date", "new" => "haxe_date_new", params: 6, returns: primitive),
            // Date.now(): Date
            map_method!(static "Date", "now" => "haxe_date_now", params: 0, returns: primitive),
            // Date.fromTime(t: Float): Date
            map_method!(static "Date", "fromTime" => "haxe_date_from_time", params: 1, returns: primitive),
            // Date.fromString(s: String): Date
            map_method!(static "Date", "fromString" => "haxe_date_from_string", params: 1, returns: primitive),

            // Instance methods - local timezone
            // date.getTime(): Float
            map_method!(instance "Date", "getTime" => "haxe_date_get_time", params: 0, returns: primitive),
            // date.getHours(): Int
            map_method!(instance "Date", "getHours" => "haxe_date_get_hours", params: 0, returns: primitive),
            // date.getMinutes(): Int
            map_method!(instance "Date", "getMinutes" => "haxe_date_get_minutes", params: 0, returns: primitive),
            // date.getSeconds(): Int
            map_method!(instance "Date", "getSeconds" => "haxe_date_get_seconds", params: 0, returns: primitive),
            // date.getFullYear(): Int
            map_method!(instance "Date", "getFullYear" => "haxe_date_get_full_year", params: 0, returns: primitive),
            // date.getMonth(): Int
            map_method!(instance "Date", "getMonth" => "haxe_date_get_month", params: 0, returns: primitive),
            // date.getDate(): Int
            map_method!(instance "Date", "getDate" => "haxe_date_get_date", params: 0, returns: primitive),
            // date.getDay(): Int
            map_method!(instance "Date", "getDay" => "haxe_date_get_day", params: 0, returns: primitive),

            // Instance methods - UTC
            // date.getUTCHours(): Int
            map_method!(instance "Date", "getUTCHours" => "haxe_date_get_utc_hours", params: 0, returns: primitive),
            // date.getUTCMinutes(): Int
            map_method!(instance "Date", "getUTCMinutes" => "haxe_date_get_utc_minutes", params: 0, returns: primitive),
            // date.getUTCSeconds(): Int
            map_method!(instance "Date", "getUTCSeconds" => "haxe_date_get_utc_seconds", params: 0, returns: primitive),
            // date.getUTCFullYear(): Int
            map_method!(instance "Date", "getUTCFullYear" => "haxe_date_get_utc_full_year", params: 0, returns: primitive),
            // date.getUTCMonth(): Int
            map_method!(instance "Date", "getUTCMonth" => "haxe_date_get_utc_month", params: 0, returns: primitive),
            // date.getUTCDate(): Int
            map_method!(instance "Date", "getUTCDate" => "haxe_date_get_utc_date", params: 0, returns: primitive),
            // date.getUTCDay(): Int
            map_method!(instance "Date", "getUTCDay" => "haxe_date_get_utc_day", params: 0, returns: primitive),

            // Timezone
            // date.getTimezoneOffset(): Int
            map_method!(instance "Date", "getTimezoneOffset" => "haxe_date_get_timezone_offset", params: 0, returns: primitive),

            // String conversion
            // date.toString(): String
            map_method!(instance "Date", "toString" => "haxe_date_to_string", params: 0, returns: primitive),
        ];

        self.register_from_tuples(mappings);
    }

    // ============================================================================
    // Bytes Methods (rayzor.Bytes)
    // ============================================================================

    fn register_bytes_methods(&mut self) {
        let mappings = vec![
            // Static methods
            // Bytes.alloc(size: Int): Bytes
            map_method!(static "Bytes", "alloc" => "haxe_bytes_alloc", params: 1, returns: primitive),
            // Bytes.ofString(s: String): Bytes
            map_method!(static "Bytes", "ofString" => "haxe_bytes_of_string", params: 1, returns: primitive),

            // Property accessor
            // bytes.length: Int
            map_method!(instance "Bytes", "length" => "haxe_bytes_length", params: 0, returns: primitive),

            // Instance methods
            // bytes.get(pos: Int): Int
            map_method!(instance "Bytes", "get" => "haxe_bytes_get", params: 1, returns: primitive),
            // bytes.set(pos: Int, value: Int): Void
            map_method!(instance "Bytes", "set" => "haxe_bytes_set", params: 2, returns: void),
            // bytes.sub(pos: Int, len: Int): Bytes
            map_method!(instance "Bytes", "sub" => "haxe_bytes_sub", params: 2, returns: primitive),
            // bytes.blit(srcPos: Int, dest: Bytes, destPos: Int, len: Int): Void
            map_method!(instance "Bytes", "blit" => "haxe_bytes_blit", params: 4, returns: void),
            // bytes.fill(pos: Int, len: Int, value: Int): Void
            map_method!(instance "Bytes", "fill" => "haxe_bytes_fill", params: 3, returns: void),
            // bytes.compare(other: Bytes): Int
            map_method!(instance "Bytes", "compare" => "haxe_bytes_compare", params: 1, returns: primitive),
            // bytes.toString(): String
            map_method!(instance "Bytes", "toString" => "haxe_bytes_to_string", params: 0, returns: primitive),

            // Integer getters (little-endian)
            // bytes.getInt16(pos: Int): Int
            map_method!(instance "Bytes", "getInt16" => "haxe_bytes_get_int16", params: 1, returns: primitive),
            // bytes.getInt32(pos: Int): Int
            map_method!(instance "Bytes", "getInt32" => "haxe_bytes_get_int32", params: 1, returns: primitive),
            // bytes.getInt64(pos: Int): Int64
            map_method!(instance "Bytes", "getInt64" => "haxe_bytes_get_int64", params: 1, returns: primitive),

            // Float getters (little-endian)
            // bytes.getFloat(pos: Int): Float
            map_method!(instance "Bytes", "getFloat" => "haxe_bytes_get_float", params: 1, returns: primitive),
            // bytes.getDouble(pos: Int): Float
            map_method!(instance "Bytes", "getDouble" => "haxe_bytes_get_double", params: 1, returns: primitive),

            // Integer setters (little-endian)
            // bytes.setInt16(pos: Int, value: Int): Void
            map_method!(instance "Bytes", "setInt16" => "haxe_bytes_set_int16", params: 2, returns: void),
            // bytes.setInt32(pos: Int, value: Int): Void
            map_method!(instance "Bytes", "setInt32" => "haxe_bytes_set_int32", params: 2, returns: void),
            // bytes.setInt64(pos: Int, value: Int64): Void
            map_method!(instance "Bytes", "setInt64" => "haxe_bytes_set_int64", params: 2, returns: void),

            // Float setters (little-endian)
            // bytes.setFloat(pos: Int, value: Float): Void
            map_method!(instance "Bytes", "setFloat" => "haxe_bytes_set_float", params: 2, returns: void),
            // bytes.setDouble(pos: Int, value: Float): Void
            map_method!(instance "Bytes", "setDouble" => "haxe_bytes_set_double", params: 2, returns: void),
        ];

        self.register_from_tuples(mappings);
    }
}

impl Default for StdlibMapping {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_methods() {
        let mapping = StdlibMapping::new();

        // Test charAt
        let sig = MethodSignature {
            class: "String",
            method: "charAt",
            is_static: false,
            is_constructor: false,
        };
        let call = mapping.get(&sig).expect("charAt should be mapped");
        assert_eq!(call.runtime_name, "haxe_string_char_at");
        assert!(!call.needs_out_param);
        assert!(call.has_self_param);
        assert_eq!(call.param_count, 1);
        assert!(call.has_return);

        // Test toUpperCase
        let sig = MethodSignature {
            class: "String",
            method: "toUpperCase",
            is_static: false,
            is_constructor: false,
        };
        let call = mapping.get(&sig).expect("toUpperCase should be mapped");
        assert_eq!(call.runtime_name, "haxe_string_to_upper_case");
        assert!(call.needs_out_param); // Returns String
        assert!(call.has_self_param);
    }

    #[test]
    fn test_array_methods() {
        let mapping = StdlibMapping::new();

        let sig = MethodSignature {
            class: "Array",
            method: "push",
            is_static: false,
            is_constructor: false,
        };
        let call = mapping.get(&sig).expect("push should be mapped");
        assert_eq!(call.runtime_name, "haxe_array_push");
        assert!(call.has_return); // Returns new length
    }

    #[test]
    fn test_math_methods() {
        let mapping = StdlibMapping::new();

        let sig = MethodSignature {
            class: "Math",
            method: "sin",
            is_static: true,
            is_constructor: false,
        };
        let call = mapping.get(&sig).expect("sin should be mapped");
        assert_eq!(call.runtime_name, "haxe_math_sin");
        assert!(!call.has_self_param); // Static method
    }

    #[test]
    fn test_has_mapping() {
        let mapping = StdlibMapping::new();

        assert!(mapping.has_mapping("String", "charAt", false));
        assert!(mapping.has_mapping("Math", "sin", true));
        assert!(!mapping.has_mapping("String", "nonexistent", false));
    }

    #[test]
    fn test_constructor_mapping() {
        let mapping = StdlibMapping::new();

        // Test Channel constructor
        let sig = MethodSignature {
            class: "Channel",
            method: "new",
            is_static: true,
            is_constructor: true,
        };
        let call = mapping.get(&sig).expect("Channel constructor should be mapped");
        assert_eq!(call.runtime_name, "Channel_init");
        assert!(call.needs_out_param); // Returns complex type (opaque pointer)
        assert!(!call.has_self_param); // Constructors don't have self
        assert_eq!(call.param_count, 1);
    }

    #[test]
    fn test_vec_methods() {
        let mapping = StdlibMapping::new();

        // Test VecI32 constructor
        let sig = MethodSignature {
            class: "VecI32",
            method: "new",
            is_static: true,
            is_constructor: true,
        };
        let call = mapping.get(&sig).expect("VecI32 constructor should be mapped");
        assert_eq!(call.runtime_name, "rayzor_vec_i32_new");
        assert!(call.needs_out_param); // Returns complex type (pointer)
        assert!(!call.has_self_param);
        assert_eq!(call.param_count, 0);

        // Test VecI32 push
        let sig = MethodSignature {
            class: "VecI32",
            method: "push",
            is_static: false,
            is_constructor: false,
        };
        let call = mapping.get(&sig).expect("VecI32.push should be mapped");
        assert_eq!(call.runtime_name, "rayzor_vec_i32_push");
        assert!(!call.needs_out_param); // Returns void
        assert!(call.has_self_param); // Instance method
        assert_eq!(call.param_count, 1);
        assert!(!call.has_return);

        // Test VecF64 get
        let sig = MethodSignature {
            class: "VecF64",
            method: "get",
            is_static: false,
            is_constructor: false,
        };
        let call = mapping.get(&sig).expect("VecF64.get should be mapped");
        assert_eq!(call.runtime_name, "rayzor_vec_f64_get");
        assert!(call.has_self_param);
        assert_eq!(call.param_count, 1);
        assert!(call.has_return);

        // Test VecPtr for reference types
        let sig = MethodSignature {
            class: "VecPtr",
            method: "push",
            is_static: false,
            is_constructor: false,
        };
        let call = mapping.get(&sig).expect("VecPtr.push should be mapped");
        assert_eq!(call.runtime_name, "rayzor_vec_ptr_push");

        // Test VecBool
        let sig = MethodSignature {
            class: "VecBool",
            method: "pop",
            is_static: false,
            is_constructor: false,
        };
        let call = mapping.get(&sig).expect("VecBool.pop should be mapped");
        assert_eq!(call.runtime_name, "rayzor_vec_bool_pop");
        assert!(call.has_return); // Returns bool
    }
}
