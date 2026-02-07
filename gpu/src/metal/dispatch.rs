//! Metal compute kernel dispatch — encodes and submits GPU work.

use objc2_metal::MTLCommandBuffer;
use objc2_metal::MTLCommandEncoder;
use objc2_metal::MTLCommandQueue;
use objc2_metal::MTLComputeCommandEncoder;
use objc2_metal::MTLSize;

use super::buffer_ops::MetalBuffer;
use super::compile::CompiledKernel;
use super::device_init::MetalContext;

/// Dispatch a compiled compute kernel with the given input/output buffers.
///
/// For binary ops: `buffers` = [a, b, result] (3 buffers)
/// For unary ops:  `buffers` = [a, result] (2 buffers)
///
/// The kernel is dispatched over `numel` threads.
pub fn dispatch(
    ctx: &MetalContext,
    kernel: &CompiledKernel,
    buffers: &[&MetalBuffer],
    numel: usize,
) -> Result<(), String> {
    if numel == 0 {
        return Ok(());
    }

    // Create command buffer
    let command_buffer = ctx
        .command_queue
        .commandBuffer()
        .ok_or("failed to create command buffer")?;

    // Create compute command encoder
    let encoder = command_buffer
        .computeCommandEncoder()
        .ok_or("failed to create compute encoder")?;

    // Set pipeline state
    encoder.setComputePipelineState(&kernel.pipeline);

    // Bind buffers
    for (i, buf) in buffers.iter().enumerate() {
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&buf.mtl_buffer), 0, i);
        }
    }

    // Calculate threadgroup size
    let threads_per_group = kernel.max_threads_per_group.min(numel);
    let grid_size = MTLSize {
        width: numel,
        height: 1,
        depth: 1,
    };
    let threadgroup_size = MTLSize {
        width: threads_per_group,
        height: 1,
        depth: 1,
    };

    // Dispatch threads
    encoder.dispatchThreads_threadsPerThreadgroup(grid_size, threadgroup_size);

    // End encoding and commit
    encoder.endEncoding();
    command_buffer.commit();
    command_buffer.waitUntilCompleted();

    Ok(())
}

/// Dispatch a compiled compute kernel with explicit threadgroup counts.
///
/// Used for reductions (1D) and matmul (2D) where we need exact control
/// over the number of threadgroups rather than total thread count.
pub fn dispatch_threadgroups(
    ctx: &MetalContext,
    kernel: &CompiledKernel,
    buffers: &[&MetalBuffer],
    num_threadgroups: MTLSize,
    threads_per_threadgroup: MTLSize,
) -> Result<(), String> {
    let command_buffer = ctx
        .command_queue
        .commandBuffer()
        .ok_or("failed to create command buffer")?;

    let encoder = command_buffer
        .computeCommandEncoder()
        .ok_or("failed to create compute encoder")?;

    encoder.setComputePipelineState(&kernel.pipeline);

    for (i, buf) in buffers.iter().enumerate() {
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&buf.mtl_buffer), 0, i);
        }
    }

    encoder.dispatchThreadgroups_threadsPerThreadgroup(num_threadgroups, threads_per_threadgroup);

    encoder.endEncoding();
    command_buffer.commit();
    command_buffer.waitUntilCompleted();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metal::compile;

    #[test]
    fn test_dispatch_add_f32() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let ctx = MetalContext::new().unwrap();

        // Compile add kernel
        let source = r#"
            #include <metal_stdlib>
            using namespace metal;

            kernel void test_add(
                device const float* a [[buffer(0)]],
                device const float* b [[buffer(1)]],
                device float* result   [[buffer(2)]],
                uint id [[thread_position_in_grid]]
            ) {
                result[id] = a[id] + b[id];
            }
        "#;
        let kernel = compile::compile_msl(&ctx, source, "test_add").unwrap();

        // Create input buffers
        let n = 1024;
        let a_data: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let b_data: Vec<f32> = (0..n).map(|i| (i * 2) as f32).collect();

        let a_buf = MetalBuffer::from_data(
            &ctx,
            a_data.as_ptr() as *const u8,
            n * std::mem::size_of::<f32>(),
        )
        .unwrap();

        let b_buf = MetalBuffer::from_data(
            &ctx,
            b_data.as_ptr() as *const u8,
            n * std::mem::size_of::<f32>(),
        )
        .unwrap();

        let result_buf = MetalBuffer::allocate(&ctx, n * std::mem::size_of::<f32>()).unwrap();

        // Dispatch
        dispatch(&ctx, &kernel, &[&a_buf, &b_buf, &result_buf], n).unwrap();

        // Verify results
        let result_ptr = result_buf.contents() as *const f32;
        for i in 0..n {
            let expected = (i + i * 2) as f32;
            let actual = unsafe { *result_ptr.add(i) };
            assert!(
                (actual - expected).abs() < 1e-6,
                "mismatch at {}: expected {}, got {}",
                i,
                expected,
                actual
            );
        }
    }
}
