#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unreachable_patterns,
    unused_mut,
    unused_assignments,
    unused_parens
)]
#![allow(
    clippy::single_component_path_imports,
    clippy::for_kv_map,
    clippy::explicit_auto_deref
)]
#![allow(
    clippy::println_empty_string,
    clippy::len_zero,
    clippy::useless_vec,
    clippy::field_reassign_with_default
)]
#![allow(
    clippy::needless_borrow,
    clippy::redundant_closure,
    clippy::bool_assert_comparison
)]
#![allow(
    clippy::empty_line_after_doc_comments,
    clippy::useless_format,
    clippy::clone_on_copy,
    clippy::vec_init_then_push
)]
//! @:cstruct end-to-end test suite
//!
//! Tests the complete pipeline for @:cstruct metadata:
//! - Flat C-compatible memory layout (no object header)
//! - Field read/write via byte offsets
//! - cdef() static method returning C typedef string
//! - Interop with TinyCC JIT (Haxe→C and C→Haxe)

use compiler::codegen::CraneliftBackend;
use compiler::compilation::{CompilationConfig, CompilationUnit};

/// Test result
#[derive(Debug)]
enum TestResult {
    Success,
    Failed { error: String },
}

impl TestResult {
    fn is_success(&self) -> bool {
        matches!(self, TestResult::Success)
    }
}

/// A single end-to-end test case
struct E2ETestCase {
    name: String,
    haxe_source: String,
}

impl E2ETestCase {
    fn new(name: &str, haxe_source: &str) -> Self {
        Self {
            name: name.to_string(),
            haxe_source: haxe_source.to_string(),
        }
    }

    fn run(&self) -> TestResult {
        println!("\n{}", "=".repeat(70));
        println!("TEST: {}", self.name);
        println!("{}", "=".repeat(70));

        let mut unit = CompilationUnit::new(CompilationConfig::fast());

        if let Err(e) = unit.load_stdlib() {
            return TestResult::Failed {
                error: format!("Failed to load stdlib: {}", e),
            };
        }

        let filename = format!("{}.hx", self.name);
        if let Err(e) = unit.add_file(&self.haxe_source, &filename) {
            return TestResult::Failed {
                error: format!("Failed to add file: {}", e),
            };
        }

        println!("  Compiling to TAST...");
        let typed_files = match unit.lower_to_tast() {
            Ok(files) => {
                println!("  ✅ TAST ({} files)", files.len());
                files
            }
            Err(errors) => {
                return TestResult::Failed {
                    error: format!("TAST failed: {:?}", errors),
                };
            }
        };

        println!("  Lowering to MIR...");
        let mir_modules = unit.get_mir_modules();
        if mir_modules.is_empty() {
            return TestResult::Failed {
                error: "No MIR modules generated".to_string(),
            };
        }
        println!("  ✅ MIR ({} modules)", mir_modules.len());

        println!("  Compiling to native...");
        let plugin = rayzor_runtime::plugin_impl::get_plugin();
        let symbols = plugin.runtime_symbols();
        let symbols_ref: Vec<(&str, *const u8)> = symbols.iter().map(|(n, p)| (*n, *p)).collect();

        let mut backend = match CraneliftBackend::with_symbols(&symbols_ref) {
            Ok(b) => b,
            Err(e) => {
                return TestResult::Failed {
                    error: format!("Backend init failed: {}", e),
                };
            }
        };

        for module in &mir_modules {
            if let Err(e) = backend.compile_module(module) {
                return TestResult::Failed {
                    error: format!("Codegen failed: {}", e),
                };
            }
        }
        println!("  ✅ Codegen succeeded");

        println!("  Executing...");
        for module in mir_modules.iter().rev() {
            if let Ok(()) = backend.call_main(module) {
                println!("  ✅ Execution succeeded");
                return TestResult::Success;
            }
        }

        TestResult::Failed {
            error: "Failed to execute main".to_string(),
        }
    }
}

fn main() {
    let mut tests = Vec::new();

    // ============================================================================
    // TEST 1: Basic @:cstruct allocation and field access
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstruct_basic",
        r#"
package test;

@:cstruct
class Vec2 {
    public var x:Float;
    public var y:Float;
}

class Main {
    static function main() {
        var v = new Vec2();
        v.x = 3.0;
        v.y = 4.0;
        trace(v.x);  // 3.0
        trace(v.y);  // 4.0
    }
}
"#,
    ));

    // ============================================================================
    // TEST 2: @:cstruct with Int fields
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstruct_int_fields",
        r#"
