//! WGSL shader compilation — WGSL source → wgpu::ComputePipeline

use wgpu;

use super::device_init::WgpuContext;

/// A compiled wgpu compute kernel ready for dispatch.
pub struct WgpuCompiledKernel {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub workgroup_size: u32,
}

/// Compile WGSL source code into a compute pipeline.
///
/// The bind group layout is auto-derived from the shader reflection,
/// so it correctly handles mixed storage/uniform bindings (matmul, reductions).
pub fn compile_wgsl(
    ctx: &WgpuContext,
    source: &str,
    entry_point: &str,
    _num_buffers: usize,
    workgroup_size: u32,
) -> Result<WgpuCompiledKernel, String> {
    let shader_module = ctx
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rayzor_compute_shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

    // Auto-derive layout from shader reflection — handles mixed storage/uniform bindings
    let pipeline = ctx
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("rayzor_compute_pipeline"),
            layout: None, // auto-derive from shader
            module: &shader_module,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            cache: None,
        });

    let bind_group_layout = pipeline.get_bind_group_layout(0);

    Ok(WgpuCompiledKernel {
        pipeline,
        bind_group_layout,
        workgroup_size,
    })
}
