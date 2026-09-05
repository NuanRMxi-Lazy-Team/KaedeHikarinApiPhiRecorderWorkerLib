use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    ptr, slice,
};

use chrono::{DateTime, Utc};
use phi_recorder_core::{load_chart_info_blocking, ChartFormat, ChartInfo};

use crate::{
    abi::{
        copy_utf8, ffi_status, phi_chart_info_view_t, phi_status_t, phi_string_view_t,
        validate_header, PHI_ABI_VERSION, PHI_CHART_FORMAT_PBC, PHI_CHART_FORMAT_PEC,
        PHI_CHART_FORMAT_PGR, PHI_CHART_FORMAT_RPE, PHI_STATUS_ABI_MISMATCH,
        PHI_STATUS_INVALID_ARGUMENT, PHI_STATUS_INVALID_CONFIG, PHI_STATUS_INVALID_UTF8,
        PHI_STATUS_OK,
    },
    context::phi_context,
};

const MAX_TAG_COUNT: usize = 1_000_000;

#[repr(C)]
pub struct phi_chart_info {
    info: ChartInfo,
    view: ChartInfoViewStorage,
}

struct ChartInfoViewStorage {
    created: Option<String>,
    updated: Option<String>,
    chart_updated: Option<String>,
    tag_views: Vec<phi_string_view_t>,
    view: phi_chart_info_view_t,
}

impl Default for ChartInfoViewStorage {
    fn default() -> Self {
        Self {
            created: None,
            updated: None,
            chart_updated: None,
            tag_views: Vec::new(),
            view: unsafe { std::mem::zeroed() },
        }
    }
}

impl phi_chart_info {
    fn new(info: ChartInfo) -> Self {
        let mut result = Self {
            info,
            view: ChartInfoViewStorage::default(),
        };
        result.refresh_view();
        result
    }

    fn refresh_view(&mut self) {
        let info = &self.info;
        let storage = &mut self.view;
        storage.created = info.created.map(|value| value.to_rfc3339());
        storage.updated = info.updated.map(|value| value.to_rfc3339());
        storage.chart_updated = info.chart_updated.map(|value| value.to_rfc3339());
        storage.tag_views = info.tags.iter().map(|tag| phi_string_view(tag)).collect();

        storage.view = phi_chart_info_view_t {
            struct_size: std::mem::size_of::<phi_chart_info_view_t>() as u32,
            abi_version: PHI_ABI_VERSION,
            id: info.id.unwrap_or_default(),
            has_id: info.id.is_some() as u8,
            guid: optional_view(info.guid.as_deref()),
            has_guid: info.guid.is_some() as u8,
            uploader: info.uploader.unwrap_or_default(),
            has_uploader: info.uploader.is_some() as u8,
            name: phi_string_view(&info.name),
            difficulty: info.difficulty,
            level: phi_string_view(&info.level),
            charter: phi_string_view(&info.charter),
            composer: phi_string_view(&info.composer),
            illustrator: phi_string_view(&info.illustrator),
            chart: phi_string_view(&info.chart),
            format: info.format.map(format_value).unwrap_or_default(),
            has_format: info.format.is_some() as u8,
            music: phi_string_view(&info.music),
            illustration: phi_string_view(&info.illustration),
            unlock_video: optional_view(info.unlock_video.as_deref()),
            has_unlock_video: info.unlock_video.is_some() as u8,
            preview_start: info.preview_start,
            preview_end: info.preview_end.unwrap_or_default(),
            has_preview_end: info.preview_end.is_some() as u8,
            aspect_ratio: info.aspect_ratio,
            force_aspect_ratio: info.force_aspect_ratio as u8,
            background_dim: info.background_dim,
            line_length: info.line_length,
            offset: info.offset,
            tip: optional_view(info.tip.as_deref()),
            has_tip: info.tip.is_some() as u8,
            tags: storage.tag_views.as_ptr(),
            tag_count: storage.tag_views.len(),
            intro: phi_string_view(&info.intro),
            hold_partial_cover: info.hold_partial_cover as u8,
            negative_length_hold: info.negative_length_hold as u8,
            note_uniform_scale: info.note_uniform_scale as u8,
            score_total: info.score_total,
            hold_particle_interval_ratio: info.hold_particle_interval_ratio,
            fold_animation: info.fold_animation as u8,
            created: optional_view(storage.created.as_deref()),
            has_created: storage.created.is_some() as u8,
            updated: optional_view(storage.updated.as_deref()),
            has_updated: storage.updated.is_some() as u8,
            chart_updated: optional_view(storage.chart_updated.as_deref()),
            has_chart_updated: storage.chart_updated.is_some() as u8,
        };
    }
}

