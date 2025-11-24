//! Runtime plugin implementation
//!
//! This registers all runtime functions as a plugin

/// Thread-safe function pointer wrapper
pub struct FunctionPtr(*const u8);

unsafe impl Send for FunctionPtr {}
unsafe impl Sync for FunctionPtr {}

impl FunctionPtr {
    pub const fn new(ptr: *const u8) -> Self {
        FunctionPtr(ptr)
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.0
    }
}

/// Runtime symbol for inventory-based registration
pub struct RuntimeSymbol {
    pub name: &'static str,
    pub ptr: FunctionPtr,
}

inventory::collect!(RuntimeSymbol);

/// Register a runtime symbol
macro_rules! register_symbol {
    ($name:expr, $func:path) => {
        inventory::submit! {
            RuntimeSymbol {
                name: $name,
                ptr: FunctionPtr::new($func as *const u8),
            }
        }
    };
}

// ============================================================================
// Vec Functions (Simple pointer-based API)
// ============================================================================
register_symbol!("haxe_vec_new_ptr", crate::vec_plugin::haxe_vec_new_ptr);
register_symbol!("haxe_vec_push_ptr", crate::vec_plugin::haxe_vec_push_ptr);
register_symbol!("haxe_vec_get_ptr", crate::vec_plugin::haxe_vec_get_ptr);
register_symbol!("haxe_vec_len_ptr", crate::vec_plugin::haxe_vec_len_ptr);
register_symbol!("haxe_vec_free_ptr", crate::vec_plugin::haxe_vec_free_ptr);

// ============================================================================
// String Functions (Comprehensive Haxe String API)
// ============================================================================

// Creation
register_symbol!("haxe_string_new", crate::haxe_string::haxe_string_new);
register_symbol!("haxe_string_from_cstr", crate::haxe_string::haxe_string_from_cstr);
register_symbol!("haxe_string_from_bytes", crate::haxe_string::haxe_string_from_bytes);

// Properties
register_symbol!("haxe_string_length", crate::haxe_string::haxe_string_length);
register_symbol!("haxe_string_char_at", crate::haxe_string::haxe_string_char_at);
register_symbol!("haxe_string_char_code_at", crate::haxe_string::haxe_string_char_code_at);

// Operations
register_symbol!("haxe_string_concat", crate::haxe_string::haxe_string_concat);
register_symbol!("haxe_string_substring", crate::haxe_string::haxe_string_substring);
register_symbol!("haxe_string_substr", crate::haxe_string::haxe_string_substr);
register_symbol!("haxe_string_to_upper_case", crate::haxe_string::haxe_string_to_upper_case);
register_symbol!("haxe_string_to_lower_case", crate::haxe_string::haxe_string_to_lower_case);
register_symbol!("haxe_string_index_of", crate::haxe_string::haxe_string_index_of);
register_symbol!("haxe_string_split", crate::haxe_string::haxe_string_split);

// Memory
register_symbol!("haxe_string_free", crate::haxe_string::haxe_string_free);

// I/O
register_symbol!("haxe_string_print", crate::haxe_string::haxe_string_print);
register_symbol!("haxe_string_println", crate::haxe_string::haxe_string_println);
register_symbol!("haxe_string_to_cstr", crate::haxe_string::haxe_string_to_cstr);

// ============================================================================
// Array Functions (Generic Dynamic Array)
// ============================================================================

// Creation
register_symbol!("haxe_array_new", crate::haxe_array::haxe_array_new);
register_symbol!("haxe_array_from_elements", crate::haxe_array::haxe_array_from_elements);

// Properties
register_symbol!("haxe_array_length", crate::haxe_array::haxe_array_length);

// Access
register_symbol!("haxe_array_get", crate::haxe_array::haxe_array_get);
register_symbol!("haxe_array_set", crate::haxe_array::haxe_array_set);
register_symbol!("haxe_array_get_ptr", crate::haxe_array::haxe_array_get_ptr);

