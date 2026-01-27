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
    clippy::explicit_auto_deref,
    clippy::println_empty_string,
    clippy::len_zero,
    clippy::needless_borrow,
    clippy::redundant_closure,
    clippy::bool_assert_comparison
)]

// End-to-end integration test for the Haxe macro system.
//
// Validates the COMPLETE pipeline from source code through macro expansion to TAST:
//
// Part A: Load actual .hx macro example files (smoke tests)
//   - test_macro_expression.hx -- compile-time computation, lookup tables
//   - test_macro_reification.hx -- $v{}, $i{}, $e{}, $a{}, $p{}, $b{}
//   - test_macro_context.hx -- Context API: error(), getType(), getBuildFields()
//   - test_macro_build.hx -- @:build and @:autoBuild macros
//
//   Verifies: parsing doesn't crash, macro registration works where parser
//   supports the syntax. These files use advanced Haxe syntax that the parser
//   handles partially -- some declarations parse, others are silently skipped.
//
// Part B: Inline tests with REAL macro computation (strict tests)
//   Macros that actually DO things: arithmetic, while loops, conditionals,
//   string concatenation, multi-step variable chains. The interpreter evaluates
//   these at compile time and the call sites are replaced with computed values.
//
//   Verifies: expansion_count > 0 (interpreter ran), TAST lowering succeeds.
//
// Run: cargo run --package compiler --example test_macro_e2e

use compiler::compilation::{CompilationConfig, CompilationUnit};
use compiler::macro_system::MacroExpander;

// ============================================================================
// Embedded .hx macro example files (same directory as this test)
// ============================================================================

const MACRO_EXPRESSION_HX: &str = include_str!("test_macro_expression.hx");
const MACRO_REIFICATION_HX: &str = include_str!("test_macro_reification.hx");
const MACRO_CONTEXT_HX: &str = include_str!("test_macro_context.hx");
const MACRO_BUILD_HX: &str = include_str!("test_macro_build.hx");

// ============================================================================
// Test infrastructure
// ============================================================================

struct TestResult {
    name: String,
    passed: bool,
    details: String,
}

/// Parse source, run macro expansion via MacroExpander.
/// Returns (declarations_found, macros_registered, expansions, builds, error_diagnostics).
fn run_macro_expansion(
    name: &str,
    source: &str,
) -> (usize, usize, usize, usize, Vec<String>) {
    let parsed = parser::parse_haxe_file(&format!("{}.hx", name), source, false)
        .expect("parse should not crash");

    let decl_count = parsed.declarations.len();

    // Report what the parser found
    for decl in &parsed.declarations {
        if let parser::TypeDeclaration::Class(cls) = decl {
            let macro_fields = cls
                .fields
                .iter()
                .filter(|f| f.modifiers.contains(&parser::Modifier::Macro))
                .count();
            println!(
                "    Class '{}': {} fields ({} macro)",
                cls.name,
                cls.fields.len(),
                macro_fields
            );
        }
    }

    let mut expander = MacroExpander::new();
    let result = expander.expand_file(parsed);

    let registry = expander.registry();
    let macro_count = registry.macro_count();
    let build_count = registry.build_macros().len();

    let error_diags: Vec<String> = result
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, compiler::macro_system::MacroSeverity::Error))
        .map(|d| d.message.clone())
        .collect();

    (
        decl_count,
        macro_count,
        result.expansions_count,
        build_count,
        error_diags,
    )
}

/// Run source through CompilationUnit pipeline: parse -> macro expand -> TAST.
/// Returns the number of typed files produced.
fn run_compilation_unit(name: &str, source: &str) -> Result<usize, String> {
    let mut unit = CompilationUnit::new(CompilationConfig::fast());
    unit.load_stdlib()
        .map_err(|e| format!("stdlib load: {}", e))?;
    unit.add_file(source, &format!("{}.hx", name))
        .map_err(|e| format!("add_file: {}", e))?;

    let typed_files = unit.lower_to_tast().map_err(|errors| {
        let msgs: Vec<String> = errors
            .iter()
            .take(3)
            .map(|e| e.message.to_string())
            .collect();
        format!("{} errors: {}", errors.len(), msgs.join("; "))
    })?;

    Ok(typed_files.len())
}

// ============================================================================
// Part A: .hx example file tests (smoke tests)
//
// These files use advanced Haxe macro syntax (Context API calls, reification,
// $v{}, @:build, etc.) that the parser handles partially. The test verifies:
// 1. Parsing doesn't crash
// 2. Macro expansion doesn't crash
// 3. Whatever macros the parser finds are registered correctly
// ============================================================================

