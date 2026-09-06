use std::{
    io::Write,
    path::Path,
};

use anyhow::{Context, Result};
use phire::core::ResourcePack;
use phi_recorder_core::{
    build_audio_filter, build_output_args, calculate_audio_layout, RenderConfig, RenderTimeline,
};
use sasa::AudioClip;

use crate::ffmpeg_writer::AudioInput;

pub struct AudioPlan {
    pub inputs: Vec<AudioInput>,
    pub output_args: Vec<String>,
    _temp_files: Vec<tempfile::NamedTempFile>,
}

impl AudioPlan {
    pub fn inputs(&self) -> &[AudioInput] {
        &self.inputs
    }

    pub fn output_args(&self) -> &[String] {
        &self.output_args
    }
}

pub fn build_audio_plan(
    config: &RenderConfig,
    chart: &phire::core::Chart,
    music: &AudioClip,
    resource_pack: &ResourcePack,
    timeline: &RenderTimeline,
    temp_dir: &Path,
) -> Result<AudioPlan> {
    std::fs::create_dir_all(temp_dir)
        .with_context(|| format!("create temp dir {}", temp_dir.display()))?;

    let sfx_protect_time = chart
        .hitsounds
        .values()
        .map(|clip| clip.length())
        .chain([
            resource_pack.sfx_click.length(),
            resource_pack.sfx_drag.length(),
            resource_pack.sfx_flick.length(),
        ])
        .max_by(|left, right| left.total_cmp(right))
        .unwrap_or(0.0);
    let layout = calculate_audio_layout(*timeline, music.sample_rate(), sfx_protect_time)
        .map_err(|error| anyhow::anyhow!(error))?;

    let output_music = if config.volume_music != 0.0 {
        mix_music_pcm(
            &music.to_vec(),
            music.sample_rate(),
            config.speed,
            timeline.offset,
            timeline.before_time,
            config.play_start_time,
            timeline.chart_length_music,
            layout.music_samples,
        )
    } else {
        vec![0.0f32; layout.music_samples]
    };

    let output_ending = if config.volume_music != 0.0 {
        tile_ending_pcm(&resource_pack.endings[0].to_vec(), layout.ending_music_samples)
    } else {
        vec![0.0f32; layout.ending_music_samples]
    };

    let output_sfx = vec![0.0f32; layout.sfx_samples];

    let mut temp_files = Vec::new();
    let mut write_input =
        |prefix: &str, samples: &[f32], sample_rate: u32| -> Result<AudioInput> {
            let mut file = tempfile::Builder::new()
                .prefix(prefix)
                .suffix(".f32le")
                .tempfile_in(temp_dir)
                .with_context(|| format!("create {prefix} temp file in {}", temp_dir.display()))?;
            let bytes = unsafe {
                std::slice::from_raw_parts(samples.as_ptr().cast::<u8>(), samples.len() * 4)
            };
            file.write_all(bytes).context("write PCM temp file")?;
            let path = file.path().to_owned();
            temp_files.push(file);
            Ok(AudioInput { path, sample_rate })
        };

    let sfx_input = write_input("sfx", &output_sfx, 48_000)?;
    let music_input = write_input("music", &output_music, music.sample_rate())?;
    let ending_input = write_input("ending", &output_ending, 48_000)?;

    let filter = build_audio_filter(
        config.volume_music,
        config.volume_sfx,
        config.speed,
        config.loudness_equalization,
        config.force_limit,
        config.limit_threshold,
        layout.ending_delay_millis,
        config.hires,
    );
    let bitrate_control = if config.dynamic_bitrate_control {
        "-crf"
    } else {
        "-b:v"
    };
    let output_args = build_output_args(
        "libx264",
        bitrate_control,
        &config.bitrate,
        &filter.filter_complex,
        config.hires,
    );

    Ok(AudioPlan {
        inputs: vec![sfx_input, music_input, ending_input],
        output_args,
        _temp_files: temp_files,
    })
}

fn mix_music_pcm(
    music: &[f32],
    music_sample_rate: u32,
    speed: f32,
    offset: f64,
    before_time: f64,
    play_start_time: f64,
    chart_length_music: f64,
    output_len: usize,
) -> Vec<f32> {
    let mut output = vec![0.0f32; output_len];
    let position_write =
        ((before_time - offset.min(0.0)) * speed as f64 * music_sample_rate as f64).ceil() as usize * 2;
    let position_read =
        ((offset.max(0.0) + play_start_time) * music_sample_rate as f64).ceil() as usize * 2;
    let music_len = (chart_length_music * music_sample_rate as f64).ceil() as usize * 2;
    let len = music
        .len()
        .saturating_sub(position_read)
        .min(output_len.saturating_sub(position_write))
        .min(music_len.saturating_sub(position_write));
    output[position_write..position_write + len]
        .copy_from_slice(&music[position_read..position_read + len]);
    output
}

fn tile_ending_pcm(ending: &[f32], output_len: usize) -> Vec<f32> {
    let mut output = vec![0.0f32; output_len];
    if ending.is_empty() {
        return output;
    }
    let mut position_write = 0;
    while position_write < output_len {
        let len = ending.len().min(output_len - position_write);
        output[position_write..position_write + len].copy_from_slice(&ending[..len]);
        position_write += len;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_mix_respects_offset_and_silence_bounds() {
        let music = [0.0f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let output = mix_music_pcm(&music, 2, 1.0, 1.0, 0.0, 0.0, 1.0, 8);
        assert_eq!(&output[..4], &[4.0, 5.0, 6.0, 7.0]);
        assert_eq!(&output[4..8], &[0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn ending_tiles_until_output_is_full() {
        let output = tile_ending_pcm(&[1.0, 2.0], 10);
        assert_eq!(output, vec![1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0]);
    }
}