// Modification
register_symbol!("haxe_array_push", crate::haxe_array::haxe_array_push);
register_symbol!("haxe_array_pop", crate::haxe_array::haxe_array_pop);
register_symbol!("haxe_array_insert", crate::haxe_array::haxe_array_insert);
register_symbol!("haxe_array_remove", crate::haxe_array::haxe_array_remove);
register_symbol!("haxe_array_reverse", crate::haxe_array::haxe_array_reverse);

// Operations
register_symbol!("haxe_array_copy", crate::haxe_array::haxe_array_copy);
register_symbol!("haxe_array_slice", crate::haxe_array::haxe_array_slice);

// Memory
register_symbol!("haxe_array_free", crate::haxe_array::haxe_array_free);

// Specialized integer operations
register_symbol!("haxe_array_push_i32", crate::haxe_array::haxe_array_push_i32);
register_symbol!("haxe_array_get_i32", crate::haxe_array::haxe_array_get_i32);
register_symbol!("haxe_array_push_i64", crate::haxe_array::haxe_array_push_i64);
register_symbol!("haxe_array_get_i64", crate::haxe_array::haxe_array_get_i64);
register_symbol!("haxe_array_push_f64", crate::haxe_array::haxe_array_push_f64);
register_symbol!("haxe_array_get_f64", crate::haxe_array::haxe_array_get_f64);

// ============================================================================
// Math Functions
// ============================================================================

// Constants
register_symbol!("haxe_math_pi", crate::haxe_math::haxe_math_pi);
register_symbol!("haxe_math_e", crate::haxe_math::haxe_math_e);

// Basic operations
register_symbol!("haxe_math_abs", crate::haxe_math::haxe_math_abs);
register_symbol!("haxe_math_min", crate::haxe_math::haxe_math_min);
register_symbol!("haxe_math_max", crate::haxe_math::haxe_math_max);
register_symbol!("haxe_math_floor", crate::haxe_math::haxe_math_floor);
register_symbol!("haxe_math_ceil", crate::haxe_math::haxe_math_ceil);
register_symbol!("haxe_math_round", crate::haxe_math::haxe_math_round);

// Trigonometric
register_symbol!("haxe_math_sin", crate::haxe_math::haxe_math_sin);
register_symbol!("haxe_math_cos", crate::haxe_math::haxe_math_cos);
register_symbol!("haxe_math_tan", crate::haxe_math::haxe_math_tan);
register_symbol!("haxe_math_asin", crate::haxe_math::haxe_math_asin);
register_symbol!("haxe_math_acos", crate::haxe_math::haxe_math_acos);
register_symbol!("haxe_math_atan", crate::haxe_math::haxe_math_atan);
register_symbol!("haxe_math_atan2", crate::haxe_math::haxe_math_atan2);

// Exponential and logarithmic
register_symbol!("haxe_math_exp", crate::haxe_math::haxe_math_exp);
register_symbol!("haxe_math_log", crate::haxe_math::haxe_math_log);
register_symbol!("haxe_math_pow", crate::haxe_math::haxe_math_pow);
register_symbol!("haxe_math_sqrt", crate::haxe_math::haxe_math_sqrt);

// Special
register_symbol!("haxe_math_is_nan", crate::haxe_math::haxe_math_is_nan);
register_symbol!("haxe_math_is_finite", crate::haxe_math::haxe_math_is_finite);
register_symbol!("haxe_math_random", crate::haxe_math::haxe_math_random);

// ============================================================================
// Sys Functions (System and I/O)
// ============================================================================

// Console I/O
register_symbol!("haxe_sys_print_int", crate::haxe_sys::haxe_sys_print_int);
register_symbol!("haxe_sys_print_float", crate::haxe_sys::haxe_sys_print_float);
register_symbol!("haxe_sys_print_bool", crate::haxe_sys::haxe_sys_print_bool);
register_symbol!("haxe_sys_println", crate::haxe_sys::haxe_sys_println);

// Program control
register_symbol!("haxe_sys_exit", crate::haxe_sys::haxe_sys_exit);
register_symbol!("haxe_sys_time", crate::haxe_sys::haxe_sys_time);
register_symbol!("haxe_sys_args_count", crate::haxe_sys::haxe_sys_args_count);

// ============================================================================
// Concurrency Functions (Thread, Arc, Mutex, Channel)
// ============================================================================

