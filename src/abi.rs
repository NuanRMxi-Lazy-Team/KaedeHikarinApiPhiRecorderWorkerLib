use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    ptr, slice, str,
};

pub const PHI_ABI_VERSION: u32 = 1;

pub type phi_status_t = i32;

pub const PHI_STATUS_OK: phi_status_t = 0;
pub const PHI_STATUS_INVALID_ARGUMENT: phi_status_t = 1;
pub const PHI_STATUS_ABI_MISMATCH: phi_status_t = 2;
pub const PHI_STATUS_BUFFER_TOO_SMALL: phi_status_t = 3;
pub const PHI_STATUS_INVALID_UTF8: phi_status_t = 4;
pub const PHI_STATUS_OUT_OF_MEMORY: phi_status_t = 5;
pub const PHI_STATUS_NOT_IMPLEMENTED: phi_status_t = 6;
pub const PHI_STATUS_BUSY: phi_status_t = 7;
pub const PHI_STATUS_GRAPHICS_UNAVAILABLE: phi_status_t = 8;
pub const PHI_STATUS_FFMPEG_UNAVAILABLE: phi_status_t = 9;
pub const PHI_STATUS_INVALID_STATE: phi_status_t = 10;
pub const PHI_STATUS_INTERNAL_ERROR: phi_status_t = 11;
pub const PHI_STATUS_PANIC: phi_status_t = 12;
pub const PHI_STATUS_CANCELED: phi_status_t = 13;
pub const PHI_STATUS_INVALID_CONFIG: phi_status_t = 14;

pub const PHI_CHALLENGE_COLOR_RAINBOW: i32 = 5;
pub const PHI_AUDIO_MIX_MODE_OPTIMIZED: i32 = 1;
pub const PHI_CHART_FORMAT_RPE: i32 = 0;
pub const PHI_CHART_FORMAT_PEC: i32 = 1;
pub const PHI_CHART_FORMAT_PGR: i32 = 2;
pub const PHI_CHART_FORMAT_PBC: i32 = 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct phi_string_view_t {
    pub data: *const u8,
    pub length: usize,
}