fn phi_string_view(value: &str) -> phi_string_view_t {
    phi_string_view_t {
        data: value.as_ptr(),
        length: value.len(),
    }
}

fn optional_view(value: Option<&str>) -> phi_string_view_t {
    value.map(phi_string_view).unwrap_or(phi_string_view_t {
        data: ptr::null(),
        length: 0,
    })
}

fn format_value(value: ChartFormat) -> i32 {
    match value {
        ChartFormat::Rpe => PHI_CHART_FORMAT_RPE,
        ChartFormat::Pec => PHI_CHART_FORMAT_PEC,
        ChartFormat::Pgr => PHI_CHART_FORMAT_PGR,
        ChartFormat::Pbc => PHI_CHART_FORMAT_PBC,
    }
}

fn parse_format(value: i32) -> Result<ChartFormat, phi_status_t> {
    match value {
        PHI_CHART_FORMAT_RPE => Ok(ChartFormat::Rpe),
        PHI_CHART_FORMAT_PEC => Ok(ChartFormat::Pec),
        PHI_CHART_FORMAT_PGR => Ok(ChartFormat::Pgr),
        PHI_CHART_FORMAT_PBC => Ok(ChartFormat::Pbc),
        _ => Err(PHI_STATUS_INVALID_CONFIG),
    }
}

unsafe fn optional_string(
    value: phi_string_view_t,
    present: u8,
) -> Result<Option<String>, phi_status_t> {
    if present == 0 {
        return Ok(None);
    }
    let value = copy_utf8(value).map_err(|status| {
        if status == crate::PHI_STATUS_INVALID_ARGUMENT {
            PHI_STATUS_INVALID_CONFIG
        } else {
            PHI_STATUS_INVALID_UTF8
        }
    })?;
    Ok(Some(value))
}

unsafe fn optional_time(
    value: phi_string_view_t,
    present: u8,
) -> Result<Option<DateTime<Utc>>, phi_status_t> {
    let Some(value) = optional_string(value, present)? else {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(&value)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|_| PHI_STATUS_INVALID_CONFIG)
}

