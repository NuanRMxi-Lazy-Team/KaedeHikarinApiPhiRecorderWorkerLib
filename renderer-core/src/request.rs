use std::path::PathBuf;

use crate::{RenderConfig, ValidationError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRoots {
    pub assets_dir: PathBuf,
    pub fonts_dir: PathBuf,
    pub resource_pack_dir: PathBuf,
    pub ffmpeg_path: PathBuf,
    pub temp_dir: PathBuf,
    pub renderer_host_path: PathBuf,
}

impl ResourceRoots {
    pub fn validate(&self) -> Result<(), ValidationError> {
        for (field, path) in [
            ("assets_dir", &self.assets_dir),
            ("fonts_dir", &self.fonts_dir),
            ("temp_dir", &self.temp_dir),
            ("renderer_host_path", &self.renderer_host_path),
        ] {
            if path.as_os_str().is_empty() {
                return Err(ValidationError::MissingPath(field));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderRequest {
    pub chart_path: PathBuf,
    pub output_path: PathBuf,
    pub config: RenderConfig,
}

impl RenderRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.chart_path.as_os_str().is_empty() {
            return Err(ValidationError::MissingPath("chart_path"));
        }
        if self.output_path.as_os_str().is_empty() {
            return Err(ValidationError::MissingPath("output_path"));
        }
        self.config.validate()
    }
}
