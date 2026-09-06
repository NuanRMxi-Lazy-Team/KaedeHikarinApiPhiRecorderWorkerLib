mod chart;
mod config;
mod control;
mod events;
mod ffmpeg;
mod request;
mod timeline;
mod validation;

pub use config::{AudioMixMode, ChallengeColor, RenderConfig, Resolution};
pub use chart::{load_chart_info, load_chart_info_blocking, ChartFormat, ChartInfo};
pub use control::{ControlError, JobControl};
pub use events::{EventSink, JobEvent, JobEventKind, JobId, NoopEventSink};
pub use ffmpeg::{
    build_audio_filter, build_output_args, build_video_input_args, select_bitrate_control,
    AudioFilterPlan, FfmpegPlan, VideoEncoderKind,
};
pub use request::{RenderRequest, ResourceRoots};
pub use timeline::{calculate_audio_layout, calculate_timeline, AudioLayout, RenderTimeline, TimingConstants};
pub use validation::ValidationError;
