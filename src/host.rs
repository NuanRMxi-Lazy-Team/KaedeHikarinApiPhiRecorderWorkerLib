use std::{
    io::{BufReader, Read},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result};
use phi_recorder_protocol::{
    decode_json, decode_protocol_version, encode_json, read_frame, write_frame, ControlCommand,
    Frame, JobEvent, JobEventPayload, MessageType, RenderRequestPayload, JSON_SCHEMA_VERSION,
    PROTOCOL_VERSION,
};

#[allow(dead_code)]
pub struct RendererHost {
    child: Mutex<Child>,
    input: Sender<Frame>,
    events: Receiver<JobEvent>,
    writer: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
    stderr: Option<JoinHandle<()>>,
    job_id: u64,
}

#[allow(dead_code)]
impl RendererHost {
    pub fn start(host_path: &Path, job_id: u64, request: &RenderRequestPayload) -> Result<Self> {
        let mut command = command_hidden(host_path);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .with_context(|| format!("start renderer host at {}", host_path.display()))?;
        let stdin = child
            .stdin
            .take()
            .context("renderer host stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("renderer host stdout unavailable")?;
        let stderr = child
            .stderr
            .take()
            .context("renderer host stderr unavailable")?;

        let (input_sender, input_receiver) = mpsc::channel::<Frame>();
        let writer = thread::spawn(move || {
            let mut stdin = stdin;
            while let Ok(frame) = input_receiver.recv() {
                if write_frame(&mut stdin, &frame).is_err() {
                    break;
                }
            }
        });

        let (event_sender, event_receiver) = mpsc::channel::<JobEvent>();
        let reader = thread::spawn(move || {
            let mut stdout = BufReader::new(stdout);
            while let Ok(Some(frame)) = read_frame(&mut stdout) {
                match frame.message_type {
                    MessageType::Event => {
                        if let Ok(payload) = decode_json::<JobEventPayload>(&frame.payload) {
                            if event_sender.send(payload.event).is_err() {
                                break;
                            }
                        }
                    }
                    MessageType::Error => {
                        let message = String::from_utf8_lossy(&frame.payload).into_owned();
                        if event_sender.send(JobEvent::Failed { message }).is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        });

        let stderr_reader = thread::spawn(move || {
            let mut buffer = String::new();
            let _ = stderr.take(0).read_to_string(&mut buffer);
        });

        let host = Self {
            child: Mutex::new(child),
            input: input_sender,
            events: event_receiver,
            writer: Some(writer),
            reader: Some(reader),
            stderr: Some(stderr_reader),
            job_id,
        };

        host.send_frame(Frame::new(
            MessageType::Hello,
            job_id,
            0,
            phi_recorder_protocol::encode_protocol_version(),
        )?)?;
        host.send_frame(Frame::new(
            MessageType::RenderRequest,
            job_id,
            0,
            encode_json(request)?,
        )?)?;

        Ok(host)
    }

    fn send_frame(&self, frame: Frame) -> Result<()> {
        self.input
            .send(frame)
            .map_err(|_| anyhow::anyhow!("renderer host writer stopped"))
    }

    pub fn send_control(&self, command: ControlCommand) -> Result<()> {
        self.send_frame(Frame::new(
            MessageType::Control,
            self.job_id,
            0,
            encode_json(&phi_recorder_protocol::ControlPayload {
                schema_version: JSON_SCHEMA_VERSION,
                job_id: self.job_id,
                command,
            })?,
        )?)
    }

    pub fn try_recv_event(&self) -> Option<JobEvent> {
        self.events.try_recv().ok()
    }

    pub fn force_kill(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn is_exited(&self) -> bool {
        self.child
            .lock()
            .map(|mut child| child.try_wait().ok().flatten().is_some())
            .unwrap_or(true)
    }

    pub fn wait_for_terminal(&self) -> Option<JobEvent> {
        while let Ok(event) = self.events.recv() {
            match event {
                JobEvent::Done { .. } | JobEvent::Canceled | JobEvent::Failed { .. } => {
                    return Some(event);
                }
                _ => {}
            }
        }
        None
    }

    fn shutdown(mut self) {
        if let Ok(frame) = Frame::new(MessageType::Shutdown, self.job_id, 0, Vec::new()) {
            let _ = self.send_frame(frame);
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.wait();
        }
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Some(stderr) = self.stderr.take() {
            let _ = stderr.join();
        }
    }
}

impl Drop for RendererHost {
    fn drop(&mut self) {
        let mut child = match self.child.lock() {
            Ok(child) => child,
            Err(_) => return,
        };
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn command_hidden(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

#[allow(dead_code)]
pub fn check_protocol(host_path: &Path) -> Result<()> {
    let mut command = command_hidden(host_path);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("start renderer host at {}", host_path.display()))?;
    let stdin = child
        .stdin
        .take()
        .context("renderer host stdin unavailable")?;
    let mut stdout = BufReader::new(child.stdout.take().context("stdout unavailable")?);

    let mut input = stdin;
    write_frame(
        &mut input,
        &Frame::new(
            MessageType::Hello,
            0,
            0,
            phi_recorder_protocol::encode_protocol_version(),
        )?,
    )?;
    let hello_ack = read_frame(&mut stdout)?;
    let ack = hello_ack.ok_or_else(|| anyhow::anyhow!("missing hello ack"))?;
    if ack.message_type != MessageType::HelloAck
        || decode_protocol_version(&ack.payload)? != PROTOCOL_VERSION
    {
        anyhow::bail!("unexpected hello ack");
    }
    write_frame(
        &mut input,
        &Frame::new(MessageType::Shutdown, 0, 0, Vec::new())?,
    )?;
    let shutdown_ack =
        read_frame(&mut stdout)?.ok_or_else(|| anyhow::anyhow!("missing shutdown ack"))?;
    if shutdown_ack.message_type != MessageType::ShutdownAck {
        anyhow::bail!("unexpected shutdown ack");
    }
    drop(input);
    let _ = child.wait();
    Ok(())
}
