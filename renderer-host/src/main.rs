use std::{
    ffi::CStr,
    io::{self, BufReader, BufWriter},
    path::Path,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::Instant,
};

use macroquad::miniquad::gl::glGetString;
use phi_recorder_core::JobControl;
use phi_recorder_protocol::{
    decode_json, decode_protocol_version, encode_error, encode_json, encode_protocol_version,
    read_frame, write_frame, ControlCommand, ControlPayload, Frame, JobEvent, JobEventPayload,
    MessageType, ProtocolError, RenderRequestPayload, JSON_SCHEMA_VERSION, PROTOCOL_VERSION,
};
mod frame;
use frame::PreparedFrameRenderer;
mod ffmpeg_writer;
use ffmpeg_writer::FfmpegWriter;
mod readback;
use readback::FrameReadback;

const GL_VENDOR: u32 = 0x1F00;
const GL_RENDERER: u32 = 0x1F01;
const GL_VERSION: u32 = 0x1F02;
const GL_SHADING_LANGUAGE_VERSION: u32 = 0x8B8C;

fn main() {
    if let Err(error) = run() {
        eprintln!("renderer-host: {error}");
        std::process::exit(1);
    }
}

enum RenderCommand {
    Start {
        job_id: u64,
        request: RenderRequestPayload,
        control: Arc<JobControl>,
    },
    CapabilityProbe { request_id: u64 },
    Shutdown,
}

fn run() -> Result<(), ProtocolError> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());

    let (output_sender, output_receiver) = mpsc::channel::<Frame>();
    let writer_thread = thread::spawn(move || {
        let stdout = io::stdout();
        let mut writer = BufWriter::new(stdout.lock());
        while let Ok(frame) = output_receiver.recv() {
            if write_frame(&mut writer, &frame).is_err() {
                break;
            }
        }
    });

    let hello = read_frame(&mut reader)?.ok_or(ProtocolError::InvalidPayload(
        "the first frame must be a hello message",
    ))?;
    if hello.message_type != MessageType::Hello {
        send_error(&output_sender, hello.request_id, "the first message must be hello")?;
        drop(output_sender);
        let _ = writer_thread.join();
        return Err(ProtocolError::InvalidPayload("missing hello message"));
    }
    if decode_protocol_version(&hello.payload)? != PROTOCOL_VERSION {
        send_error(&output_sender, hello.request_id, "protocol version mismatch")?;
        drop(output_sender);
        let _ = writer_thread.join();
        return Err(ProtocolError::UnsupportedVersion(
            decode_protocol_version(&hello.payload)?,
        ));
    }
    send_frame(
        &output_sender,
        Frame::new(
            MessageType::HelloAck,
            hello.request_id,
            0,
            encode_protocol_version(),
        )?,
    )?;

    let (render_sender, render_receiver) = mpsc::channel();
    let active_job = Arc::new(Mutex::new(None::<u64>));
    let active_control = Arc::new(Mutex::new(None::<Arc<JobControl>>));
    let render_active_job = Arc::clone(&active_job);
    let render_active_control = Arc::clone(&active_control);
    let render_output = output_sender.clone();
    let render_thread = thread::spawn(move || {
        render_loop(
            render_receiver,
            render_output,
            render_active_job,
            render_active_control,
        );
    });

    while let Some(frame) = read_frame(&mut reader)? {
        match frame.message_type {
            MessageType::Shutdown => {
                if let Some(control) = active_control
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .as_ref()
                {
                    control.cancel();
                }
                let _ = render_sender.send(RenderCommand::Shutdown);
                let _ = render_thread.join();
                send_frame(
                    &output_sender,
                    Frame::new(MessageType::ShutdownAck, frame.request_id, 0, Vec::new())?,
                )?;
                break;
            }
            MessageType::RenderRequest => {
                let request: RenderRequestPayload = match decode_json(&frame.payload) {
                    Ok(request) => request,
                    Err(error) => {
                        send_error(&output_sender, frame.request_id, &error.to_string())?;
                        continue;
                    }
                };
                if let Err(error) = request.validate() {
                    send_error(&output_sender, frame.request_id, &error.to_string())?;
                    continue;
                }
                let mut active = active_job.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if active.is_some() {
                    send_error(&output_sender, frame.request_id, "another render job is active")?;
                    continue;
                }
                *active = Some(frame.request_id);
                drop(active);
                let control = Arc::new(JobControl::default());
                *active_control
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Arc::clone(&control));
                render_sender
                    .send(RenderCommand::Start {
                        job_id: frame.request_id,
                        request,
                        control,
                    })
                    .map_err(|_| ProtocolError::InvalidPayload("render thread stopped"))?;
            }
            MessageType::CapabilityProbe => {
                render_sender
                    .send(RenderCommand::CapabilityProbe {
                        request_id: frame.request_id,
                    })
                    .map_err(|_| ProtocolError::InvalidPayload("render thread stopped"))?;
            }
            MessageType::Control => {
                let control: ControlPayload = match decode_json(&frame.payload) {
                    Ok(control) => control,
                    Err(error) => {
                        send_error(&output_sender, frame.request_id, &error.to_string())?;
                        continue;
                    }
                };
                if control.schema_version != JSON_SCHEMA_VERSION {
                    send_error(&output_sender, frame.request_id, "unsupported control schema")?;
                    continue;
                }
                let active = *active_job.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if active != Some(control.job_id) {
                    send_error(&output_sender, frame.request_id, "unknown render job")?;
                    continue;
                }
                let Some(job_control) = active_control
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .as_ref()
                    .cloned()
                else {
                    send_error(&output_sender, frame.request_id, "render control is unavailable")?;
                    continue;
                };
                match control.command {
                    ControlCommand::Pause => job_control.pause(),
                    ControlCommand::Resume => job_control.resume(),
                    ControlCommand::Cancel => job_control.cancel(),
                }
            }
            MessageType::Hello
            | MessageType::HelloAck
            | MessageType::Event
            | MessageType::ShutdownAck
            | MessageType::Error
            | MessageType::CapabilityResult => {
                send_error(&output_sender, frame.request_id, "unexpected message type")?;
            }
        }
    }

    drop(render_sender);
    drop(output_sender);
    let _ = writer_thread.join();
    Ok(())
}

