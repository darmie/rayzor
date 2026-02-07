//! MSL code generation for matrix multiplication.
//!
//! Generates a naive matmul kernel: C[row,col] = sum_k(A[row,k] * B[k,col]).
//! Dispatched as a 2D grid of (N, M) threads. Dimensions (M, K, N) are passed
//! via a constant buffer at runtime, so the kernel is cached by dtype only.

use super::msl::dtype_to_msl;

/// Generate MSL source for matrix multiplication.
///
/// Buffers: A (M×K), B (K×N), C (M×N), dims (uint4: M, K, N, 0)
pub fn emit_matmul(dtype: u8) -> String {
    let msl_type = dtype_to_msl(dtype);
    let fn_name = format!("rayzor_matmul_{}", msl_type);

    format!(
        r#"#include <metal_stdlib>
using namespace metal;

kernel void {fn_name}(
    device const {msl_type}* A [[buffer(0)]],
    device const {msl_type}* B [[buffer(1)]],
    device {msl_type}* C [[buffer(2)]],
    constant uint4& dims [[buffer(3)]],
    uint2 gid [[thread_position_in_grid]]
) {{
    uint M = dims.x;
    uint K = dims.y;
    uint N = dims.z;

    uint row = gid.y;
    uint col = gid.x;

    if (row >= M || col >= N) return;

    {msl_type} sum = 0;
    for (uint i = 0; i < K; i++) {{
        sum = fma(A[row * K + i], B[i * N + col], sum);
    }}
    C[row * N + col] = sum;
}}
"#
    )
}

/// Kernel function name for matmul.
pub fn matmul_fn_name(dtype: u8) -> String {
    format!("rayzor_matmul_{}", dtype_to_msl(dtype))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matmul_f32() {
        let src = emit_matmul(crate::buffer::DTYPE_F32);
        assert!(src.contains("kernel void rayzor_matmul_float"));
        assert!(src.contains("device const float* A"));
        assert!(src.contains("device const float* B"));
        assert!(src.contains("device float* C"));
        assert!(src.contains("constant uint4& dims"));
        assert!(src.contains("uint2 gid"));
        assert!(src.contains("fma("));
    }
}
