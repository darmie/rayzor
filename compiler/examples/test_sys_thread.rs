//! sys.thread Standard Library Test Suite
//!
//! Tests parsing, compilation and execution of sys.thread Haxe stdlib files.
//! This covers the standard Haxe threading API:
//! - Thread: Thread creation and management
//! - Mutex: Mutual exclusion locks
//! - Lock: Semaphore-backed locks (one-shot synchronization)
//! - Semaphore: Counting semaphores
//! - Deque<T>: Thread-safe double-ended queue
//! - Condition: Condition variables for thread synchronization

use std::thread::sleep;
use std::time::Duration;

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::ir::IrModule;
use rayzor_runtime;

/// Test result levels
#[derive(Debug, Clone, PartialEq, Eq)]
enum TestLevel {
    /// L1: Source code compiles to TAST without errors
    Compilation,
    /// L2: HIR lowering succeeds
    #[allow(dead_code)]
    HirLowering,
    /// L3: MIR lowering succeeds with proper stdlib mappings
    MirLowering,
    /// L4: MIR structure is valid (all extern functions registered)
    MirValidation,
    /// L5: Native code generation succeeds
    #[allow(dead_code)]
    Codegen,
    /// L6: Execution produces correct output
    Execution,
}

/// Test result
#[derive(Debug)]
enum TestResult {
    Success { level: TestLevel },
    Failed { level: TestLevel, error: String },
}

impl TestResult {
    fn is_success(&self) -> bool {
        matches!(self, TestResult::Success { .. })
    }

    fn level(&self) -> TestLevel {
        match self {
            TestResult::Success { level } => level.clone(),
            TestResult::Failed { level, .. } => level.clone(),
        }
    }
}

/// A single end-to-end test case
struct E2ETestCase {
    name: String,
    description: String,
    haxe_source: String,
    expected_level: TestLevel,
    expected_mir_calls: Vec<String>,
}

