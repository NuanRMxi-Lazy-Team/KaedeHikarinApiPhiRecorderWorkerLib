use std::io::{self, BufReader, BufWriter};

use phi_recorder_protocol::{
    decode_protocol_version, encode_error, encode_protocol_version, read_frame, write_frame, Frame,
    MessageType, ProtocolError, PROTOCOL_VERSION,
};

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
                send_error(
                    &mut writer,
                    frame.request_id,
                    "render backend is not connected to the private host yet",
                )?;
            }
            MessageType::Control => {
                send_error(
                    &mut writer,
                    frame.request_id,
                    "control commands are not implemented in the protocol skeleton",
                )?;
            }
            MessageType::Hello
            | MessageType::HelloAck
            | MessageType::Event
            | MessageType::ShutdownAck
            | MessageType::Error => {
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
