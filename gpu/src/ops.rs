//! GPU compute operations — elementwise binary and unary ops.
//!
//! Elementwise ops are **lazy** — they build a computation DAG instead of
//! dispatching immediately. When materialization is triggered (by `toTensor`,
//! a reduction, or matmul), the entire chain is fused into a single kernel.
//!
//! Non-fuseable ops (reductions, matmul) materialize their inputs first.

use std::rc::Rc;

use crate::buffer::{self, GpuBuffer};
use crate::device::GpuContext;
use crate::kernel_ir::KernelOp;
use crate::lazy::{LazyNode, LazyOp};

#[cfg(target_os = "macos")]
use crate::buffer::GpuBufferKind;
#[cfg(target_os = "macos")]
use crate::codegen::msl_reduction::REDUCE_THREADGROUP_SIZE;
#[cfg(target_os = "macos")]
use crate::metal::{buffer_ops::MetalBuffer, dispatch};
#[cfg(target_os = "macos")]
use objc2_metal::MTLSize;

// ---------------------------------------------------------------------------
// Internal helpers — lazy elementwise (macOS only)
// ---------------------------------------------------------------------------

/// Convert a GpuBuffer reference to a LazyOp node.
///
/// If the buffer is already lazy, returns a clone of its Rc'd op tree.
/// If materialized, wraps the Metal buffer in an Input leaf (bumping the ObjC refcount).
#[cfg(target_os = "macos")]
fn buf_to_lazy_op(buf: &GpuBuffer) -> Rc<LazyOp> {
    match &buf.kind {
        GpuBufferKind::Lazy(node) => node.op.clone(),
        GpuBufferKind::Materialized(metal_buf) => {
            Rc::new(LazyOp::Input(metal_buf.mtl_buffer.clone()))
        }
    }
}

/// Create a lazy binary elementwise GpuBuffer.
#[cfg(target_os = "macos")]
unsafe fn binary_lazy(a: i64, b: i64, op: KernelOp) -> i64 {
    if a == 0 || b == 0 {
        return 0;
    }

    let a_buf = &*(a as *const GpuBuffer);
    let b_buf = &*(b as *const GpuBuffer);

    if a_buf.dtype != b_buf.dtype || a_buf.numel != b_buf.numel {
        return 0;
    }

    let lhs = buf_to_lazy_op(a_buf);
    let rhs = buf_to_lazy_op(b_buf);

    let node = LazyNode {
        op: Rc::new(LazyOp::Binary { op, lhs, rhs }),
        dtype: a_buf.dtype,
        numel: a_buf.numel,
    };

    let result = GpuBuffer::lazy(node, a_buf.numel, a_buf.dtype);
    Box::into_raw(Box::new(result)) as i64
}

/// Create a lazy unary elementwise GpuBuffer.
#[cfg(target_os = "macos")]
unsafe fn unary_lazy(a: i64, op: KernelOp) -> i64 {
    if a == 0 {
        return 0;
    }

    let a_buf = &*(a as *const GpuBuffer);
    let input = buf_to_lazy_op(a_buf);

    let node = LazyNode {
        op: Rc::new(LazyOp::Unary { op, input }),
        dtype: a_buf.dtype,
        numel: a_buf.numel,
    };

    let result = GpuBuffer::lazy(node, a_buf.numel, a_buf.dtype);
    Box::into_raw(Box::new(result)) as i64
}

