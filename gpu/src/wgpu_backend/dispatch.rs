//! WebGPU compute kernel dispatch — encodes and submits GPU work.

use wgpu;

use super::buffer_ops::WgpuBuffer;
use super::compile::WgpuCompiledKernel;
use super::device_init::WgpuContext;

/// Dispatch a compiled compute kernel over `numel` elements.
///
/// Automatically calculates workgroup count from `numel / workgroup_size`.
pub fn dispatch(
    ctx: &WgpuContext,
    kernel: &WgpuCompiledKernel,
    buffers: &[&WgpuBuffer],
    numel: usize,
) -> Result<(), String> {
    if numel == 0 {
        return Ok(());
    }

    let wg_size = kernel.workgroup_size as usize;
    let num_workgroups = numel.div_ceil(wg_size);
    dispatch_workgroups(ctx, kernel, buffers, (num_workgroups, 1, 1))
}

/// Dispatch a compiled compute kernel with explicit workgroup counts.
///
/// Used for reductions (1D) and matmul (2D).
pub fn dispatch_workgroups(
    ctx: &WgpuContext,
    kernel: &WgpuCompiledKernel,
    buffers: &[&WgpuBuffer],
    workgroups: (usize, usize, usize),
) -> Result<(), String> {
    // Build bind group entries
    let entries: Vec<wgpu::BindGroupEntry> = buffers
        .iter()
        .enumerate()
        .map(|(i, buf)| wgpu::BindGroupEntry {
            binding: i as u32,
            resource: buf.buffer.as_entire_binding(),
        })
        .collect();

    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rayzor_dispatch_bg"),
        layout: &kernel.bind_group_layout,
        entries: &entries,
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("rayzor_dispatch"),
        });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("rayzor_compute_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&kernel.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(
            workgroups.0 as u32,
            workgroups.1 as u32,
            workgroups.2 as u32,
        );
    }

    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    Ok(())
}
