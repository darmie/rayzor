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

    /// Register a stdlib method -> runtime function mapping
    fn register(&mut self, sig: MethodSignature, call: RuntimeFunctionCall) {
        self.mappings.insert(sig, call);
    }
}

/// Macro to register stdlib methods more concisely
macro_rules! map_method {
    // Instance method returning primitive
    (instance $class:expr, $method:expr => $runtime:expr, params: $params:expr, returns: primitive) => {
        (
            MethodSignature {
                class: $class,
                method: $method,
                is_static: false,
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: true,
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
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: true,
                has_self_param: true,
                param_count: $params,
                has_return: false,
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
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: true,
                param_count: $params,
                has_return: false,
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
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: false,
                param_count: $params,
                has_return: true,
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
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: true,
                has_self_param: false,
                param_count: $params,
                has_return: false,
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
            },
            RuntimeFunctionCall {
                runtime_name: $runtime,
                needs_out_param: false,
                has_self_param: false,
                param_count: $params,
                has_return: false,
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
            // Modification methods
            map_method!(instance "Array", "push" => "haxe_array_push", params: 1, returns: primitive),
            map_method!(instance "Array", "pop" => "haxe_array_pop", params: 0, returns: complex),
            map_method!(instance "Array", "reverse" => "haxe_array_reverse", params: 0, returns: void),
            map_method!(instance "Array", "insert" => "haxe_array_insert", params: 2, returns: void),
            map_method!(instance "Array", "remove" => "haxe_array_remove", params: 1, returns: primitive),

            // Extraction methods
            map_method!(instance "Array", "slice" => "haxe_array_slice", params: 2, returns: complex),
            map_method!(instance "Array", "copy" => "haxe_array_copy", params: 0, returns: complex),

            // Search methods
            map_method!(instance "Array", "indexOf" => "haxe_array_index_of", params: 2, returns: primitive),
            map_method!(instance "Array", "lastIndexOf" => "haxe_array_last_index_of", params: 2, returns: primitive),
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
}
