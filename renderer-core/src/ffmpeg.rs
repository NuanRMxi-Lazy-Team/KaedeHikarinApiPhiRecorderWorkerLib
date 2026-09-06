#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoEncoderKind {
    AvcHardware,
    HevcHardware,
    Mpeg4,
    Software,
    Custom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioFilterPlan {
    pub filter_complex: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FfmpegPlan {
    pub video_input_args: Vec<String>,
    pub audio_input_specs: Vec<Vec<String>>,
    pub output_args: Vec<String>,
    pub audio_filter: AudioFilterPlan,
}

pub fn select_bitrate_control(
    dynamic: bool,
    encoder: &str,
    encoder_list: [&str; 4],
    mpeg4: bool,
    custom_encoder: Option<&str>,
) -> &'static str {
    if !dynamic {
        return "-b:v";
    }
    if encoder == encoder_list[0] && !mpeg4 {
        "-cq"
    } else if encoder == encoder_list[1] || mpeg4 || encoder == encoder_list[3] {
        "-q"
    } else if encoder == encoder_list[2] {
        "-qp_p"
    } else if custom_encoder == Some(encoder) {
        "-q"
    } else {
        "-crf"
    }
}

pub fn build_audio_filter(
    volume_music: f32,
    volume_sfx: f32,
    speed: f32,
    loudness_equalization: bool,
    force_limit: bool,
    limit_threshold: f32,
    ending_delay_millis: f64,
    hires: bool,
) -> AudioFilterPlan {
    let mut music = if loudness_equalization {
        "[2:a]loudnorm=I=-16:LRA=24:TP=-1,aresample=48000:resampler=swr".to_owned()
    } else {
        "[2:a]aresample=48000:resampler=swr".to_owned()
    };
    music.push_str(&format!(",volume={volume_music}"));
    if speed != 1.0 {
        music.push_str(&format!(",rubberband=tempo={speed}"));
    }
    music.push_str("[a2];");

    let mut sfx = format!("[1:a]volume={volume_sfx}");
    if force_limit {
        sfx.push_str(&format!(
            ",alimiter=limit={limit_threshold}:level=false:attack=0.1:release=1"
        ));
    }
    sfx.push_str("[a1];");

    let ending = format!(
        "[3:a]volume={volume_music},adelay={ending_delay_millis}|{ending_delay_millis}[a3];"
    );
    let mix = if hires {
        "[a1][a2][a3]amix=inputs=3:duration=first:normalize=0[a]".to_owned()
    } else {
        "[a1][a2][a3]amix=inputs=3:duration=first:normalize=0[aa];[aa]alimiter=limit=1.0:level=false:attack=0.1:release=1[a]".to_owned()
    };

    AudioFilterPlan {
        filter_complex: format!("{music}{sfx}{ending}{mix}"),
    }
}

pub fn build_output_args(
    encoder: &str,
    bitrate_control: &str,
    bitrate: &str,
    filter_complex: &str,
    hires: bool,
) -> Vec<String> {
    let (audio_codec, audio_bitrate, container) = if hires {
        ("pcm_f32le", None, "mov")
    } else {
        ("aac", Some("320k"), "mp4")
    };
    let mut args = vec![
        "-c:a".to_owned(),
        audio_codec.to_owned(),
        "-c:v".to_owned(),
        encoder.to_owned(),
        "-movflags".to_owned(),
        "+faststart".to_owned(),
        "-pix_fmt".to_owned(),
        "yuv420p".to_owned(),
        bitrate_control.to_owned(),
        bitrate.to_owned(),
    ];
    if let Some(audio_bitrate) = audio_bitrate {
        args.extend(["-b:a".to_owned(), audio_bitrate.to_owned()]);
    }
    args.extend([
        "-filter_complex".to_owned(),
        filter_complex.to_owned(),
        "-map".to_owned(),
        "0:v:0".to_owned(),
        "-map".to_owned(),
        "[a]".to_owned(),
        "-f".to_owned(),
        container.to_owned(),
    ]);
    args
}

pub fn build_video_input_args(
    width: u32,
    height: u32,
    fps: u32,
    encoder: &str,
    hardware_encoder: &str,
) -> Vec<String> {
    let mut args = vec![
        "-probesize".to_owned(),
        "50M".to_owned(),
        "-y".to_owned(),
        "-f".to_owned(),
        "rawvideo".to_owned(),
        "-c:v".to_owned(),
        "rawvideo".to_owned(),
        "-color_range".to_owned(),
        "full".to_owned(),
    ];
    if encoder == hardware_encoder {
        args.extend(["-hwaccel_output_format", "cuda"].map(str::to_owned));
    }
    args.extend(
        [
            "-s",
            &format!("{width}x{height}"),
            "-r",
            &fps.to_string(),
            "-pix_fmt",
            "yuv420p",
            "-thread_queue_size",
            "1024",
            "-i",
            "pipe:0",
        ]
        .map(str::to_owned),
    );
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrate_policy_matches_legacy_encoder_order() {
        let encoders = ["h264_nvenc", "h264_qsv", "h264_amf", "h264_vaapi"];
        assert_eq!(select_bitrate_control(true, "h264_nvenc", encoders, false, None), "-cq");
        assert_eq!(select_bitrate_control(true, "h264_qsv", encoders, false, None), "-q");
        assert_eq!(select_bitrate_control(true, "h264_amf", encoders, false, None), "-qp_p");
        assert_eq!(select_bitrate_control(false, "libx264", encoders, false, None), "-b:v");
    }

    #[test]
    fn filter_graph_keeps_legacy_audio_input_indexes() {
        let plan = build_audio_filter(0.5, 0.4, 1.0, false, true, 0.5, 123.0, false);
        assert!(plan.filter_complex.starts_with("[2:a]"));
        assert!(plan.filter_complex.contains("[1:a]"));
        assert!(plan.filter_complex.contains("[3:a]"));
        assert!(plan.filter_complex.contains("adelay=123|123"));
    }

    #[test]
    fn video_args_are_raw_yuv_input() {
        let args = build_video_input_args(1920, 1080, 60, "h264_nvenc", "h264_nvenc");
        assert!(args.windows(2).any(|pair| pair[0] == "-pix_fmt" && pair[1] == "yuv420p"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-hwaccel_output_format" && pair[1] == "cuda"));
    }

    #[test]
    fn output_args_preserve_mp4_and_hires_containers() {
        let filter = "[a1][a2][a3]amix=inputs=3[a]";
        let mp4 = build_output_args("libx264", "-crf", "28", filter, false);
        let mov = build_output_args("libx265", "-b:v", "28", filter, true);

        assert!(mp4.windows(2).any(|pair| pair[0] == "-f" && pair[1] == "mp4"));
        assert!(mov.windows(2).any(|pair| pair[0] == "-f" && pair[1] == "mov"));
        assert!(mp4.windows(2).any(|pair| pair[0] == "-b:a" && pair[1] == "320k"));
        assert!(!mov.iter().any(|arg| arg == "320k"));
    }
}
