//! WGSL code generation for reduction kernels (sum, max, min).
//!
//! Reductions use workgroup shared memory and a two-pass strategy:
//! - Pass 1: Each workgroup reduces a chunk via grid-stride loop + tree reduction
//! - Pass 2: Single workgroup reduces partial results from pass 1
//!
//! The same compiled kernel is used for both passes (numel is a uniform parameter).

use crate::buffer;
use crate::kernel_ir::KernelOp;

use super::wgsl::dtype_to_wgsl;

/// Fixed workgroup size for all reduction kernels.
pub const REDUCE_WORKGROUP_SIZE: u32 = 256;

/// Generate WGSL source for a reduction kernel.
pub fn emit_reduction(op: KernelOp, dtype: u8) -> String {
    let wgsl_type = dtype_to_wgsl(dtype);
    let fn_name = format!("rayzor_{}_{}", op.name(), wgsl_type);

    let (identity, accumulate, combine) = match op {
        KernelOp::ReduceSum => (
            format!("{wgsl_type}(0)"),
            "acc = acc + input[i]".to_string(),
            "shared_data[tid] = shared_data[tid] + shared_data[tid + s]".to_string(),
        ),
        KernelOp::ReduceMax => {
            let id = match dtype {
                buffer::DTYPE_F32 => format!("{wgsl_type}(-3.402823e+38)"),
                buffer::DTYPE_I32 => format!("{wgsl_type}(-2147483647)"),
                _ => format!("{wgsl_type}(-3.402823e+38)"),
            };
            (
                id,
                "acc = max(acc, input[i])".to_string(),
                "shared_data[tid] = max(shared_data[tid], shared_data[tid + s])".to_string(),
            )
        }
        KernelOp::ReduceMin => {
            let id = match dtype {
                buffer::DTYPE_F32 => format!("{wgsl_type}(3.402823e+38)"),
                buffer::DTYPE_I32 => format!("{wgsl_type}(2147483647)"),
                _ => format!("{wgsl_type}(3.402823e+38)"),
            };
            (
                id,
                "acc = min(acc, input[i])".to_string(),
                "shared_data[tid] = min(shared_data[tid], shared_data[tid + s])".to_string(),
            )
        }
        _ => unreachable!("not a reduction op"),
    };

    format!(
        r#"@group(0) @binding(0) var<storage, read> input: array<{wgsl_type}>;
@group(0) @binding(1) var<storage, read_write> output: array<{wgsl_type}>;
@group(0) @binding(2) var<uniform> numel: u32;

var<workgroup> shared_data: array<{wgsl_type}, {REDUCE_WORKGROUP_SIZE}>;

@compute @workgroup_size({REDUCE_WORKGROUP_SIZE})
fn {fn_name}(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) wg_id: vec3<u32>,
    @builtin(num_workgroups) num_wgs: vec3<u32>
) {{
    let gid = global_id.x;
    let tid = local_id.x;
    let tg_size = {REDUCE_WORKGROUP_SIZE}u;
    let stride = tg_size * num_wgs.x;

    var acc = {identity};
    var i = gid;
    loop {{
        if (i >= numel) {{
            break;
        }}
        {accumulate};
        i = i + stride;
    }}

    shared_data[tid] = acc;
    workgroupBarrier();

    var s = tg_size / 2u;
    loop {{
        if (s == 0u) {{
            break;
        }}
        if (tid < s) {{
            {combine};
        }}
        workgroupBarrier();
        s = s / 2u;
    }}

    if (tid == 0u) {{
        output[wg_id.x] = shared_data[0];
    }}
}}
"#
    )
}

/// Kernel function name for a reduction.
pub fn reduction_fn_name(op: KernelOp, dtype: u8) -> String {
    format!("rayzor_{}_{}", op.name(), dtype_to_wgsl(dtype))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reduce_sum_f32() {
        let src = emit_reduction(KernelOp::ReduceSum, buffer::DTYPE_F32);
        assert!(src.contains("fn rayzor_reduce_sum_f32"));
        assert!(src.contains("var<workgroup> shared_data: array<f32, 256>"));
        assert!(src.contains("var acc = f32(0)"));
        assert!(src.contains("var<uniform> numel: u32"));
        assert!(src.contains("workgroupBarrier"));
    }

    #[test]
    fn test_reduce_max_f32() {
        let src = emit_reduction(KernelOp::ReduceMax, buffer::DTYPE_F32);
        assert!(src.contains("rayzor_reduce_max_f32"));
        assert!(src.contains("f32(-3.402823e+38)"));
        assert!(src.contains("max("));
    }

    #[test]
    fn test_reduce_min_i32() {
        let src = emit_reduction(KernelOp::ReduceMin, buffer::DTYPE_I32);
        assert!(src.contains("rayzor_reduce_min_i32"));
        assert!(src.contains("i32(2147483647)"));
        assert!(src.contains("min("));
    }
}
