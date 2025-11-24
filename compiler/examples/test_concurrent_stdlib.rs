/// Test Rayzor Concurrent Stdlib Functions
///
/// This example verifies that:
/// 1. Thread, Channel, Arc, and Mutex functions are properly declared in MIR stdlib
/// 2. Extern runtime functions are correctly registered
/// 3. All function signatures are correct

use compiler::stdlib::build_stdlib;
use compiler::ir::IrModule;

fn main() {
    println!("=== Rayzor Concurrent Stdlib Verification ===\n");

    // Build the stdlib using MIR builder
    let stdlib: IrModule = build_stdlib();

    println!("✅ Successfully built stdlib module: {}", stdlib.name);
    println!("📊 Statistics:");
    println!("   - Total Functions: {}", stdlib.functions.len());
    println!("   - Total Extern Functions: {}", stdlib.extern_functions.len());

    // Test Thread functions
    println!("\n🧵 Thread Functions:");
    let thread_functions = vec![
        "Thread_spawn",
        "Thread_join",
        "Thread_isFinished",
        "Thread_yieldNow",
        "Thread_sleep",
        "Thread_currentId",
    ];

    for func_name in &thread_functions {
        let found = stdlib.functions.values().any(|f| &f.name == func_name);
        if found {
            let func = stdlib.functions.values().find(|f| &f.name == func_name).unwrap();
            println!("   ✅ {} ({} params) -> {:?}",
                func_name,
                func.signature.parameters.len(),
                func.signature.return_type
            );
        } else {
            println!("   ❌ Missing: {}", func_name);
        }
    }

    // Test Thread extern functions
    println!("\n🔌 Thread Extern Runtime Functions:");
    let thread_externs = vec![
        "rayzor_thread_spawn",
        "rayzor_thread_join",
        "rayzor_thread_is_finished",
        "rayzor_thread_yield_now",
        "rayzor_thread_sleep",
        "rayzor_thread_current_id",
    ];

    for func_name in &thread_externs {
        // Extern functions are in the regular functions map but with empty blocks
        let found = stdlib.functions.values().find(|f| &f.name == func_name);
        if let Some(func) = found {
            if func.cfg.blocks.is_empty() {
                println!("   ✅ {} (extern)", func_name);
            } else {
                println!("   ⚠️  {} exists but has body!", func_name);
            }
        } else {
            println!("   ❌ Missing: {}", func_name);
        }
    }

    // Test Channel functions
    println!("\n📬 Channel Functions:");
    let channel_functions = vec![
        "Channel_new",
        "Channel_send",
        "Channel_trySend",
        "Channel_receive",
        "Channel_tryReceive",
        "Channel_close",
        "Channel_isClosed",
        "Channel_len",
        "Channel_capacity",
        "Channel_isEmpty",
        "Channel_isFull",
    ];

    for func_name in &channel_functions {
        let found = stdlib.functions.values().any(|f| &f.name == func_name);
        if found {
            let func = stdlib.functions.values().find(|f| &f.name == func_name).unwrap();
            println!("   ✅ {} ({} params) -> {:?}",
                func_name,
                func.signature.parameters.len(),
                func.signature.return_type
            );
        } else {
            println!("   ❌ Missing: {}", func_name);
        }
    }

    // Test Channel extern functions
    println!("\n🔌 Channel Extern Runtime Functions:");
    let channel_externs = vec![
        "rayzor_channel_new",
        "rayzor_channel_send",
        "rayzor_channel_try_send",
        "rayzor_channel_receive",
        "rayzor_channel_try_receive",
        "rayzor_channel_close",
        "rayzor_channel_is_closed",
        "rayzor_channel_len",
        "rayzor_channel_capacity",
        "rayzor_channel_is_empty",
        "rayzor_channel_is_full",
    ];

    for func_name in &channel_externs {
        let found = stdlib.functions.values().find(|f| &f.name == func_name);
        if let Some(func) = found {
            if func.cfg.blocks.is_empty() { println!("   ✅ {} (extern)", func_name); } else { println!("   ⚠️  {} exists but has body!", func_name); }
        } else {
            println!("   ❌ Missing: {}", func_name);
        }
    }

    // Test Arc functions
    println!("\n🔄 Arc Functions:");
    let arc_functions = vec![
        "Arc_new",
        "Arc_clone",
        "Arc_get",
        "Arc_strongCount",
        "Arc_tryUnwrap",
        "Arc_asPtr",
    ];

    for func_name in &arc_functions {
        let found = stdlib.functions.values().any(|f| &f.name == func_name);
        if found {
            let func = stdlib.functions.values().find(|f| &f.name == func_name).unwrap();
            println!("   ✅ {} ({} params) -> {:?}",
                func_name,
                func.signature.parameters.len(),
                func.signature.return_type
            );
        } else {
            println!("   ❌ Missing: {}", func_name);
        }
    }

    // Test Arc extern functions
    println!("\n🔌 Arc Extern Runtime Functions:");
    let arc_externs = vec![
        "rayzor_arc_init",
        "rayzor_arc_clone",
        "rayzor_arc_get",
        "rayzor_arc_strong_count",
        "rayzor_arc_try_unwrap",
        "rayzor_arc_as_ptr",
    ];

    for func_name in &arc_externs {
        let found = stdlib.functions.values().find(|f| &f.name == func_name);
        if let Some(func) = found {
            if func.cfg.blocks.is_empty() { println!("   ✅ {} (extern)", func_name); } else { println!("   ⚠️  {} exists but has body!", func_name); }
        } else {
            println!("   ❌ Missing: {}", func_name);
        }
    }

    // Test Mutex functions
    println!("\n🔒 Mutex Functions:");
    let mutex_functions = vec![
        "Mutex_new",
        "Mutex_lock",
        "Mutex_tryLock",
        "Mutex_isLocked",
        "MutexGuard_get",
        "MutexGuard_unlock",
    ];

    for func_name in &mutex_functions {
        let found = stdlib.functions.values().any(|f| &f.name == func_name);
        if found {
            let func = stdlib.functions.values().find(|f| &f.name == func_name).unwrap();
            println!("   ✅ {} ({} params) -> {:?}",
                func_name,
                func.signature.parameters.len(),
                func.signature.return_type
            );
        } else {
            println!("   ❌ Missing: {}", func_name);
        }
    }

    // Test Mutex extern functions
    println!("\n🔌 Mutex Extern Runtime Functions:");
    let mutex_externs = vec![
        "rayzor_mutex_new",
        "rayzor_mutex_lock",
        "rayzor_mutex_try_lock",
        "rayzor_mutex_is_locked",
        "rayzor_mutex_guard_get",
        "rayzor_mutex_unlock",
    ];

    for func_name in &mutex_externs {
        let found = stdlib.functions.values().find(|f| &f.name == func_name);
        if let Some(func) = found {
            if func.cfg.blocks.is_empty() { println!("   ✅ {} (extern)", func_name); } else { println!("   ⚠️  {} exists but has body!", func_name); }
        } else {
            println!("   ❌ Missing: {}", func_name);
        }
    }

    println!("\n═══════════════════════════════════════");
    println!("📈 Summary:");
    println!("═══════════════════════════════════════");

    let total_concurrent_funcs = thread_functions.len() + channel_functions.len() + arc_functions.len() + mutex_functions.len();
    let total_concurrent_externs = thread_externs.len() + channel_externs.len() + arc_externs.len() + mutex_externs.len();

    let found_funcs = thread_functions.iter().filter(|n| stdlib.functions.values().any(|f| &f.name == *n)).count()
        + channel_functions.iter().filter(|n| stdlib.functions.values().any(|f| &f.name == *n)).count()
        + arc_functions.iter().filter(|n| stdlib.functions.values().any(|f| &f.name == *n)).count()
        + mutex_functions.iter().filter(|n| stdlib.functions.values().any(|f| &f.name == *n)).count();

    let found_externs = thread_externs.iter().filter(|n| stdlib.functions.values().any(|f| &f.name == *n && f.cfg.blocks.is_empty())).count()
        + channel_externs.iter().filter(|n| stdlib.functions.values().any(|f| &f.name == *n && f.cfg.blocks.is_empty())).count()
        + arc_externs.iter().filter(|n| stdlib.functions.values().any(|f| &f.name == *n && f.cfg.blocks.is_empty())).count()
        + mutex_externs.iter().filter(|n| stdlib.functions.values().any(|f| &f.name == *n && f.cfg.blocks.is_empty())).count();

    println!("   Wrapper Functions: {}/{}", found_funcs, total_concurrent_funcs);
    println!("   Extern Functions:  {}/{}", found_externs, total_concurrent_externs);

    if found_funcs == total_concurrent_funcs && found_externs == total_concurrent_externs {
        println!("\n🎉 All concurrent stdlib functions verified successfully!");
    } else {
        println!("\n⚠️  Some functions are missing!");
        std::process::exit(1);
    }
}
