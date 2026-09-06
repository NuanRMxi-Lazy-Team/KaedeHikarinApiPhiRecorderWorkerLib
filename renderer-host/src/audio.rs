use std::{io::Write, path::Path};

use anyhow::{Context, Result};
use num_complex::Complex;
use phi_recorder_core::{
    build_audio_filter, build_output_args, calculate_audio_layout, AudioMixMode, RenderConfig,
    RenderTimeline,
};
use phire::core::ResourcePack;
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
        tile_ending_pcm(
            &resource_pack.endings[0].to_vec(),
            layout.ending_music_samples,
        )
    } else {
        vec![0.0f32; layout.ending_music_samples]
    };

    let output_sfx = mix_sfx(config, chart, resource_pack, timeline, layout.sfx_samples)?;

    let mut temp_files = Vec::new();
    let mut write_input = |prefix: &str, samples: &[f32], sample_rate: u32| -> Result<AudioInput> {
        let mut file = tempfile::Builder::new()
            .prefix(prefix)
            .suffix(".f32le")
            .tempfile_in(temp_dir)
            .with_context(|| format!("create {prefix} temp file in {}", temp_dir.display()))?;
        let bytes =
            unsafe { std::slice::from_raw_parts(samples.as_ptr().cast::<u8>(), samples.len() * 4) };
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
    let position_write = ((before_time - offset.min(0.0)) * speed as f64 * music_sample_rate as f64)
        .ceil() as usize
        * 2;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SfxSound {
    Click,
    Drag,
    Flick,
    Custom(usize),
}

impl SfxSound {
    fn sort_key(self) -> usize {
        match self {
            Self::Click => 0,
            Self::Drag => 1,
            Self::Flick => 2,
            Self::Custom(index) => 3 + index,
        }
    }
}

#[derive(Debug, Clone)]
struct SfxNote {
    time: f64,
    fake: bool,
    sound: SfxSound,
}

fn collect_sfx_notes(chart: &phire::core::Chart) -> (Vec<SfxNote>, Vec<String>) {
    let mut custom_index = 0usize;
    let mut names: Vec<String> = Vec::new();
    let mut name_index = std::collections::HashMap::new();
    let notes = chart
        .lines
        .iter()
        .flat_map(|line| &line.notes)
        .filter_map(|note| {
            let sound = match &note.hitsound {
                phire::judge::HitSound::None => return None,
                phire::judge::HitSound::Click => SfxSound::Click,
                phire::judge::HitSound::Drag => SfxSound::Drag,
                phire::judge::HitSound::Flick => SfxSound::Flick,
                phire::judge::HitSound::Custom(name) => {
                    let index = *name_index.entry(name.clone()).or_insert_with(|| {
                        let index = custom_index;
                        custom_index += 1;
                        names.push(name.clone());
                        index
                    });
                    SfxSound::Custom(index)
                }
            };
            Some(SfxNote {
                time: note.time,
                fake: note.fake,
                sound,
            })
        })
        .collect();
    (notes, names)
}

fn get_clip<'a>(
    sound: SfxSound,
    resource_pack: &'a ResourcePack,
    chart: &'a phire::core::Chart,
    names: &'a [String],
) -> Option<Vec<f32>> {
    match sound {
        SfxSound::Click => Some(resource_pack.sfx_click.to_vec()),
        SfxSound::Drag => Some(resource_pack.sfx_drag.to_vec()),
        SfxSound::Flick => Some(resource_pack.sfx_flick.to_vec()),
        SfxSound::Custom(index) => names
            .get(index)
            .and_then(|name| chart.hitsounds.get(name))
            .map(|clip| clip.to_vec()),
    }
}

fn add_clip(output: &mut [f32], position: usize, clip: &[f32]) {
    let start = position.min(output.len());
    let end = (position + clip.len()).min(output.len());
    for index in start..end {
        output[index] += clip[index - position];
    }
}

fn mix_sfx(
    config: &RenderConfig,
    chart: &phire::core::Chart,
    resource_pack: &ResourcePack,
    timeline: &RenderTimeline,
    output_len: usize,
) -> Result<Vec<f32>> {
    let mut output = vec![0.0f32; output_len];
    if config.volume_sfx == 0.0 {
        return Ok(output);
    }

    let judge_offset = config.judge_offset;
    let start_time = config.play_start_time - judge_offset;
    let end_time = start_time + timeline.chart_length_sfx;
    let ratio = timeline.speed_time_ratio;

    match config.audio_mix_mode {
        AudioMixMode::Fft => {
            let (notes, names) = collect_sfx_notes(chart);
            let mut groups: Vec<(SfxSound, Vec<usize>)> = Vec::new();
            for note in notes
                .into_iter()
                .filter(|note| !note.fake && note.time > start_time && note.time < end_time)
            {
                let position = ((timeline.before_time + note.time * ratio + judge_offset
                    - config.play_start_time * ratio)
                    * 48_000.0)
                    .ceil() as usize
                    * 2;
                if let Some((_, positions)) =
                    groups.iter_mut().find(|(sound, _)| *sound == note.sound)
                {
                    positions.push(position);
                } else {
                    groups.push((note.sound, vec![position]));
                }
            }
            let mut prepared_groups: Vec<(Vec<f32>, Vec<usize>)> = groups
                .into_iter()
                .filter_map(|(sound, positions)| {
                    get_clip(sound, resource_pack, chart, &names).map(|clip| (clip, positions))
                })
                .collect();
            mix_sfx_fft(&mut output, &mut prepared_groups)?;
        }
        AudioMixMode::Traditional => {
            let (notes, names) = collect_sfx_notes(chart);
            let mut placements: Vec<(usize, SfxSound)> = notes
                .into_iter()
                .filter(|note| !note.fake && note.time > start_time && note.time < end_time)
                .filter_map(|note| {
                    let position = ((timeline.before_time + note.time * ratio + judge_offset
                        - config.play_start_time * ratio)
                        * 48_000.0)
                        .ceil() as usize
                        * 2;
                    Some((position, note.sound))
                })
                .collect();
            placements.sort_unstable_by_key(|(position, sound)| (*position, sound.sort_key()));
            for (position, sound) in placements {
                if let Some(clip) = get_clip(sound, resource_pack, chart, &names) {
                    add_clip(&mut output, position, &clip);
                }
            }
        }
        AudioMixMode::Optimized => {
            let (notes, names) = collect_sfx_notes(chart);
            let mut counts: std::collections::HashMap<(i64, SfxSound), u8> =
                std::collections::HashMap::new();
            let mut placements: Vec<(i64, usize, SfxSound)> = notes
                .into_iter()
                .filter(|note| !note.fake && note.time > start_time && note.time < end_time)
                .filter_map(|note| {
                    let bucket = ((timeline.before_time + note.time * ratio + judge_offset
                        - config.play_start_time * ratio)
                        * 200.0)
                        .round() as i64;
                    let count = counts.entry((bucket, note.sound)).or_insert(0);
                    if *count < 3 {
                        *count += 1;
                        let position = (bucket as f64 * 0.005 * 48_000.0).ceil() as usize * 2;
                        Some((bucket, position, note.sound))
                    } else {
                        None
                    }
                })
                .collect();
            placements.sort_unstable_by_key(|(bucket, _, sound)| (*bucket, sound.sort_key()));
            for (_, position, sound) in placements {
                if let Some(clip) = get_clip(sound, resource_pack, chart, &names) {
                    add_clip(&mut output, position, &clip);
                }
            }
        }
    }
    Ok(output)
}

