use std::{
    path::Path,
    process::{Command, Stdio},
};

use phi_recorder_core::{select_bitrate_control, RenderConfig};

pub const ENCODER_LIST_HEVC: [&str; 4] = ["hevc_nvenc", "hevc_qsv", "hevc_amf", "hevc_vaapi"];
pub const ENCODER_LIST_AVC: [&str; 4] = ["h264_nvenc", "h264_qsv", "h264_amf", "h264_vaapi"];

pub struct VideoEncoderPlan {
    pub encoder: String,
    pub encoder_list: [&'static str; 4],
    pub bitrate_control: &'static str,
    pub device_args: Vec<String>,
    pub output_video_filter: Option<String>,
}

impl VideoEncoderPlan {
    /// 仅 NVENC 需要 `-hwaccel_output_format cuda` 输入提示。
    pub fn hardware_encoder_hint(&self) -> &str {
        if self.encoder.ends_with("_nvenc") {
            self.encoder_list[0]
        } else {
            ""
        }
    }
}

/// 选择视频编码器：自定义编码器 → mpeg4 → 硬件编码（逐一实测可用性）→ 软件编码回退。
pub fn plan_video_encoder(ffmpeg_path: &Path, config: &RenderConfig) -> VideoEncoderPlan {
    let encoder_list = if config.hevc {
        ENCODER_LIST_HEVC
    } else {
        ENCODER_LIST_AVC
    };

    let encoder = if let Some(custom) = config
        .custom_encoder
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        custom.to_owned()
    } else if config.mpeg4 {
        "mpeg4".to_owned()
    } else if config.hardware_accel {
        let selected = encoder_list.into_iter().find(|candidate| {
            eprintln!("renderer-host: probe encoder {candidate}");
            test_encoder(ffmpeg_path, candidate)
        });
        match selected {
            Some(encoder) => encoder.to_owned(),
            None => {
                eprintln!(
                    "renderer-host: no hardware encoder available, falling back to software encoder"
                );
                software_encoder(config)
            }
        }
    } else {
        software_encoder(config)
    };

    let (device_args, output_video_filter) = if is_vaapi(&encoder) {
        (
            vec!["-vaapi_device".to_owned(), "/dev/dri/renderD128".to_owned()],
            Some("format=nv12,hwupload".to_owned()),
        )
    } else {
        (Vec::new(), None)
    };

    let bitrate_control = select_bitrate_control(
        config.dynamic_bitrate_control,
        &encoder,
        encoder_list,
        config.mpeg4,
        config.custom_encoder.as_deref(),
    );

    VideoEncoderPlan {
        encoder,
        encoder_list,
        bitrate_control,
        device_args,
        output_video_filter,
    }
}

fn software_encoder(config: &RenderConfig) -> String {
    if config.hevc {
        "libx265".to_owned()
    } else {
        "libx264".to_owned()
    }
}

fn is_vaapi(encoder: &str) -> bool {
    encoder.ends_with("_vaapi")
}

/// 用 1 秒 testsrc 实际转码一次验证编码器可用性。
fn test_encoder(ffmpeg_path: &Path, encoder: &str) -> bool {
    let mut command = Command::new(ffmpeg_path);
    if is_vaapi(encoder) {
        command.args(["-vaapi_device", "/dev/dri/renderD128"]);
    }
    command.args([
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=128x128:rate=5:duration=1",
        "-pix_fmt",
        "yuv420p",
    ]);
    if is_vaapi(encoder) {
        command.args(["-vf", "format=nv12,hwupload"]);
    }
    command
        .args(["-c:v", encoder, "-f", "null", "-"])
        .args(["-loglevel", "error"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    match command.output() {
        Ok(output) => output.status.success(),
        Err(error) => {
            eprintln!("renderer-host: run ffmpeg probe: {error}");
            false
        }
    }
}
