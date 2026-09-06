# AGENTS.md — Phi Recorder Native

## Project Overview

Pure native library that renders Phigros rhythm game charts (RPE/PGR) to video.

- **Root crate**: `phi-recorder-native` (cdylib/staticlib/rlib), the only public entry point is the C ABI declared in `include/phi_recorder.h`
- **renderer-core**: pure logic (config, timeline, ffmpeg plan, chart info, events, job control)
- **renderer-protocol**: private PHIR binary protocol between the DLL and `renderer-host`
- **renderer-host**: private helper executable (headless macroquad + phire + sasa + ffmpeg pipe) spawned and managed by the DLL for real force-cancel process isolation
- **External git deps**: `phire` (chart parser), `macroquad` (OpenGL rendering), `sasa` (audio)

## Commands

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo build -p phi-renderer-host   # required before end-to-end tests that spawn the host

# C header smoke test
gcc -std=c11 -Wall -Wextra -Werror -I include -c tests/c_header_smoke.c -o <temp>/phi_recorder_c_header_smoke.o
```

## Architecture

- `src/` — C ABI surface (`abi.rs`, `context.rs`, `job.rs`, `host.rs`, `chart.rs`, `config.rs`, `lib.rs`)
- `include/phi_recorder.h` — public C header, mirrors the ABI structs
- `assets/` — runtime assets required by the renderer (fonts, UI textures, `respack/`, `rank/`); the caller passes the assets directory explicitly via `phi_context_options_t`
- `tests/` — Rust integration tests plus the C layout smoke file

## Key Conventions

- **Rust**: edition 2021, min rustc 1.77.2; release profile uses LTO + strip
- All C structs start with `struct_size + abi_version`; strings are UTF-8 `ptr + length`; inputs are deep-copied by the native side immediately
- One active job per context; a second submit returns `BUSY`
- Callbacks run on a per-job dispatcher thread; re-entering the native API from a callback is forbidden
- Never commit `TODO.md`
- Commits are stepwise with the template `feat: 中文描述`

## Gotchas

- The `phire` crate uses a custom macro `tl_file!` for localized error messages
- phire's filesystem layer needs a Tokio runtime; `renderer-host` enters a current-thread runtime on its render thread (`renderer-host/src/main.rs`)
- Keep a single macroquad/sasa git source identity across the workspace (same URL form, same rev), otherwise duplicate global symbols break linking
- Console window is hidden on Windows in non-debug mode via WinAPI