fn mix_sfx_fft(output: &mut [f32], groups: &mut [(Vec<f32>, Vec<usize>)]) -> Result<()> {
    if output.is_empty() {
        return Ok(());
    }
    let max_clip_len = groups
        .iter()
        .filter(|(clip, positions)| !clip.is_empty() && !positions.is_empty())
        .map(|(clip, _)| clip.len())
        .max()
        .unwrap_or(0);
    if max_clip_len == 0 {
        return Ok(());
    }

    const TARGET_BLOCK_LEN: usize = 1 << 17;
    let fft_size = (max_clip_len + TARGET_BLOCK_LEN).next_power_of_two();
    let overlap = max_clip_len - 1;
    let block_len = ((fft_size - overlap) / 2) * 2;
    let block_count = output.len().div_ceil(block_len);

    let mut planner = realfft::RealFftPlanner::<f32>::new();
    let forward = planner.plan_fft_forward(fft_size);
    let inverse = planner.plan_fft_inverse(fft_size);
    let mut prepared = Vec::new();
    for (clip, positions) in groups.iter_mut() {
        if clip.is_empty() || positions.is_empty() {
            continue;
        }
        positions.sort_unstable();
        let mut input = vec![0.0f32; fft_size];
        input[..clip.len()].copy_from_slice(clip);
        let mut spectrum = vec![Complex::new(0.0, 0.0); fft_size / 2 + 1];
        forward.process(&mut input, &mut spectrum)?;
        prepared.push((positions.clone(), spectrum));
    }
    if prepared.is_empty() {
        return Ok(());
    }

    let mut impulse = vec![0.0f32; fft_size];
    let mut impulse_fft = vec![Complex::new(0.0, 0.0); fft_size / 2 + 1];
    let mut total_fft = vec![Complex::new(0.0, 0.0); fft_size / 2 + 1];
    let mut mixed = vec![0.0f32; fft_size];
    for (block_index, block) in output.chunks_mut(block_len).enumerate() {
        let block_start = block_index * block_len;
        let input_start = block_start as isize - overlap as isize;
        let input_end = input_start + fft_size as isize;
        total_fft.fill(Complex::new(0.0, 0.0));
        for (positions, spectrum) in &prepared {
            impulse.fill(0.0);
            let first = positions.partition_point(|&position| (position as isize) < input_start);
            let last = positions.partition_point(|&position| (position as isize) < input_end);
            for &position in &positions[first..last] {
                let impulse_position = position as isize - input_start;
                if impulse_position >= 0 {
                    impulse[impulse_position as usize] += 1.0;
                }
            }
            forward.process(&mut impulse, &mut impulse_fft)?;
            for index in 0..total_fft.len() {
                total_fft[index] += impulse_fft[index] * spectrum[index];
            }
        }
        inverse.process(&mut total_fft, &mut mixed)?;
        let scale = 1.0 / fft_size as f32;
        let valid_len = block.len().min(block_len);
        for (target, &value) in block[..valid_len]
            .iter_mut()
            .zip(&mixed[overlap..overlap + valid_len])
        {
            *target = value * scale;
        }
    }
    let _ = block_count;
    Ok(())
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
        assert_eq!(
            output,
            vec![1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0]
        );
    }

    #[test]
    fn add_clip_clamps_to_output_bounds() {
        let mut output = vec![0.0f32; 4];
        add_clip(&mut output, 2, &[1.0, 2.0, 3.0]);
        assert_eq!(output, vec![0.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn fft_mix_reconstructs_a_single_click_at_the_origin() {
        let mut groups = vec![(vec![1.0f32, 2.0], vec![0usize])];
        let mut output = vec![0.0f32; 16];
        mix_sfx_fft(&mut output, &mut groups).unwrap();
        assert!((output[0] - 1.0).abs() < 1e-3);
        assert!((output[1] - 2.0).abs() < 1e-3);
        assert!(output[2..].iter().all(|sample| sample.abs() < 1e-3));
    }
}
