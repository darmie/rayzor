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

    /// Which parameters need to be converted from values to pointers
    /// This is a bitmask where bit N indicates parameter N needs pointer conversion.
    /// For example: params_need_ptr_conversion = 0b10 means param[1] needs conversion
    /// (bit 0 = param 0, bit 1 = param 1, etc.)
    /// This is used for runtime functions that take `*const u8` for data parameters.
    pub params_need_ptr_conversion: u32,
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
        mapping.register_thread_methods();
        mapping.register_channel_methods();
        mapping.register_arc_methods();
        mapping.register_mutex_methods();

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

    /// Register a stdlib method -> runtime function mapping
    fn register(&mut self, sig: MethodSignature, call: RuntimeFunctionCall) {
        self.mappings.insert(sig, call);
    }
}

/// Macro to register stdlib methods more concisely
macro_rules! map_method {
    // Constructor - returns complex type (opaque pointer to extern class)
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
            }
        )
    };

    // Instance method returning primitive with pointer conversion metadata
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
            }
        )
    };

    // Instance method returning void with pointer conversion metadata
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
            map_method!(static "String", "fromCharCode" => "haxe_string_from_char_code", params: 1, returns: complex),

            // Instance methods - character access
            map_method!(instance "String", "charAt" => "haxe_string_char_at", params: 1, returns: primitive),
            map_method!(instance "String", "charCodeAt" => "haxe_string_char_code_at", params: 1, returns: primitive),

            // Search operations
            map_method!(instance "String", "indexOf" => "haxe_string_index_of", params: 2, returns: primitive),
            map_method!(instance "String", "lastIndexOf" => "haxe_string_last_index_of", params: 2, returns: primitive),

            // String transformations
            map_method!(instance "String", "split" => "haxe_string_split", params: 1, returns: complex),
            map_method!(instance "String", "substr" => "haxe_string_substr", params: 2, returns: complex),
            map_method!(instance "String", "substring" => "haxe_string_substring", params: 2, returns: complex),
            map_method!(instance "String", "toLowerCase" => "haxe_string_to_lower_case", params: 0, returns: complex),
            map_method!(instance "String", "toUpperCase" => "haxe_string_to_upper_case", params: 0, returns: complex),
            map_method!(instance "String", "toString" => "haxe_string_copy", params: 0, returns: complex),
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
            map_method!(instance "Array", "pop" => "haxe_array_pop", params: 0, returns: complex),
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
            map_method!(static "Sys", "print" => "haxe_string_print", params: 1, returns: void),
            map_method!(static "Sys", "println" => "haxe_sys_println", params: 0, returns: void),
            map_method!(static "Sys", "exit" => "haxe_sys_exit", params: 1, returns: void),
            map_method!(static "Sys", "time" => "haxe_sys_time", params: 0, returns: primitive),
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
}
