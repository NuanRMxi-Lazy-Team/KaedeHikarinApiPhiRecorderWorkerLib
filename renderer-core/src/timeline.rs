use crate::{RenderConfig, ValidationError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimingConstants {
    pub loading_total_time: f64,
    pub before_duration: f64,
    pub wait_time: f64,
    pub wait_after_time: f64,
    pub ending_bpm_wait_time: f64,
}

impl TimingConstants {
    pub fn phire_defaults() -> Self {
        Self {
            loading_total_time: phire::scene::LoadingScene::TOTAL_TIME,
            before_duration: phire::scene::GameScene::BEFORE_DURATION,
            wait_time: phire::scene::game::WAIT_TIME,
            wait_after_time: phire::scene::GameScene::WAIT_AFTER_TIME,
            ending_bpm_wait_time: phire::scene::EndingScene::BPM_WAIT_TIME,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderTimeline {
    pub offset: f64,
    pub speed_time_ratio: f64,
    pub before_time: f64,
    pub before_time_music: f64,
    pub chart_length: f64,
    pub chart_length_music: f64,
    pub chart_length_sfx: f64,
    pub video_length: f64,
    pub video_length_music: f64,
    pub video_frames: u64,
    pub ending_music_delay: f64,
}

pub fn calculate_timeline(
    config: &RenderConfig,
    chart_offset: f64,
    info_offset: f64,
    music_length: f64,
    constants: TimingConstants,
) -> Result<RenderTimeline, ValidationError> {
    if !music_length.is_finite() || music_length < 0.0 {
        return Err(ValidationError::NonNegative("music_length"));
    }
    config.validate()?;

    let offset = chart_offset + info_offset;
    let speed_time_ratio = 1.0 / config.speed as f64;
    let before_time = if config.render_loading {
        constants.loading_total_time + constants.before_duration * speed_time_ratio
    } else {
        0.0
    };
    let before_time_music = if config.render_loading {
        constants.loading_total_time * config.speed as f64 + constants.before_duration
    } else {
        0.0
    };
    let end_time = config
        .play_end_time
        .unwrap_or(music_length)
        .min(music_length);
    let chart_length = before_time + end_time * speed_time_ratio
        - config.play_start_time * speed_time_ratio
        - offset
        + constants.wait_time * speed_time_ratio;
    let chart_length_music =
        before_time_music + end_time - config.play_start_time - offset + constants.wait_time;
    let chart_length_sfx = end_time - config.play_start_time - offset + constants.wait_time;
    let video_length = chart_length + config.ending_length;
    let video_length_music = chart_length_music + config.ending_length;
    if !video_length.is_finite() || video_length <= 0.0 {
        return Err(ValidationError::Positive("video_length"));
    }
    let video_frames = (video_length * config.fps as f64).ceil() as u64;
    if video_frames == 0 {
        return Err(ValidationError::Positive("video_frames"));
    }
    let ending_music_delay = chart_length
        + constants.wait_after_time * speed_time_ratio
        + constants.ending_bpm_wait_time;

    Ok(RenderTimeline {
        offset,
        speed_time_ratio,
        before_time,
        before_time_music,
        chart_length,
        chart_length_music,
        chart_length_sfx,
        video_length,
        video_length_music,
        video_frames,
        ending_music_delay,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioLayout {
    pub music_samples: usize,
    pub sfx_samples: usize,
    pub ending_music_samples: usize,
    pub ending_delay_millis: f64,
}

pub fn calculate_audio_layout(
    timeline: RenderTimeline,
    music_sample_rate: u32,
    sfx_protect_time: f64,
) -> Result<AudioLayout, ValidationError> {
    if music_sample_rate == 0 {
        return Err(ValidationError::NonPositive("music_sample_rate"));
    }
    if !sfx_protect_time.is_finite() || sfx_protect_time < 0.0 {
        return Err(ValidationError::NonNegative("sfx_protect_time"));
    }

    let music_samples =
        (timeline.video_length_music * music_sample_rate as f64).ceil() as usize * 2;
    let sfx_samples = ((timeline.video_length + sfx_protect_time) * 48_000.0).ceil() as usize * 2;
    let ending_music_samples =
        ((timeline.video_length - timeline.ending_music_delay).max(0.0) * 48_000.0).ceil() as usize
            * 2;

    Ok(AudioLayout {
        music_samples,
        sfx_samples,
        ending_music_samples,
        ending_delay_millis: timeline.ending_music_delay * 1000.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constants() -> TimingConstants {
        TimingConstants {
            loading_total_time: 1.0,
            before_duration: 1.2,
            wait_time: 0.5,
            wait_after_time: 0.8,
            ending_bpm_wait_time: 0.2,
        }
    }

    #[test]
    fn timeline_keeps_speed_and_offsets_separate_for_music_and_video() {
        let mut config = RenderConfig::default();
        config.speed = 2.0;
        config.play_end_time = Some(10.0);

        let timeline = calculate_timeline(&config, 0.25, -0.1, 12.0, constants()).unwrap();

        assert_eq!(timeline.offset, 0.15);
        assert_eq!(timeline.speed_time_ratio, 0.5);
        assert!(timeline.chart_length_music > timeline.chart_length);
        assert!(timeline.video_frames > 0);
    }

    #[test]
    fn audio_layout_uses_music_rate_but_fixed_sfx_rate() {
        let config = RenderConfig::default();
        let timeline = calculate_timeline(&config, 0.0, 0.0, 4.0, constants()).unwrap();
        let layout = calculate_audio_layout(timeline, 44_100, 0.25).unwrap();

        assert_eq!(layout.music_samples % 2, 0);
        assert_eq!(layout.sfx_samples % 2, 0);
        assert_eq!(layout.ending_music_samples % 2, 0);
    }
}
