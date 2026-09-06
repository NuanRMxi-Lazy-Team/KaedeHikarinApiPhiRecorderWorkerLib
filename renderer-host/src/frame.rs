use std::{cell::RefCell, ops::DerefMut, path::PathBuf, rc::Rc};

use anyhow::{Context, Result};
use macroquad::prelude::*;
use phire::{
    core::{MSRenderTarget, ResourcePack},
    fs::FileSystem,
    scene::{BasicPlayer, GameMode, GameScene, Main},
    time::TimeManager,
    ui::{FontArc, TextPainter},
};

use phi_recorder_core::{
    calculate_timeline, ChartInfo, JobControl, RenderConfig, ResourceRoots, TimingConstants,
};
use phi_recorder_protocol::RenderRequestPayload;

use crate::{audio::AudioPlan, ffmpeg_writer::AudioInput};

pub struct PreparedFrameRenderer {
    main: Main,
    target: Rc<MSRenderTarget>,
    time: Rc<RefCell<f64>>,
    painter: TextPainter,
    audio_plan: AudioPlan,
    width: u32,
    height: u32,
    fps: u32,
}

impl PreparedFrameRenderer {
    pub async fn prepare(
        request: &RenderRequestPayload,
        control: &JobControl,
    ) -> Result<(Self, f64, u32, u64)> {
        if control.is_cancel_requested() {
            anyhow::bail!("render canceled");
        }

        let roots = ResourceRoots {
            assets_dir: PathBuf::from(&request.resource_roots.assets_dir),
            fonts_dir: PathBuf::from(&request.resource_roots.fonts_dir),
            resource_pack_dir: PathBuf::from(&request.resource_roots.resource_pack_dir),
            ffmpeg_path: PathBuf::from(&request.resource_roots.ffmpeg_path),
            temp_dir: PathBuf::from(&request.resource_roots.temp_dir),
            renderer_host_path: PathBuf::from(&request.resource_roots.renderer_host_path),
        };
        roots.validate().map_err(|error| anyhow::anyhow!(error))?;

        let config: RenderConfig =
            serde_json::from_str(&request.render_config_json).context("invalid render config")?;
        config.validate().map_err(|error| anyhow::anyhow!(error))?;
        let mut phire_config = config.to_phire_config();
        phire_config.mods = phire::config::Mods::AUTOPLAY;

        macroquad::file::set_pc_assets_folder(&roots.assets_dir.to_string_lossy());
        let mut filesystem: Box<dyn FileSystem + 'static> =
            phire::fs::fs_from_file(PathBuf::from(&request.chart_path).as_path())?;
        let info = if let Some(info_json) = &request.chart_info_json {
            serde_json::from_str::<ChartInfo>(info_json).context("invalid chart info")?
        } else {
            phire::fs::load_info(filesystem.deref_mut()).await?.into()
        };
        let info: phire::info::ChartInfo = info.into();
        if control.is_cancel_requested() {
            anyhow::bail!("render canceled");
        }

        let (chart, format) = GameScene::load_chart(filesystem.deref_mut(), &info, &phire_config)
            .await
            .context("load chart")?;
        let resource_pack_path = phire_config
            .res_pack_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| roots.resource_pack_dir.clone());
        let resource_pack = ResourcePack::from_path(Some(&resource_pack_path))
            .await
            .context("load resource pack")?;
        let music_data = filesystem.load_file(&info.music).await?;
        let music = sasa::AudioClip::new(music_data).context("decode music")?;
        let music_length = music.length();
        let music_sample_rate = music.sample_rate();
        let timeline = calculate_timeline(
            &config,
            chart.offset,
            info.offset,
            music_length,
            TimingConstants::phire_defaults(),
        )
        .map_err(|error| anyhow::anyhow!(error))?;
        let audio_plan = crate::audio::build_audio_plan(
            &config,
            &chart,
            &music,
            &resource_pack,
            &timeline,
            &roots.temp_dir,
        )?;

        let fonts = vec![FontArc::try_from_vec(
            macroquad::file::load_file("font.ttf").await?,
        )?];
        let painter = TextPainter::new(fonts);
        let player = BasicPlayer {
            avatar: None,
            id: 0,
            rks: config.player_rks,
        };
        let target = Rc::new(MSRenderTarget::new(
            (config.resolution.width, config.resolution.height),
            config.sample_count,
        ));
        let time = Rc::new(RefCell::new(0.0));
        let time_manager = TimeManager::manual({
            let time = Rc::clone(&time);
            Box::new(move || *time.borrow())
        });
        let (illustration, background) = phire::scene::LoadingScene::load_background(
            &mut filesystem,
            &phire_config,
            &info.illustration,
        )
        .await
        .unwrap_or_else(|_| {
            let black = Texture2D::from_rgba8(1, 1, &[0, 0, 0, 255]);
            (black.clone(), black)
        });
        let main = Main::new(
            Box::new(
                GameScene::new(
                    Some((chart, format)),
                    GameMode::Normal,
                    info,
                    phire_config,
                    filesystem,
                    Some(player),
                    background.into(),
                    illustration.into(),
                    None,
                    None,
                )
                .await?,
            ),
            time_manager,
            {
                let target = Rc::clone(&target);
                let mut count = 0;
                move || {
                    count += 1;
                    if count == 1 || count == 3 {
                        Some(target.input())
                    } else {
                        Some(target.output())
                    }
                }
            },
        )
        .await?;
        Ok((
            Self {
                main,
                target,
                time,
                painter,
                audio_plan,
                width: config.resolution.width,
                height: config.resolution.height,
                fps: config.fps,
            },
            music_length,
            music_sample_rate,
            timeline.video_frames,
        ))
    }

    pub fn render_one_frame(&mut self, time_seconds: f64) -> Result<()> {
        *self.time.borrow_mut() = time_seconds;
        self.main.update()?;
        self.main.render(&mut self.painter)?;
        unsafe { get_internal_gl().flush() };
        self.target.blit();
        Ok(())
    }

    pub fn output_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn output_texture(&self) -> Texture2D {
        self.target.output().texture
    }

    pub fn audio_inputs(&self) -> &[AudioInput] {
        self.audio_plan.inputs()
    }

    pub fn output_args(&self) -> &[String] {
        self.audio_plan.output_args()
    }
}