unsafe fn copy_tags(
    tags: *const phi_string_view_t,
    count: usize,
) -> Result<Vec<String>, phi_status_t> {
    if count > MAX_TAG_COUNT {
        return Err(PHI_STATUS_INVALID_CONFIG);
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    if tags.is_null() {
        return Err(PHI_STATUS_INVALID_CONFIG);
    }
    slice::from_raw_parts(tags, count)
        .iter()
        .map(|tag| copy_utf8(*tag).map_err(|_| PHI_STATUS_INVALID_CONFIG))
        .collect()
}

unsafe fn from_view(view: &phi_chart_info_view_t) -> Result<ChartInfo, phi_status_t> {
    if validate_header(
        view.struct_size,
        view.abi_version,
        std::mem::size_of::<phi_chart_info_view_t>(),
    ) != PHI_STATUS_OK
    {
        return Err(PHI_STATUS_ABI_MISMATCH);
    }

    Ok(ChartInfo {
        id: (view.has_id != 0).then_some(view.id),
        guid: optional_string(view.guid, view.has_guid)?,
        uploader: (view.has_uploader != 0).then_some(view.uploader),
        name: copy_utf8(view.name).map_err(|_| PHI_STATUS_INVALID_CONFIG)?,
        difficulty: view.difficulty,
        level: copy_utf8(view.level).map_err(|_| PHI_STATUS_INVALID_CONFIG)?,
        charter: copy_utf8(view.charter).map_err(|_| PHI_STATUS_INVALID_CONFIG)?,
        composer: copy_utf8(view.composer).map_err(|_| PHI_STATUS_INVALID_CONFIG)?,
        illustrator: copy_utf8(view.illustrator).map_err(|_| PHI_STATUS_INVALID_CONFIG)?,
        chart: copy_utf8(view.chart).map_err(|_| PHI_STATUS_INVALID_CONFIG)?,
        format: if view.has_format != 0 {
            Some(parse_format(view.format)?)
        } else {
            None
        },
        music: copy_utf8(view.music).map_err(|_| PHI_STATUS_INVALID_CONFIG)?,
        illustration: copy_utf8(view.illustration).map_err(|_| PHI_STATUS_INVALID_CONFIG)?,
        unlock_video: optional_string(view.unlock_video, view.has_unlock_video)?,
        preview_start: view.preview_start,
        preview_end: (view.has_preview_end != 0).then_some(view.preview_end),
        aspect_ratio: view.aspect_ratio,
        force_aspect_ratio: view.force_aspect_ratio != 0,
        background_dim: view.background_dim,
        line_length: view.line_length,
        offset: view.offset,
        tip: optional_string(view.tip, view.has_tip)?,
        tags: copy_tags(view.tags, view.tag_count)?,
        intro: copy_utf8(view.intro).map_err(|_| PHI_STATUS_INVALID_CONFIG)?,
        hold_partial_cover: view.hold_partial_cover != 0,
        negative_length_hold: view.negative_length_hold != 0,
        note_uniform_scale: view.note_uniform_scale != 0,
        score_total: view.score_total,
        hold_particle_interval_ratio: view.hold_particle_interval_ratio,
        fold_animation: view.fold_animation != 0,
        created: optional_time(view.created, view.has_created)?,
        updated: optional_time(view.updated, view.has_updated)?,
        chart_updated: optional_time(view.chart_updated, view.has_chart_updated)?,
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_chart_info_load(
    context: *mut phi_context,
    chart_path: phi_string_view_t,
    out_info: *mut *mut phi_chart_info,
) -> crate::phi_status_t {
    ffi_status(|| {
        if context.is_null() || out_info.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }
        *out_info = ptr::null_mut();
        let path = match copy_utf8(chart_path) {
            Ok(path) if !path.is_empty() => PathBuf::from(path),
            Ok(_) => return PHI_STATUS_INVALID_CONFIG,
            Err(status) => return status,
        };

        let result = std::thread::spawn(move || load_chart_info_blocking(path)).join();
        let info = match result {
            Ok(Ok(info)) => info,
            Ok(Err(error)) => {
                (*context).set_error(format!("{error:#}"));
                return PHI_STATUS_INVALID_CONFIG;
            }
            Err(_) => {
                (*context).set_error("chart parser thread panicked");
                return crate::PHI_STATUS_PANIC;
            }
        };

        *out_info = Box::into_raw(Box::new(phi_chart_info::new(info)));
        PHI_STATUS_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_chart_info_destroy(info: *mut phi_chart_info) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !info.is_null() {
            drop(Box::from_raw(info));
        }
    }));
}

#[no_mangle]
pub unsafe extern "C" fn phi_chart_info_get_view(
    info: *const phi_chart_info,
    out_view: *mut phi_chart_info_view_t,
) -> crate::phi_status_t {
    ffi_status(|| {
        if info.is_null() || out_view.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }
        *out_view = (*info).view.view;
        PHI_STATUS_OK
    })
}

#[no_mangle]
pub unsafe extern "C" fn phi_chart_info_set_view(
    info: *mut phi_chart_info,
    view: *const phi_chart_info_view_t,
) -> crate::phi_status_t {
    ffi_status(|| {
        if info.is_null() || view.is_null() {
            return PHI_STATUS_INVALID_ARGUMENT;
        }
        let replacement = match from_view(&*view) {
            Ok(value) => value,
            Err(status) => return status,
        };
        (*info).info = replacement;
        (*info).refresh_view();
        PHI_STATUS_OK
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_info_view_round_trips_owned_values() {
        let info = phi_chart_info::new(ChartInfo::default());
        let view = info.view.view;
        let replacement = unsafe { from_view(&view) }.unwrap();

        assert_eq!(replacement.name, "UK");
        assert_eq!(replacement.tags, Vec::<String>::new());
    }
}
