# Phi Recorder Native

Phigros 谱面渲染纯 native 库 (RPE / PGR charts to video).

Fast Simple Lightweight Convenient

## 架构

- `phi-recorder-native` — 唯一公开入口是 C ABI (`include/phi_recorder.h`)，供 Worker 通过 P/Invoke 调用
- `renderer-core` — 纯逻辑：配置、时间轴、FFmpeg 参数计划、ChartInfo、事件与任务控制
- `renderer-protocol` — DLL 与私有渲染进程之间的 PHIR 二进制协议
- `renderer-host` — 私有 headless 渲染进程（macroquad + phire + sasa + ffmpeg），由 DLL 管理，用于进程隔离与强制取消
- `assets/` — 渲染运行时资源（字体、UI 贴图、`respack/`、`rank/`），由调用方通过 `phi_context_options_t` 显式传入

## 构建

```bash
cargo check --workspace --all-targets
cargo test --workspace
cargo build -p phi-renderer-host
```

## 注意事项 / Notice

- 请不要伪造游玩成绩、官方内容等，以免造成不好的影响
- 游玩界面与本家有明显区分, 如 COMBO 处文本与 `COMBO` 有明显区分
- 不建议向不使用 `Re: PhiEdit` 的玩家分享本软件
