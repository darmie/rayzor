//! TinyCC-based in-process linker for LLVM AOT object files.
//!
//! Replaces the system linker + dlopen approach with TCC's built-in
//! ELF linker and in-memory relocator. This eliminates the need for
//! a system C compiler at runtime and reduces temp file I/O.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;

use crate::ir::IrFunctionId;

// FFI bindings to libtcc (linked statically via build.rs)
#[allow(non_camel_case_types)]
type TCCState = std::ffi::c_void;

extern "C" {
    fn tcc_new() -> *mut TCCState;
    fn tcc_delete(s: *mut TCCState);
    fn tcc_set_output_type(s: *mut TCCState, output_type: i32) -> i32;
    fn tcc_set_options(s: *mut TCCState, str: *const i8) -> i32;
    fn tcc_add_file(s: *mut TCCState, filename: *const i8) -> i32;
    fn tcc_add_symbol(s: *mut TCCState, name: *const i8, val: *const std::ffi::c_void) -> i32;
    fn tcc_relocate(s: *mut TCCState) -> i32;
    fn tcc_get_symbol(s: *mut TCCState, name: *const i8) -> *mut std::ffi::c_void;
    fn tcc_set_error_func(
        s: *mut TCCState,
        error_opaque: *mut std::ffi::c_void,
        error_func: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *const i8)>,
    );
}

const TCC_OUTPUT_MEMORY: i32 = 1;

/// Error callback that collects TCC error messages
unsafe extern "C" fn tcc_error_callback(opaque: *mut std::ffi::c_void, msg: *const i8) {
    let errors = &mut *(opaque as *mut Vec<String>);
    if let Ok(s) = CStr::from_ptr(msg).to_str() {
        errors.push(s.to_string());
    }
}

/// In-process linker using TinyCC to load ELF object files into executable memory.
pub struct TccLinker {
    state: *mut TCCState,
    /// Collected error messages from TCC (boxed for stable pointer passed to C callback)
    #[allow(clippy::box_collection)]
    errors: Box<Vec<String>>,
    /// Whether relocate() has been called
    relocated: bool,
}

// TCC state is not Send/Sync safe (uses globals), but we only use it
// under the global LLVM lock so this is safe in practice.
unsafe impl Send for TccLinker {}

impl TccLinker {
    /// Create a new TCC linker context and register runtime symbols.
    ///
    /// Each symbol in `runtime_symbols` is registered via `tcc_add_symbol`
    /// so that the loaded object file can call host-process functions.
    pub fn new(runtime_symbols: &[(String, usize)]) -> Result<Self, String> {
        let state = unsafe { tcc_new() };
        if state.is_null() {
            return Err("Failed to create TCC state".to_string());
        }

        let mut errors = Box::new(Vec::new());

        // Set error callback
        unsafe {
            tcc_set_error_func(
                state,
                errors.as_mut() as *mut Vec<String> as *mut std::ffi::c_void,
                Some(tcc_error_callback),
            );
        }

        // Set output type to memory (JIT-style loading)
        let ret = unsafe { tcc_set_output_type(state, TCC_OUTPUT_MEMORY) };
        if ret < 0 {
            unsafe { tcc_delete(state) };
            return Err("Failed to set TCC output type to memory".to_string());
        }

        // Disable standard library linking — we only need to relocate pre-compiled
        // object files with our own runtime symbols, not link against libc/libtcc1.
        let nostdlib = CString::new("-nostdlib").unwrap();
        unsafe { tcc_set_options(state, nostdlib.as_ptr()) };

        // Register libc symbols that LLVM-generated code may reference.
        // With -nostdlib, TCC won't resolve these automatically.
        {
            extern "C" {
                fn malloc(size: usize) -> *mut std::ffi::c_void;
                fn realloc(ptr: *mut std::ffi::c_void, size: usize) -> *mut std::ffi::c_void;
                fn calloc(nmemb: usize, size: usize) -> *mut std::ffi::c_void;
                fn free(ptr: *mut std::ffi::c_void);
                fn memcpy(
                    dst: *mut std::ffi::c_void,
                    src: *const std::ffi::c_void,
                    n: usize,
                ) -> *mut std::ffi::c_void;
                fn memset(s: *mut std::ffi::c_void, c: i32, n: usize) -> *mut std::ffi::c_void;
                fn memmove(
                    dst: *mut std::ffi::c_void,
                    src: *const std::ffi::c_void,
                    n: usize,
                ) -> *mut std::ffi::c_void;
                fn abort();
            }
            let libc_syms: &[(&str, *const std::ffi::c_void)] = &[
                ("malloc", malloc as *const std::ffi::c_void),
                ("realloc", realloc as *const std::ffi::c_void),
                ("calloc", calloc as *const std::ffi::c_void),
                ("free", free as *const std::ffi::c_void),
                ("memcpy", memcpy as *const std::ffi::c_void),
                ("memset", memset as *const std::ffi::c_void),
                ("memmove", memmove as *const std::ffi::c_void),
                ("abort", abort as *const std::ffi::c_void),
            ];
            for (name, addr) in libc_syms {
                let c_name = CString::new(*name).unwrap();
                unsafe { tcc_add_symbol(state, c_name.as_ptr(), *addr) };
            }
        }

        // Register all runtime symbols
        for (name, addr) in runtime_symbols {
            let c_name = CString::new(name.as_str())
                .map_err(|e| format!("Invalid symbol name '{}': {}", name, e))?;
            let ret =
                unsafe { tcc_add_symbol(state, c_name.as_ptr(), *addr as *const std::ffi::c_void) };
            if ret < 0 {
                let errs = errors.join("; ");
                unsafe { tcc_delete(state) };
                return Err(format!("Failed to add symbol '{}': {}", name, errs));
            }
        }

        tracing::trace!("[TCC] Registered {} runtime symbols", runtime_symbols.len());

        Ok(Self {
            state,
            errors,
            relocated: false,
        })
    }

