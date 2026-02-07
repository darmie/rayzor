//! GPU buffer management — CPU↔GPU data transfer
//!
//! GpuBuffer wraps a Metal buffer (or future CUDA/WebGPU buffer) with
//! metadata about element count and dtype, enabling typed tensor interop.

use crate::device::GpuContext;

#[cfg(target_os = "macos")]
use crate::metal::buffer_ops;

/// DType tags matching runtime/src/tensor.rs
pub const DTYPE_F32: u8 = 0;
pub const DTYPE_F64: u8 = 1;
pub const DTYPE_I32: u8 = 2;
pub const DTYPE_I64: u8 = 3;

/// Byte size per element for each dtype.
pub fn dtype_byte_size(dtype: u8) -> usize {
    match dtype {
        DTYPE_F32 => 4,
        DTYPE_F64 => 8,
        DTYPE_I32 => 4,
        DTYPE_I64 => 8,
        _ => 8, // default to f64
    }
}

/// Opaque GPU buffer handle.
pub struct GpuBuffer {
    #[cfg(target_os = "macos")]
    pub(crate) inner: buffer_ops::MetalBuffer,
    #[cfg(not(target_os = "macos"))]
    _placeholder: (),
    pub numel: usize,
    pub dtype: u8,
}

// ---------------------------------------------------------------------------
// Extern C API
// ---------------------------------------------------------------------------

