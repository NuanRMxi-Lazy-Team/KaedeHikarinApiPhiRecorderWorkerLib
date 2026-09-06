use std::{
    ffi::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use phi_recorder_protocol::{ControlCommand, JobEvent, RenderRequestPayload, ResourceRootsPayload};

use crate::{
    abi::{
        copy_utf8, ffi_status, phi_job_callback_fn, phi_job_event_t, phi_job_snapshot_t,
        phi_job_state_t, phi_render_request_t, phi_status_t, phi_string_view_t, PHI_ABI_VERSION,
        PHI_STATUS_BUSY, PHI_STATUS_INVALID_ARGUMENT, PHI_STATUS_INVALID_CONFIG, PHI_STATUS_OK,
    },
    context::phi_context,
    host::RendererHost,
};

static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(1);

#[repr(C)]
pub struct phi_job {
    job_id: u64,
    host: Mutex<Option<RendererHost>>,
    snapshot: Mutex<phi_job_snapshot_t>,
    callback: Option<phi_job_callback_fn>,
    user_data: *mut c_void,
    dispatcher: Mutex<Option<JoinHandle<()>>>,
    cancel_requested: AtomicBool,
    pause_requested: AtomicBool,
    context: *mut phi_context,
}

unsafe impl Sync for phi_job {}

impl phi_job {
    fn snapshot(&self) -> phi_job_snapshot_t {
        *self
            .snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn snapshot_init(job_id: u64) -> phi_job_snapshot_t {
    phi_job_snapshot_t {
        struct_size: std::mem::size_of::<phi_job_snapshot_t>() as u32,
        abi_version: PHI_ABI_VERSION,
        job_id,
        state: phi_job_state_t::Pending,
        progress: 0.0,
        fps: 0.0,
        estimated_seconds: 0.0,
        duration_seconds: 0.0,
        frame: 0,
        total_frames: 0,
    }
}

fn event_to_state(event: &JobEvent) -> phi_job_state_t {
    match event {
        JobEvent::Started | JobEvent::Loading => phi_job_state_t::Loading,
        JobEvent::Mixing | JobEvent::MixingSfx { .. } | JobEvent::ResourcesReady { .. } => {
            phi_job_state_t::Mixing
        }
        JobEvent::Rendering { .. } | JobEvent::Resumed | JobEvent::FrameReady { .. } => {
            phi_job_state_t::Rendering
        }
        JobEvent::Paused => phi_job_state_t::Paused,
        JobEvent::Done { .. } => phi_job_state_t::Done,
        JobEvent::Canceled => phi_job_state_t::Canceled,
        JobEvent::Failed { .. } => phi_job_state_t::Failed,
    }
}

fn is_terminal(event: &JobEvent) -> bool {
    matches!(
        event,
        JobEvent::Done { .. } | JobEvent::Canceled | JobEvent::Failed { .. }
    )
}

fn update_snapshot(snapshot: &mut phi_job_snapshot_t, event: &JobEvent) {
    snapshot.state = event_to_state(event);
    match event {
        JobEvent::MixingSfx { completed, total } => {
            snapshot.progress = *completed as f64 / (*total as f64).max(1.0);
        }
        JobEvent::Rendering {
            completed,
            total,
            fps,
            estimated_seconds,
        } => {
            snapshot.frame = *completed;
            snapshot.total_frames = *total;
            snapshot.progress = *completed as f64 / (*total as f64).max(1.0);
            snapshot.fps = *fps;
            snapshot.estimated_seconds = *estimated_seconds;
        }
        JobEvent::Paused => {}
        JobEvent::Done { duration_seconds } => {
            snapshot.progress = 1.0;
            snapshot.duration_seconds = *duration_seconds;
        }
        _ => {}
    }
}

fn dispatcher_loop(job: &phi_job) {
    let host = match job
        .host
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
    {
        Some(host) => host,
        None => return,
    };
    let mut pause_sent = false;
    let terminal_emitted = false;

    loop {
        if job.cancel_requested.load(Ordering::SeqCst) {
            host.force_kill();
            emit(&job, phi_job_state_t::Canceled, 0.0, 0.0, 0.0, 0.0, "");
            break;
        }
        let pause_requested = job.pause_requested.load(Ordering::SeqCst);
        if pause_requested != pause_sent {
            let command = if pause_requested {
                ControlCommand::Pause
            } else {
                ControlCommand::Resume
            };
            let _ = host.send_control(command);
            pause_sent = pause_requested;
        }

        let event = match host.try_recv_event() {
            Some(event) => event,
            None => {
                if host.is_exited() {
                    if !terminal_emitted {
                        let tail = host.stderr_tail();
                        let tail = tail.trim();
                        let message = if tail.is_empty() {
                            "renderer host exited before a terminal event".to_owned()
                        } else {
                            let start = tail.len().saturating_sub(1024);
                            format!(
                                "renderer host exited before a terminal event: {}",
                                &tail[start..]
                            )
                        };
                        emit(&job, phi_job_state_t::Failed, 0.0, 0.0, 0.0, 0.0, &message);
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
        };

        {
            let mut snapshot = job
                .snapshot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            update_snapshot(&mut snapshot, &event);
        }
        let snapshot = job.snapshot();
        let message = match &event {
            JobEvent::Failed { message } => message.as_str(),
            _ => "",
        };
        emit(
            &job,
            snapshot.state,
            snapshot.progress,
            snapshot.fps,
            snapshot.estimated_seconds,
            snapshot.duration_seconds,
            message,
        );
        if is_terminal(&event) {
            break;
        }
    }

    if let Some(context) = unsafe { job.context.as_ref() } {
        if let Ok(mut active) = context.active_job.lock() {
            if *active == Some(job as *const phi_job as *mut phi_job) {
                *active = None;
            }
        }
    }
}

fn emit(
    job: &phi_job,
    state: phi_job_state_t,
    progress: f64,
    fps: f64,
    estimated_seconds: f64,
    duration_seconds: f64,
    message: &str,
) {
    let Some(callback) = job.callback else {
        return;
    };
    let event = phi_job_event_t {
        struct_size: std::mem::size_of::<phi_job_event_t>() as u32,
        abi_version: PHI_ABI_VERSION,
        job_id: job.job_id,
        state,
        progress,
        fps,
        estimated_seconds,
        duration_seconds,
        message: phi_string_view_t {
            data: message.as_ptr(),
            length: message.len(),
        },
    };
    unsafe { callback(&event, job.user_data) };
}

fn build_request_payload(
    context: &phi_context,
    request: &phi_render_request_t,
) -> Result<RenderRequestPayload, phi_status_t> {
    let roots = context.resource_roots();
    let config = unsafe { (&*request.config).to_core() }?;
    let render_config_json =
        serde_json::to_string(&config).map_err(|_| PHI_STATUS_INVALID_CONFIG)?;
    let chart_info_json = if request.chart_info.is_null() {
        None
    } else {
        let info = unsafe { &*request.chart_info }.info();
        Some(serde_json::to_string(info).map_err(|_| PHI_STATUS_INVALID_CONFIG)?)
    };
    Ok(RenderRequestPayload {
        schema_version: phi_recorder_protocol::JSON_SCHEMA_VERSION,
        chart_path: unsafe { copy_utf8(request.chart_path) }?,
        output_path: unsafe { copy_utf8(request.output_path) }?,
        resource_roots: ResourceRootsPayload {
            assets_dir: roots.assets_dir.to_string_lossy().into_owned(),
            fonts_dir: roots.fonts_dir.to_string_lossy().into_owned(),
            resource_pack_dir: roots.resource_pack_dir.to_string_lossy().into_owned(),
            ffmpeg_path: roots.ffmpeg_path.to_string_lossy().into_owned(),
            temp_dir: roots.temp_dir.to_string_lossy().into_owned(),
            renderer_host_path: roots.renderer_host_path.to_string_lossy().into_owned(),
        },
        render_config_json,
        chart_info_json,
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_render_submit(
    context: *mut phi_context,
    request: *const phi_render_request_t,
    callback: Option<phi_job_callback_fn>,
    user_data: *mut c_void,
    out_job: *mut *mut phi_job,
) -> phi_status_t {
    ffi_status(|| {
        if context.is_null() || request.is_null() || out_job.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }
        *out_job = ptr::null_mut();
        let context_ref = &*context;
        let request_ref = &*request;
        if request_ref.config.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }
        if crate::abi::validate_header(
            request_ref.struct_size,
            request_ref.abi_version,
            std::mem::size_of::<phi_render_request_t>(),
        ) != PHI_STATUS_OK
        {
            return crate::PHI_STATUS_ABI_MISMATCH;
        }

        let mut active = context_ref
            .active_job
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.is_some() {
            return PHI_STATUS_BUSY;
        }

        let payload = match build_request_payload(context_ref, request_ref) {
            Ok(payload) => payload,
            Err(status) => return status,
        };

        let job_id = NEXT_JOB_ID.fetch_add(1, Ordering::SeqCst);
        let host = match RendererHost::start(
            &PathBuf::from(&payload.resource_roots.renderer_host_path),
            job_id,
            &payload,
        ) {
            Ok(host) => host,
            Err(error) => {
                context_ref.set_error(format!("{error:#}"));
                return crate::PHI_STATUS_INTERNAL_ERROR;
            }
        };

        let job = Box::new(phi_job {
            job_id,
            host: Mutex::new(Some(host)),
            snapshot: Mutex::new(snapshot_init(job_id)),
            callback,
            user_data,
            dispatcher: Mutex::new(None),
            cancel_requested: AtomicBool::new(false),
            pause_requested: AtomicBool::new(false),
            context,
        });
        let job_ptr: *mut phi_job = Box::into_raw(job);
        *active = Some(job_ptr);
        drop(active);

        let job_address = job_ptr as usize;
        let dispatcher =
            thread::spawn(move || dispatcher_loop(unsafe { &*(job_address as *mut phi_job) }));
        *(*job_ptr)
            .dispatcher
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(dispatcher);

        *out_job = job_ptr;
        PHI_STATUS_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_job_get_snapshot(
    job: *const phi_job,
    out_snapshot: *mut phi_job_snapshot_t,
) -> phi_status_t {
    ffi_status(|| {
        if job.is_null() || out_snapshot.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }
        *out_snapshot = (*job).snapshot();
        PHI_STATUS_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_job_cancel(job: *mut phi_job) -> phi_status_t {
    ffi_status(|| {
        if job.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }
        (*job).cancel_requested.store(true, Ordering::SeqCst);
        PHI_STATUS_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_job_pause(job: *mut phi_job) -> phi_status_t {
    ffi_status(|| {
        if job.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }
        (*job).pause_requested.store(true, Ordering::SeqCst);
        PHI_STATUS_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_job_resume(job: *mut phi_job) -> phi_status_t {
    ffi_status(|| {
        if job.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }
        (*job).pause_requested.store(false, Ordering::SeqCst);
        PHI_STATUS_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_job_destroy(job: *mut phi_job) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if job.is_null() {
            return;
        }
        (*job).cancel_requested.store(true, Ordering::SeqCst);
        if let Some(host) = (*job)
            .host
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
        {
            host.force_kill();
        }
        if let Some(dispatcher) = (*job)
            .dispatcher
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            let _ = dispatcher.join();
        }
        drop(Box::from_raw(job));
    }));
}
