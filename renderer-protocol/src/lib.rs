use std::{
    fmt,
    io::{self, Read, Write},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const FRAME_MAGIC: [u8; 4] = *b"PHIR";
pub const FRAME_HEADER_SIZE: usize = 24;
pub const MAX_PAYLOAD_SIZE: usize = 64 * 1024 * 1024;
pub const JSON_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    Hello = 1,
    HelloAck = 2,
    RenderRequest = 3,
    Control = 4,
    Event = 5,
    Shutdown = 6,
    ShutdownAck = 7,
    Error = 8,
    CapabilityProbe = 9,
    CapabilityResult = 10,
}

impl MessageType {
    fn from_raw(value: u16) -> Result<Self, ProtocolError> {
        match value {
            1 => Ok(Self::Hello),
            2 => Ok(Self::HelloAck),
            3 => Ok(Self::RenderRequest),
            4 => Ok(Self::Control),
            5 => Ok(Self::Event),
            6 => Ok(Self::Shutdown),
            7 => Ok(Self::ShutdownAck),
            8 => Ok(Self::Error),
            9 => Ok(Self::CapabilityProbe),
            10 => Ok(Self::CapabilityResult),
            _ => Err(ProtocolError::UnknownMessageType(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub message_type: MessageType,
    pub request_id: u64,
    pub flags: u32,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(
        message_type: MessageType,
        request_id: u64,
        flags: u32,
        payload: Vec<u8>,
    ) -> Result<Self, ProtocolError> {
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::PayloadTooLarge(payload.len()));
        }

        Ok(Self {
            message_type,
            request_id,
            flags,
            payload,
        })
    }
}

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    InvalidMagic([u8; 4]),
    UnsupportedVersion(u16),
    UnknownMessageType(u16),
    PayloadTooLarge(usize),
    InvalidPayload(&'static str),
    Json(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::InvalidMagic(magic) => write!(formatter, "invalid frame magic: {magic:?}"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported protocol version: {version}")
            }
            Self::UnknownMessageType(message_type) => {
                write!(formatter, "unknown message type: {message_type}")
            }
            Self::PayloadTooLarge(size) => write!(formatter, "payload is too large: {size}"),
            Self::InvalidPayload(message) => write!(formatter, "invalid payload: {message}"),
            Self::Json(message) => write!(formatter, "JSON payload error: {message}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<io::Error> for ProtocolError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn read_frame<R: Read>(reader: &mut R) -> Result<Option<Frame>, ProtocolError> {
    let mut header = [0u8; FRAME_HEADER_SIZE];
    let first_read = reader.read(&mut header[..4])?;
    if first_read == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[first_read..])?;

    let magic = [header[0], header[1], header[2], header[3]];
    if magic != FRAME_MAGIC {
        return Err(ProtocolError::InvalidMagic(magic));
    }

    let version = u16::from_le_bytes([header[4], header[5]]);
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(version));
    }

    let message_type = MessageType::from_raw(u16::from_le_bytes([header[6], header[7]]))?;
    let request_id = u64::from_le_bytes(header[8..16].try_into().unwrap());
    let payload_size = u32::from_le_bytes(header[16..20].try_into().unwrap()) as usize;
    let flags = u32::from_le_bytes(header[20..24].try_into().unwrap());

    if payload_size > MAX_PAYLOAD_SIZE {
        return Err(ProtocolError::PayloadTooLarge(payload_size));
    }

    let mut payload = vec![0u8; payload_size];
    reader.read_exact(&mut payload)?;

    Ok(Some(Frame {
        message_type,
        request_id,
        flags,
        payload,
    }))
}

pub fn write_frame<W: Write>(writer: &mut W, frame: &Frame) -> Result<(), ProtocolError> {
    if frame.payload.len() > MAX_PAYLOAD_SIZE {
        return Err(ProtocolError::PayloadTooLarge(frame.payload.len()));
    }

    let mut header = [0u8; FRAME_HEADER_SIZE];
    header[0..4].copy_from_slice(&FRAME_MAGIC);
    header[4..6].copy_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    header[6..8].copy_from_slice(&(frame.message_type as u16).to_le_bytes());
    header[8..16].copy_from_slice(&frame.request_id.to_le_bytes());
    header[16..20].copy_from_slice(&(frame.payload.len() as u32).to_le_bytes());
    header[20..24].copy_from_slice(&frame.flags.to_le_bytes());

    writer.write_all(&header)?;
    writer.write_all(&frame.payload)?;
    writer.flush()?;
    Ok(())
}

pub fn encode_protocol_version() -> Vec<u8> {
    PROTOCOL_VERSION.to_le_bytes().to_vec()
}

pub fn decode_protocol_version(payload: &[u8]) -> Result<u16, ProtocolError> {
    let bytes: [u8; 2] = payload
        .try_into()
        .map_err(|_| ProtocolError::InvalidPayload("protocol version must be two bytes"))?;
    Ok(u16::from_le_bytes(bytes))
}

pub fn encode_error(message: &str) -> Vec<u8> {
    message.as_bytes().to_vec()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceRootsPayload {
    pub assets_dir: String,
    pub fonts_dir: String,
    pub resource_pack_dir: String,
    pub ffmpeg_path: String,
    pub temp_dir: String,
    pub renderer_host_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderRequestPayload {
    pub schema_version: u16,
    pub chart_path: String,
    pub output_path: String,
    pub resource_roots: ResourceRootsPayload,
    pub render_config_json: String,
    pub chart_info_json: Option<String>,
}

impl RenderRequestPayload {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != JSON_SCHEMA_VERSION {
            return Err(ProtocolError::InvalidPayload("unsupported render request schema"));
        }
        if self.chart_path.is_empty() || self.output_path.is_empty() {
            return Err(ProtocolError::InvalidPayload(
                "render request paths cannot be empty",
            ));
        }
        if self.render_config_json.is_empty() {
            return Err(ProtocolError::InvalidPayload("render config cannot be empty"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ControlCommand {
    Pause,
    Resume,
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlPayload {
    pub schema_version: u16,
    pub job_id: u64,
    pub command: ControlCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobEvent {
    Started,
    Loading,
    Mixing,
    MixingSfx {
        completed: u64,
        total: u64,
    },
    ResourcesReady {
        music_seconds: f64,
        music_sample_rate: u32,
    },
    Rendering {
        completed: u64,
        total: u64,
        fps: f64,
        estimated_seconds: f64,
    },
    Paused,
    Resumed,
    Done {
        duration_seconds: f64,
    },
    Canceled,
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobEventPayload {
    pub schema_version: u16,
    pub job_id: u64,
    pub event: JobEvent,
}

pub fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
    serde_json::to_vec(value).map_err(|error| ProtocolError::Json(error.to_string()))
}

pub fn decode_json<T: DeserializeOwned>(payload: &[u8]) -> Result<T, ProtocolError> {
    serde_json::from_slice(payload).map_err(|error| ProtocolError::Json(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trip_preserves_header_and_payload() {
        let frame = Frame::new(MessageType::Event, 42, 7, b"payload".to_vec()).unwrap();
        let mut encoded = Vec::new();
        write_frame(&mut encoded, &frame).unwrap();

        let decoded = read_frame(&mut encoded.as_slice()).unwrap().unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn eof_before_a_frame_is_not_an_error() {
        let decoded = read_frame(&mut (&[][..])).unwrap();
        assert_eq!(decoded, None);
    }

    #[test]
    fn truncated_frame_is_an_error() {
        let result = read_frame(&mut (&FRAME_MAGIC[..])).unwrap_err();
        assert!(matches!(result, ProtocolError::Io(_)));
    }

    #[test]
    fn oversized_payload_is_rejected_before_allocation() {
        let mut encoded = [0u8; FRAME_HEADER_SIZE];
        encoded[0..4].copy_from_slice(&FRAME_MAGIC);
        encoded[4..6].copy_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        encoded[6..8].copy_from_slice(&(MessageType::Event as u16).to_le_bytes());
        encoded[16..20].copy_from_slice(&((MAX_PAYLOAD_SIZE as u32) + 1).to_le_bytes());

        let result = read_frame(&mut encoded.as_slice()).unwrap_err();
        assert!(matches!(result, ProtocolError::PayloadTooLarge(_)));
    }

    #[test]
    fn capability_messages_have_stable_numeric_values() {
        assert_eq!(MessageType::CapabilityProbe as u16, 9);
        assert_eq!(MessageType::CapabilityResult as u16, 10);
    }

    #[test]
    fn render_request_round_trip_preserves_schema_and_paths() {
        let request = RenderRequestPayload {
            schema_version: JSON_SCHEMA_VERSION,
            chart_path: "chart.pez".to_owned(),
            output_path: "output.mp4".to_owned(),
            resource_roots: ResourceRootsPayload {
                assets_dir: "assets".to_owned(),
                fonts_dir: "fonts".to_owned(),
                resource_pack_dir: "respacks".to_owned(),
                ffmpeg_path: "ffmpeg".to_owned(),
                temp_dir: "temp".to_owned(),
                renderer_host_path: "renderer-host".to_owned(),
            },
            render_config_json: "{}".to_owned(),
            chart_info_json: None,
        };

        let encoded = encode_json(&request).unwrap();
        let decoded: RenderRequestPayload = decode_json(&encoded).unwrap();
        assert_eq!(decoded, request);
        assert!(decoded.validate().is_ok());
    }

    #[test]
    fn job_event_has_explicit_terminal_failure() {
        let event = JobEventPayload {
            schema_version: JSON_SCHEMA_VERSION,
            job_id: 7,
            event: JobEvent::Failed {
                message: "not implemented".to_owned(),
            },
        };

        let encoded = encode_json(&event).unwrap();
        let decoded: JobEventPayload = decode_json(&encoded).unwrap();
        assert_eq!(decoded, event);
    }
}
