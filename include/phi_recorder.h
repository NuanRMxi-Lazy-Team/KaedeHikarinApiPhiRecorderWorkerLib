#ifndef PHI_RECORDER_H
#define PHI_RECORDER_H

#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32)
#if defined(PHI_RECORDER_BUILD)
#define PHI_API __declspec(dllexport)
#else
#define PHI_API __declspec(dllimport)
#endif
#define PHI_CALL __cdecl
#else
#define PHI_API __attribute__((visibility("default")))
#define PHI_CALL
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define PHI_ABI_VERSION UINT32_C(1)

typedef int32_t phi_status_t;

#define PHI_STATUS_OK INT32_C(0)
#define PHI_STATUS_INVALID_ARGUMENT INT32_C(1)
#define PHI_STATUS_ABI_MISMATCH INT32_C(2)
#define PHI_STATUS_BUFFER_TOO_SMALL INT32_C(3)
#define PHI_STATUS_INVALID_UTF8 INT32_C(4)
#define PHI_STATUS_OUT_OF_MEMORY INT32_C(5)
#define PHI_STATUS_NOT_IMPLEMENTED INT32_C(6)
#define PHI_STATUS_BUSY INT32_C(7)
#define PHI_STATUS_GRAPHICS_UNAVAILABLE INT32_C(8)
#define PHI_STATUS_FFMPEG_UNAVAILABLE INT32_C(9)
#define PHI_STATUS_INVALID_STATE INT32_C(10)
#define PHI_STATUS_INTERNAL_ERROR INT32_C(11)
#define PHI_STATUS_PANIC INT32_C(12)
#define PHI_STATUS_CANCELED INT32_C(13)
#define PHI_STATUS_INVALID_CONFIG INT32_C(14)

typedef struct phi_string_view {
    const uint8_t* data;
    size_t length;
} phi_string_view_t;

typedef struct phi_struct_header {
    uint32_t struct_size;
    uint32_t abi_version;
} phi_struct_header_t;

typedef struct phi_resolution {
    uint32_t width;
    uint32_t height;
} phi_resolution_t;

typedef struct phi_context_options {
    uint32_t struct_size;
    uint32_t abi_version;
    phi_string_view_t assets_dir;
    phi_string_view_t fonts_dir;
    phi_string_view_t resource_pack_dir;
    phi_string_view_t ffmpeg_path;
    phi_string_view_t temp_dir;
    phi_string_view_t renderer_host_path;
} phi_context_options_t;

typedef struct phi_render_config {
    uint32_t struct_size;
    uint32_t abi_version;

    phi_resolution_t resolution;
    double ending_length;
    uint8_t render_loading;
    uint8_t hires;
    float chart_debug_line;
    float chart_debug_note;
    float chart_ratio;
    uint8_t all_good;
    uint8_t all_bad;
    uint32_t fps;
    uint8_t hardware_accel;
    uint8_t hevc;
    uint8_t mpeg4;
    phi_string_view_t custom_encoder;
    uint8_t dynamic_bitrate_control;
    phi_string_view_t bitrate;

    uint8_t aggressive_chart;
    uint8_t aggressive_note;
    uint8_t aggressive_particle;
    int32_t challenge_color;
    uint32_t challenge_rank;
    float note_scale;
    uint8_t particle;
    phi_string_view_t player_avatar;
    phi_string_view_t player_name;
    float player_rks;
    uint32_t sample_count;
    uint8_t fxaa;
    phi_string_view_t resource_pack_path;
    float speed;
    float volume_music;
    float volume_sfx;
    uint8_t force_limit;
    float limit_threshold;
    uint8_t loudness_equalization;
    int32_t audio_mix_mode;
    phi_string_view_t watermark;
    uint8_t roman;
    uint8_t chinese;
    phi_string_view_t combo;
    phi_string_view_t difficulty;
    double judge_offset;
    phi_string_view_t file_name_format;

    uint8_t render_line;
    uint8_t render_line_extra;
    uint8_t render_note;
    uint8_t render_double_hint;
    uint8_t render_ui_pause;
    uint8_t render_ui_name;
    uint8_t render_ui_level;
    uint8_t render_ui_score;
    uint8_t render_ui_combo;
    uint8_t render_ui_bar;
    uint8_t render_bg;
    uint8_t render_bg_dim;
    uint8_t preserve_framebuffer;
    uint8_t render_extra;
    float background_blurriness;

    uint64_t max_particles;
    double play_start_time;
    double play_end_time;
    uint8_t has_play_end_time;
    float fade;
    uint8_t alpha_tint;
} phi_render_config_t;

