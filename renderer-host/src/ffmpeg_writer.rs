use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
};

use anyhow::{Context, Result};
use phi_recorder_core::build_video_input_args;

pub struct FfmpegWriter {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    output_path: PathBuf,
}

impl FfmpegWriter {
    pub fn start(
        ffmpeg_path: &Path,
        width: u32,
        height: u32,
        fps: u32,
        output_path: &Path,
    ) -> Result<Self> {
        let args = build_video_input_args(width, height, fps, "libx264", "");
        let mut command = Command::new(ffmpeg_path);
        command
            .args(args)
            .args(["-an", "-c:v", "libx264", "-preset", "ultrafast"])
            .args(["-y", "-loglevel", "error"])
            .arg(output_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .with_context(|| format!("start FFmpeg at {}", ffmpeg_path.display()))?;
        let stdin = child.stdin.take().context("FFmpeg stdin unavailable")?;

        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            output_path: output_path.to_owned(),
        })
    }

    pub fn write_frame(&mut self, frame: &[u8]) -> Result<()> {
        self.stdin
            .as_mut()
            .context("FFmpeg writer already closed")?
            .write_all(frame)
            .context("write raw frame to FFmpeg")
    }

    pub fn finish(mut self) -> Result<PathBuf> {
        drop(self.stdin.take());
        let child = self.child.take().context("FFmpeg child already closed")?;
        let output = child.wait_with_output().context("wait for FFmpeg")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("FFmpeg exited with {}: {}", output.status, stderr.trim());
        }
        Ok(self.output_path.clone())
    }
}

impl Drop for FfmpegWriter {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_one_cpu_encoded_frame_when_ffmpeg_is_available() {
        let ffmpeg = std::env::var_os("PHI_FFMPEG_PATH")
            .map(PathBuf::from)
            .or_else(|| which_ffmpeg());
        let Some(ffmpeg) = ffmpeg else {
            return;
        };

        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("one-frame.mp4");
        let mut writer = FfmpegWriter::start(&ffmpeg, 16, 16, 60, &output).unwrap();
        writer.write_frame(&vec![16u8; 16 * 16 * 3 / 2]).unwrap();
        let output_path = writer.finish().unwrap();

        assert_eq!(output_path, output);
        assert!(output.is_file());
        assert!(std::fs::metadata(output).unwrap().len() > 0);
    }

    fn which_ffmpeg() -> Option<PathBuf> {
        let candidate = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        std::env::split_paths(&std::env::var_os("PATH")?)
            .map(|directory| directory.join(candidate))
            .find(|path| path.is_file())
    }
}