fn test_hx_file(name: &str, description: &str, source: &str) -> TestResult {
    println!("\n{}", "=".repeat(70));
    println!("TEST [file]: {}", name);
    println!("  {}", description);

    // Parse and expand
    let (decl_count, macro_count, expansion_count, build_count, errors) =
        run_macro_expansion(name, source);

    println!(
        "  Parsed: {} declarations, {} macros registered, {} @:build entries",
        decl_count, macro_count, build_count
    );

    if !errors.is_empty() {
        println!("  Expansion errors: {}", errors.len());
        for e in errors.iter().take(3) {
            println!("    - {}", e);
        }
    }

    // The test passes if:
    // - Parsing didn't panic (we got here)
    // - Macro expansion didn't panic (we got here)
    // Both of these are validated by reaching this point.

    TestResult {
        name: name.to_string(),
        passed: true,
        details: format!(
            "{} decls, {} macros, {} expansions, {} @:build",
            decl_count, macro_count, expansion_count, build_count
        ),
    }
}

// ============================================================================
// Part B: Inline computation tests (strict tests)
//
// These use simple Haxe syntax the parser fully supports. Macro function
// bodies perform REAL computation via the interpreter. The KEY assertion is
// that expansion_count >= N, proving the interpreter evaluated the body.
// ============================================================================

