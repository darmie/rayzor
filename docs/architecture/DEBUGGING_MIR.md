# Debugging MIR with `rayzor dump`

The `rayzor dump` command compiles a Haxe source file and prints the resulting
MIR (Mid-level IR) in a human-readable, LLVM-like textual format. This is the
primary tool for understanding what the compiler produces and for diagnosing
codegen bugs.

## Basic Usage

```bash
# Dump at default optimization level (O2)
rayzor dump src/Main.hx

# Dump unoptimized (O0 — forced inlining + SRA only)
rayzor dump src/Main.hx -O0

# Dump aggressively optimized
rayzor dump src/Main.hx -O3

# Dump a single function by name
rayzor dump src/Main.hx --function advance

# Dump only the control flow graph structure
rayzor dump src/Main.hx --cfg-only

# Write output to a file
rayzor dump src/Main.hx -O2 -o mir_output.txt
```

## CLI Reference

```
rayzor dump <FILE> [OPTIONS]

Arguments:
  <FILE>                 Haxe source file to compile

Options:
  -O, --opt-level <0-3>  Optimization level (default: 2)
      --function <NAME>  Show only the function matching NAME (substring)
      --cfg-only         Show block structure without instructions
  -o, --output <PATH>    Write to file instead of stdout
```

## Reading MIR Output

### Module Header

```
; Module: main
; Functions: 5
```

### Function Signatures

```
fn @NBody_advance(i64, f64) -> void {
```

Parameters are listed by type. The function name is prefixed with `@`.
Register IDs (`$0`, `$1`, ...) map to parameters in order.

### Basic Blocks

```
  bb0: ; entry
    ; preds:
    $2 = const 0.01
    $3 = load i64 $0, offset 0
    jump bb1
```

- `bb0` — block ID
- `; entry` — optional label
- `; preds: bb3, bb5` — predecessor blocks (empty for entry)
- Instructions follow, one per line
- Last line is always a terminator (`jump`, `br_if`, `ret`, etc.)

### Phi Nodes

```
  bb1: ; loop_header
    ; preds: bb0, bb3
    $10 = phi i32 [bb0: $4], [bb3: $25]
```

Phi nodes appear at the top of a block, before instructions. Each incoming
edge lists the predecessor block and the value arriving from that edge.

### Instructions

| Syntax | Meaning |
| ------ | ------- |
| `$5 = const 42i32` | Integer constant |
| `$6 = const 3.14f64` | Float constant |
| `$7 = copy $5` | Register copy |
| `$8 = add $5, $6` | Binary operation |
| `$9 = cmp lt $8, $10` | Comparison |
| `$11 = load i64 $ptr` | Load from pointer |
| `$11 = load i64 $ptr, offset 16` | Load at byte offset |
| `store $ptr, $value` | Store to pointer |
| `$12 = alloc i32` | Stack allocation |
| `$13 = alloc i64 x 100` | Array allocation |
| `$14 = gep i32 $ptr, [0, 1]` | Get element pointer (field access) |
| `$15 = ptradd $ptr, 8 (type i64)` | Pointer arithmetic |
| `$16 = call fn5($a, $b)` | Direct function call |
| `call fn10($a)` | Void function call |
| `$17 = cast i32 $val to f64` | Type cast |
| `$18 = bitcast $val to i64` | Reinterpret bits |
| `free $ptr` | Free heap allocation |

### Terminators

| Syntax | Meaning |
| ------ | ------- |
| `jump bb5` | Unconditional branch |
| `br_if $cond, bb3, bb4` | Conditional branch (true, false) |
| `switch $val [1 => bb1, 2 => bb2] default bb3` | Multi-way branch |
| `ret $value` | Return value |
| `ret void` | Return nothing |
| `unreachable` | Should never execute |

### Types

| Format | Meaning |
| ------ | ------- |
| `void` | No value |
| `bool` | Boolean |
| `i8`, `i16`, `i32`, `i64` | Signed integers |
| `u8`, `u16`, `u32`, `u64` | Unsigned integers |
| `f32`, `f64` | Floating-point |
| `*i64` | Pointer to i64 |
| `%Body{ f64, f64, f64 }` | Named struct |
| `fn(i64, f64) -> void` | Function type |

