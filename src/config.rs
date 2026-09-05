use crate::abi::{
    copy_utf8, phi_render_config_t, phi_string_view, phi_string_view_t, PHI_ABI_VERSION,
    PHI_AUDIO_MIX_MODE_OPTIMIZED, PHI_CHALLENGE_COLOR_RAINBOW, PHI_STATUS_INVALID_CONFIG,
};

use phi_recorder_core::{AudioMixMode, ChallengeColor, RenderConfig, Resolution, ValidationError};

fn map_validation_error(_error: ValidationError) -> crate::phi_status_t {
    PHI_STATUS_INVALID_CONFIG
}

unsafe fn optional_string(value: phi_string_view_t) -> Result<Option<String>, crate::phi_status_t> {
    let value = copy_utf8(value)?;
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

unsafe fn optional_path(
    value: phi_string_view_t,
) -> Result<Option<std::path::PathBuf>, crate::phi_status_t> {
    Ok(optional_string(value)?.map(std::path::PathBuf::from))
}

impl phi_render_config_t {
    pub(crate) fn defaults() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: PHI_ABI_VERSION,
            resolution: crate::phi_resolution_t {
                width: 1920,
                height: 1080,
            },
            ending_length: 0.0,
            render_loading: 0,
            hires: 0,
            chart_debug_line: 0.0,
            chart_debug_note: 0.0,
            chart_ratio: 1.0,
            all_good: 0,
            all_bad: 0,
            fps: 60,
            hardware_accel: 1,
            hevc: 0,
            mpeg4: 0,
            custom_encoder: phi_string_view_t::empty(),
            dynamic_bitrate_control: 1,
            bitrate: phi_string_view("28"),
            aggressive_chart: 1,
            aggressive_note: 0,
            aggressive_particle: 0,
            challenge_color: PHI_CHALLENGE_COLOR_RAINBOW,
            challenge_rank: 3,
            note_scale: 1.0,
            particle: 1,
            player_avatar: phi_string_view_t::empty(),
            player_name: phi_string_view("HLMC"),
            player_rks: 16.0,
            sample_count: 8,
            fxaa: 0,
            resource_pack_path: phi_string_view_t::empty(),
            speed: 1.0,
            volume_music: 0.5,
            volume_sfx: 0.4,
            force_limit: 1,
            limit_threshold: 0.5,
            loudness_equalization: 0,
            audio_mix_mode: PHI_AUDIO_MIX_MODE_OPTIMIZED,
            watermark: phi_string_view_t::empty(),
            roman: 0,
            chinese: 0,
            combo: phi_string_view("AUTOPLAY"),
            difficulty: phi_string_view_t::empty(),
            judge_offset: 0.0,
            file_name_format: phi_string_view("%date% %time% %info.name%_%level_prefix%"),
            render_line: 1,
            render_line_extra: 1,
            render_note: 1,
            render_double_hint: 1,
            render_ui_pause: 1,
            render_ui_name: 1,
            render_ui_level: 1,
            render_ui_score: 1,
            render_ui_combo: 1,
            render_ui_bar: 1,
            render_bg: 1,
            render_bg_dim: 1,
            preserve_framebuffer: 0,
            render_extra: 1,
            background_blurriness: 80.0,
            max_particles: 5000,
            play_start_time: 0.0,
            play_end_time: 0.0,
            has_play_end_time: 0,
            fade: 0.0,
            alpha_tint: 0,
        }
    }

    pub(crate) unsafe fn to_core(&self) -> Result<RenderConfig, crate::phi_status_t> {
        let core = RenderConfig {
            resolution: Resolution {
                width: self.resolution.width,
                height: self.resolution.height,
            },
            ending_length: self.ending_length,
            render_loading: self.render_loading != 0,
            hires: self.hires != 0,
            chart_debug_line: self.chart_debug_line,
            chart_debug_note: self.chart_debug_note,
            chart_ratio: self.chart_ratio,
            all_good: self.all_good != 0,
            all_bad: self.all_bad != 0,
            fps: self.fps,
            hardware_accel: self.hardware_accel != 0,
            hevc: self.hevc != 0,
            mpeg4: self.mpeg4 != 0,
            custom_encoder: optional_string(self.custom_encoder)?,
            dynamic_bitrate_control: self.dynamic_bitrate_control != 0,
            bitrate: copy_utf8(self.bitrate)?,
            aggressive_chart: self.aggressive_chart != 0,
            aggressive_note: self.aggressive_note != 0,
            aggressive_particle: self.aggressive_particle != 0,
            challenge_color: ChallengeColor::try_from(self.challenge_color)
                .map_err(map_validation_error)?,
            challenge_rank: self.challenge_rank,
            note_scale: self.note_scale,
            particle: self.particle != 0,
            player_avatar: optional_path(self.player_avatar)?,
            player_name: copy_utf8(self.player_name)?,
            player_rks: self.player_rks,
            sample_count: self.sample_count,
            fxaa: self.fxaa != 0,
            resource_pack_path: optional_path(self.resource_pack_path)?,
            speed: self.speed,
            volume_music: self.volume_music,
            volume_sfx: self.volume_sfx,
            force_limit: self.force_limit != 0,
            limit_threshold: self.limit_threshold,
            loudness_equalization: self.loudness_equalization != 0,
            audio_mix_mode: AudioMixMode::try_from(self.audio_mix_mode)
                .map_err(map_validation_error)?,
            watermark: copy_utf8(self.watermark)?,
            roman: self.roman != 0,
            chinese: self.chinese != 0,
            combo: copy_utf8(self.combo)?,
            difficulty: copy_utf8(self.difficulty)?,
            judge_offset: self.judge_offset,
            file_name_format: copy_utf8(self.file_name_format)?,
            render_line: self.render_line != 0,
            render_line_extra: self.render_line_extra != 0,
            render_note: self.render_note != 0,
            render_double_hint: self.render_double_hint != 0,
            render_ui_pause: self.render_ui_pause != 0,
            render_ui_name: self.render_ui_name != 0,
            render_ui_level: self.render_ui_level != 0,
            render_ui_score: self.render_ui_score != 0,
            render_ui_combo: self.render_ui_combo != 0,
            render_ui_bar: self.render_ui_bar != 0,
            render_bg: self.render_bg != 0,
            render_bg_dim: self.render_bg_dim != 0,
            preserve_framebuffer: self.preserve_framebuffer != 0,
            render_extra: self.render_extra != 0,
            background_blurriness: self.background_blurriness,
            max_particles: self.max_particles,
            play_start_time: self.play_start_time,
            play_end_time: if self.has_play_end_time != 0 {
                Some(self.play_end_time)
            } else {
                None
            },
            fade: self.fade,
            alpha_tint: self.alpha_tint != 0,
        };

        core.validate().map_err(map_validation_error)?;
        Ok(core)
    }
}
