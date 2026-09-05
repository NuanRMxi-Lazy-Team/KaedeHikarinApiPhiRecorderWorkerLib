use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    sync::Mutex,
};

use crate::abi::{
    copy_utf8, ffi_status, phi_context_options_t, phi_status_t, validate_header, write_bytes,
    PHI_STATUS_INTERNAL_ERROR, PHI_STATUS_INVALID_ARGUMENT, PHI_STATUS_OK,
};

#[repr(C)]
pub struct phi_context {
    #[allow(dead_code)]
    options: OwnedContextOptions,
    last_error: Mutex<String>,
}

#[allow(dead_code)]
struct OwnedContextOptions {
    assets_dir: String,
    fonts_dir: String,
    resource_pack_dir: String,
    ffmpeg_path: String,
    temp_dir: String,
    renderer_host_path: String,
}

impl OwnedContextOptions {
    unsafe fn from_ffi(options: &phi_context_options_t) -> Result<Self, phi_status_t> {
        Ok(Self {
            assets_dir: copy_utf8(options.assets_dir)?,
            fonts_dir: copy_utf8(options.fonts_dir)?,
            resource_pack_dir: copy_utf8(options.resource_pack_dir)?,
            ffmpeg_path: copy_utf8(options.ffmpeg_path)?,
            temp_dir: copy_utf8(options.temp_dir)?,
            renderer_host_path: copy_utf8(options.renderer_host_path)?,
        })
    }
}

#[allow(dead_code)]
impl phi_context {
    pub(crate) fn set_error(&self, message: impl Into<String>) {
        if let Ok(mut error) = self.last_error.lock() {
            *error = message.into();
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn phi_context_create(
    options: *const phi_context_options_t,
    out_context: *mut *mut phi_context,
) -> phi_status_t {
    ffi_status(|| {
        if options.is_null() || out_context.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }

        *out_context = ptr::null_mut();
        let options = &*options;
        let status = validate_header(
            options.struct_size,
            options.abi_version,
            std::mem::size_of::<phi_context_options_t>(),
        );
        if status != PHI_STATUS_OK {
            return status;
        }

        let owned_options = match OwnedContextOptions::from_ffi(options) {
            Ok(options) => options,
            Err(status) => return status,
        };

        let context = Box::new(phi_context {
            options: owned_options,
            last_error: Mutex::new(String::new()),
        });
        *out_context = Box::into_raw(context);
        PHI_STATUS_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_context_destroy(context: *mut phi_context) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !context.is_null() {
            drop(Box::from_raw(context));
        }
    }));
}

#[no_mangle]
pub unsafe extern "C" fn phi_context_clear_error(context: *mut phi_context) -> phi_status_t {
    ffi_status(|| {
        if context.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }

        if let Ok(mut error) = (*context).last_error.lock() {
            error.clear();
        }
        PHI_STATUS_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_context_get_last_error(
    context: *const phi_context,
    buffer: *mut u8,
    capacity: usize,
    required: *mut usize,
) -> phi_status_t {
    ffi_status(|| {
        if context.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }

        let Ok(error) = (*context).last_error.lock() else {
            return PHI_STATUS_INTERNAL_ERROR;
        };
        write_bytes(error.as_bytes(), buffer, capacity, required)
    })
}
