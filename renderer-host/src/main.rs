use std::{
    ffi::CStr,
    io::{self, BufReader, BufWriter},
    sync::mpsc,
};

use phi_recorder_protocol::{
    decode_json, decode_protocol_version, encode_error, encode_json, encode_protocol_version,
    read_frame, write_frame, ControlPayload, Frame, JobEvent, JobEventPayload, MessageType,
    ProtocolError, RenderRequestPayload, JSON_SCHEMA_VERSION, PROTOCOL_VERSION,
};
use macroquad::miniquad::gl::glGetString;

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

fn run() -> Result<(), ProtocolError> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    let hello = read_frame(&mut reader)?.ok_or(ProtocolError::InvalidPayload(
        "the first frame must be a hello message",
    ))?;
    if hello.message_type != MessageType::Hello {
        send_error(
            &mut writer,
            hello.request_id,
            "the first message must be hello",
        )?;
        return Err(ProtocolError::InvalidPayload("missing hello message"));
    }
    if decode_protocol_version(&hello.payload)? != PROTOCOL_VERSION {
        send_error(
            &mut writer,
            hello.request_id,
            "protocol version mismatch",
        )?;
        return Err(ProtocolError::UnsupportedVersion(
            decode_protocol_version(&hello.payload)?,
        ));
    }

    write_frame(
        &mut writer,
        &Frame::new(
            MessageType::HelloAck,
            hello.request_id,
            0,
            encode_protocol_version(),
        )?,
    )?;

    while let Some(frame) = read_frame(&mut reader)? {
        match frame.message_type {
            MessageType::Shutdown => {
                write_frame(
                    &mut writer,
                    &Frame::new(MessageType::ShutdownAck, frame.request_id, 0, Vec::new())?,
                )?;
                return Ok(());
            }
            MessageType::RenderRequest => {
                let request: RenderRequestPayload = match decode_json(&frame.payload) {
                    Ok(request) => request,
                    Err(error) => {
                        send_error(&mut writer, frame.request_id, &error.to_string())?;
                        continue;
                    }
                };
                if let Err(error) = request.validate() {
                    send_error(&mut writer, frame.request_id, &error.to_string())?;
                    continue;
                }
                send_event(
                    &mut writer,
                    frame.request_id,
                    JobEvent::Started,
                )?;
                send_event(
                    &mut writer,
                    frame.request_id,
                    JobEvent::Failed {
                        message: "render backend is not connected to the private host yet"
                            .to_owned(),
                    },
                )?;
            }
            MessageType::CapabilityProbe => {
                match probe_headless_context() {
                    Ok(payload) => write_frame(
                        &mut writer,
                        &Frame::new(
                            MessageType::CapabilityResult,
                            frame.request_id,
                            0,
                            payload,
                        )?,
                    )?,
                    Err(message) => send_error(&mut writer, frame.request_id, &message)?,
                }
            }
            MessageType::Control => {
                let control: ControlPayload = match decode_json(&frame.payload) {
                    Ok(control) => control,
                    Err(error) => {
                        send_error(&mut writer, frame.request_id, &error.to_string())?;
                        continue;
                    }
                };
                if control.schema_version != JSON_SCHEMA_VERSION {
                    send_error(&mut writer, frame.request_id, "unsupported control schema")?;
                    continue;
                }
                send_error(
                    &mut writer,
                    frame.request_id,
                    "control command received before a render backend is connected",
                )?;
            }
            MessageType::Hello
            | MessageType::HelloAck
            | MessageType::Event
            | MessageType::ShutdownAck
            | MessageType::Error
            | MessageType::CapabilityResult => {
                send_error(
                    &mut writer,
                    frame.request_id,
                    "unexpected message type",
                )?;
            }
        }
    }

    Ok(())
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

fn send_error<W: io::Write>(
    writer: &mut W,
    request_id: u64,
    message: &str,
) -> Result<(), ProtocolError> {
    write_frame(
        writer,
        &Frame::new(MessageType::Error, request_id, 0, encode_error(message))?,
    )
}

fn send_event<W: io::Write>(
    writer: &mut W,
    job_id: u64,
    event: JobEvent,
) -> Result<(), ProtocolError> {
    let payload = encode_json(&JobEventPayload {
        schema_version: JSON_SCHEMA_VERSION,
        job_id,
        event,
    })?;
    write_frame(
        writer,
        &Frame::new(MessageType::Event, job_id, 0, payload)?,
    )
}
