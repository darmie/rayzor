//! LLVM AOT Backend — AOT-specific operations on top of LLVMJitBackend
//!
//! Free functions for cross-compilation, main wrapper generation, and multiple
//! output format support without modifying the JIT code path.

#[cfg(feature = "llvm-backend")]
use inkwell::{
    module::Module,
    passes::PassBuilderOptions,
    targets::{
        CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
    },
    values::AsValueRef,
    OptimizationLevel,
};

#[cfg(feature = "llvm-backend")]
use std::path::Path;

#[cfg(feature = "llvm-backend")]
use std::sync::Once;

#[cfg(feature = "llvm-backend")]
static AOT_INIT: Once = Once::new();

/// Initialize LLVM for AOT compilation (all targets, no MCJIT).
#[cfg(feature = "llvm-backend")]
pub fn init_llvm_aot() {
    AOT_INIT.call_once(|| {
        Target::initialize_native(&InitializationConfig::default())
            .expect("Failed to initialize LLVM native target");
        Target::initialize_all(&InitializationConfig::default());
    });
}

#[cfg(feature = "llvm-backend")]
fn create_target_machine(
    module: &Module,
    target_triple: Option<&str>,
    reloc_mode: RelocMode,
    opt_level: OptimizationLevel,
) -> Result<TargetMachine, String> {
    let (triple, cpu, features) = if let Some(triple_str) = target_triple {
        (
            TargetTriple::create(triple_str),
            "generic".to_string(),
            String::new(),
        )
    } else {
        let triple = TargetMachine::get_default_triple();
        let cpu = TargetMachine::get_host_cpu_name()
            .to_str()
            .unwrap_or("generic")
            .to_string();
        let features = TargetMachine::get_host_cpu_features()
            .to_str()
            .unwrap_or("")
            .to_string();
        (triple, cpu, features)
    };

    module.set_triple(&triple);

    let target = Target::from_triple(&triple)
        .map_err(|e| format!("Failed to get target for triple: {}", e))?;

    target
        .create_target_machine(
            &triple,
            &cpu,
            &features,
            opt_level,
            reloc_mode,
            CodeModel::Default,
        )
        .ok_or_else(|| "Failed to create target machine".to_string())
}

#[cfg(feature = "llvm-backend")]
fn run_opt_passes(
    module: &Module,
    target_machine: &TargetMachine,
    opt_level: OptimizationLevel,
) -> Result<(), String> {
    if opt_level != OptimizationLevel::None {
        let passes = match opt_level {
            OptimizationLevel::None => "default<O0>",
            OptimizationLevel::Less => "default<O1>",
            OptimizationLevel::Default => "default<O2>",
            OptimizationLevel::Aggressive => "default<O3>",
        };
        let pass_options = PassBuilderOptions::create();
        module
            .run_passes(passes, target_machine, pass_options)
            .map_err(|e| format!("Failed to run optimization passes: {}", e))?;
    }
    Ok(())
}

/// Compile to object file with configurable target and relocation mode.
#[cfg(feature = "llvm-backend")]
pub fn compile_to_object_file(
    module: &Module,
    output_path: &Path,
    target_triple: Option<&str>,
    reloc_mode: RelocMode,
    opt_level: OptimizationLevel,
) -> Result<(), String> {
    if let Err(msg) = module.verify() {
        return Err(format!(
            "LLVM module verification failed: {}",
            msg.to_string()
        ));
    }

    let target_machine = create_target_machine(module, target_triple, reloc_mode, opt_level)?;
    run_opt_passes(module, &target_machine, opt_level)?;

    target_machine
        .write_to_file(module, FileType::Object, output_path)
        .map_err(|e| format!("Failed to write object file: {}", e))
}

/// Emit LLVM IR text (.ll).
#[cfg(feature = "llvm-backend")]
pub fn emit_llvm_ir(module: &Module, output_path: &Path) -> Result<(), String> {
    if let Err(msg) = module.verify() {
        return Err(format!(
            "LLVM module verification failed: {}",
            msg.to_string()
        ));
    }
    let ir_str = module.print_to_string().to_string();
    std::fs::write(output_path, ir_str).map_err(|e| format!("Failed to write LLVM IR: {}", e))
}

/// Emit LLVM bitcode (.bc).
#[cfg(feature = "llvm-backend")]
pub fn emit_llvm_bitcode(module: &Module, output_path: &Path) -> Result<(), String> {
    if let Err(msg) = module.verify() {
        return Err(format!(
            "LLVM module verification failed: {}",
            msg.to_string()
        ));
    }
    if module.write_bitcode_to_path(output_path) {
        Ok(())
    } else {
        Err("Failed to write LLVM bitcode".to_string())
    }
}

/// Emit native assembly (.s).
#[cfg(feature = "llvm-backend")]
pub fn emit_assembly(
    module: &Module,
    output_path: &Path,
    target_triple: Option<&str>,
    opt_level: OptimizationLevel,
) -> Result<(), String> {
    if let Err(msg) = module.verify() {
        return Err(format!(
            "LLVM module verification failed: {}",
            msg.to_string()
        ));
    }

    let target_machine =
        create_target_machine(module, target_triple, RelocMode::Default, opt_level)?;
    run_opt_passes(module, &target_machine, opt_level)?;

    target_machine
        .write_to_file(module, FileType::Assembly, output_path)
        .map_err(|e| format!("Failed to write assembly: {}", e))
}

/// Generate a C main() wrapper that calls the Haxe entry point.
///
/// Creates: `int main(int argc, char** argv) { <entry>(0); return 0; }`
/// If entry is named "main", renames it to "_haxe_main" first.
#[cfg(feature = "llvm-backend")]
pub fn generate_main_wrapper(module: &Module, entry_func_name: &str) -> Result<(), String> {
    let entry_func = module.get_function(entry_func_name).ok_or_else(|| {
        format!(
            "Entry function '{}' not found in LLVM module",
            entry_func_name
        )
    })?;

    // Rename if collides with C main
    let actual_name = if entry_func_name == "main" {
        unsafe {
            use std::ffi::CString;
            let new_name = CString::new("_haxe_main").unwrap();
            llvm_sys::core::LLVMSetValueName2(
                entry_func.as_value_ref(),
                new_name.as_ptr(),
                "_haxe_main".len(),
            );
        }
        "_haxe_main"
    } else {
        entry_func_name
    };

    let context = module.get_context();
    let i32_type = context.i32_type();
    let i64_type = context.i64_type();
    let i8_ptr_type = context.ptr_type(inkwell::AddressSpace::default());

    let main_fn_type = i32_type.fn_type(&[i32_type.into(), i8_ptr_type.into()], false);
    let main_fn = module.add_function("main", main_fn_type, None);
    let entry_bb = context.append_basic_block(main_fn, "entry");

    let builder = context.create_builder();
    builder.position_at_end(entry_bb);

    let haxe_entry = module
        .get_function(actual_name)
        .ok_or_else(|| format!("Renamed entry function '{}' not found", actual_name))?;

    let zero = i64_type.const_int(0, false);
    builder
        .build_call(haxe_entry, &[zero.into()], "")
        .map_err(|e| format!("Failed to build call to entry: {}", e))?;

    builder
        .build_return(Some(&i32_type.const_int(0, false)))
        .map_err(|e| format!("Failed to build return: {}", e))?;

    Ok(())
}
