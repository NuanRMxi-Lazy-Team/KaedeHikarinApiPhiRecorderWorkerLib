#![allow(non_camel_case_types)]

mod abi;
mod chart;
mod config;
mod context;
mod host;
mod job;

pub use abi::*;
pub use chart::phi_chart_info;
pub use context::phi_context;
pub use host::RendererHost;
pub use job::phi_job;

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
        let status = validate_header(
            config.struct_size,
            config.abi_version,
            std::mem::size_of::<phi_render_config_t>(),
        );
        if status != PHI_STATUS_OK {
            return status;
        }

        match config.to_core() {
            Ok(_) => PHI_STATUS_OK,
            Err(status) => status,
        }
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

    #[test]
    fn default_config_converts_to_core_config() {
        let config = phi_render_config_t::defaults();

        assert!(unsafe { config.to_core() }.is_ok());
    }

    #[test]
    fn invalid_enum_is_rejected_before_rendering() {
        let mut config = phi_render_config_t::defaults();
        config.audio_mix_mode = 99;

        assert_eq!(unsafe { config.to_core() }, Err(PHI_STATUS_INVALID_CONFIG));
    }

    #[test]
    fn renderer_host_handshake_smoke_when_binary_exists() {
        let suffix = if cfg!(windows) { ".exe" } else { "" };
        let host_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug")
            .join(format!("phi-renderer-host{suffix}"));
        if !host_path.exists() {
            return;
        }
        host::check_protocol(&host_path).unwrap();
    }

    #[test]
    fn render_submit_reaches_terminal_state_when_host_exists() {
        use crate::context::{phi_context_create, phi_context_destroy};
        use crate::job::{phi_job_destroy, phi_job_get_snapshot, phi_render_submit};

        let suffix = if cfg!(windows) { ".exe" } else { "" };
        let host_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug")
            .join(format!("phi-renderer-host{suffix}"));
        if !host_path.exists() {
            return;
        }

        unsafe fn view(value: &str) -> phi_string_view_t {
            phi_string_view_t {
                data: value.as_ptr(),
                length: value.len(),
            }
        }

        unsafe {
            let options = phi_context_options_t {
                struct_size: std::mem::size_of::<phi_context_options_t>() as u32,
                abi_version: PHI_ABI_VERSION,
                assets_dir: view("assets"),
                fonts_dir: view("assets"),
                resource_pack_dir: view("respacks"),
                ffmpeg_path: view("ffmpeg"),
                temp_dir: view("temp"),
                renderer_host_path: view(&host_path.to_string_lossy()),
            };
            let mut context: *mut phi_context = std::ptr::null_mut();
            assert_eq!(phi_context_create(&options, &mut context), PHI_STATUS_OK);

            let config = phi_render_config_t::defaults();
            let request = phi_render_request_t {
                struct_size: std::mem::size_of::<phi_render_request_t>() as u32,
                abi_version: PHI_ABI_VERSION,
                chart_path: view("missing.pez"),
                output_path: view("out.mp4"),
                config: &config as *const phi_render_config_t,
                chart_info: std::ptr::null(),
            };
            let mut job: *mut phi_job = std::ptr::null_mut();
            assert_eq!(
                phi_render_submit(context, &request, None, std::ptr::null_mut(), &mut job),
                PHI_STATUS_OK
            );

            let mut terminal = false;
            for _ in 0..200 {
                let mut snapshot = std::mem::MaybeUninit::<phi_job_snapshot_t>::uninit();
                assert_eq!(
                    phi_job_get_snapshot(job, snapshot.as_mut_ptr()),
                    PHI_STATUS_OK
                );
                let snapshot = snapshot.assume_init();
                if matches!(
                    snapshot.state,
                    phi_job_state_t::Done | phi_job_state_t::Canceled | phi_job_state_t::Failed
                ) {
                    terminal = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            assert!(terminal, "render job did not reach a terminal state");

            phi_job_destroy(job);
            phi_context_destroy(context);
        }
    }

    #[test]
    fn end_to_end_render_produces_video_when_fixture_and_host_exist() {
        use crate::context::{phi_context_create, phi_context_destroy};
        use crate::job::{phi_job_destroy, phi_job_get_snapshot, phi_render_submit};

        let suffix = if cfg!(windows) { ".exe" } else { "" };
        let host_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug")
            .join(format!("phi-renderer-host{suffix}"));
        if !host_path.exists() {
            return;
        }
        let ffmpeg = std::env::var_os("PHI_FFMPEG_PATH")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let candidate = if cfg!(windows) {
                    "ffmpeg.exe"
                } else {
                    "ffmpeg"
                };
                std::env::split_paths(&std::env::var_os("PATH")?)
                    .map(|dir| dir.join(candidate))
                    .find(|path| path.is_file())
            });
        let Some(ffmpeg) = ffmpeg else {
            return;
        };
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        if !assets.exists() {
            return;
        }

        let fixture = tempfile::tempdir().unwrap();
        std::fs::copy(assets.join("click.ogg"), fixture.path().join("click.ogg")).unwrap();
        std::fs::copy(assets.join("back.png"), fixture.path().join("back.png")).unwrap();
        std::fs::write(
            fixture.path().join("info.txt"),
            "#\nName: Smoke\nLevel: Smoke 1\nChart: chart.json\nMusic: click.ogg\nPicture: back.png\nIllustrator: Smoke\nArtist: Smoke\nCharter: Smoke\n",
        )
        .unwrap();
        std::fs::write(
            fixture.path().join("chart.json"),
            r#"{"META":{"RPEVersion":1,"offset":0},"BPMList":[{"bpm":120,"startTime":[0,0,1]}],"judgeLineList":[]}"#,
        )
        .unwrap();
        let output = fixture.path().join("out.mp4");

        unsafe fn view(value: &str) -> phi_string_view_t {
            phi_string_view_t {
                data: value.as_ptr(),
                length: value.len(),
            }
        }

        unsafe {
            let assets_dir = assets.to_string_lossy().into_owned();
            let resource_pack_dir = assets.join("respack").to_string_lossy().into_owned();
            let ffmpeg_path = ffmpeg.to_string_lossy().into_owned();
            let fixture_dir = fixture.path().to_string_lossy().into_owned();
            let host_dir = host_path.to_string_lossy().into_owned();
            let output_dir = output.to_string_lossy().into_owned();
            let options = phi_context_options_t {
                struct_size: std::mem::size_of::<phi_context_options_t>() as u32,
                abi_version: PHI_ABI_VERSION,
                assets_dir: view(&assets_dir),
                fonts_dir: view(&assets_dir),
                resource_pack_dir: view(&resource_pack_dir),
                ffmpeg_path: view(&ffmpeg_path),
                temp_dir: view(&fixture_dir),
                renderer_host_path: view(&host_dir),
            };
            let mut context: *mut phi_context = std::ptr::null_mut();
            assert_eq!(phi_context_create(&options, &mut context), PHI_STATUS_OK);

            let mut config = phi_render_config_t::defaults();
            config.resolution.width = 320;
            config.resolution.height = 180;
            config.fps = 30;
            config.play_end_time = 0.5;
            config.has_play_end_time = 1;
            let chart_path = fixture.path().to_string_lossy().into_owned();
            let request = phi_render_request_t {
                struct_size: std::mem::size_of::<phi_render_request_t>() as u32,
                abi_version: PHI_ABI_VERSION,
                chart_path: view(&chart_path),
                output_path: view(&output_dir),
                config: &config as *const phi_render_config_t,
                chart_info: std::ptr::null(),
            };
            let mut job: *mut phi_job = std::ptr::null_mut();
            assert_eq!(
                phi_render_submit(context, &request, None, std::ptr::null_mut(), &mut job),
                PHI_STATUS_OK
            );

            let mut terminal = false;
            let mut final_state = phi_job_state_t::Pending;
            for _ in 0..600 {
                let mut snapshot = std::mem::MaybeUninit::<phi_job_snapshot_t>::uninit();
                assert_eq!(
                    phi_job_get_snapshot(job, snapshot.as_mut_ptr()),
                    PHI_STATUS_OK
                );
                let snapshot = snapshot.assume_init();
                if matches!(
                    snapshot.state,
                    phi_job_state_t::Done | phi_job_state_t::Canceled | phi_job_state_t::Failed
                ) {
                    terminal = true;
                    final_state = snapshot.state;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            assert!(terminal, "render job did not reach a terminal state");
            assert_eq!(
                final_state as i32,
                phi_job_state_t::Done as i32,
                "render job did not finish"
            );
            let metadata = std::fs::metadata(&output).expect("render output was not produced");
            assert!(metadata.len() > 0, "render output is empty");

            phi_job_destroy(job);
            phi_context_destroy(context);
        }
    }
}