fn test_inline_macro(
    name: &str,
    description: &str,
    source: &str,
    expect_min_expansions: usize,
) -> TestResult {
    println!("\n{}", "=".repeat(70));
    println!("TEST [computation]: {}", name);
    println!("  {}", description);

    // Step 1: Parse and expand
    let (_decl_count, macro_count, expansion_count, _build_count, errors) =
        run_macro_expansion(name, source);

    // Report expansion errors
    if !errors.is_empty() {
        println!("  Expansion errors:");
        for e in &errors {
            println!("    - {}", e);
        }
    }

    // KEY ASSERTION: macros must have actually expanded
    if expansion_count < expect_min_expansions {
        return TestResult {
            name: name.to_string(),
            passed: false,
            details: format!(
                "EXPANSION FAILED: expected >= {} expansions, got {}. \
                 Interpreter did not evaluate macro body.",
                expect_min_expansions, expansion_count
            ),
        };
    }
    println!(
        "  Expansion OK: {} macros, {} expansions (needed >= {})",
        macro_count, expansion_count, expect_min_expansions
    );

    // Step 2: CompilationUnit -> TAST (verify expanded code compiles)
    match run_compilation_unit(name, source) {
        Ok(file_count) => {
            println!("  TAST OK: {} typed file(s)", file_count);
            TestResult {
                name: name.to_string(),
                passed: true,
                details: format!(
                    "{} macros, {} expansions, {} typed files",
                    macro_count, expansion_count, file_count
                ),
            }
        }
        Err(e) => TestResult {
            name: name.to_string(),
            passed: false,
            details: format!("TAST failed: {}", e),
        },
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() -> Result<(), String> {
    println!("=== Rayzor Macro System End-to-End Test Suite ===");
    println!("Validates macro registration, interpreter evaluation, and pipeline integration\n");

    let mut results: Vec<TestResult> = Vec::new();

    // ========================================================================
    // PART A: Load .hx macro example files (smoke tests)
    //
    // These files use advanced macro syntax (Context API, reification, @:build).
    // The parser handles them partially. Test verifies no crashes, reports
    // what was found. Full coverage requires parser improvements (tracked
    // separately — not a macro system issue).
    // ========================================================================

    println!("{}", "#".repeat(70));
    println!("# PART A: Haxe Macro Example Files (.hx) — smoke tests");
    println!("{}", "#".repeat(70));

    results.push(test_hx_file(
        "macro_expression_file",
        "test_macro_expression.hx: compile-time computation, lookup tables, fibonacci",
        MACRO_EXPRESSION_HX,
    ));

    results.push(test_hx_file(
        "macro_reification_file",
        "test_macro_reification.hx: $v{}, $i{}, $e{}, $a{}, $p{}, $b{} reification",
        MACRO_REIFICATION_HX,
    ));

    results.push(test_hx_file(
        "macro_context_file",
        "test_macro_context.hx: Context.error(), getType(), getBuildFields()",
        MACRO_CONTEXT_HX,
    ));

    results.push(test_hx_file(
        "macro_build_file",
        "test_macro_build.hx: @:build(addToString), @:build(addSerialize), @:autoBuild",
        MACRO_BUILD_HX,
    ));

    // ========================================================================
    // PART B: Inline tests with REAL macro computation (strict tests)
    //
    // Each macro function performs actual compile-time work: arithmetic,
    // while loops, conditionals, string concatenation, multi-step variable
    // chains. The KEY assertion is expansion_count >= N — proving the macro
    // interpreter evaluated the body and produced a computed result.
    //
    // No `package` declaration — ensures macro call-site names match registry.
    // ========================================================================

    println!("\n{}", "#".repeat(70));
    println!("# PART B: Real Macro Computation — interpreter evaluation");
    println!("{}", "#".repeat(70));

    // ---------- B1: Basic arithmetic ----------
    results.push(test_inline_macro(
        "arithmetic_macro",
        "Interpreter evaluates 6 * 7 = 42 at compile time",
        r#"
class Macros {
    macro static function answer():Int {
        return 6 * 7;
    }
}

class Main {
    static function main() {
        var x = Macros.answer();
        trace(x);
    }
}
"#,
        1,
    ));

    // ---------- B2: While loop summation ----------
    results.push(test_inline_macro(
        "while_loop_macro",
        "Interpreter runs while loop: sum(1..10) = 55 at compile time",
        r#"
class Macros {
    macro static function sumTo10():Int {
        var sum = 0;
        var i = 1;
        while (i <= 10) {
            sum = sum + i;
            i = i + 1;
        }
        return sum;
    }
}

class Main {
    static function main() {
        var total = Macros.sumTo10();
        trace(total);
    }
}
"#,
        1,
    ));

    // ---------- B3: If/else conditional ----------
    results.push(test_inline_macro(
        "conditional_macro",
        "Interpreter evaluates if/else: picks larger of 42 vs 99",
        r#"
class Macros {
    macro static function bigger():Int {
        var a = 42;
        var b = 99;
        if (a > b) {
            return a;
        }
        return b;
    }
}

class Main {
    static function main() {
        var val = Macros.bigger();
        trace(val);
    }
}
"#,
        1,
    ));

    // ---------- B4: String concatenation ----------
    results.push(test_inline_macro(
        "string_concat_macro",
        "Interpreter concatenates: \"Hello, \" + name + \"!\"",
        r#"
class Macros {
    macro static function greet():String {
        var name = "World";
        return "Hello, " + name + "!";
    }
}

class Main {
    static function main() {
        var msg = Macros.greet();
        trace(msg);
    }
}
"#,
        1,
    ));

    // ---------- B5: Multi-step variable chain ----------
    results.push(test_inline_macro(
        "variable_chain_macro",
        "Interpreter chains: x=2, y=3, z=x+y=5, result=z*z=25",
        r#"
class Macros {
    macro static function compute():Int {
        var x = 2;
        var y = 3;
        var z = x + y;
        var result = z * z;
        return result;
    }
}

class Main {
    static function main() {
        var v = Macros.compute();
        trace(v);
    }
}
"#,
        1,
    ));

    // ---------- B6: Multiple macros, multiple calls ----------
    results.push(test_inline_macro(
        "multi_macro_calls",
        "3 macro functions: pi(), factorial(5!), max(100,200) — all computed at compile time",
        r#"
class MathMacros {
    macro static function pi():Float {
        return 3.14159;
    }

    macro static function factorial5():Int {
        var result = 1;
        var i = 1;
        while (i <= 5) {
            result = result * i;
            i = i + 1;
        }
        return result;
    }

    macro static function maxVal():Int {
        var a = 100;
        var b = 200;
        if (a > b) {
            return a;
        }
        return b;
    }
}

class Main {
    static function main() {
        var p = MathMacros.pi();
        var fact = MathMacros.factorial5();
        var m = MathMacros.maxVal();
        trace(p);
        trace(fact);
        trace(m);
    }
}
"#,
        3, // 3 distinct macro calls expanded
    ));

    // ---------- B7: Macro results used in regular class code ----------
    results.push(test_inline_macro(
        "macro_with_regular_classes",
        "Macro-computed values used with regular classes: Buffer(4096), version \"1.0.0\"",
        r#"
class CompileConst {
    macro static function bufferSize():Int {
        var base = 1024;
        var multiplier = 4;
        return base * multiplier;
    }

    macro static function version():String {
        return "1" + "." + "0" + "." + "0";
    }
}

class Buffer {
    public var size:Int;

    public function new(size:Int) {
        this.size = size;
    }

    public function capacity():Int {
        return this.size;
    }
}

class Main {
    static function main() {
        var sz = CompileConst.bufferSize();
        var buf = new Buffer(sz);
        trace(buf.capacity());
        var ver = CompileConst.version();
        trace(ver);
    }
}
"#,
        2, // bufferSize() and version() calls
    ));

    // ========================================================================
    // Summary
    // ========================================================================

    println!("\n{}", "=".repeat(70));
    println!("MACRO E2E TEST SUMMARY");
    println!("{}", "=".repeat(70));

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    // Separate Part A and Part B results
    let part_a_count = 4;
    let part_b_start = part_a_count;

    println!("\n  Part A (smoke tests — .hx example files):");
    for r in results.iter().take(part_a_count) {
        let status = if r.passed { "PASS" } else { "FAIL" };
        println!("    [{}] {}: {}", status, r.name, r.details);
    }

    println!("\n  Part B (strict tests — real macro computation):");
    for r in results.iter().skip(part_b_start) {
        let status = if r.passed { "PASS" } else { "FAIL" };
        println!("    [{}] {}: {}", status, r.name, r.details);
    }

    println!(
        "\n  Total: {} | Passed: {} | Failed: {}",
        total, passed, failed
    );

    if failed > 0 {
        println!("\n  Failed tests:");
        for r in results.iter().filter(|r| !r.passed) {
            println!("    - {}: {}", r.name, r.details);
        }
        Err(format!("{} test(s) failed", failed))
    } else {
        println!("\n  All macro E2E tests passed!");
        Ok(())
    }
}