## Environment Variables

### `RAYZOR_RAW_MIR=1`

Skip all optimization passes. Dumps the MIR exactly as it comes out of the
HIR-to-MIR lowering phase — no inlining, no SRA, no DCE. Useful for
understanding the raw lowering output.

```bash
RAYZOR_RAW_MIR=1 rayzor dump src/Main.hx
```

### `RAYZOR_PASS_DEBUG=1`

Run optimization passes one at a time, printing which pass ran and whether it
modified the IR. Useful for isolating which pass introduces a bug.

```bash
RAYZOR_PASS_DEBUG=1 rayzor dump src/Main.hx -O2
```

Output includes lines like:

```
[pass] InsertFree: modified=true
[pass] Inlining: modified=true
[pass] DCE: modified=true
[pass] SRA: modified=true
...
```

### `RAYZOR_NO_SRA=1`

Disable scalar replacement of aggregates. If you suspect SRA is miscompiling
a function, disable it and compare the output.

```bash
RAYZOR_NO_SRA=1 rayzor dump src/Main.hx -O2 --function suspect_fn
```

### `RAYZOR_NO_FMA=1`

Disable fused multiply-add formation. FMA changes floating-point rounding, so
this is useful when verifying numerical results against a reference
implementation.

```bash
RAYZOR_NO_FMA=1 rayzor run src/Mandelbrot.hx
```

## Debugging Workflows

### Comparing Optimization Levels

Dump the same function at different optimization levels to see what each pass
contributes:

```bash
rayzor dump src/Main.hx -O0 --function hot_loop -o O0.mir
rayzor dump src/Main.hx -O2 --function hot_loop -o O2.mir
diff O0.mir O2.mir
```

### Finding Which Pass Breaks a Function

1. Dump with raw MIR to confirm the unoptimized code is correct:
   ```bash
   RAYZOR_RAW_MIR=1 rayzor dump src/Main.hx --function broken_fn
   ```

2. Enable pass-by-pass debugging to find which pass introduces the problem:
   ```bash
   RAYZOR_PASS_DEBUG=1 rayzor dump src/Main.hx -O2 --function broken_fn
   ```

3. Disable the suspect pass and re-check:
   ```bash
   RAYZOR_NO_SRA=1 rayzor dump src/Main.hx -O2 --function broken_fn
   ```

### Verifying Inlining Decisions

Compare O0 output (forced inlining of `inline` functions only) with raw MIR
(no inlining at all):

```bash
RAYZOR_RAW_MIR=1 rayzor dump src/Main.hx --function caller -o raw.mir
rayzor dump src/Main.hx -O0 --function caller -o inlined.mir
```

In the raw output, you will see `call fn<id>(...)` instructions. In the
inlined output, the callee body replaces the call with its basic blocks
spliced into the caller's CFG.

### Checking Escape Analysis (InsertFree)

Look for `free` instructions in the dump. If an allocation escapes (passed to
another function, stored to a struct field, returned), no `free` will appear
for it. If `free` appears but the object is still needed at that point, the
escape analysis has a bug.

```bash
rayzor dump src/Main.hx -O0 --function constructor | grep -E 'alloc|free'
```

### Inspecting Loop Optimization

Dump at O2 and look for:
- **LICM effects**: invariant loads and computations appearing in a preheader
  block (a block that jumps to the loop header and has no other predecessors)
- **BCE effects**: `haxe_array_get_ptr` calls replaced by inline `load` +
  `mul` + `ptradd` sequences
- **SRA effects**: `alloc` instructions disappearing, replaced by scalar phi
  nodes in the loop header

```bash
rayzor dump src/Main.hx -O2 --function loop_body
```

### LLVM IR Inspection

When using the LLVM backend (O3 or `--llvm`), dump the LLVM IR to see what
LLVM receives:

```bash
RAYZOR_DUMP_LLVM_IR=1 rayzor run src/Main.hx -O3
```

This prints the LLVM IR module to stderr before and after LLVM's own
optimization passes.
