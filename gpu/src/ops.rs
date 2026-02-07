//! GPU compute operations — elementwise binary and unary ops.
//!
//! Each operation:
//! 1. Looks up or compiles the MSL kernel via KernelCache
//! 2. Allocates a result buffer on GPU
//! 3. Dispatches the kernel
//! 4. Returns the result buffer handle

use crate::buffer::{self, GpuBuffer};
use crate::device::GpuContext;
use crate::kernel_ir::KernelOp;

#[cfg(target_os = "macos")]
use crate::codegen::msl_reduction::REDUCE_THREADGROUP_SIZE;
#[cfg(target_os = "macos")]
use crate::metal::{buffer_ops::MetalBuffer, dispatch};
#[cfg(target_os = "macos")]
use objc2_metal::MTLSize;

// ---------------------------------------------------------------------------
// Internal helpers (macOS only)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
unsafe fn binary_op_impl(ctx: i64, a: i64, b: i64, op: KernelOp) -> i64 {
    if ctx == 0 || a == 0 || b == 0 {
        return 0;
    }

    let gpu_ctx = &mut *(ctx as *mut GpuContext);
    let a_buf = &*(a as *const GpuBuffer);
    let b_buf = &*(b as *const GpuBuffer);

    // Inputs must have matching dtype and element count
    if a_buf.dtype != b_buf.dtype || a_buf.numel != b_buf.numel {
        return 0;
    }

    let dtype = a_buf.dtype;
    let numel = a_buf.numel;
    let byte_size = numel * buffer::dtype_byte_size(dtype);

    let cached = match gpu_ctx
        .kernel_cache
        .get_or_compile(&gpu_ctx.inner, op, dtype)
    {
        Ok(k) => k,
        Err(_) => return 0,
    };

    let result_inner = match MetalBuffer::allocate(&gpu_ctx.inner, byte_size) {
        Some(b) => b,
        None => return 0,
    };

    if dispatch::dispatch(
        &gpu_ctx.inner,
        &cached.compiled,
        &[&a_buf.inner, &b_buf.inner, &result_inner],
        numel,
    )
    .is_err()
    {
        return 0;
    }

    let result = GpuBuffer {
        inner: result_inner,
        numel,
        dtype,
    };
    Box::into_raw(Box::new(result)) as i64
}

#[cfg(target_os = "macos")]
unsafe fn unary_op_impl(ctx: i64, a: i64, op: KernelOp) -> i64 {
    if ctx == 0 || a == 0 {
        return 0;
    }

    let gpu_ctx = &mut *(ctx as *mut GpuContext);
    let a_buf = &*(a as *const GpuBuffer);

    let dtype = a_buf.dtype;
    let numel = a_buf.numel;
    let byte_size = numel * buffer::dtype_byte_size(dtype);

    let cached = match gpu_ctx
        .kernel_cache
        .get_or_compile(&gpu_ctx.inner, op, dtype)
    {
        Ok(k) => k,
        Err(_) => return 0,
    };

    let result_inner = match MetalBuffer::allocate(&gpu_ctx.inner, byte_size) {
        Some(b) => b,
        None => return 0,
    };

    if dispatch::dispatch(
        &gpu_ctx.inner,
        &cached.compiled,
        &[&a_buf.inner, &result_inner],
        numel,
    )
    .is_err()
    {
        return 0;
    }

    let result = GpuBuffer {
        inner: result_inner,
        numel,
        dtype,
    };
    Box::into_raw(Box::new(result)) as i64
}

// ---------------------------------------------------------------------------
// Extern C API — Binary ops: (ctx, a, b) -> result
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_add(ctx: i64, a: i64, b: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        binary_op_impl(ctx, a, b, KernelOp::Add)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a, b);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_sub(ctx: i64, a: i64, b: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        binary_op_impl(ctx, a, b, KernelOp::Sub)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a, b);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_mul(ctx: i64, a: i64, b: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        binary_op_impl(ctx, a, b, KernelOp::Mul)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a, b);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_div(ctx: i64, a: i64, b: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        binary_op_impl(ctx, a, b, KernelOp::Div)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a, b);
        0
    }
}

