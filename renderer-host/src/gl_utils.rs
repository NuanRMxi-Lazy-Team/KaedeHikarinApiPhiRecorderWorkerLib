use std::ffi::CStr;

use macroquad::miniquad::gl;

pub fn renderer_name() -> Option<String> {
    unsafe {
        let value = gl::glGetString(crate::GL_RENDERER);
        if value.is_null() {
            return None;
        }
        CStr::from_ptr(value.cast())
            .to_str()
            .ok()
            .map(str::to_owned)
    }
}

pub fn is_software_renderer(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "llvmpipe",
        "softpipe",
        "swrast",
        "software rasterizer",
        "swiftshader",
    ]
    .iter()
    .any(|marker| name.contains(marker))
}
