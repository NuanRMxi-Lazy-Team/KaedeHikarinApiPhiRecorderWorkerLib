use std::{ops::DerefMut, path::Path};

use anyhow::Result;
use chrono::{DateTime, Utc};
use phire::{fs, fs::FileSystem, info as phire_info};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartFormat {
    Rpe,
    Pec,
    Pgr,
    Pbc,
}

impl From<phire_info::ChartFormat> for ChartFormat {
    fn from(value: phire_info::ChartFormat) -> Self {
        match value {
            phire_info::ChartFormat::Rpe => Self::Rpe,
            phire_info::ChartFormat::Pec => Self::Pec,
            phire_info::ChartFormat::Pgr => Self::Pgr,
            phire_info::ChartFormat::Pbc => Self::Pbc,
        }
    }
}

impl From<ChartFormat> for phire_info::ChartFormat {
    fn from(value: ChartFormat) -> Self {
        match value {
            ChartFormat::Rpe => Self::Rpe,
            ChartFormat::Pec => Self::Pec,
            ChartFormat::Pgr => Self::Pgr,
            ChartFormat::Pbc => Self::Pbc,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChartInfo {
    pub id: Option<i32>,
    pub guid: Option<String>,
    pub uploader: Option<i32>,
    pub name: String,
    pub difficulty: f32,
    pub level: String,
    pub charter: String,
    pub composer: String,
    pub illustrator: String,
    pub chart: String,
    pub format: Option<ChartFormat>,
    pub music: String,
    pub illustration: String,
    pub unlock_video: Option<String>,
    pub preview_start: f64,
    pub preview_end: Option<f64>,
    pub aspect_ratio: f32,
    pub force_aspect_ratio: bool,
    pub background_dim: f32,
    pub line_length: f32,
    pub offset: f64,
    pub tip: Option<String>,
    pub tags: Vec<String>,
    pub intro: String,
    pub hold_partial_cover: bool,
    pub negative_length_hold: bool,
    pub note_uniform_scale: bool,
    pub score_total: u32,
    pub hold_particle_interval_ratio: f32,
    pub fold_animation: bool,
    pub created: Option<DateTime<Utc>>,
    pub updated: Option<DateTime<Utc>>,
    pub chart_updated: Option<DateTime<Utc>>,
}

impl Default for ChartInfo {
    fn default() -> Self {
        phire_info::ChartInfo::default().into()
    }
}

impl From<phire_info::ChartInfo> for ChartInfo {
    fn from(value: phire_info::ChartInfo) -> Self {
        Self {
            id: value.id,
            guid: value.guid,
            uploader: value.uploader,
            name: value.name,
            difficulty: value.difficulty,
            level: value.level,
            charter: value.charter,
            composer: value.composer,
            illustrator: value.illustrator,
            chart: value.chart,
            format: value.format.map(Into::into),
            music: value.music,
            illustration: value.illustration,
            unlock_video: value.unlock_video,
            preview_start: value.preview_start,
            preview_end: value.preview_end,
            aspect_ratio: value.aspect_ratio,
            force_aspect_ratio: value.force_aspect_ratio,
            background_dim: value.background_dim,
            line_length: value.line_length,
            offset: value.offset,
            tip: value.tip,
            tags: value.tags,
            intro: value.intro,
            hold_partial_cover: value.hold_partial_cover,
            negative_length_hold: value.negative_length_hold,
            note_uniform_scale: value.note_uniform_scale,
            score_total: value.score_total,
            hold_particle_interval_ratio: value.hold_particle_interval_ratio,
            fold_animation: value.fold_animation,
            created: value.created,
            updated: value.updated,
            chart_updated: value.chart_updated,
        }
    }
}

impl From<ChartInfo> for phire_info::ChartInfo {
    fn from(value: ChartInfo) -> Self {
        Self {
            id: value.id,
            guid: value.guid,
            uploader: value.uploader,
            name: value.name,
            difficulty: value.difficulty,
            level: value.level,
            charter: value.charter,
            composer: value.composer,
            illustrator: value.illustrator,
            chart: value.chart,
            format: value.format.map(Into::into),
            music: value.music,
            illustration: value.illustration,
            unlock_video: value.unlock_video,
            preview_start: value.preview_start,
            preview_end: value.preview_end,
            aspect_ratio: value.aspect_ratio,
            force_aspect_ratio: value.force_aspect_ratio,
            background_dim: value.background_dim,
            line_length: value.line_length,
            offset: value.offset,
            tip: value.tip,
            tags: value.tags,
            intro: value.intro,
            hold_partial_cover: value.hold_partial_cover,
            negative_length_hold: value.negative_length_hold,
            note_uniform_scale: value.note_uniform_scale,
            score_total: value.score_total,
            hold_particle_interval_ratio: value.hold_particle_interval_ratio,
            fold_animation: value.fold_animation,
            created: value.created,
            updated: value.updated,
            chart_updated: value.chart_updated,
        }
    }
}

pub async fn load_chart_info(path: impl AsRef<Path>) -> Result<ChartInfo> {
    let mut filesystem: Box<dyn FileSystem + Send + Sync + 'static> =
        fs::fs_from_file(path.as_ref())?;
    let info = fs::load_info(filesystem.deref_mut()).await?;
    Ok(info.into())
}

pub fn load_chart_info_blocking(path: impl AsRef<Path>) -> Result<ChartInfo> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(load_chart_info(path))
}

#[cfg(test)]
mod tests {
    use super::load_chart_info_blocking;
    use std::fs;

    #[test]
    fn loads_info_from_an_explicit_chart_directory() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("info.yml"),
            "name: Test Chart\ndifficulty: 3\nlevel: Test 3\ncharter: Tester\n",
        )
        .unwrap();

        let info = load_chart_info_blocking(directory.path()).unwrap();

        assert_eq!(info.name, "Test Chart");
        assert_eq!(info.difficulty, 3.0);
        assert_eq!(info.level, "Test 3");
        assert_eq!(info.charter, "Tester");
    }
}