impl phi_string_view_t {
    pub(crate) const fn empty() -> Self {
        Self {
            data: ptr::null(),
            length: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct phi_struct_header_t {
    pub struct_size: u32,
    pub abi_version: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct phi_resolution_t {
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
pub struct phi_context_options_t {
    pub struct_size: u32,
    pub abi_version: u32,
    pub assets_dir: phi_string_view_t,
    pub fonts_dir: phi_string_view_t,
    pub resource_pack_dir: phi_string_view_t,
    pub ffmpeg_path: phi_string_view_t,
    pub temp_dir: phi_string_view_t,
    pub renderer_host_path: phi_string_view_t,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct phi_chart_info_view_t {
    pub struct_size: u32,
    pub abi_version: u32,

    pub id: i32,
    pub has_id: u8,
    pub guid: phi_string_view_t,
    pub has_guid: u8,
    pub uploader: i32,
    pub has_uploader: u8,

    pub name: phi_string_view_t,
    pub difficulty: f32,
    pub level: phi_string_view_t,
    pub charter: phi_string_view_t,
    pub composer: phi_string_view_t,
    pub illustrator: phi_string_view_t,
    pub chart: phi_string_view_t,
    pub format: i32,
    pub has_format: u8,
    pub music: phi_string_view_t,
    pub illustration: phi_string_view_t,
    pub unlock_video: phi_string_view_t,
    pub has_unlock_video: u8,

    pub preview_start: f64,
    pub preview_end: f64,
    pub has_preview_end: u8,
    pub aspect_ratio: f32,
    pub force_aspect_ratio: u8,
    pub background_dim: f32,
    pub line_length: f32,
    pub offset: f64,
    pub tip: phi_string_view_t,
    pub has_tip: u8,
    pub tags: *const phi_string_view_t,
    pub tag_count: usize,

    pub intro: phi_string_view_t,
    pub hold_partial_cover: u8,
    pub negative_length_hold: u8,
    pub note_uniform_scale: u8,
    pub score_total: u32,
    pub hold_particle_interval_ratio: f32,
    pub fold_animation: u8,

    pub created: phi_string_view_t,
    pub has_created: u8,
    pub updated: phi_string_view_t,
    pub has_updated: u8,
    pub chart_updated: phi_string_view_t,
    pub has_chart_updated: u8,
}

#[repr(C)]
pub struct phi_render_request_t {
    pub struct_size: u32,
    pub abi_version: u32,
    pub chart_path: phi_string_view_t,
    pub output_path: phi_string_view_t,
    pub config: *const phi_render_config_t,
    pub chart_info: *const crate::chart::phi_chart_info,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum phi_job_state_t {
    Pending = 0,
    Loading = 1,
    Mixing = 2,
    Rendering = 3,
    Paused = 4,
    Done = 5,
    Canceled = 6,
    Failed = 7,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct phi_job_snapshot_t {
    pub struct_size: u32,
    pub abi_version: u32,
    pub job_id: u64,
    pub state: phi_job_state_t,
    pub progress: f64,
    pub fps: f64,
    pub estimated_seconds: f64,
    pub duration_seconds: f64,
    pub frame: u64,
    pub total_frames: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct phi_job_event_t {
    pub struct_size: u32,
    pub abi_version: u32,
    pub job_id: u64,
    pub state: phi_job_state_t,
    pub progress: f64,
    pub fps: f64,
    pub estimated_seconds: f64,
    pub duration_seconds: f64,
    pub message: phi_string_view_t,
}

pub type phi_job_callback_fn =
    unsafe extern "C" fn(event: *const phi_job_event_t, user_data: *mut std::ffi::c_void);

#[repr(C)]
pub struct phi_render_config_t {
    pub struct_size: u32,
    pub abi_version: u32,

    pub resolution: phi_resolution_t,
    pub ending_length: f64,
    pub render_loading: u8,
    pub hires: u8,
    pub chart_debug_line: f32,
    pub chart_debug_note: f32,
    pub chart_ratio: f32,
    pub all_good: u8,
    pub all_bad: u8,
    pub fps: u32,
    pub hardware_accel: u8,
    pub hevc: u8,
    pub mpeg4: u8,
    pub custom_encoder: phi_string_view_t,
    pub dynamic_bitrate_control: u8,
    pub bitrate: phi_string_view_t,

    pub aggressive_chart: u8,
    pub aggressive_note: u8,
    pub aggressive_particle: u8,
    pub challenge_color: i32,
    pub challenge_rank: u32,
    pub note_scale: f32,
    pub particle: u8,
    pub player_avatar: phi_string_view_t,
    pub player_name: phi_string_view_t,
    pub player_rks: f32,
    pub sample_count: u32,
    pub fxaa: u8,
    pub resource_pack_path: phi_string_view_t,
    pub speed: f32,
    pub volume_music: f32,
    pub volume_sfx: f32,
    pub force_limit: u8,
    pub limit_threshold: f32,
    pub loudness_equalization: u8,
    pub audio_mix_mode: i32,
    pub watermark: phi_string_view_t,
    pub roman: u8,
    pub chinese: u8,
    pub combo: phi_string_view_t,
    pub difficulty: phi_string_view_t,
    pub judge_offset: f64,
    pub file_name_format: phi_string_view_t,

    pub render_line: u8,
    pub render_line_extra: u8,
    pub render_note: u8,
    pub render_double_hint: u8,
    pub render_ui_pause: u8,
    pub render_ui_name: u8,
    pub render_ui_level: u8,
    pub render_ui_score: u8,
    pub render_ui_combo: u8,
    pub render_ui_bar: u8,
    pub render_bg: u8,
    pub render_bg_dim: u8,
    pub preserve_framebuffer: u8,
    pub render_extra: u8,
    pub background_blurriness: f32,

    pub max_particles: u64,
    pub play_start_time: f64,
    pub play_end_time: f64,
    pub has_play_end_time: u8,
    pub fade: f32,
    pub alpha_tint: u8,
}

pub(crate) const fn phi_string_view(value: &'static str) -> phi_string_view_t {
    phi_string_view_t {
        data: value.as_ptr(),
        length: value.len(),
    }
}

pub(crate) fn validate_header(
    struct_size: u32,
    abi_version: u32,
    expected_size: usize,
) -> phi_status_t {
    if abi_version != PHI_ABI_VERSION || (struct_size as usize) < expected_size {
        PHI_STATUS_ABI_MISMATCH
    } else {
        PHI_STATUS_OK
    }
}

pub(crate) fn ffi_status(function: impl FnOnce() -> phi_status_t) -> phi_status_t {
    match catch_unwind(AssertUnwindSafe(function)) {
        Ok(status) => status,
        Err(_) => PHI_STATUS_PANIC,
    }
}

pub(crate) unsafe fn copy_utf8(value: phi_string_view_t) -> Result<String, phi_status_t> {
    if value.length == 0 {
        return Ok(String::new());
    }
    if value.data.is_null() {
        return Err(PHI_STATUS_INVALID_ARGUMENT);
    }

    let bytes = slice::from_raw_parts(value.data, value.length);
    str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| PHI_STATUS_INVALID_UTF8)
}

pub(crate) unsafe fn write_bytes(
    value: &[u8],
    buffer: *mut u8,
    capacity: usize,
    required: *mut usize,
) -> phi_status_t {
    if required.is_null() {
        return PHI_STATUS_INVALID_ARGUMENT;
    }

    *required = value.len();
    if value.is_empty() {
        return PHI_STATUS_OK;
    }
    if buffer.is_null() {
        return PHI_STATUS_INVALID_ARGUMENT;
    }
    if capacity < value.len() {
        return PHI_STATUS_BUFFER_TOO_SMALL;
    }

    ptr::copy_nonoverlapping(value.as_ptr(), buffer, value.len());
    PHI_STATUS_OK
}

const _: () = {
    let _string_view_size: [(); std::mem::size_of::<usize>() * 2] =
        [(); std::mem::size_of::<phi_string_view_t>()];
    let _header_size: [(); 8] = [(); std::mem::size_of::<phi_struct_header_t>()];
    let _resolution_size: [(); 8] = [(); std::mem::size_of::<phi_resolution_t>()];
    let _context_options_size: [(); std::mem::size_of::<phi_struct_header_t>()
        + std::mem::size_of::<phi_string_view_t>() * 6] =
        [(); std::mem::size_of::<phi_context_options_t>()];
};
