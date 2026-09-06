use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    ptr,
    sync::Mutex,
};

use crate::abi::{
    copy_utf8, ffi_status, phi_context_options_t, phi_status_t, validate_header, write_bytes,
    PHI_STATUS_INTERNAL_ERROR, PHI_STATUS_INVALID_ARGUMENT, PHI_STATUS_OK,
};
use phi_recorder_core::ResourceRoots;

#[repr(C)]
pub struct phi_context {
    resource_roots: ResourceRoots,
    last_error: Mutex<String>,
    pub(crate) active_job: Mutex<Option<*mut crate::job::phi_job>>,
}

unsafe fn resource_roots_from_ffi(
    options: &phi_context_options_t,
) -> Result<ResourceRoots, phi_status_t> {
    let roots = ResourceRoots {
        assets_dir: PathBuf::from(copy_utf8(options.assets_dir)?),
        fonts_dir: PathBuf::from(copy_utf8(options.fonts_dir)?),
        resource_pack_dir: PathBuf::from(copy_utf8(options.resource_pack_dir)?),
        ffmpeg_path: PathBuf::from(copy_utf8(options.ffmpeg_path)?),
        temp_dir: PathBuf::from(copy_utf8(options.temp_dir)?),
        renderer_host_path: PathBuf::from(copy_utf8(options.renderer_host_path)?),
    };

    roots
        .validate()
        .map_err(|_| crate::PHI_STATUS_INVALID_CONFIG)?;
    Ok(roots)
}

#[allow(dead_code)]
impl phi_context {
    pub(crate) fn set_error(&self, message: impl Into<String>) {
        if let Ok(mut error) = self.last_error.lock() {
            *error = message.into();
        }
    }

    pub(crate) fn resource_roots(&self) -> &ResourceRoots {
        &self.resource_roots
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

        let resource_roots = match resource_roots_from_ffi(options) {
            Ok(options) => options,
            Err(status) => return status,
        };

        let context = Box::new(phi_context {
            resource_roots,
            last_error: Mutex::new(String::new()),
            active_job: Mutex::new(None),
        });
        *out_context = Box::into_raw(context);
        PHI_STATUS_OK
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::phi_string_view_t;

    fn view(value: &'static str) -> phi_string_view_t {
        phi_string_view_t {
            data: value.as_ptr(),
            length: value.len(),
        }
    }

    fn options() -> phi_context_options_t {
        phi_context_options_t {
            struct_size: std::mem::size_of::<phi_context_options_t>() as u32,
            abi_version: crate::PHI_ABI_VERSION,
            assets_dir: view("assets"),
            fonts_dir: view("fonts"),
            resource_pack_dir: view("respacks"),
            ffmpeg_path: view("ffmpeg"),
            temp_dir: view("temp"),
            renderer_host_path: view("renderer-host"),
        }
    }

    #[test]
    fn context_owns_explicit_resource_roots() {
        let options = options();
        let roots = unsafe { resource_roots_from_ffi(&options) }.unwrap();

        assert_eq!(roots.assets_dir, PathBuf::from("assets"));
        assert_eq!(roots.ffmpeg_path, PathBuf::from("ffmpeg"));
    }

    #[test]
    fn context_rejects_missing_resource_roots() {
        let mut options = options();
        options.ffmpeg_path = phi_string_view_t::empty();

        assert_eq!(
            unsafe { resource_roots_from_ffi(&options) },
            Err(crate::PHI_STATUS_INVALID_CONFIG)
        );
    }
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
