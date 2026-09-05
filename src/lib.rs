#![allow(non_camel_case_types)]

mod abi;
mod config;
mod context;

pub use abi::*;
pub use context::phi_context;

#[no_mangle]
pub extern "C" fn phi_abi_version() -> u32 {
    PHI_ABI_VERSION
}

#[no_mangle]
pub unsafe extern "C" fn phi_render_config_init_default(
    config: *mut phi_render_config_t,
) -> phi_status_t {
    ffi_status(|| {
        if config.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }

        *config = phi_render_config_t::defaults();
        PHI_STATUS_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_render_config_validate(
    config: *const phi_render_config_t,
) -> phi_status_t {
    ffi_status(|| {
        if config.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }

        let config = &*config;
        validate_header(
            config.struct_size,
            config.abi_version,
            std::mem::size_of::<phi_render_config_t>(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_the_current_abi_header() {
        let config = phi_render_config_t::defaults();

        assert_eq!(
            config.struct_size as usize,
            std::mem::size_of::<phi_render_config_t>()
        );
        assert_eq!(config.abi_version, PHI_ABI_VERSION);
        assert_eq!(config.resolution.width, 1920);
        assert_eq!(config.resolution.height, 1080);
        assert_eq!(config.fps, 60);
        assert_eq!(config.has_play_end_time, 0);
    }

    #[test]
    fn default_string_views_are_valid_for_the_library_lifetime() {
        let config = phi_render_config_t::defaults();

        let bitrate =
            unsafe { std::slice::from_raw_parts(config.bitrate.data, config.bitrate.length) };
        let player_name = unsafe {
            std::slice::from_raw_parts(config.player_name.data, config.player_name.length)
        };

        assert_eq!(bitrate, b"28");
        assert_eq!(player_name, b"HLMC");
    }
}