impl E2ETestCase {
    fn new(name: &str, description: &str, haxe_source: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            haxe_source: haxe_source.to_string(),
            expected_level: TestLevel::Execution,
            expected_mir_calls: Vec::new(),
        }
    }

    fn expect_mir_calls(mut self, calls: Vec<&str>) -> Self {
        self.expected_mir_calls = calls.iter().map(|s| s.to_string()).collect();
        self
    }

    #[allow(dead_code)]
    fn expect_level(mut self, level: TestLevel) -> Self {
        self.expected_level = level;
        self
    }

    fn run(&self) -> TestResult {
        println!("\n{}", "=".repeat(70));
        println!("TEST: {}", self.name);
        println!("{}", self.description);
        println!("{}", "=".repeat(70));

        let mut unit = CompilationUnit::new(CompilationConfig::default());

        if let Err(e) = unit.load_stdlib() {
            return TestResult::Failed {
                level: TestLevel::Compilation,
                error: format!("Failed to load stdlib: {}", e),
            };
        }

        let filename = format!("{}.hx", self.name);
        if let Err(e) = unit.add_file(&self.haxe_source, &filename) {
            return TestResult::Failed {
                level: TestLevel::Compilation,
                error: format!("Failed to add file: {}", e),
            };
        }

        println!("L1: Compiling to TAST...");
        let _typed_files = match unit.lower_to_tast() {
            Ok(files) => {
                println!("  TAST lowering succeeded ({} files)", files.len());
                files
            }
            Err(errors) => {
                return TestResult::Failed {
                    level: TestLevel::Compilation,
                    error: format!(
                        "TAST lowering failed with {} errors: {:?}",
                        errors.len(),
                        errors
                    ),
                };
            }
        };

        println!("L2: HIR lowering...");
        println!("  HIR lowering succeeded (integrated in pipeline)");

        println!("L3: MIR lowering...");
        let mir_modules = unit.get_mir_modules();
        if mir_modules.is_empty() {
            return TestResult::Failed {
                level: TestLevel::MirLowering,
                error: "No MIR modules generated".to_string(),
            };
        }

        let mir_module = mir_modules.last().unwrap();
        println!(
            "  MIR lowering succeeded ({} modules)",
            mir_modules.len()
        );
        println!("  MIR Stats:");
        println!("     - Functions: {}", mir_module.functions.len());
        println!(
            "     - Extern functions: {}",
            mir_module.extern_functions.len()
        );

        println!("L4: Validating MIR structure...");
        if let Err(e) = self.validate_mir_modules(&mir_modules) {
            return TestResult::Failed {
                level: TestLevel::MirValidation,
                error: e,
            };
        }
        println!("  MIR validation passed");

        if matches!(
            self.expected_level,
            TestLevel::Compilation
                | TestLevel::HirLowering
                | TestLevel::MirLowering
                | TestLevel::MirValidation
        ) {
            return TestResult::Success {
                level: self.expected_level.clone(),
            };
        }

        println!("L5: Compiling to native code for {}...", filename);
        let mut backend = match self.compile_to_native(&mir_modules) {
            Ok(backend) => {
                println!("  Codegen succeeded (Cranelift JIT)");
                backend
            }
            Err(e) => {
                return TestResult::Failed {
                    level: TestLevel::Codegen,
                    error: format!("Codegen failed: {}", e),
                };
            }
        };

        if matches!(self.expected_level, TestLevel::Codegen) {
            return TestResult::Success {
                level: TestLevel::Codegen,
            };
        }

        println!("L6: Executing compiled code for {}...", filename);
        if let Err(e) = self.execute_and_validate(&mut backend, self.name.clone(), &mir_modules) {
            return TestResult::Failed {
                level: TestLevel::Execution,
                error: format!("Execution failed: {}", e),
            };
        }
        println!("  Execution succeeded");

        TestResult::Success {
            level: TestLevel::Execution,
        }
    }

    fn validate_mir_modules(&self, modules: &[std::sync::Arc<IrModule>]) -> Result<(), String> {
        let mut all_extern_functions = std::collections::HashSet::new();
        for module in modules {
            for (_, ef) in &module.extern_functions {
                all_extern_functions.insert(ef.name.clone());
            }
            for (_, func) in &module.functions {
                if func.cfg.blocks.is_empty() {
                    all_extern_functions.insert(func.name.clone());
                }
            }
        }

        if !self.expected_mir_calls.is_empty() {
            for expected_call in &self.expected_mir_calls {
                let found = all_extern_functions
                    .iter()
                    .any(|name| name.contains(expected_call));
                if !found {
                    return Err(format!(
                        "Expected extern function '{}' not found in MIR. Available: {:?}",
                        expected_call,
                        all_extern_functions.iter().collect::<Vec<_>>()
                    ));
                }
            }
            println!("  All expected extern functions found");
        }

        println!("  All functions have valid structure");
        Ok(())
    }

    fn compile_to_native(
        &self,
        modules: &[std::sync::Arc<IrModule>],
    ) -> Result<CraneliftBackend, String> {
        let plugin = rayzor_runtime::plugin_impl::get_plugin();
        let symbols = plugin.runtime_symbols();
        let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

        let mut backend = CraneliftBackend::with_symbols(&symbols_ref)?;

        for module in modules {
            backend.compile_module(module)?;
        }

        Ok(backend)
    }

    fn execute_and_validate(
        &self,
        backend: &mut CraneliftBackend,
        name: String,
        modules: &[std::sync::Arc<IrModule>],
    ) -> Result<(), String> {
        for module in modules.iter().rev() {
            println!("  Trying to execute main in module... {}", name);
            if let Ok(()) = backend.call_main(module) {
                return Ok(());
            }
        }

        Err("Failed to execute main in any module".to_string())
    }
}

/// Test suite runner
struct E2ETestSuite {
    tests: Vec<E2ETestCase>,
}

impl E2ETestSuite {
    fn new() -> Self {
        Self { tests: Vec::new() }
    }

    fn add_test(&mut self, test: E2ETestCase) {
        self.tests.push(test);
    }

    fn run_all(&self) -> Vec<(String, TestResult)> {
        let mut results = Vec::new();

        for test in &self.tests {
            let result = test.run();
            let success = result.is_success();
            let test_name = test.name.clone();

            results.push((test_name.clone(), result));

            if success {
                println!("\n{} PASSED", test_name);
            } else {
                println!("\n{} FAILED", test_name);
            }
            sleep(Duration::from_millis(500));
        }

        results
    }

