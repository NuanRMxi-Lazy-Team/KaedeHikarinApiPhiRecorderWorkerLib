#include "phi_recorder.h"

#include <stddef.h>

_Static_assert(sizeof(phi_struct_header_t) == 8, "unexpected ABI header size");
_Static_assert(sizeof(phi_resolution_t) == 8, "unexpected resolution size");
_Static_assert(
    offsetof(phi_context_options_t, assets_dir) == sizeof(phi_struct_header_t),
    "context options header offset changed");
_Static_assert(
    offsetof(phi_render_config_t, resolution) == sizeof(phi_struct_header_t),
    "render config header offset changed");
_Static_assert(
    offsetof(phi_render_config_t, custom_encoder) > offsetof(phi_render_config_t, mpeg4),
    "render config field order changed");
_Static_assert(
    offsetof(phi_chart_info_view_t, name) > offsetof(phi_chart_info_view_t, has_uploader),
    "chart info view field order changed");
_Static_assert(
    offsetof(phi_chart_info_view_t, tags) > offsetof(phi_chart_info_view_t, tip),
    "chart info tags field order changed");
_Static_assert(
    offsetof(phi_render_request_t, config) > offsetof(phi_render_request_t, output_path),
    "render request field order changed");
_Static_assert(
    offsetof(phi_job_snapshot_t, state) > offsetof(phi_job_snapshot_t, job_id),
    "job snapshot field order changed");

int main(void) {
    phi_context_options_t options = {0};
    phi_render_config_t config = {0};

    options.struct_size = (uint32_t)sizeof(options);
    options.abi_version = PHI_ABI_VERSION;
    config.struct_size = (uint32_t)sizeof(config);
    config.abi_version = PHI_ABI_VERSION;

    return (options.struct_size == 0 || config.struct_size == 0) ? 1 : 0;
}
