use std::path::PathBuf;

use crate::ValidationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeColor {
    White,
    Green,
    Blue,
    Red,
    Golden,
    Rainbow,
}

impl TryFrom<i32> for ChallengeColor {
    type Error = ValidationError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::White),
            1 => Ok(Self::Green),
            2 => Ok(Self::Blue),
            3 => Ok(Self::Red),
            4 => Ok(Self::Golden),
            5 => Ok(Self::Rainbow),
            _ => Err(ValidationError::InvalidEnum {
                field: "challenge_color",
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMixMode {
    Traditional,
    Optimized,
    Fft,
}

impl TryFrom<i32> for AudioMixMode {
    type Error = ValidationError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Traditional),
            1 => Ok(Self::Optimized),
            2 => Ok(Self::Fft),
            _ => Err(ValidationError::InvalidEnum {
                field: "audio_mix_mode",
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderConfig {
    pub resolution: Resolution,
    pub ending_length: f64,
    pub render_loading: bool,
    pub hires: bool,
    pub chart_debug_line: f32,
    pub chart_debug_note: f32,
    pub chart_ratio: f32,
    pub all_good: bool,
    pub all_bad: bool,
    pub fps: u32,
    pub hardware_accel: bool,
    pub hevc: bool,
    pub mpeg4: bool,
    pub custom_encoder: Option<String>,
    pub dynamic_bitrate_control: bool,
    pub bitrate: String,

    pub aggressive_chart: bool,
    pub aggressive_note: bool,
    pub aggressive_particle: bool,
    pub challenge_color: ChallengeColor,
    pub challenge_rank: u32,
    pub note_scale: f32,
    pub particle: bool,
    pub player_avatar: Option<PathBuf>,
    pub player_name: String,
    pub player_rks: f32,
    pub sample_count: u32,
    pub fxaa: bool,
    pub resource_pack_path: Option<PathBuf>,
    pub speed: f32,
    pub volume_music: f32,
    pub volume_sfx: f32,
    pub force_limit: bool,
    pub limit_threshold: f32,
    pub loudness_equalization: bool,
    pub audio_mix_mode: AudioMixMode,
    pub watermark: String,
    pub roman: bool,
    pub chinese: bool,
    pub combo: String,
    pub difficulty: String,
    pub judge_offset: f64,
    pub file_name_format: String,

    pub render_line: bool,
    pub render_line_extra: bool,
    pub render_note: bool,
    pub render_double_hint: bool,
    pub render_ui_pause: bool,
    pub render_ui_name: bool,
    pub render_ui_level: bool,
    pub render_ui_score: bool,
    pub render_ui_combo: bool,
    pub render_ui_bar: bool,
    pub render_bg: bool,
    pub render_bg_dim: bool,
    pub preserve_framebuffer: bool,
    pub render_extra: bool,
    pub background_blurriness: f32,

    pub max_particles: u64,
    pub play_start_time: f64,
    pub play_end_time: Option<f64>,
    pub fade: f32,
    pub alpha_tint: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            resolution: Resolution {
                width: 1920,
                height: 1080,
            },
            ending_length: 0.0,
            render_loading: false,
            hires: false,
            chart_debug_line: 0.0,
            chart_debug_note: 0.0,
            chart_ratio: 1.0,
            all_good: false,
            all_bad: false,
            fps: 60,
            hardware_accel: true,
            hevc: false,
            mpeg4: false,
            custom_encoder: None,
            dynamic_bitrate_control: true,
            bitrate: "28".to_owned(),
            aggressive_chart: true,
            aggressive_note: false,
            aggressive_particle: false,
            challenge_color: ChallengeColor::Rainbow,
            challenge_rank: 3,
            note_scale: 1.0,
            particle: true,
            player_avatar: None,
            player_name: "HLMC".to_owned(),
            player_rks: 16.0,
            sample_count: 8,
            fxaa: false,
            resource_pack_path: None,
            speed: 1.0,
            volume_music: 0.5,
            volume_sfx: 0.4,
            force_limit: true,
            limit_threshold: 0.5,
            loudness_equalization: false,
            audio_mix_mode: AudioMixMode::Optimized,
            watermark: String::new(),
            roman: false,
            chinese: false,
            combo: "AUTOPLAY".to_owned(),
            difficulty: String::new(),
            judge_offset: 0.0,
            file_name_format: "%date% %time% %info.name%_%level_prefix%".to_owned(),
            render_line: true,
            render_line_extra: true,
            render_note: true,
            render_double_hint: true,
            render_ui_pause: true,
            render_ui_name: true,
            render_ui_level: true,
            render_ui_score: true,
            render_ui_combo: true,
            render_ui_bar: true,
            render_bg: true,
            render_bg_dim: true,
            preserve_framebuffer: false,
            render_extra: true,
            background_blurriness: 80.0,
            max_particles: 5000,
            play_start_time: 0.0,
            play_end_time: None,
            fade: 0.0,
            alpha_tint: false,
        }
    }
}

impl RenderConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.resolution.width == 0 || self.resolution.height == 0 {
            return Err(ValidationError::NonPositive("resolution"));
        }
        if self.resolution.width % 2 != 0 || self.resolution.height % 2 != 0 {
            return Err(ValidationError::EvenResolution);
        }
        if self.fps == 0 {
            return Err(ValidationError::NonPositive("fps"));
        }
        if !self.ending_length.is_finite() || self.ending_length < 0.0 {
            return Err(ValidationError::NonNegative("ending_length"));
        }
        if !self.speed.is_finite() || self.speed <= 0.0 {
            return Err(ValidationError::Positive("speed"));
        }
        if !self.play_start_time.is_finite() || self.play_start_time < 0.0 {
            return Err(ValidationError::NonNegative("play_start_time"));
        }
        if let Some(play_end_time) = self.play_end_time {
            if !play_end_time.is_finite() || play_end_time <= self.play_start_time {
                return Err(ValidationError::GreaterThan {
                    field: "play_end_time",
                    other: "play_start_time",
                });
            }
        }
        for (field, value) in [
            ("volume_music", self.volume_music),
            ("volume_sfx", self.volume_sfx),
            ("limit_threshold", self.limit_threshold),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(ValidationError::NonNegative(field));
            }
        }
        if self.max_particles == 0 {
            return Err(ValidationError::NonPositive("max_particles"));
        }

        Ok(())
    }
}