package test;

@:cstruct
class Point {
    public var x:Int;
    public var y:Int;
    public var z:Int;
}

class Main {
    static function main() {
        var p = new Point();
        p.x = 10;
        p.y = 20;
        p.z = 30;
        trace(p.x);  // 10
        trace(p.y);  // 20
        trace(p.z);  // 30
    }
}
"#,
    ));

    // ============================================================================
    // TEST 3: @:cstruct with mixed field types
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstruct_mixed_types",
        r#"
package test;

@:cstruct
class Entity {
    public var x:Float;
    public var y:Float;
    public var id:Int;
    public var health:Int;
}

class Main {
    static function main() {
        var e = new Entity();
        e.x = 1.5;
        e.y = 2.5;
        e.id = 42;
        e.health = 100;
        trace(e.x);       // 1.5
        trace(e.y);       // 2.5
        trace(e.id);      // 42
        trace(e.health);  // 100
    }
}
"#,
    ));

    // ============================================================================
    // TEST 4: @:cstruct field mutation (read-write-read)
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstruct_mutation",
        r#"
package test;

@:cstruct
class Counter {
    public var value:Int;
}

class Main {
    static function main() {
        var c = new Counter();
        c.value = 0;
        trace(c.value);  // 0
        c.value = 42;
        trace(c.value);  // 42
        c.value = c.value + 1;
        trace(c.value);  // 43
    }
}
"#,
    ));

    // ============================================================================
    // TEST 5: @:cstruct cdef() — auto-generated C typedef string
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstruct_cdef",
        r#"
package test;

@:cstruct
class Particle {
    public var x:Float;
    public var y:Float;
    public var mass:Float;
}

@:cstruct
class Point2D {
    public var px:Int;
    public var py:Int;
}

class Main {
    static function main() {
        trace(Particle.cdef());
        trace(Point2D.cdef());
    }
}
"#,
    ));

    // ============================================================================
    // TEST 6: @:cstruct + CC — C computes from struct fields via pointer
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstruct_cc_interop",
        r#"
package test;

import rayzor.runtime.CC;
import rayzor.Usize;

@:cstruct
class Vec2 {
    public var x:Int;
    public var y:Int;
}

class Main {
    static function main() {
        var v = new Vec2();
        v.x = 10;
        v.y = 20;

        // Compile C code using auto-generated cdef()
        var cc = CC.create();
        cc.compile(Vec2.cdef() + "
            long read_x(Vec2* v) { return v->x; }
            long read_y(Vec2* v) { return v->y; }
            long sum_xy(Vec2* v) { return v->x + v->y; }
            void set_xy(Vec2* v, long x, long y) { v->x = x; v->y = y; }
        ");
        cc.relocate();

        var readX = cc.getSymbol("read_x");
        var readY = cc.getSymbol("read_y");
        var sumXY = cc.getSymbol("sum_xy");
        var setXY = cc.getSymbol("set_xy");

        // @:cstruct object refs are raw pointers — get address via Usize
        var addr = Usize.fromPtr(v);

        // C reads struct fields via typed pointer
        trace(CC.call1(readX, addr));   // 10 (v.x)
        trace(CC.call1(readY, addr));   // 20 (v.y)
        trace(CC.call1(sumXY, addr));   // 30 (v.x + v.y)

        // C writes to struct fields — Haxe reads back (zero-copy)
        CC.call3(setXY, addr, 99, 77);
        trace(v.x);  // 99  (written by C)
        trace(v.y);  // 77  (written by C)

        cc.delete();
    }
}
"#,
    ));

    // ============================================================================
    // TEST 7: @:cstruct with Ptr<T> and Usize fields — C-type mapping
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstruct_ptr_usize",
        r#"
package test;

import rayzor.Ptr;
import rayzor.Usize;

@:cstruct
class Buffer {
    public var data:Ptr<Int>;
    public var len:Usize;
    public var cap:Usize;
}