fn render_loop(
    receiver: Receiver<RenderCommand>,
    output: Sender<Frame>,
    active_job: Arc<Mutex<Option<u64>>>,
    active_control: Arc<Mutex<Option<Arc<JobControl>>>>,
) {
    while let Ok(command) = receiver.recv() {
        match command {
            RenderCommand::Start {
                job_id,
                request,
                control,
            } => {
                let events = prepare_resources(request, control);
                for event in events {
                    let terminal = matches!(
                        event,
                        JobEvent::Done { .. } | JobEvent::Canceled | JobEvent::Failed { .. }
                    );
                    let _ = send_event(&output, job_id, event);
                    if terminal {
                        *active_job.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
                        *active_control
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
                    }
                }
            }
            RenderCommand::CapabilityProbe { request_id } => {
                match probe_headless_context() {
                    Ok(payload) => {
                        let _ = send_frame(
                            &output,
                            Frame::new(MessageType::CapabilityResult, request_id, 0, payload)
                                .unwrap(),
                        );
                    }
                    Err(message) => {
                        let _ = send_error(&output, request_id, &message);
                    }
                }
            }
            RenderCommand::Shutdown => break,
        }
    }
}

fn send_frame(sender: &Sender<Frame>, frame: Frame) -> Result<(), ProtocolError> {
    sender
        .send(frame)
        .map_err(|_| ProtocolError::InvalidPayload("protocol writer stopped"))
}

fn send_error(sender: &Sender<Frame>, request_id: u64, message: &str) -> Result<(), ProtocolError> {
    send_frame(
        sender,
        Frame::new(MessageType::Error, request_id, 0, encode_error(message))?,
    )
}

fn send_event(sender: &Sender<Frame>, job_id: u64, event: JobEvent) -> Result<(), ProtocolError> {
    send_frame(
        sender,
        Frame::new(
            MessageType::Event,
            job_id,
            0,
            encode_json(&JobEventPayload {
                schema_version: JSON_SCHEMA_VERSION,
                job_id,
                event,
            })?,
        )?,
    )
}

