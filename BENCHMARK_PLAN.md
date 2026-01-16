# Rayzor Benchmark Suite Plan

## Overview

This plan outlines a benchmark suite to compare Rayzor against the highest-performing Haxe targets, based on the official [Haxe Benchmarks](https://benchs.haxe.org/).

## Target Comparison

### Haxe Targets (Competition)

| Target | Description | Expected Performance |
|--------|-------------|---------------------|
| **C++** | Native compilation via hxcpp | Fastest (baseline) |
| **C++ (GC Gen)** | C++ with generational GC | ~Same as C++ |
| **HashLink** | Haxe's optimized VM | ~2-5x slower than C++ |
| **HashLink/C** | HashLink compiled to C | Close to C++ |
| **JVM** | Java Virtual Machine | ~3-10x slower than C++ |
| **NodeJS** | JavaScript via V8 | ~5-20x slower than C++ |

### Rayzor Targets

| Target | Description | Expected Performance |
|--------|-------------|---------------------|
| **Rayzor Interpreter** | MIR interpreter (Phase 0) | ~10-50x slower than C++ |
| **Rayzor Cranelift** | JIT via Cranelift | ~2-5x slower than C++ |
| **Rayzor Bundle** | Pre-compiled .rzb bundle | Same as interpreter (instant startup) |
| **Rayzor LLVM** | (Future) LLVM backend | Match C++ performance |

---

## Benchmarks to Implement

Based on https://benchs.haxe.org/, we'll implement these 10 benchmarks:

### 1. Allocation Benchmark

**Source:** https://benchs.haxe.org/alloc/index.html

**What it tests:** Byte allocation and manipulation performance

**Parameters:**
- 5 test cases with varying byte sizes
- 100 bytes × 500,000 allocations
- 1,000 bytes × 500,000 allocations
- 101 bytes × 500,000 allocations
- 1,001 bytes × 500,000 allocations
- 102 bytes × 500,000 allocations

**Measures:** Memory allocation speed, GC pressure, array operations

**Haxe Code Pattern:**
```haxe
class AllocationBmark {
    static function main() {
        for (size in [100, 1000, 101, 1001, 102]) {
            runAllocationTest(size, 500000);
        }
    }

    static function runAllocationTest(size:Int, count:Int) {
        var arr = new Array<haxe.io.Bytes>();
        for (i in 0...count) {
            var bytes = haxe.io.Bytes.alloc(size);
            bytes.fill(0, size, i % 256);
            arr.push(bytes);
        }
    }
}
```

---

### 2. Mandelbrot Benchmark (Classes)

**Source:** https://benchs.haxe.org/mandelbrot/index.html

**What it tests:** CPU-intensive computation with class-based objects

**Parameters:**
- 875 × 500 pixel scene
- Maximum 1000 iterations per pixel

**Measures:** Floating-point arithmetic, loop performance, class instantiation

**Haxe Code Pattern:**
```haxe
class Complex {
    public var re:Float;
    public var im:Float;

    public function new(re:Float, im:Float) {
        this.re = re;
        this.im = im;
    }

    public function add(c:Complex):Complex {
        return new Complex(re + c.re, im + c.im);
    }

    public function mul(c:Complex):Complex {
        return new Complex(re * c.re - im * c.im, re * c.im + im * c.re);
    }

    public function abs():Float {
        return Math.sqrt(re * re + im * im);
    }
}

class Mandelbrot {
    static inline var WIDTH = 875;
    static inline var HEIGHT = 500;
    static inline var MAX_ITER = 1000;

    static function main() {
        var output = new Array<Int>();
        for (y in 0...HEIGHT) {
            for (x in 0...WIDTH) {
                var c = new Complex(
                    (x - WIDTH / 2) * 4.0 / WIDTH,
                    (y - HEIGHT / 2) * 4.0 / HEIGHT
                );
                output.push(iterate(c));
            }
        }
    }

    static function iterate(c:Complex):Int {
        var z = new Complex(0, 0);
        for (i in 0...MAX_ITER) {
            z = z.mul(z).add(c);
            if (z.abs() > 2.0) return i;
        }
        return MAX_ITER;
    }
}
```

---

### 3. Mandelbrot Benchmark (Anonymous Objects)

**Source:** https://benchs.haxe.org/mandelbrot_anon_objects/index.html

**What it tests:** Same as Mandelbrot but using anonymous objects instead of classes

**Measures:** Anonymous object allocation, structural typing performance

---

### 4. N-Body Benchmark

**Source:** https://benchs.haxe.org/nbody/index.html

**What it tests:** N-body physics simulation (gravitational interactions)

**Parameters:**
- Simulates planetary system
- Multiple iterations of position/velocity updates

**Measures:** Floating-point arithmetic, array iteration, physics calculations

**Haxe Code Pattern:**
```haxe
class Body {
    public var x:Float;
    public var y:Float;
    public var z:Float;
    public var vx:Float;
    public var vy:Float;
    public var vz:Float;
    public var mass:Float;
}

class NBody {
    static var bodies:Array<Body>;

    static function advance(dt:Float) {
        for (i in 0...bodies.length) {
            var bi = bodies[i];
            for (j in (i + 1)...bodies.length) {
                var bj = bodies[j];
                var dx = bi.x - bj.x;
                var dy = bi.y - bj.y;
                var dz = bi.z - bj.z;
                var dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
                var mag = dt / (dist * dist * dist);
                bi.vx -= dx * bj.mass * mag;
                bi.vy -= dy * bj.mass * mag;
                bi.vz -= dz * bj.mass * mag;
                bj.vx += dx * bi.mass * mag;
                bj.vy += dy * bi.mass * mag;
                bj.vz += dz * bi.mass * mag;
            }
        }
        for (b in bodies) {
            b.x += dt * b.vx;
            b.y += dt * b.vy;
            b.z += dt * b.vz;
        }
    }
}
```

---

### 5. SHA256 Benchmark

**Source:** https://benchs.haxe.org/sha256/index.html

**What it tests:** Cryptographic hashing performance

**Parameters:**
- 100 strings of 1000 characters each
- Repeated 100 times

**Measures:** Bitwise operations, array manipulation, cryptographic primitives

---

### 6. SHA512 Benchmark

**Source:** https://benchs.haxe.org/sha512/index.html

**What it tests:** 64-bit cryptographic hashing

**Parameters:** Similar to SHA256 but with 64-bit operations

---

### 7. JSON Benchmark

**Source:** https://benchs.haxe.org/json/index.html

**What it tests:** JSON parsing and stringification

**Measures:** String processing, dynamic object creation, serialization

---

### 8. BCrypt Benchmark

**Source:** https://benchs.haxe.org/bcrypt/index.html

**What it tests:** Password hashing algorithm

**Measures:** CPU-intensive cryptographic operations

---

### 9. Formatter Benchmark

**Source:** https://benchs.haxe.org/formatter/index.html

**What it tests:** Code formatting operations

**Variants:**
- With file I/O
- Without I/O (pure computation)

---

### 10. Dox Benchmark

**Source:** https://benchs.haxe.org/dox/index.html

**What it tests:** Documentation generation

**Measures:** String processing, file I/O, complex data structures

---

## Implementation Plan

### Phase 1: Core Benchmarks (Priority)

These benchmarks are CPU-intensive and best showcase Rayzor's performance:

| Benchmark | Priority | Complexity | Dependencies |
|-----------|----------|------------|--------------|
| Mandelbrot (classes) | High | Low | Float, Classes |
| N-Body | High | Medium | Float, Arrays |
| Allocation | High | Low | Bytes, Arrays |
| SHA256 | Medium | Medium | Bitwise, Bytes |

### Phase 2: Extended Benchmarks

| Benchmark | Priority | Complexity | Dependencies |
|-----------|----------|------------|--------------|
| Mandelbrot (anon) | Medium | Low | Anonymous objects |
| SHA512 | Medium | Medium | 64-bit integers |
| JSON | Medium | High | Dynamic typing, Reflection |
| BCrypt | Low | High | Complex crypto |

### Phase 3: I/O Benchmarks

| Benchmark | Priority | Complexity | Dependencies |
|-----------|----------|------------|--------------|
| Formatter (no I/O) | Medium | High | String processing |
| Formatter (with I/O) | Low | High | File system |
| Dox | Low | Very High | Many stdlib features |

---

## Benchmark Harness Design

### Directory Structure

```
compiler/benchmarks/
├── src/
│   ├── mandelbrot.hx
│   ├── mandelbrot_anon.hx
│   ├── nbody.hx
│   ├── alloc.hx
│   ├── sha256.hx
│   ├── sha512.hx
│   └── json.hx
├── results/
│   └── (generated JSON results)
├── charts/
│   └── (generated HTML charts)
└── runner.rs
```

### Benchmark Runner (Rust)

```rust
// compiler/benchmarks/runner.rs

struct BenchmarkResult {
    name: String,
    target: String,
    compile_time_ms: f64,
    runtime_ms: f64,
    iterations: u32,
}

struct BenchmarkSuite {
    benchmarks: Vec<Benchmark>,
    targets: Vec<Target>,
}

enum Target {
    RayzorInterpreter,
    RayzorCranelift,
    RayzorBundle,
    HaxeCpp,        // For comparison
    HashLink,       // For comparison
}

fn run_benchmark(bench: &Benchmark, target: Target) -> BenchmarkResult {
    // 1. Compile if needed
    // 2. Warm up (3 runs)
    // 3. Measure (10 runs)
    // 4. Return median
}
```

### Output Format

```json
{
    "benchmark": "mandelbrot",
    "date": "2025-01-16",
    "results": [
        {
            "target": "rayzor-cranelift",
            "compile_ms": 14.2,
            "runtime_ms": 156.3,
            "iterations": 10
        },
        {
            "target": "rayzor-interpreter",
            "compile_ms": 0,
            "runtime_ms": 2340.1,
            "iterations": 10
        },
        {
            "target": "haxe-cpp",
            "compile_ms": 5200,
            "runtime_ms": 89.4,
            "iterations": 10
        }
    ]
}
```

### Chart Generation

Generate HTML charts similar to benchs.haxe.org using:
- Chart.js or similar
- Bar charts comparing targets
- Line charts showing trends over time

---

## Metrics to Track

### Runtime Performance

| Metric | Description |
|--------|-------------|
| **Execution Time** | Time to run benchmark (ms) |
| **Throughput** | Operations per second |
| **Relative Speed** | vs C++ baseline |

### Compilation Performance

| Metric | Description |
|--------|-------------|
| **Cold Compile** | First compilation time |
| **Warm Compile** | With BLADE cache |
| **Bundle Load** | .rzb load time |

### Startup Performance

| Metric | Description |
|--------|-------------|
| **Time to First Instruction** | Total startup latency |
| **Compilation Overhead** | % of time spent compiling |

---

## Success Criteria

### Performance Targets

| Target | vs C++ | vs HashLink | Status |
|--------|--------|-------------|--------|
| Rayzor Interpreter | 10-50x slower | 2-10x slower | Goal |
| Rayzor Cranelift | 2-5x slower | 0.5-2x | Goal |
| Rayzor LLVM | 0.8-1.2x | 2-5x faster | Future |

### Startup Targets

| Target | Cold Start | Bundle Start |
|--------|------------|--------------|
| Rayzor | < 20ms | < 1ms |
| C++ | 2-5 seconds | N/A |
| HashLink | < 100ms | N/A |

---

## Implementation Tasks

### Week 1: Infrastructure

- [ ] Create benchmark directory structure
- [ ] Implement benchmark runner in Rust
- [ ] Set up result storage (JSON)
- [ ] Basic chart generation

### Week 2: Core Benchmarks

- [ ] Implement Mandelbrot (classes)
- [ ] Implement N-Body
- [ ] Implement Allocation
- [ ] Run initial comparisons

### Week 3: Extended Benchmarks

- [ ] Implement SHA256
- [ ] Implement Mandelbrot (anon)
- [ ] Implement JSON benchmark
- [ ] Generate comparison charts

### Week 4: Analysis & Documentation

- [ ] Compare against official Haxe benchmarks
- [ ] Document performance findings
- [ ] Identify optimization opportunities
- [ ] Publish results

---

## Chart Examples

### Bar Chart: Runtime Comparison

```
Mandelbrot (875x500, 1000 iter)
═══════════════════════════════════════════════════════

C++            ████████████████████ 89ms (baseline)
HashLink/C     ████████████████████████ 112ms (1.3x)
HashLink       ████████████████████████████████ 178ms (2.0x)
Rayzor JIT     ████████████████████████████████████ 223ms (2.5x)
JVM            ████████████████████████████████████████ 267ms (3.0x)
Rayzor Interp  ██████████████████████████████████████████████████████████████ 890ms (10x)
NodeJS         ████████████████████████████████████████████████████████████████████ 1120ms (12.6x)
```

### Line Chart: Startup Time

```
Cold Start Performance
═══════════════════════════════════════════════════════

                 Compile    Execute    Total
────────────────────────────────────────────────
C++              5200ms     89ms       5289ms
Rayzor JIT       14ms       223ms      237ms
Rayzor Bundle    0ms        890ms      890ms (load: 0.5ms)
HashLink         50ms       178ms      228ms
```

---

## References

- Official Haxe Benchmarks: https://benchs.haxe.org/
- Computer Language Benchmarks Game: https://benchmarksgame-team.pages.debian.net/
- HashLink Performance: https://hashlink.haxe.org/

---

## Notes

1. **Fair Comparison**: Ensure benchmarks use equivalent code across all targets
2. **Warm-up**: Always warm up JIT before measuring
3. **Multiple Runs**: Use median of 10+ runs to reduce variance
4. **Reproducibility**: Document exact hardware and software versions
5. **Tiered Results**: Show both interpreter and JIT results for Rayzor