    /// Add an ELF object file to be linked.
    ///
    /// The object file must be in ELF format (not Mach-O).
    /// On macOS, LLVM must be configured to emit ELF via target triple override.
    pub fn add_object_file(&mut self, path: &Path) -> Result<(), String> {
        let c_path = CString::new(path.to_str().ok_or("Invalid object file path")?)
            .map_err(|e| format!("Invalid path: {}", e))?;

        let ret = unsafe { tcc_add_file(self.state, c_path.as_ptr()) };
        if ret < 0 {
            let errs = self.errors.join("; ");
            return Err(format!(
                "TCC failed to load object file '{}': {}",
                path.display(),
                if errs.is_empty() {
                    "unknown error"
                } else {
                    &errs
                }
            ));
        }

        tracing::trace!("[TCC] Loaded object file: {:?}", path);
        Ok(())
    }

    /// Relocate all loaded code into executable memory.
    ///
    /// After this call, function pointers can be retrieved via `get_symbol`.
    /// The memory is owned by the TCC state and remains valid until the
    /// `TccLinker` is dropped (or leaked).
    pub fn relocate(&mut self) -> Result<(), String> {
        let ret = unsafe { tcc_relocate(self.state) };
        if ret < 0 {
            let errs = self.errors.join("; ");
            return Err(format!(
                "TCC relocation failed: {}",
                if errs.is_empty() {
                    "unknown error"
                } else {
                    &errs
                }
            ));
        }

        self.relocated = true;
        tracing::trace!("[TCC] Relocation succeeded");
        Ok(())
    }

    /// Get a function pointer by symbol name.
    ///
    /// Must be called after `relocate()`.
    pub fn get_symbol(&self, name: &str) -> Result<usize, String> {
        if !self.relocated {
            return Err("Cannot get symbol before relocate()".to_string());
        }

        let c_name =
            CString::new(name).map_err(|e| format!("Invalid symbol name '{}': {}", name, e))?;
        let ptr = unsafe { tcc_get_symbol(self.state, c_name.as_ptr()) };
        if ptr.is_null() {
            return Err(format!("Symbol '{}' not found", name));
        }

        Ok(ptr as usize)
    }

    /// Load an object file, relocate, and extract function pointers.
    ///
    /// Convenience method combining add_object_file + relocate + get_symbol.
    pub fn link_object_file(
        &mut self,
        obj_path: &Path,
        function_symbols: &HashMap<IrFunctionId, String>,
    ) -> Result<HashMap<IrFunctionId, usize>, String> {
        self.add_object_file(obj_path)?;
        self.relocate()?;

        let mut pointers = HashMap::new();
        for (func_id, symbol_name) in function_symbols {
            match self.get_symbol(symbol_name) {
                Ok(ptr) if ptr != 0 => {
                    pointers.insert(*func_id, ptr);
                }
                Ok(_) => {
                    tracing::warn!("[TCC] Symbol '{}' resolved to null", symbol_name);
                }
                Err(e) => {
                    tracing::debug!("[TCC] Symbol '{}' not found: {}", symbol_name, e);
                }
            }
        }

        tracing::trace!(
            "[TCC] Loaded {} function pointers from object file",
            pointers.len()
        );

        Ok(pointers)
    }
}

impl Drop for TccLinker {
    fn drop(&mut self) {
        if !self.state.is_null() {
            unsafe { tcc_delete(self.state) };
            self.state = ptr::null_mut();
        }
    }
}