typedef struct phi_context phi_context_t;
typedef struct phi_chart_info phi_chart_info_t;

typedef struct phi_chart_info_view {
    uint32_t struct_size;
    uint32_t abi_version;

    int32_t id;
    uint8_t has_id;
    phi_string_view_t guid;
    uint8_t has_guid;
    int32_t uploader;
    uint8_t has_uploader;

    phi_string_view_t name;
    float difficulty;
    phi_string_view_t level;
    phi_string_view_t charter;
    phi_string_view_t composer;
    phi_string_view_t illustrator;
    phi_string_view_t chart;
    int32_t format;
    uint8_t has_format;
    phi_string_view_t music;
    phi_string_view_t illustration;
    phi_string_view_t unlock_video;
    uint8_t has_unlock_video;

    double preview_start;
    double preview_end;
    uint8_t has_preview_end;
    float aspect_ratio;
    uint8_t force_aspect_ratio;
    float background_dim;
    float line_length;
    double offset;
    phi_string_view_t tip;
    uint8_t has_tip;
    const phi_string_view_t* tags;
    size_t tag_count;

    phi_string_view_t intro;
    uint8_t hold_partial_cover;
    uint8_t negative_length_hold;
    uint8_t note_uniform_scale;
    uint32_t score_total;
    float hold_particle_interval_ratio;
    uint8_t fold_animation;

    phi_string_view_t created;
    uint8_t has_created;
    phi_string_view_t updated;
    uint8_t has_updated;
    phi_string_view_t chart_updated;
    uint8_t has_chart_updated;
} phi_chart_info_view_t;

/* Returns the ABI version implemented by the loaded library. */
PHI_API uint32_t PHI_CALL phi_abi_version(void);

/* The caller owns the returned context and must destroy it exactly once. */
PHI_API phi_status_t PHI_CALL phi_context_create(
    const phi_context_options_t* options,
    phi_context_t** out_context);

/* Destroy waits for all future context-owned work in the complete library. */
PHI_API void PHI_CALL phi_context_destroy(phi_context_t* context);

PHI_API phi_status_t PHI_CALL phi_context_clear_error(phi_context_t* context);

/* The required size excludes a terminator because strings are ptr+length values. */
PHI_API phi_status_t PHI_CALL phi_context_get_last_error(
    const phi_context_t* context,
    uint8_t* buffer,
    size_t capacity,
    size_t* required);

/* Initializes the code-defined ABI defaults; it never reads a config file. */
PHI_API phi_status_t PHI_CALL phi_render_config_init_default(
    phi_render_config_t* config);

PHI_API phi_status_t PHI_CALL phi_render_config_validate(
    const phi_render_config_t* config);

PHI_API phi_status_t PHI_CALL phi_chart_info_load(
    phi_context_t* context,
    phi_string_view_t chart_path,
    phi_chart_info_t** out_info);

PHI_API void PHI_CALL phi_chart_info_destroy(phi_chart_info_t* info);

/*
 * The returned view borrows storage owned by info. Its pointers remain valid
 * until the next set_view call or destroy call for the same handle.
 */
PHI_API phi_status_t PHI_CALL phi_chart_info_get_view(
    const phi_chart_info_t* info,
    phi_chart_info_view_t* out_view);

/* Deep-copies all strings, timestamps and tags before replacing the handle. */
PHI_API phi_status_t PHI_CALL phi_chart_info_set_view(
    phi_chart_info_t* info,
    const phi_chart_info_view_t* view);

#ifdef __cplusplus
}
#endif

#endif /* PHI_RECORDER_H */