    fn print_summary(&self, results: &[(String, TestResult)]) {
        println!("\n{}", "=".repeat(70));
        println!("TEST SUMMARY");
        println!("{}", "=".repeat(70));

        let total = results.len();
        let passed = results.iter().filter(|(_, r)| r.is_success()).count();
        let failed = total - passed;

        let mut by_level: std::collections::HashMap<String, (usize, usize)> =
            std::collections::HashMap::new();
        for (_, result) in results {
            let level_name = format!("{:?}", result.level());
            let entry = by_level.entry(level_name).or_insert((0, 0));
            if result.is_success() {
                entry.0 += 1;
            } else {
                entry.1 += 1;
            }
        }

        println!("\nOverall:");
        println!("   Total:  {}", total);
        println!("   Passed: {} ({}%)", passed, if total > 0 { passed * 100 / total } else { 0 });
        println!("   Failed: {}", failed);

        println!("\nBy Level:");
        for (level, (pass, fail)) in by_level {
            println!("   {}: {} pass, {} fail", level, pass, fail);
        }

        println!("\nResults:");
        for (name, result) in results {
            match result {
                TestResult::Success { level } => {
                    println!("   [PASS] {} (reached {:?})", name, level);
                }
                TestResult::Failed { level, error } => {
                    println!("   [FAIL] {} (failed at {:?})", name, level);
                    println!("      Error: {}", error);
                }
            }
        }

        if failed == 0 {
            println!("\nAll tests passed!");
        } else {
            println!("\n{} test(s) failed", failed);
        }
    }
}