// ---------------------------------------------------------------------------
// Extern C API — Binary ops: (ctx, a, b) -> result (lazy)
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_add(_ctx: i64, a: i64, b: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        binary_lazy(a, b, KernelOp::Add)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (_ctx, a, b);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_sub(_ctx: i64, a: i64, b: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        binary_lazy(a, b, KernelOp::Sub)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (_ctx, a, b);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_mul(_ctx: i64, a: i64, b: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        binary_lazy(a, b, KernelOp::Mul)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (_ctx, a, b);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_div(_ctx: i64, a: i64, b: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        binary_lazy(a, b, KernelOp::Div)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (_ctx, a, b);
        0
    }
}

// ---------------------------------------------------------------------------
// Extern C API — Unary ops: (ctx, a) -> result (lazy)
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_neg(_ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_lazy(a, KernelOp::Neg)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (_ctx, a);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_abs(_ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_lazy(a, KernelOp::Abs)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (_ctx, a);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_sqrt(_ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_lazy(a, KernelOp::Sqrt)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (_ctx, a);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_exp(_ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_lazy(a, KernelOp::Exp)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (_ctx, a);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_log(_ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_lazy(a, KernelOp::Log)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (_ctx, a);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_relu(_ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_lazy(a, KernelOp::Relu)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (_ctx, a);
        0
    }
}

// ---------------------------------------------------------------------------
// Internal helpers — Reductions (macOS only)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn next_power_of_2(n: usize) -> usize {
    let mut v = n.max(1);
    v -= 1;
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v |= v >> 32;
    v + 1
}

/// Perform a GPU reduction and return the scalar result as f64.
///
/// Materializes the input buffer first if it's lazy.
///
/// Two-pass strategy:
/// - Pass 1: N threadgroups of 256 threads, each produces one partial result
/// - Pass 2 (if N > 1): 1 threadgroup reduces the N partial results
#[cfg(target_os = "macos")]
unsafe fn reduce_impl(ctx: i64, buf: i64, op: KernelOp) -> f64 {
    if ctx == 0 || buf == 0 {
        return 0.0;
    }

    let gpu_ctx = &mut *(ctx as *mut GpuContext);
    let a_buf = &mut *(buf as *mut GpuBuffer);

    // Materialize lazy inputs before reduction
    if a_buf.ensure_materialized(gpu_ctx).is_err() {
        return 0.0;
    }

    let metal_buf = a_buf.metal_buffer();
    let dtype = a_buf.dtype;
    let numel = a_buf.numel;
    let elem_size = buffer::dtype_byte_size(dtype);

    if numel == 0 {
        return 0.0;
    }

    // Compile reduction kernel
    let cached = match gpu_ctx
        .kernel_cache
        .get_or_compile(&gpu_ctx.inner, op, dtype)
    {
        Ok(k) => k,
        Err(_) => return 0.0,
    };

    // Determine threadgroup/grid sizing
    let tg_size = REDUCE_THREADGROUP_SIZE.min(next_power_of_2(numel));
    let num_tgs = if numel <= tg_size {
        1
    } else {
        numel.div_ceil(tg_size).min(256)
    };

    // Create numel constant buffer
    let numel_u32 = numel as u32;
    let numel_buf = match MetalBuffer::from_value(&gpu_ctx.inner, &numel_u32) {
        Some(b) => b,
        None => return 0.0,
    };

    // Allocate partial results buffer
    let partial_buf = match MetalBuffer::allocate(&gpu_ctx.inner, num_tgs * elem_size) {
        Some(b) => b,
        None => return 0.0,
    };

    // Pass 1: reduce input → partial results
    let tg_count = MTLSize {
        width: num_tgs,
        height: 1,
        depth: 1,
    };
    let tg_threads = MTLSize {
        width: tg_size,
        height: 1,
        depth: 1,
    };

    if dispatch::dispatch_threadgroups(
        &gpu_ctx.inner,
        &cached.compiled,
        &[metal_buf, &partial_buf, &numel_buf],
        tg_count,
        tg_threads,
    )
    .is_err()
    {
        return 0.0;
    }

    // Pass 2 (if needed): reduce partial results → single value
    let result_buf = if num_tgs > 1 {
        let final_buf = match MetalBuffer::allocate(&gpu_ctx.inner, elem_size) {
            Some(b) => b,
            None => return 0.0,
        };

        let pass2_numel = num_tgs as u32;
        let pass2_numel_buf = match MetalBuffer::from_value(&gpu_ctx.inner, &pass2_numel) {
            Some(b) => b,
            None => return 0.0,
        };

        let pass2_tg_size = next_power_of_2(num_tgs);
        let pass2_tg_count = MTLSize {
            width: 1,
            height: 1,
            depth: 1,
        };
        let pass2_tg_threads = MTLSize {
            width: pass2_tg_size,
            height: 1,
            depth: 1,
        };

        if dispatch::dispatch_threadgroups(
            &gpu_ctx.inner,
            &cached.compiled,
            &[&partial_buf, &final_buf, &pass2_numel_buf],
            pass2_tg_count,
            pass2_tg_threads,
        )
        .is_err()
        {
            return 0.0;
        }

        final_buf
    } else {
        partial_buf
    };

    // Read back single scalar result
    let ptr = result_buf.contents();
    match dtype {
        buffer::DTYPE_F32 => *(ptr as *const f32) as f64,
        buffer::DTYPE_F64 => *(ptr as *const f64),
        buffer::DTYPE_I32 => *(ptr as *const i32) as f64,
        buffer::DTYPE_I64 => *(ptr as *const i64) as f64,
        _ => 0.0,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers — Matmul (macOS only)
// ---------------------------------------------------------------------------

/// Perform GPU matrix multiplication: C(M×N) = A(M×K) × B(K×N).
/// Returns a new GpuBuffer handle, or 0 on failure.
///
/// Materializes input buffers if lazy.
#[cfg(target_os = "macos")]
unsafe fn matmul_impl(ctx: i64, a: i64, b: i64, m: usize, k: usize, n: usize) -> i64 {
    if ctx == 0 || a == 0 || b == 0 || m == 0 || k == 0 || n == 0 {
        return 0;
    }

    let gpu_ctx = &mut *(ctx as *mut GpuContext);

    // Materialize lazy inputs
    let a_buf = &mut *(a as *mut GpuBuffer);
    let b_buf = &mut *(b as *mut GpuBuffer);
    if a_buf.ensure_materialized(gpu_ctx).is_err() {
        return 0;
    }
    if b_buf.ensure_materialized(gpu_ctx).is_err() {
        return 0;
    }

    let dtype = a_buf.dtype;

    // Compile matmul kernel
    let cached = match gpu_ctx
        .kernel_cache
        .get_or_compile(&gpu_ctx.inner, KernelOp::Matmul, dtype)
    {
        Ok(k) => k,
        Err(_) => return 0,
    };

    let elem_size = buffer::dtype_byte_size(dtype);

    // Allocate result buffer C (M × N)
    let result_inner = match MetalBuffer::allocate(&gpu_ctx.inner, m * n * elem_size) {
        Some(b) => b,
        None => return 0,
    };

    // Create dims buffer: uint4(M, K, N, 0)
    let dims: [u32; 4] = [m as u32, k as u32, n as u32, 0];
    let dims_buf = match MetalBuffer::from_value(&gpu_ctx.inner, &dims) {
        Some(b) => b,
        None => return 0,
    };

    // Dispatch as 2D grid
    let threads_per_tg = 16usize;
    let tg_count = MTLSize {
        width: n.div_ceil(threads_per_tg),
        height: m.div_ceil(threads_per_tg),
        depth: 1,
    };
    let tg_threads = MTLSize {
        width: threads_per_tg,
        height: threads_per_tg,
        depth: 1,
    };

    if dispatch::dispatch_threadgroups(
        &gpu_ctx.inner,
        &cached.compiled,
        &[
            a_buf.metal_buffer(),
            b_buf.metal_buffer(),
            &result_inner,
            &dims_buf,
        ],
        tg_count,
        tg_threads,
    )
    .is_err()
    {
        return 0;
    }

    let result = GpuBuffer::materialized(result_inner, m * n, dtype);
    Box::into_raw(Box::new(result)) as i64
}

// ---------------------------------------------------------------------------
// Extern C API — Reductions: (ctx, buf) -> f64
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_sum(ctx: i64, buf: i64) -> f64 {
    #[cfg(target_os = "macos")]
    {
        reduce_impl(ctx, buf, KernelOp::ReduceSum)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, buf);
        0.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_mean(ctx: i64, buf: i64) -> f64 {
    #[cfg(target_os = "macos")]
    {
        if buf == 0 {
            return 0.0;
        }
        let a_buf = &*(buf as *const GpuBuffer);
        let numel = a_buf.numel;
        if numel == 0 {
            return 0.0;
        }
        reduce_impl(ctx, buf, KernelOp::ReduceSum) / numel as f64
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, buf);
        0.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_max(ctx: i64, buf: i64) -> f64 {
    #[cfg(target_os = "macos")]
    {
        reduce_impl(ctx, buf, KernelOp::ReduceMax)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, buf);
        0.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_min(ctx: i64, buf: i64) -> f64 {
    #[cfg(target_os = "macos")]
    {
        reduce_impl(ctx, buf, KernelOp::ReduceMin)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, buf);
        0.0
    }
}

// ---------------------------------------------------------------------------
// Extern C API — Dot product: (ctx, a, b) -> f64
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_dot(ctx: i64, a: i64, b: i64) -> f64 {
    #[cfg(target_os = "macos")]
    {
        // Elementwise multiply (lazy), then reduce sum (materializes the fused mul)
        let product = rayzor_gpu_compute_mul(ctx, a, b);
        if product == 0 {
            return 0.0;
        }
        let result = reduce_impl(ctx, product, KernelOp::ReduceSum);
        let _ = Box::from_raw(product as *mut GpuBuffer);
        result
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a, b);
        0.0
    }
}

// ---------------------------------------------------------------------------
// Extern C API — Matmul: (ctx, a, b, m, k, n) -> GpuBuffer handle
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_matmul(
    ctx: i64,
    a: i64,
    b: i64,
    m: i64,
    k: i64,
    n: i64,
) -> i64 {
    #[cfg(target_os = "macos")]
    {
        matmul_impl(ctx, a, b, m as usize, k as usize, n as usize)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a, b, m, k, n);
        0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    use crate::metal::{buffer_ops::MetalBuffer, device_init::MetalContext};

    #[cfg(target_os = "macos")]
    use crate::kernel_cache::KernelCache;

    #[cfg(target_os = "macos")]
    use std::collections::HashMap;

    #[cfg(target_os = "macos")]
    fn make_ctx() -> i64 {
        if !MetalContext::is_available() {
            return 0;
        }
        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
            fused_cache: HashMap::new(),
        };
        Box::into_raw(Box::new(gpu_ctx)) as i64
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_add_f32() {
        let ctx = make_ctx();
        if ctx == 0 {
            return;
        }

        let n = 1024;
        let a_data: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let b_data: Vec<f32> = (0..n).map(|i| (i * 2) as f32).collect();

        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };
        let b_buf = unsafe { create_test_buffer(ctx, &b_data) };

        // add is now lazy — result is a lazy buffer
        let result = unsafe { rayzor_gpu_compute_add(ctx, a_buf, b_buf) };
        assert_ne!(result, 0, "add returned null");

        // Materialize by accessing via ensure_materialized
        let gpu_ctx = unsafe { &mut *(ctx as *mut GpuContext) };
        let result_buf = unsafe { &mut *(result as *mut GpuBuffer) };
        assert!(
            matches!(result_buf.kind, buffer::GpuBufferKind::Lazy(_)),
            "add result should be lazy"
        );
        result_buf.ensure_materialized(gpu_ctx).unwrap();
        assert!(
            matches!(result_buf.kind, buffer::GpuBufferKind::Materialized(_)),
            "should be materialized now"
        );

        assert_eq!(result_buf.numel, n);
        assert_eq!(result_buf.dtype, buffer::DTYPE_F32);

        let ptr = result_buf.metal_buffer().contents() as *const f32;
        for i in 0..n {
            let expected = (i + i * 2) as f32;
            let actual = unsafe { *ptr.add(i) };
            assert!(
                (actual - expected).abs() < 1e-6,
                "add mismatch at {}: expected {}, got {}",
                i,
                expected,
                actual
            );
        }

        unsafe {
            let _ = Box::from_raw(result as *mut GpuBuffer);
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(b_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_fused_add_mul_relu() {
        let ctx = make_ctx();
        if ctx == 0 {
            return;
        }

        let n = 256;
        let a_data: Vec<f32> = (0..n).map(|i| (i as f32) - 128.0).collect(); // -128..127
        let b_data: Vec<f32> = vec![2.0; n];
        let c_data: Vec<f32> = vec![0.5; n];

        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };
        let b_buf = unsafe { create_test_buffer(ctx, &b_data) };
        let c_buf = unsafe { create_test_buffer(ctx, &c_data) };

        // Chain: relu(add(a, b) * c) — should fuse into 1 kernel
        let add_result = unsafe { rayzor_gpu_compute_add(ctx, a_buf, b_buf) };
        let mul_result = unsafe { rayzor_gpu_compute_mul(ctx, add_result, c_buf) };
        let relu_result = unsafe { rayzor_gpu_compute_relu(ctx, mul_result) };

        assert_ne!(relu_result, 0);

        // Verify it's still lazy (3 deep)
        let result_buf = unsafe { &mut *(relu_result as *mut GpuBuffer) };
        assert!(matches!(result_buf.kind, buffer::GpuBufferKind::Lazy(_)));

        // Materialize
        let gpu_ctx = unsafe { &mut *(ctx as *mut GpuContext) };
        result_buf.ensure_materialized(gpu_ctx).unwrap();

        let ptr = result_buf.metal_buffer().contents() as *const f32;
        for i in 0..n {
            let a = (i as f32) - 128.0;
            let expected = f32::max(0.0, (a + 2.0) * 0.5);
            let actual = unsafe { *ptr.add(i) };
            assert!(
                (actual - expected).abs() < 1e-5,
                "fused mismatch at {}: expected {}, got {}",
                i,
                expected,
                actual
            );
        }

        // Check fused_cache was used: should have 1 compiled fused kernel
        assert!(
            !gpu_ctx.fused_cache.is_empty(),
            "fused cache should be populated"
        );

        unsafe {
            let _ = Box::from_raw(relu_result as *mut GpuBuffer);
            let _ = Box::from_raw(mul_result as *mut GpuBuffer);
            let _ = Box::from_raw(add_result as *mut GpuBuffer);
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(b_buf as *mut GpuBuffer);
            let _ = Box::from_raw(c_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_sum_f32() {
        let ctx = make_ctx();
        if ctx == 0 {
            return;
        }

        // sum of 1..=1024 = 524800
        let n = 1024;
        let a_data: Vec<f32> = (1..=n).map(|i| i as f32).collect();
        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };

        let result = unsafe { rayzor_gpu_compute_sum(ctx, a_buf) };
        let expected = (n * (n + 1) / 2) as f64;
        assert!(
            (result - expected).abs() < 1.0,
            "sum: expected {}, got {}",
            expected,
            result
        );

        unsafe {
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_lazy_sum_materializes() {
        let ctx = make_ctx();
        if ctx == 0 {
            return;
        }

        // gpu.add(a, b) is lazy, then gpu.sum() should materialize it
        let n = 512;
        let a_data: Vec<f32> = vec![3.0; n];
        let b_data: Vec<f32> = vec![7.0; n];
        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };
        let b_buf = unsafe { create_test_buffer(ctx, &b_data) };

        let add_result = unsafe { rayzor_gpu_compute_add(ctx, a_buf, b_buf) };
        assert_ne!(add_result, 0);

        // sum should materialize the lazy add, then reduce
        let sum = unsafe { rayzor_gpu_compute_sum(ctx, add_result) };
        let expected = (3.0 + 7.0) * n as f64;
        assert!(
            (sum - expected).abs() < 1.0,
            "lazy sum: expected {}, got {}",
            expected,
            sum
        );

        unsafe {
            let _ = Box::from_raw(add_result as *mut GpuBuffer);
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(b_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_matmul_f32() {
        let ctx = make_ctx();
        if ctx == 0 {
            return;
        }

        // A = [[1,2],[3,4]], B = [[5,6],[7,8]]
        // C = [[19, 22], [43, 50]]
        let a_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let b_data: Vec<f32> = vec![5.0, 6.0, 7.0, 8.0];
        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };
        let b_buf = unsafe { create_test_buffer(ctx, &b_data) };

        let result = unsafe { rayzor_gpu_compute_matmul(ctx, a_buf, b_buf, 2, 2, 2) };
        assert_ne!(result, 0, "matmul returned null");

        let result_buf = unsafe { &*(result as *const GpuBuffer) };
        assert_eq!(result_buf.numel, 4);

        let ptr = result_buf.metal_buffer().contents() as *const f32;
        let expected = [19.0f32, 22.0, 43.0, 50.0];
        for (i, &exp) in expected.iter().enumerate() {
            let actual = unsafe { *ptr.add(i) };
            assert!(
                (actual - exp).abs() < 1e-3,
                "matmul[{}]: expected {}, got {}",
                i,
                exp,
                actual
            );
        }

        unsafe {
            let _ = Box::from_raw(result as *mut GpuBuffer);
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(b_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    /// Helper: create a materialized GpuBuffer from f32 slice data.
    #[cfg(target_os = "macos")]
    unsafe fn create_test_buffer(ctx: i64, data: &[f32]) -> i64 {
        let gpu_ctx = &*(ctx as *const GpuContext);
        let byte_size = std::mem::size_of_val(data);
        let inner = MetalBuffer::from_data(&gpu_ctx.inner, data.as_ptr() as *const u8, byte_size)
            .expect("failed to create test buffer");

        let buf = GpuBuffer::materialized(inner, data.len(), buffer::DTYPE_F32);
        Box::into_raw(Box::new(buf)) as i64
    }
}
