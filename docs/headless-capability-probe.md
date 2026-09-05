# Headless Capability Probe

The private `phi-renderer-host` now supports a `CapabilityProbe` protocol
message. It creates a minimal Macroquad headless context and returns a JSON
`CapabilityResult` containing:

- `graphicsContext`
- `softwareRenderer`
- `glVendor`
- `glRenderer`
- `glVersion`
- `glslVersion`

The host keeps protocol frames on stdout and diagnostics on stderr. The probe
was verified on the development machine with an NVIDIA GeForce RTX 5070 Laptop
GPU and OpenGL 3.1. This confirms the current Windows environment, but it is
not a cross-platform guarantee; each deployment target must run the probe
before accepting render jobs.