class Main {
    static function main() {
        // Verify cdef() produces correct C types for Ptr and Usize
        trace(Buffer.cdef());
        // Buffer should be: typedef struct { void* data; size_t len; size_t cap; } Buffer;

        var b = new Buffer();
        b.len = 10;
        b.cap = 20;
        trace(b.len);   // 10
        trace(b.cap);   // 20
    }
}
"#,
    ));

    // ============================================================================
    // TEST 8: @:cstruct with nested @:cstruct field — cdef() includes deps
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstruct_nested",
        r#"
package test;

@:cstruct
class Vec2 {
    public var x:Float;
    public var y:Float;
}

@:cstruct
class Particle {
    public var pos:Vec2;
    public var mass:Float;
}

class Main {
    static function main() {
        // cdef() should include Vec2 typedef before Particle typedef
        trace(Particle.cdef());
        trace(42);
    }
}
"#,
    ));

    // ============================================================================
    // TEST 9: @:cstruct with Ptr<CStruct> field — typed pointer in cdef
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstruct_ptr_to_cstruct",
        r#"
package test;

import rayzor.Ptr;

@:cstruct
class Node {
    public var value:Int;
    public var next:Ptr<Node>;
}

class Main {
    static function main() {
        // cdef() should produce Node* for the next field
        trace(Node.cdef());

        var n = new Node();
        n.value = 42;
        trace(n.value);  // 42
    }
}
"#,
    ));

    // ============================================================================
    // TEST 10: @:cstruct with CString field — maps to char* in cdef
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstruct_cstring_field",
        r#"
package test;

import rayzor.CString;

@:cstruct
class NamedEntity {
    public var id:Int;
    public var name:CString;
}

class Main {
    static function main() {
        trace(NamedEntity.cdef());
    }
}
"#,
    ));

    // ============================================================================
    // TEST 11: CString basic — from, toString, raw
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstring_basic",
        r#"
package test;

import rayzor.CString;

class Main {
    static function main() {
        var cs = CString.from("hello world");
        trace(cs.raw() != 0);   // true — allocated
        trace(cs.toHaxeString());  // "hello world"
        cs.free();
    }
}
"#,
    ));

    // ============================================================================
    // TEST 12: CString + CC interop — pass char* to C code
    // ============================================================================
    tests.push(E2ETestCase::new(
        "cstring_cc_interop",
        r#"
package test;

import rayzor.CString;
import rayzor.runtime.CC;

class Main {
    static function main() {
        var cs = CString.from("rayzor");

        var cc = CC.create();
        cc.compile("
            long str_len(long addr) {
                const char* s = (const char*)addr;
                long n = 0;
                while (s[n]) n++;
                return n;
            }
            long first_char(long addr) {
                const char* s = (const char*)addr;
                return (long)s[0];
            }
        ");
        cc.relocate();

        var strLenFn = cc.getSymbol("str_len");
        var firstCharFn = cc.getSymbol("first_char");

        // Pass CString raw address to C
        trace(CC.call1(strLenFn, cs.raw()));     // 6
        trace(CC.call1(firstCharFn, cs.raw()));  // 114 ('r')

        cs.free();
        cc.delete();
    }
}
"#,
    ));

    // Run all tests
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║             @:cstruct Metadata — E2E Test Suite                    ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");

    let results: Vec<(String, TestResult)> =
        tests.iter().map(|t| (t.name.clone(), t.run())).collect();

    println!("\n\n{}", "=".repeat(70));
    println!("TEST SUMMARY");
    println!("{}", "=".repeat(70));

    let total = results.len();
    let passed = results.iter().filter(|(_, r)| r.is_success()).count();
    let failed = total - passed;

    println!("\n📊 Overall:");
    println!("   Total:  {}", total);
    println!("   Passed: {} ({}%)", passed, passed * 100 / total);
    println!("   Failed: {}", failed);

    println!("\n📋 Results:");
    for (name, result) in &results {
        match result {
            TestResult::Success => {
                println!("   ✅ {} (reached Execution)", name);
            }
            TestResult::Failed { error } => {
                println!("   ❌ {} — {}", name, error);
            }
        }
    }

    if failed == 0 {
        println!("\n🎉 All tests passed!");
    } else {
        println!("\n⚠️  {} test(s) failed", failed);
        std::process::exit(1);
    }
}
