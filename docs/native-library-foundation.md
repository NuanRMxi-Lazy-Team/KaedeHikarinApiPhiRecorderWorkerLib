# Native Library Foundation

The root Cargo package is the new native-library build target. The existing
`src-tauri` package is intentionally left untouched during the migration so
that every step remains independently reversible.

## Current boundary

- `phi_abi_version` reports the C ABI version.
- `phi_context_create` copies all path values from the caller.
- `phi_context_destroy` owns and releases one context.
- `phi_context_get_last_error` uses a caller-owned UTF-8 buffer.
- `phi_render_config_init_default` provides code-defined defaults.
- `phi_render_config_validate` checks the ABI header.
- `phi_chart_info_load` parses a directory or chart archive through `phire`
  without using Tauri or the process working directory.
- `phi_chart_info_get_view` exposes a borrowed, complete ChartInfo view.
- `phi_chart_info_set_view` deep-copies the complete view before replacement.

## ABI rules

- Complex structures begin with `struct_size` and `abi_version`.
- Strings are UTF-8 pointer-plus-length views.
- Input views are borrowed only for the duration of the call and are copied by
  the native library when they become context-owned data.
- Boolean values use `uint8_t`.
- No Rust-owned value crosses the C ABI by value.
- Output strings do not include a terminating NUL byte.
- ChartInfo view strings and tag arrays are borrowed until the next mutation or
  destruction of that ChartInfo handle.
- The callback and job API will be added only after the renderer-host protocol
  and cancellation lifecycle are verified.

## Migration safety

The old Tauri application remains buildable through:

```text
cargo check --manifest-path src-tauri/Cargo.toml
```

The new library is checked independently from the WorkerLib root:

```text
cargo check
```