// ---------------------------------------------------------------------------
// Extern C API — Unary ops: (ctx, a) -> result
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_neg(ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_op_impl(ctx, a, KernelOp::Neg)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_abs(ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_op_impl(ctx, a, KernelOp::Abs)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_sqrt(ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_op_impl(ctx, a, KernelOp::Sqrt)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_exp(ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_op_impl(ctx, a, KernelOp::Exp)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_log(ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_op_impl(ctx, a, KernelOp::Log)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a);
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_relu(ctx: i64, a: i64) -> i64 {
    #[cfg(target_os = "macos")]
    {
        unary_op_impl(ctx, a, KernelOp::Relu)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (ctx, a);
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
/// Two-pass strategy:
/// - Pass 1: N threadgroups of 256 threads, each produces one partial result
/// - Pass 2 (if N > 1): 1 threadgroup reduces the N partial results
#[cfg(target_os = "macos")]
unsafe fn reduce_impl(ctx: i64, buf: i64, op: KernelOp) -> f64 {
    if ctx == 0 || buf == 0 {
        return 0.0;
    }

    let gpu_ctx = &mut *(ctx as *mut GpuContext);
    let a_buf = &*(buf as *const GpuBuffer);

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
        &[&a_buf.inner, &partial_buf, &numel_buf],
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
#[cfg(target_os = "macos")]
unsafe fn matmul_impl(ctx: i64, a: i64, b: i64, m: usize, k: usize, n: usize) -> i64 {
    if ctx == 0 || a == 0 || b == 0 || m == 0 || k == 0 || n == 0 {
        return 0;
    }

    let gpu_ctx = &mut *(ctx as *mut GpuContext);
    let a_buf = &*(a as *const GpuBuffer);
    let b_buf = &*(b as *const GpuBuffer);

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
        &[&a_buf.inner, &b_buf.inner, &result_inner, &dims_buf],
        tg_count,
        tg_threads,
    )
    .is_err()
    {
        return 0;
    }

    let result = GpuBuffer {
        inner: result_inner,
        numel: m * n,
        dtype,
    };
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
        // Elementwise multiply, then sum
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

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_add_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        let n = 1024;
        let a_data: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let b_data: Vec<f32> = (0..n).map(|i| (i * 2) as f32).collect();

        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };
        let b_buf = unsafe { create_test_buffer(ctx, &b_data) };

        let result = unsafe { rayzor_gpu_compute_add(ctx, a_buf, b_buf) };
        assert_ne!(result, 0, "add returned null");

        // Verify results
        let result_buf = unsafe { &*(result as *const GpuBuffer) };
        assert_eq!(result_buf.numel, n);
        assert_eq!(result_buf.dtype, buffer::DTYPE_F32);

        let ptr = result_buf.inner.contents() as *const f32;
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

        // Cleanup
        unsafe {
            let _ = Box::from_raw(result as *mut GpuBuffer);
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(b_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_mul_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        let n = 512;
        let a_data: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let b_data: Vec<f32> = (0..n).map(|_| 3.0f32).collect();

        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };
        let b_buf = unsafe { create_test_buffer(ctx, &b_data) };

        let result = unsafe { rayzor_gpu_compute_mul(ctx, a_buf, b_buf) };
        assert_ne!(result, 0, "mul returned null");

        let result_buf = unsafe { &*(result as *const GpuBuffer) };
        let ptr = result_buf.inner.contents() as *const f32;
        for i in 0..n {
            let expected = i as f32 * 3.0;
            let actual = unsafe { *ptr.add(i) };
            assert!(
                (actual - expected).abs() < 1e-6,
                "mul mismatch at {}: expected {}, got {}",
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
    fn test_gpu_sqrt_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        let n = 256;
        let a_data: Vec<f32> = (0..n).map(|i| (i * i) as f32).collect();

        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };

        let result = unsafe { rayzor_gpu_compute_sqrt(ctx, a_buf) };
        assert_ne!(result, 0, "sqrt returned null");

        let result_buf = unsafe { &*(result as *const GpuBuffer) };
        let ptr = result_buf.inner.contents() as *const f32;
        for i in 0..n {
            let expected = i as f32;
            let actual = unsafe { *ptr.add(i) };
            assert!(
                (actual - expected).abs() < 1e-3,
                "sqrt mismatch at {}: expected {}, got {}",
                i,
                expected,
                actual
            );
        }

        unsafe {
            let _ = Box::from_raw(result as *mut GpuBuffer);
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_neg_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        let n = 128;
        let a_data: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };

        let result = unsafe { rayzor_gpu_compute_neg(ctx, a_buf) };
        assert_ne!(result, 0, "neg returned null");

        let result_buf = unsafe { &*(result as *const GpuBuffer) };
        let ptr = result_buf.inner.contents() as *const f32;
        for i in 0..n {
            let expected = -(i as f32);
            let actual = unsafe { *ptr.add(i) };
            assert!(
                (actual - expected).abs() < 1e-6,
                "neg mismatch at {}: expected {}, got {}",
                i,
                expected,
                actual
            );
        }

        unsafe {
            let _ = Box::from_raw(result as *mut GpuBuffer);
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_relu_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        let n = 256;
        // Mix of negative and positive values
        let a_data: Vec<f32> = (0..n).map(|i| (i as f32) - 128.0).collect();
        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };

        let result = unsafe { rayzor_gpu_compute_relu(ctx, a_buf) };
        assert_ne!(result, 0, "relu returned null");

        let result_buf = unsafe { &*(result as *const GpuBuffer) };
        let ptr = result_buf.inner.contents() as *const f32;
        for i in 0..n {
            let input = (i as f32) - 128.0;
            let expected = if input > 0.0 { input } else { 0.0 };
            let actual = unsafe { *ptr.add(i) };
            assert!(
                (actual - expected).abs() < 1e-6,
                "relu mismatch at {}: expected {}, got {}",
                i,
                expected,
                actual
            );
        }

        unsafe {
            let _ = Box::from_raw(result as *mut GpuBuffer);
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_kernel_caching() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        let n = 64;
        let a_data: Vec<f32> = vec![1.0; n];
        let b_data: Vec<f32> = vec![2.0; n];

        // Run add twice — second call should use cached kernel
        for _ in 0..2 {
            let a_buf = unsafe { create_test_buffer(ctx, &a_data) };
            let b_buf = unsafe { create_test_buffer(ctx, &b_data) };
            let result = unsafe { rayzor_gpu_compute_add(ctx, a_buf, b_buf) };
            assert_ne!(result, 0);

            unsafe {
                let _ = Box::from_raw(result as *mut GpuBuffer);
                let _ = Box::from_raw(a_buf as *mut GpuBuffer);
                let _ = Box::from_raw(b_buf as *mut GpuBuffer);
            }
        }

        // Verify cache has exactly 1 entry (same op+dtype reused)
        let gpu_ctx = unsafe { &*(ctx as *const GpuContext) };
        assert_eq!(gpu_ctx.kernel_cache.len(), 1);

        unsafe {
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_sum_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        // sum of 1..=1024 = 1024 * 1025 / 2 = 524800
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
    fn test_gpu_sum_large() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        // Large array: 100K elements of 1.0 → sum = 100000
        let n = 100_000;
        let a_data: Vec<f32> = vec![1.0; n];
        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };

        let result = unsafe { rayzor_gpu_compute_sum(ctx, a_buf) };
        assert!(
            (result - n as f64).abs() < 1.0,
            "large sum: expected {}, got {}",
            n,
            result
        );

        unsafe {
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_mean_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        let n = 1000;
        let a_data: Vec<f32> = vec![5.0; n];
        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };

        let result = unsafe { rayzor_gpu_compute_mean(ctx, a_buf) };
        assert!(
            (result - 5.0).abs() < 1e-3,
            "mean: expected 5.0, got {}",
            result
        );

        unsafe {
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_max_min_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        let n = 512;
        let a_data: Vec<f32> = (0..n).map(|i| (i as f32) - 100.0).collect();
        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };

        let max_result = unsafe { rayzor_gpu_compute_max(ctx, a_buf) };
        let min_result = unsafe { rayzor_gpu_compute_min(ctx, a_buf) };

        assert!(
            (max_result - 411.0).abs() < 1e-3,
            "max: expected 411.0, got {}",
            max_result
        );
        assert!(
            (min_result - (-100.0)).abs() < 1e-3,
            "min: expected -100.0, got {}",
            min_result
        );

        unsafe {
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_dot_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        // dot([1,2,3,4], [1,2,3,4]) = 1+4+9+16 = 30
        let a_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let b_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };
        let b_buf = unsafe { create_test_buffer(ctx, &b_data) };

        let result = unsafe { rayzor_gpu_compute_dot(ctx, a_buf, b_buf) };
        assert!(
            (result - 30.0).abs() < 1e-3,
            "dot: expected 30.0, got {}",
            result
        );

        unsafe {
            let _ = Box::from_raw(a_buf as *mut GpuBuffer);
            let _ = Box::from_raw(b_buf as *mut GpuBuffer);
            let _ = Box::from_raw(ctx as *mut GpuContext);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_gpu_matmul_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let metal_ctx = MetalContext::new().unwrap();
        let gpu_ctx = GpuContext {
            inner: metal_ctx,
            kernel_cache: KernelCache::new(),
        };
        let ctx = Box::into_raw(Box::new(gpu_ctx)) as i64;

        // A = [[1,2],[3,4]] (2x2), B = [[5,6],[7,8]] (2x2)
        // C = [[1*5+2*7, 1*6+2*8], [3*5+4*7, 3*6+4*8]]
        //   = [[19, 22], [43, 50]]
        let a_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let b_data: Vec<f32> = vec![5.0, 6.0, 7.0, 8.0];
        let a_buf = unsafe { create_test_buffer(ctx, &a_data) };
        let b_buf = unsafe { create_test_buffer(ctx, &b_data) };

        let result = unsafe { rayzor_gpu_compute_matmul(ctx, a_buf, b_buf, 2, 2, 2) };
        assert_ne!(result, 0, "matmul returned null");

        let result_buf = unsafe { &*(result as *const GpuBuffer) };
        assert_eq!(result_buf.numel, 4);

        let ptr = result_buf.inner.contents() as *const f32;
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

    /// Helper: create a GpuBuffer from f32 slice data.
    #[cfg(target_os = "macos")]
    unsafe fn create_test_buffer(ctx: i64, data: &[f32]) -> i64 {
        let gpu_ctx = &*(ctx as *const GpuContext);
        let byte_size = std::mem::size_of_val(data);
        let inner = MetalBuffer::from_data(&gpu_ctx.inner, data.as_ptr() as *const u8, byte_size)
            .expect("failed to create test buffer");

        let buf = GpuBuffer {
            inner,
            numel: data.len(),
            dtype: buffer::DTYPE_F32,
        };
        Box::into_raw(Box::new(buf)) as i64
    }
}