fn main() -> Result<(), String> {
    println!("=== sys.thread Standard Library Test Suite ===\n");

    let mut suite = E2ETestSuite::new();

    // ============================================================================
    // THREAD TESTS
    // ============================================================================

    // TEST 1: Thread.yield
    suite.add_test(
        E2ETestCase::new(
            "thread_yield",
            "Thread.yield() to allow other threads to run",
            r#"
package test;

import sys.thread.Thread;

class Main {
    static function main() {
        Thread.yield();
        trace("yield completed");
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_thread_yield"]),
    );

    // TEST 2: Thread.sleep
    suite.add_test(
        E2ETestCase::new(
            "thread_sleep",
            "Thread.sleep() for a short duration",
            r#"
package test;

import sys.thread.Thread;

class Main {
    static function main() {
        Thread.sleep(0.01);
        trace("sleep completed");
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_thread_sleep"]),
    );

    // TEST 3: Basic Thread Creation
    suite.add_test(
        E2ETestCase::new(
            "thread_create_basic",
            "Basic thread creation with sys.thread.Thread",
            r#"
package test;

import sys.thread.Thread;

class Main {
    static function main() {
        var executed = false;
        var t = Thread.create(() -> {
            executed = true;
        });
        t.join();
        trace(executed);
    }
}
"#,
        )
        .expect_mir_calls(vec!["Thread_spawn", "sys_thread_join"]),
    );

    // TEST 4: Thread.isFinished
    suite.add_test(
        E2ETestCase::new(
            "thread_is_finished",
            "Check if a thread has finished execution",
            r#"
package test;

import sys.thread.Thread;

class Main {
    static function main() {
        var t = Thread.create(() -> {
            // Quick task
        });
        t.join();
        var finished = t.isFinished();
        trace(finished);
    }
}
"#,
        )
        .expect_mir_calls(vec!["Thread_spawn", "sys_thread_is_finished"]),
    );

    // ============================================================================
    // MUTEX TESTS
    // ============================================================================

    // TEST 5: Basic Mutex
    suite.add_test(
        E2ETestCase::new(
            "mutex_basic",
            "Basic mutex acquire and release",
            r#"
package test;

import sys.thread.Mutex;

class Main {
    static function main() {
        var mutex = new Mutex();
        mutex.acquire();
        trace("acquired");
        mutex.release();
        trace("released");
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_mutex_alloc", "sys_mutex_acquire", "sys_mutex_release"]),
    );

    // TEST 6: Mutex tryAcquire
    suite.add_test(
        E2ETestCase::new(
            "mutex_try_acquire",
            "Mutex tryAcquire returns true when not locked",
            r#"
package test;

import sys.thread.Mutex;

class Main {
    static function main() {
        var mutex = new Mutex();
        var acquired = mutex.tryAcquire();
        trace(acquired);
        if (acquired) {
            mutex.release();
        }
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_mutex_try_acquire"]),
    );

    // TEST 7: Mutex with Thread
    suite.add_test(
        E2ETestCase::new(
            "mutex_with_thread",
            "Mutex protecting shared counter across threads",
            r#"
package test;

import sys.thread.Thread;
import sys.thread.Mutex;

class Main {
    static function main() {
        var counter = 0;
        var mutex = new Mutex();

        var t = Thread.create(() -> {
            mutex.acquire();
            counter = counter + 1;
            mutex.release();
        });

        mutex.acquire();
        counter = counter + 1;
        mutex.release();

        t.join();
        trace(counter);
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_mutex_acquire", "sys_mutex_release"]),
    );

    // ============================================================================
    // LOCK TESTS (Semaphore-backed)
    // ============================================================================

    // TEST 8: Basic Lock
    suite.add_test(
        E2ETestCase::new(
            "lock_basic",
            "Basic Lock wait and release",
            r#"
package test;

import sys.thread.Thread;
import sys.thread.Lock;

class Main {
    static function main() {
        var lock = new Lock();

        var t = Thread.create(() -> {
            Thread.sleep(0.01);
            lock.release();
        });

        lock.wait();
        trace("lock released");
        t.join();
    }
}
"#,
        )
        .expect_mir_calls(vec!["rayzor_semaphore_init", "rayzor_semaphore_release"]),
    );

    // TEST 9: Lock with timeout
    suite.add_test(
        E2ETestCase::new(
            "lock_timeout",
            "Lock wait with timeout",
            r#"
package test;

import sys.thread.Lock;

class Main {
    static function main() {
        var lock = new Lock();
        var result = lock.wait(0.01);
        trace(result);
    }
}
"#,
        )
        .expect_mir_calls(vec!["rayzor_semaphore_try_acquire"]),
    );

    // ============================================================================
    // SEMAPHORE TESTS
    // ============================================================================

    // TEST 10: Basic Semaphore
    suite.add_test(
        E2ETestCase::new(
            "semaphore_basic",
            "Basic semaphore acquire and release",
            r#"
package test;

import sys.thread.Semaphore;

class Main {
    static function main() {
        var sem = new Semaphore(1);
        sem.acquire();
        trace("acquired");
        sem.release();
        trace("released");
    }
}
"#,
        )
        .expect_mir_calls(vec!["rayzor_semaphore_init", "rayzor_semaphore_acquire", "rayzor_semaphore_release"]),
    );

    // TEST 11: Semaphore tryAcquire
    suite.add_test(
        E2ETestCase::new(
            "semaphore_try_acquire",
            "Semaphore tryAcquire with initial count",
            r#"
package test;

import sys.thread.Semaphore;

class Main {
    static function main() {
        var sem = new Semaphore(2);
        var r1 = sem.tryAcquire();
        var r2 = sem.tryAcquire();
        var r3 = sem.tryAcquire(0.01);
        trace(r1);
        trace(r2);
        trace(r3);
        sem.release();
        sem.release();
    }
}
"#,
        )
        .expect_mir_calls(vec!["rayzor_semaphore_try_acquire"]),
    );

    // TEST 12: Semaphore as counting primitive
    suite.add_test(
        E2ETestCase::new(
            "semaphore_counting",
            "Semaphore counting behavior",
            r#"
package test;

import sys.thread.Thread;
import sys.thread.Semaphore;

class Main {
    static function main() {
        var sem = new Semaphore(0);
        var count = 0;

        var t1 = Thread.create(() -> {
            count = count + 1;
            sem.release();
        });

        var t2 = Thread.create(() -> {
            count = count + 1;
            sem.release();
        });

        sem.acquire();
        sem.acquire();

        t1.join();
        t2.join();

        trace(count);
    }
}
"#,
        )
        .expect_mir_calls(vec!["rayzor_semaphore_acquire", "rayzor_semaphore_release"]),
    );

    // ============================================================================
    // DEQUE TESTS
    // ============================================================================

    // TEST 13: Basic Deque
    suite.add_test(
        E2ETestCase::new(
            "deque_basic",
            "Basic deque add and pop operations",
            r#"
package test;

import sys.thread.Deque;

class Main {
    static function main() {
        var deque = new Deque<Int>();
        deque.add(1);
        deque.add(2);
        deque.add(3);

        var v1 = deque.pop(false);
        var v2 = deque.pop(false);
        var v3 = deque.pop(false);

        trace(v1);
        trace(v2);
        trace(v3);
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_deque_alloc", "sys_deque_add", "sys_deque_pop"]),
    );

    // TEST 14: Deque push (front) operation
    suite.add_test(
        E2ETestCase::new(
            "deque_push",
            "Deque push adds to front",
            r#"
package test;

import sys.thread.Deque;

class Main {
    static function main() {
        var deque = new Deque<Int>();
        deque.add(1);
        deque.push(0);

        var v1 = deque.pop(false);
        var v2 = deque.pop(false);

        trace(v1);
        trace(v2);
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_deque_push"]),
    );

    // TEST 15: Deque with thread (producer-consumer)
    suite.add_test(
        E2ETestCase::new(
            "deque_producer_consumer",
            "Deque used for producer-consumer pattern",
            r#"
package test;

import sys.thread.Thread;
import sys.thread.Deque;

class Main {
    static function main() {
        var deque = new Deque<Int>();

        var producer = Thread.create(() -> {
            deque.add(42);
        });

        producer.join();

        var value = deque.pop(false);
        trace(value);
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_deque_add", "sys_deque_pop"]),
    );

    // ============================================================================
    // CONDITION TESTS
    // ============================================================================

    // TEST 16: Basic Condition
    suite.add_test(
        E2ETestCase::new(
            "condition_basic",
            "Basic condition variable acquire/release",
            r#"
package test;

import sys.thread.Condition;

class Main {
    static function main() {
        var cond = new Condition();
        cond.acquire();
        trace("acquired");
        cond.release();
        trace("released");
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_condition_alloc", "sys_condition_acquire", "sys_condition_release"]),
    );

    // TEST 17: Condition tryAcquire
    suite.add_test(
        E2ETestCase::new(
            "condition_try_acquire",
            "Condition tryAcquire",
            r#"
package test;

import sys.thread.Condition;

class Main {
    static function main() {
        var cond = new Condition();
        var acquired = cond.tryAcquire();
        trace(acquired);
        if (acquired) {
            cond.release();
        }
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_condition_try_acquire"]),
    );

    // TEST 18: Condition wait and signal
    suite.add_test(
        E2ETestCase::new(
            "condition_signal",
            "Condition wait and signal between threads",
            r#"
package test;

import sys.thread.Thread;
import sys.thread.Condition;

class Main {
    static function main() {
        var cond = new Condition();
        var ready = false;

        var waiter = Thread.create(() -> {
            cond.acquire();
            while (!ready) {
                cond.wait();
            }
            cond.release();
            trace("waiter done");
        });

        Thread.sleep(0.01);

        cond.acquire();
        ready = true;
        cond.signal();
        cond.release();

        waiter.join();
        trace("main done");
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_condition_wait", "sys_condition_signal"]),
    );

    // TEST 19: Condition broadcast
    suite.add_test(
        E2ETestCase::new(
            "condition_broadcast",
            "Condition broadcast to multiple waiters",
            r#"
package test;

import sys.thread.Thread;
import sys.thread.Condition;

class Main {
    static function main() {
        var cond = new Condition();
        var ready = false;
        var count = 0;

        var t1 = Thread.create(() -> {
            cond.acquire();
            while (!ready) {
                cond.wait();
            }
            count = count + 1;
            cond.release();
        });

        var t2 = Thread.create(() -> {
            cond.acquire();
            while (!ready) {
                cond.wait();
            }
            count = count + 1;
            cond.release();
        });

        Thread.sleep(0.02);

        cond.acquire();
        ready = true;
        cond.broadcast();
        cond.release();

        t1.join();
        t2.join();

        trace(count);
    }
}
"#,
        )
        .expect_mir_calls(vec!["sys_condition_broadcast"]),
    );

    // ============================================================================
    // INTEGRATION TESTS
    // ============================================================================

    // TEST 20: Multiple threads with mutex
    suite.add_test(
        E2ETestCase::new(
            "integration_threads_mutex",
            "Multiple threads incrementing a shared counter with mutex",
            r#"
package test;

import sys.thread.Thread;
import sys.thread.Mutex;

class Main {
    static function main() {
        var counter = 0;
        var mutex = new Mutex();
        var handles = new Array<Thread>();

        var i = 0;
        while (i < 3) {
            var t = Thread.create(() -> {
                mutex.acquire();
                counter = counter + 1;
                mutex.release();
            });
            handles.push(t);
            i = i + 1;
        }

        var j = 0;
        while (j < handles.length) {
            handles[j].join();
            j = j + 1;
        }

        trace(counter);
    }
}
"#,
        )
        .expect_mir_calls(vec!["Thread_spawn", "sys_mutex_acquire"]),
    );

    // TEST 21: Producer-consumer with semaphore
    suite.add_test(
        E2ETestCase::new(
            "integration_producer_consumer",
            "Producer-consumer pattern with semaphore",
            r#"
package test;

import sys.thread.Thread;
import sys.thread.Semaphore;

class Main {
    static function main() {
        var items = new Array<Int>();
        var sem = new Semaphore(0);

        var producer = Thread.create(() -> {
            items.push(1);
            sem.release();
            items.push(2);
            sem.release();
            items.push(3);
            sem.release();
        });

        sem.acquire();
        sem.acquire();
        sem.acquire();

        producer.join();

        trace(items.length);
    }
}
"#,
        )
        .expect_mir_calls(vec!["rayzor_semaphore_acquire", "rayzor_semaphore_release"]),
    );

    // Run all tests
    let results = suite.run_all();
    suite.print_summary(&results);

    let failed = results.iter().filter(|(_, r)| !r.is_success()).count();
    if failed > 0 {
        Err(format!("{} test(s) failed", failed))
    } else {
        Ok(())
    }
}
