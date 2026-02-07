//! Metal shader compilation — MSL source → MTLComputePipelineState

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{MTLComputePipelineState, MTLDevice, MTLLibrary};

use super::device_init::MetalContext;

/// A compiled Metal compute kernel ready for dispatch.
pub struct CompiledKernel {
    pub pipeline: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    /// Maximum threads per threadgroup for this pipeline.
    pub max_threads_per_group: usize,
}

/// Compile MSL source code into a compute pipeline state.
///
/// `fn_name` must match the kernel function name in the MSL source.
pub fn compile_msl(
    ctx: &MetalContext,
    source: &str,
    fn_name: &str,
) -> Result<CompiledKernel, String> {
    // Compile MSL source → MTLLibrary
    let source_ns = NSString::from_str(source);
    let library: Retained<ProtocolObject<dyn MTLLibrary>> = ctx
        .device
        .newLibraryWithSource_options_error(&source_ns, None)
        .map_err(|e| format!("MSL compilation failed: {}", e))?;

    // Get kernel function from library
    let fn_name_ns = NSString::from_str(fn_name);
    let function = library.newFunctionWithName(&fn_name_ns).ok_or_else(|| {
        format!(
            "kernel function '{}' not found in compiled library",
            fn_name
        )
    })?;

    // Create compute pipeline state
    let pipeline: Retained<ProtocolObject<dyn MTLComputePipelineState>> = ctx
        .device
        .newComputePipelineStateWithFunction_error(&function)
        .map_err(|e| format!("pipeline creation failed: {}", e))?;

    let max_threads_per_group = pipeline.maxTotalThreadsPerThreadgroup() as usize;

    Ok(CompiledKernel {
        pipeline,
        max_threads_per_group,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_simple_kernel() {
        if !MetalContext::is_available() {
            println!("Metal not available, skipping");
            return;
        }

        let ctx = MetalContext::new().unwrap();
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

        let kernel = compile_msl(&ctx, source, "test_add");
        assert!(kernel.is_ok(), "compilation failed: {:?}", kernel.err());
        let kernel = kernel.unwrap();
        assert!(kernel.max_threads_per_group > 0);
        println!("max_threads_per_group: {}", kernel.max_threads_per_group);
    }
}