// Thread functions
register_symbol!("rayzor_thread_spawn", crate::concurrency::rayzor_thread_spawn);
register_symbol!("rayzor_thread_join", crate::concurrency::rayzor_thread_join);
register_symbol!("rayzor_thread_is_finished", crate::concurrency::rayzor_thread_is_finished);
register_symbol!("rayzor_thread_yield_now", crate::concurrency::rayzor_thread_yield_now);
register_symbol!("rayzor_thread_sleep", crate::concurrency::rayzor_thread_sleep);
register_symbol!("rayzor_thread_current_id", crate::concurrency::rayzor_thread_current_id);
// Thread tracking for JIT safety
register_symbol!("rayzor_wait_all_threads", crate::concurrency::rayzor_wait_all_threads);
register_symbol!("rayzor_active_thread_count", crate::concurrency::rayzor_active_thread_count);

// Arc functions
register_symbol!("rayzor_arc_init", crate::concurrency::rayzor_arc_init);
register_symbol!("rayzor_arc_clone", crate::concurrency::rayzor_arc_clone);
register_symbol!("rayzor_arc_get", crate::concurrency::rayzor_arc_get);
register_symbol!("rayzor_arc_strong_count", crate::concurrency::rayzor_arc_strong_count);
register_symbol!("rayzor_arc_try_unwrap", crate::concurrency::rayzor_arc_try_unwrap);
register_symbol!("rayzor_arc_as_ptr", crate::concurrency::rayzor_arc_as_ptr);

// Mutex functions
register_symbol!("rayzor_mutex_init", crate::concurrency::rayzor_mutex_init);
register_symbol!("rayzor_mutex_lock", crate::concurrency::rayzor_mutex_lock);
register_symbol!("rayzor_mutex_try_lock", crate::concurrency::rayzor_mutex_try_lock);
register_symbol!("rayzor_mutex_is_locked", crate::concurrency::rayzor_mutex_is_locked);
register_symbol!("rayzor_mutex_guard_get", crate::concurrency::rayzor_mutex_guard_get);
register_symbol!("rayzor_mutex_unlock", crate::concurrency::rayzor_mutex_unlock);

// Channel functions
register_symbol!("rayzor_channel_init", crate::concurrency::rayzor_channel_init);
register_symbol!("rayzor_channel_send", crate::concurrency::rayzor_channel_send);
register_symbol!("rayzor_channel_try_send", crate::concurrency::rayzor_channel_try_send);
register_symbol!("rayzor_channel_receive", crate::concurrency::rayzor_channel_receive);
register_symbol!("rayzor_channel_try_receive", crate::concurrency::rayzor_channel_try_receive);
register_symbol!("rayzor_channel_close", crate::concurrency::rayzor_channel_close);
register_symbol!("rayzor_channel_is_closed", crate::concurrency::rayzor_channel_is_closed);
register_symbol!("rayzor_channel_len", crate::concurrency::rayzor_channel_len);
register_symbol!("rayzor_channel_capacity", crate::concurrency::rayzor_channel_capacity);
register_symbol!("rayzor_channel_is_empty", crate::concurrency::rayzor_channel_is_empty);
register_symbol!("rayzor_channel_is_full", crate::concurrency::rayzor_channel_is_full);

/// Rayzor Runtime Plugin
pub struct RayzorRuntimePlugin;

impl RayzorRuntimePlugin {
    pub fn new() -> Self {
        RayzorRuntimePlugin
    }
}

/// Get the runtime plugin instance
pub fn get_plugin() -> Box<dyn rayzor_plugin::RuntimePlugin> {
    Box::new(RayzorRuntimePlugin)
}

// Note: We manually implement RuntimePlugin instead of using proc macros
// to avoid the complexity of creating a separate proc-macro crate for now

impl rayzor_plugin::RuntimePlugin for RayzorRuntimePlugin {
    fn name(&self) -> &str {
        "rayzor_runtime"
    }

    fn runtime_symbols(&self) -> Vec<(&'static str, *const u8)> {
        inventory::iter::<RuntimeSymbol>
            .into_iter()
            .map(|sym| (sym.name, sym.ptr.as_ptr()))
            .collect()
    }
}