fn prepare_resources(request: RenderRequestPayload, control: Arc<JobControl>) -> Vec<JobEvent> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let config = macroquad::window::Conf {
        window_title: "Phi Recorder Renderer".to_owned(),
        window_width: 16,
        window_height: 16,
        window_resizable: false,
        headless: true,
        ..Default::default()
    };

    macroquad::Window::from_config(config, async move {
        let mut events = vec![JobEvent::Started, JobEvent::Loading];
        match PreparedFrameRenderer::prepare(&request, &control).await {
            Ok((mut renderer, music_seconds, music_sample_rate)) => {
                if control.is_cancel_requested() {
                    events.push(JobEvent::Canceled);
                } else {
                    let start = Instant::now();
                    events.push(JobEvent::ResourcesReady {
                        music_seconds,
                        music_sample_rate,
                    });
                    match renderer.render_one_frame(0.0).and_then(|()| {
                        let readback = FrameReadback::new(&renderer)?;
                        let frame = readback.read_frame(&renderer)?;
                        let (width, height) = renderer.output_size();
                        let ffmpeg_path = Path::new(&request.resource_roots.ffmpeg_path);
                        let output_path = Path::new(&request.output_path);
                        let mut writer = FfmpegWriter::start(
                            ffmpeg_path,
                            width,
                            height,
                            renderer.fps(),
                            output_path,
                        )?;
                        writer.write_frame(&frame)?;
                        writer.finish()?;
                        Ok(())
                    }) {
                        Ok(()) => {
                            let (width, height) = renderer.output_size();
                            events.push(JobEvent::FrameReady { width, height });
                            events.push(JobEvent::Done {
                                duration_seconds: start.elapsed().as_secs_f64(),
                            });
                        }
                        Err(error) => events.push(JobEvent::Failed {
                            message: format!("render first frame: {error:#}"),
                        }),
                    }
                }
            }
            Err(error) => {
                if control.is_cancel_requested() {
                    events.push(JobEvent::Canceled);
                } else {
                    events.push(JobEvent::Failed {
                        message: error.to_string(),
                    });
                }
            }
        }
        let _ = sender.send(events);
        macroquad::window::miniquad::window::quit();
    });

    receiver.recv().unwrap_or_else(|_| {
        vec![JobEvent::Failed {
            message: "renderer job did not return a result".to_owned(),
        }]
    })
}

fn probe_headless_context() -> Result<Vec<u8>, String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let config = macroquad::window::Conf {
        window_title: "Phi Recorder Renderer Probe".to_owned(),
        window_width: 16,
        window_height: 16,
        window_resizable: false,
        headless: true,
        ..Default::default()
    };

    macroquad::Window::from_config(config, async move {
        let result = unsafe { collect_graphics_capabilities() };
        let _ = sender.send(result);
        macroquad::window::miniquad::window::quit();
    });

    receiver
        .recv()
        .map_err(|_| "headless renderer probe did not return a result".to_owned())?
}

unsafe fn collect_graphics_capabilities() -> Result<Vec<u8>, String> {
    let internal = macroquad::window::get_internal_gl();
    let vendor = read_gl_string(GL_VENDOR);
    let renderer = read_gl_string(GL_RENDERER);
    let version = read_gl_string(GL_VERSION);
    let shading_language_version = read_gl_string(GL_SHADING_LANGUAGE_VERSION);
    drop(internal);

    if vendor.is_none() || renderer.is_none() || version.is_none() {
        return Err("headless graphics context was created without GL identity strings".to_owned());
    }

    let renderer_name = renderer.clone().unwrap_or_default();
    let renderer_lower = renderer_name.to_ascii_lowercase();
    let software_renderer = ["llvmpipe", "software", "swiftshader", "softpipe"]
        .iter()
        .any(|name| renderer_lower.contains(name));

    serde_json::to_vec(&serde_json::json!({
        "status": "ok",
        "headless": true,
        "graphicsContext": true,
        "softwareRenderer": software_renderer,
        "glVendor": vendor,
        "glRenderer": renderer,
        "glVersion": version,
        "glslVersion": shading_language_version,
    }))
    .map_err(|error| error.to_string())
}

unsafe fn read_gl_string(name: u32) -> Option<String> {
    let value = glGetString(name);
    if value.is_null() {
        return None;
    }
    CStr::from_ptr(value.cast())
        .to_str()
        .ok()
        .map(str::to_owned)
}