/// Create a GPU buffer from a RayzorTensor.
/// Copies tensor data to GPU-accessible memory.
///
/// tensor_ptr: i64 pointer to RayzorTensor struct
/// Returns: i64 pointer to GpuBuffer, or 0 on failure
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_create_buffer(ctx: i64, tensor_ptr: i64) -> i64 {
    if ctx == 0 || tensor_ptr == 0 {
        return 0;
    }

    let gpu_ctx = &*(ctx as *const GpuContext);

    // RayzorTensor layout: { data: *mut u8, shape: *mut usize, strides: *mut usize,
    //                        ndim: usize, numel: usize, dtype: u8, owns_data: bool }
    // RayzorTensor field offsets on 64-bit: data=0, shape=8, strides=16, ndim=24, numel=32, dtype=40
    let tensor = tensor_ptr as *const u8;
    let data_ptr = *(tensor as *const *const u8);
    let numel = *(tensor.add(32) as *const usize);
    let dtype = *tensor.add(40);
    let byte_size = numel * dtype_byte_size(dtype);

    #[cfg(target_os = "macos")]
    {
        match buffer_ops::MetalBuffer::from_data(&gpu_ctx.inner, data_ptr, byte_size) {
            Some(inner) => {
                let buf = GpuBuffer {
                    inner,
                    numel,
                    dtype,
                };
                Box::into_raw(Box::new(buf)) as i64
            }
            None => 0,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (data_ptr, byte_size);
        0
    }
}

/// Allocate an empty GPU buffer with the given element count and dtype.
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_alloc_buffer(ctx: i64, numel: i64, dtype: i64) -> i64 {
    if ctx == 0 || numel <= 0 {
        return 0;
    }

    let gpu_ctx = &*(ctx as *const GpuContext);
    let numel = numel as usize;
    let dtype = dtype as u8;
    let byte_size = numel * dtype_byte_size(dtype);

    #[cfg(target_os = "macos")]
    {
        match buffer_ops::MetalBuffer::allocate(&gpu_ctx.inner, byte_size) {
            Some(inner) => {
                let buf = GpuBuffer {
                    inner,
                    numel,
                    dtype,
                };
                Box::into_raw(Box::new(buf)) as i64
            }
            None => 0,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = byte_size;
        0
    }
}

/// Copy GPU buffer data back to a new RayzorTensor.
/// Returns i64 pointer to a newly allocated RayzorTensor.
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_to_tensor(ctx: i64, buffer_ptr: i64) -> i64 {
    if ctx == 0 || buffer_ptr == 0 {
        return 0;
    }

    let buf = &*(buffer_ptr as *const GpuBuffer);
    let byte_size = buf.numel * dtype_byte_size(buf.dtype);

    #[cfg(target_os = "macos")]
    {
        let src_ptr = buf.inner.contents();
        if src_ptr.is_null() {
            return 0;
        }

        // Allocate tensor data
        let data = libc::malloc(byte_size) as *mut u8;
        if data.is_null() {
            return 0;
        }
        std::ptr::copy_nonoverlapping(src_ptr, data, byte_size);

        // Build 1D shape: [numel]
        let shape = libc::malloc(std::mem::size_of::<usize>()) as *mut usize;
        if shape.is_null() {
            libc::free(data as *mut libc::c_void);
            return 0;
        }
        *shape = buf.numel;

        let strides = libc::malloc(std::mem::size_of::<usize>()) as *mut usize;
        if strides.is_null() {
            libc::free(data as *mut libc::c_void);
            libc::free(shape as *mut libc::c_void);
            return 0;
        }
        *strides = 1;

        // Allocate RayzorTensor struct (48 bytes, 8-aligned)
        // Layout: data(*u8), shape(*usize), strides(*usize), ndim(usize), numel(usize), dtype(u8), owns_data(bool)
        let tensor_size: usize = 48;
        let tensor = libc::malloc(tensor_size) as *mut u8;
        if tensor.is_null() {
            libc::free(data as *mut libc::c_void);
            libc::free(shape as *mut libc::c_void);
            libc::free(strides as *mut libc::c_void);
            return 0;
        }

        // Write fields
        *(tensor as *mut *mut u8) = data; // data: offset 0
        *(tensor.add(8) as *mut *mut usize) = shape; // shape: offset 8
        *(tensor.add(16) as *mut *mut usize) = strides; // strides: offset 16
        *(tensor.add(24) as *mut usize) = 1; // ndim: offset 24
        *(tensor.add(32) as *mut usize) = buf.numel; // numel: offset 32
        *tensor.add(40) = buf.dtype; // dtype: offset 40
        *tensor.add(41) = 1; // owns_data: offset 41

        tensor as i64
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = byte_size;
        0
    }
}

/// Free a GPU buffer.
///
/// Takes (ctx, buffer) for consistency with the instance method calling convention,
/// though ctx is unused. This allows the compiler to call the extern directly
/// without a MIR wrapper.
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_free_buffer(_ctx: i64, buffer_ptr: i64) {
    if buffer_ptr == 0 {
        return;
    }
    let _ = Box::from_raw(buffer_ptr as *mut GpuBuffer);
}

/// Get the number of elements in a GPU buffer.
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_buffer_numel(buffer_ptr: i64) -> i64 {
    if buffer_ptr == 0 {
        return 0;
    }
    let buf = &*(buffer_ptr as *const GpuBuffer);
    buf.numel as i64
}

/// Get the dtype tag of a GPU buffer.
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_buffer_dtype(buffer_ptr: i64) -> i64 {
    if buffer_ptr == 0 {
        return 0;
    }
    let buf = &*(buffer_ptr as *const GpuBuffer);
    buf.dtype as i64
}

// ---------------------------------------------------------------------------
// Structured buffer API for @:gpuStruct
// ---------------------------------------------------------------------------

/// Create a GPU buffer from an array of @:gpuStruct instances.
///
/// Packs `count` structs into a contiguous GPU buffer. Each struct is
/// `struct_size` bytes on the CPU side (matching the GPU layout since
/// @:gpuStruct already uses GPU-compatible layout on CPU).
///
/// Arguments: (ctx, array_ptr, count, struct_size)
/// - array_ptr: pointer to Haxe Array of gpuStruct pointers
/// - count: number of elements
/// - struct_size: byte size of each struct (from gpuSize())
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_create_struct_buffer(
    ctx: i64,
    array_ptr: i64,
    count: i64,
    struct_size: i64,
) -> i64 {
    if ctx == 0 || array_ptr == 0 || count <= 0 || struct_size <= 0 {
        return 0;
    }

    let gpu_ctx = &*(ctx as *const GpuContext);
    let count = count as usize;
    let struct_size = struct_size as usize;
    let total_bytes = count * struct_size;

    // Allocate staging buffer and pack structs contiguously
    let staging = libc::malloc(total_bytes) as *mut u8;
    if staging.is_null() {
        return 0;
    }

    // Haxe Array layout: first 8 bytes = data pointer (pointer to array of pointers)
    // Each element is a pointer to a @:gpuStruct (flat malloc'd block)
    let array_data = *(array_ptr as *const *const i64);
    for i in 0..count {
        let struct_ptr = *array_data.add(i) as *const u8;
        if !struct_ptr.is_null() {
            std::ptr::copy_nonoverlapping(struct_ptr, staging.add(i * struct_size), struct_size);
        } else {
            std::ptr::write_bytes(staging.add(i * struct_size), 0, struct_size);
        }
    }

    #[cfg(target_os = "macos")]
    {
        use crate::metal::buffer_ops::MetalBuffer;
        match MetalBuffer::from_data(&gpu_ctx.inner, staging, total_bytes) {
            Some(inner) => {
                libc::free(staging as *mut libc::c_void);
                // Store as raw bytes — numel = total_bytes, dtype = DTYPE_I32 (placeholder)
                let buf = GpuBuffer {
                    inner,
                    numel: count,
                    dtype: DTYPE_F32, // struct buffers use F32 as marker
                };
                Box::into_raw(Box::new(buf)) as i64
            }
            None => {
                libc::free(staging as *mut libc::c_void);
                0
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        libc::free(staging as *mut libc::c_void);
        0
    }
}

/// Allocate an empty GPU buffer for `count` structs of `struct_size` bytes.
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_alloc_struct_buffer(
    ctx: i64,
    count: i64,
    struct_size: i64,
) -> i64 {
    if ctx == 0 || count <= 0 || struct_size <= 0 {
        return 0;
    }

    let gpu_ctx = &*(ctx as *const GpuContext);
    let count = count as usize;
    let struct_size = struct_size as usize;
    let total_bytes = count * struct_size;

    #[cfg(target_os = "macos")]
    {
        use crate::metal::buffer_ops::MetalBuffer;
        match MetalBuffer::allocate(&gpu_ctx.inner, total_bytes) {
            Some(inner) => {
                let buf = GpuBuffer {
                    inner,
                    numel: count,
                    dtype: DTYPE_F32,
                };
                Box::into_raw(Box::new(buf)) as i64
            }
            None => 0,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = total_bytes;
        0
    }
}

/// Read a single f32 field from a structured GPU buffer, promote to f64.
///
/// Arguments: (ctx, buffer, index, field_byte_offset)
/// Returns: f64 value of the field
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_read_struct_float(
    _ctx: i64,
    buffer_ptr: i64,
    index: i64,
    struct_size: i64,
    field_offset: i64,
) -> f64 {
    if buffer_ptr == 0 {
        return 0.0;
    }

    #[cfg(target_os = "macos")]
    {
        let buf = &*(buffer_ptr as *const GpuBuffer);
        let ptr = buf.inner.contents();
        if ptr.is_null() {
            return 0.0;
        }
        let byte_offset = (index as usize) * (struct_size as usize) + (field_offset as usize);
        let val = *(ptr.add(byte_offset) as *const f32);
        val as f64
    }
    #[cfg(not(target_os = "macos"))]
    {
        0.0
    }
}

/// Read a single i32 field from a structured GPU buffer, extend to i64.
///
/// Arguments: (ctx, buffer, index, struct_size, field_byte_offset)
/// Returns: i64 value of the field
#[no_mangle]
pub unsafe extern "C" fn rayzor_gpu_compute_read_struct_int(
    _ctx: i64,
    buffer_ptr: i64,
    index: i64,
    struct_size: i64,
    field_offset: i64,
) -> i64 {
    if buffer_ptr == 0 {
        return 0;
    }

    #[cfg(target_os = "macos")]
    {
        let buf = &*(buffer_ptr as *const GpuBuffer);
        let ptr = buf.inner.contents();
        if ptr.is_null() {
            return 0;
        }
        let byte_offset = (index as usize) * (struct_size as usize) + (field_offset as usize);
        let val = *(ptr.add(byte_offset) as *const i32);
        val as i64
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}
