//! Metal buffer operations — GPU memory allocation and data transfer

use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLBuffer, MTLDevice, MTLResourceOptions};

use super::device_init::MetalContext;

/// Metal-specific GPU buffer wrapping an MTLBuffer.
pub struct MetalBuffer {
    pub(crate) mtl_buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
    pub(crate) byte_size: usize,
}

impl MetalBuffer {
    /// Create a Metal buffer by copying data from a CPU pointer.
    pub fn from_data(ctx: &MetalContext, data: *const u8, byte_size: usize) -> Option<Self> {
        if data.is_null() || byte_size == 0 {
            return None;
        }

        let ptr = NonNull::new(data as *mut std::ffi::c_void)?;
        let mtl_buffer = unsafe {
            ctx.device.newBufferWithBytes_length_options(
                ptr,
                byte_size,
                MTLResourceOptions::StorageModeShared,
            )
        }?;

        Some(MetalBuffer {
            mtl_buffer,
            byte_size,
        })
    }

    /// Allocate an empty Metal buffer of the given size.
    pub fn allocate(ctx: &MetalContext, byte_size: usize) -> Option<Self> {
        if byte_size == 0 {
            return None;
        }

        let mtl_buffer = ctx
            .device
            .newBufferWithLength_options(byte_size, MTLResourceOptions::StorageModeShared)?;

        Some(MetalBuffer {
            mtl_buffer,
            byte_size,
        })
    }

    /// Get a raw CPU-accessible pointer to the buffer contents.
    pub fn contents(&self) -> *mut u8 {
        self.mtl_buffer.contents().as_ptr() as *mut u8
    }

    /// Get the byte size of the buffer.
    pub fn byte_size(&self) -> usize {
        self.byte_size
    }
}
