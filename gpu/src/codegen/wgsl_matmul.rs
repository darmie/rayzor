//! WGSL code generation for matrix multiplication.
//!
//! Generates a naive matmul kernel: C[row,col] = sum_k(A[row,k] * B[k,col]).
//! Dispatched as a 2D grid of ceil(N/16) x ceil(M/16) workgroups.
//! Dimensions (M, K, N) are passed via a uniform buffer.

use super::wgsl::dtype_to_wgsl;

/// Generate WGSL source for matrix multiplication.
///
/// Buffers: A (M×K), B (K×N), C (M×N), dims (vec4<u32>: M, K, N, 0)
pub fn emit_matmul(dtype: u8) -> String {
    let wgsl_type = dtype_to_wgsl(dtype);
    let fn_name = format!("rayzor_matmul_{}", wgsl_type);

    format!(
        r#"@group(0) @binding(0) var<storage, read> A: array<{wgsl_type}>;
@group(0) @binding(1) var<storage, read> B: array<{wgsl_type}>;
@group(0) @binding(2) var<storage, read_write> C: array<{wgsl_type}>;
@group(0) @binding(3) var<uniform> dims: vec4<u32>;

@compute @workgroup_size(16, 16)
fn {fn_name}(@builtin(global_invocation_id) gid: vec3<u32>) {{
    let M = dims.x;
    let K = dims.y;
    let N = dims.z;

    let row = gid.y;
    let col = gid.x;

    if (row >= M || col >= N) {{
        return;
    }}

    var sum = {wgsl_type}(0);
    for (var i = 0u; i < K; i = i + 1u) {{
        sum = fma(A[row * K + i], B[i * N + col], sum);
    }}
    C[row * N + col] = sum;
}}
"#
    )
}

/// Kernel function name for matmul.
pub fn matmul_fn_name(dtype: u8) -> String {
    format!("rayzor_matmul_{}", dtype_to_wgsl(dtype))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matmul_f32() {
        let src = emit_matmul(crate::buffer::DTYPE_F32);
        assert!(src.contains("fn rayzor_matmul_f32"));
        assert!(src.contains("var<storage, read> A: array<f32>"));
        assert!(src.contains("var<storage, read> B: array<f32>"));
        assert!(src.contains("var<storage, read_write> C: array<f32>"));
        assert!(src.contains("var<uniform> dims: vec4<u32>"));
        assert!(src.contains("@workgroup_size(16, 16)"));
        assert!(src.contains("fma("));
    }
}
