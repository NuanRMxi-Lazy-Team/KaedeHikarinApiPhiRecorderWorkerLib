use anyhow::{Context, Result};
use macroquad::{
    miniquad::gl::{self, GLuint},
    prelude::*,
};

use phire::core::internal_id;

use crate::frame::PreparedFrameRenderer;

const YUV_VERTEX_SHADER: &str = r#"
#version 130
in vec3 position;
in vec2 texcoord;
out vec2 fragTexCoord;
void main() {
    gl_Position = vec4(position, 1.0);
    fragTexCoord = texcoord;
}
"#;

const YUV_FRAGMENT_SHADER: &str = r#"
#version 130
in vec2 fragTexCoord;
uniform sampler2D screenTexture;
uniform ivec2 screenSize;
uniform ivec2 targetSize;
uniform bool uFlipY;
out vec4 outColor;

vec3 getPixel(int x, int y) {
    return texelFetch(screenTexture, ivec2(x, y), 0).xyz;
}

float getY(int x, int y) {
    return dot(getPixel(x, y), vec3(0.299, 0.587, 0.114));
}

float getU(int x, int y) {
    vec3 pixel = (getPixel(x, y) + getPixel(x, y + 1) + getPixel(x + 1, y) + getPixel(x + 1, y + 1)) * 0.25;
    return dot(pixel, vec3(-0.168736, -0.331264, 0.5)) + 0.5;
}

float getV(int x, int y) {
    vec3 pixel = (getPixel(x, y) + getPixel(x, y + 1) + getPixel(x + 1, y) + getPixel(x + 1, y + 1)) * 0.25;
    return dot(pixel, vec3(0.5, -0.418688, -0.081312)) + 0.5;
}

float getYI(int index) { return getY(index % screenSize.x, index / screenSize.x); }
float getUI(int index) { return getU((index % (screenSize.x / 2)) * 2, index / (screenSize.x / 2) * 2); }
float getVI(int index) { return getV((index % (screenSize.x / 2)) * 2, index / (screenSize.x / 2) * 2); }

void main() {
    int w = screenSize.x;
    int h = screenSize.y;
    ivec2 curr_pos = ivec2(fragTexCoord * vec2(targetSize));
    if (!uFlipY) curr_pos.y = h - curr_pos.y - 1;
    int byte_index = (curr_pos.x + curr_pos.y * w) * 4;
    int y_bytes = w * h;
    int uv_bytes = y_bytes / 4;
    if (byte_index < y_bytes) {
        outColor = vec4(getYI(byte_index), getYI(byte_index + 1), getYI(byte_index + 2), getYI(byte_index + 3));
    } else if (byte_index < y_bytes + uv_bytes) {
        int pixel_index = byte_index - y_bytes;
        outColor = vec4(getUI(pixel_index), getUI(pixel_index + 1), getUI(pixel_index + 2), getUI(pixel_index + 3));
    } else if (byte_index < y_bytes + uv_bytes * 2) {
        int pixel_index = byte_index - y_bytes - uv_bytes;
        outColor = vec4(getVI(pixel_index), getVI(pixel_index + 1), getVI(pixel_index + 2), getVI(pixel_index + 3));
    } else {
        outColor = vec4(0);
    }
}
"#;

enum ReadbackMode {
    Cpu,
    Gpu {
        target: RenderTarget,
        material: Material,
        pbo: GLuint,
    },
}

pub struct FrameReadback {
    width: u32,
    height: u32,
    yuv_height: u32,
    output_byte_size: usize,
    mode: ReadbackMode,
}

impl FrameReadback {
    pub fn new(renderer: &PreparedFrameRenderer) -> Result<Self> {
        let (width, height) = renderer.output_size();
        if width % 2 != 0 || height % 2 != 0 {
            anyhow::bail!("YUV420 readback requires even dimensions");
        }
        let yuv_height = (height * 3).div_ceil(8);
        let output_byte_size = (width * height * 3 / 2) as usize;

        if let Some(renderer_name) = crate::gl_utils::renderer_name()
            .filter(|name| crate::gl_utils::is_software_renderer(name))
        {
            eprintln!(
                "renderer-host: software GL renderer detected ({renderer_name}), using CPU YUV readback"
            );
            return Ok(Self {
                width,
                height,
                yuv_height,
                output_byte_size,
                mode: ReadbackMode::Cpu,
            });
        }

        let packed_byte_size = (width * yuv_height * 4) as usize;
        let target = render_target(width, yuv_height);
        let material = load_material(
            ShaderSource::Glsl {
                vertex: YUV_VERTEX_SHADER,
                fragment: YUV_FRAGMENT_SHADER,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("screenSize", UniformType::Int2),
                    UniformDesc::new("targetSize", UniformType::Int2),
                    UniformDesc::new("uFlipY", UniformType::Int1),
                ],
                textures: vec!["screenTexture".to_owned()],
                ..Default::default()
            },
        )
        .context("load YUV420 readback shader")?;
        material.set_uniform("screenSize", [width as i32, height as i32]);
        material.set_uniform("targetSize", [width as i32, yuv_height as i32]);
        material.set_uniform("uFlipY", 1i32);

        let mut pbo = 0;
        unsafe {
            gl::glGenBuffers(1, &mut pbo);
            gl::glBindBuffer(gl::GL_PIXEL_PACK_BUFFER, pbo);
            gl::glBufferData(
                gl::GL_PIXEL_PACK_BUFFER,
                packed_byte_size as _,
                std::ptr::null(),
                gl::GL_STREAM_READ,
            );
            gl::glBindBuffer(gl::GL_PIXEL_PACK_BUFFER, 0);
        }

        Ok(Self {
            width,
            height,
            yuv_height,
            output_byte_size,
            mode: ReadbackMode::Gpu {
                target,
                material,
                pbo,
            },
        })
    }

    pub fn read_frame(&self, renderer: &PreparedFrameRenderer) -> Result<Vec<u8>> {
        match &self.mode {
            ReadbackMode::Cpu => {
                rgb_texture_to_yuv420(&renderer.output_texture(), self.width, self.height)
            }
            ReadbackMode::Gpu {
                target,
                material,
                pbo,
            } => {
                set_camera(&Camera2D {
                    zoom: vec2(1., 1.),
                    render_target: Some(target.clone()),
                    ..Default::default()
                });
                material.set_texture("screenTexture", renderer.output_texture());
                gl_use_material(material);
                draw_rectangle(-1., -1., 2., 2., WHITE);
                gl_use_default_material();
                unsafe { get_internal_gl().flush() };

                let mut output = vec![0u8; self.output_byte_size];
                unsafe {
                    gl::glBindFramebuffer(gl::GL_READ_FRAMEBUFFER, internal_id(target.clone()));
                    gl::glBindBuffer(gl::GL_PIXEL_PACK_BUFFER, *pbo);
                    gl::glReadPixels(
                        0,
                        0,
                        self.width as _,
                        self.yuv_height as _,
                        gl::GL_RGBA,
                        gl::GL_UNSIGNED_BYTE,
                        std::ptr::null_mut(),
                    );
                    gl::glBindBuffer(gl::GL_PIXEL_PACK_BUFFER, *pbo);
                    let source =
                        gl::glMapBuffer(gl::GL_PIXEL_PACK_BUFFER, 0x88B8 /* GL_READ_ONLY */);
                    if source.is_null() {
                        gl::glBindBuffer(gl::GL_PIXEL_PACK_BUFFER, 0);
                        anyhow::bail!("glMapBuffer returned null");
                    }
                    std::ptr::copy_nonoverlapping(
                        source.cast::<u8>(),
                        output.as_mut_ptr(),
                        self.output_byte_size,
                    );
                    gl::glUnmapBuffer(gl::GL_PIXEL_PACK_BUFFER);
                    gl::glBindBuffer(gl::GL_PIXEL_PACK_BUFFER, 0);
                }

                Ok(output)
            }
        }
    }
}

impl Drop for FrameReadback {
    fn drop(&mut self) {
        if let ReadbackMode::Gpu { pbo, .. } = &self.mode {
            unsafe {
                gl::glDeleteBuffers(1, pbo);
            }
        }
    }
}

fn rgb_texture_to_yuv420(texture: &Texture2D, width: u32, height: u32) -> Result<Vec<u8>> {
    let image = texture.get_texture_data();
    if image.width as u32 != width || image.height as u32 != height {
        anyhow::bail!(
            "texture readback size mismatch: expected {}x{}, got {}x{}",
            width,
            height,
            image.width,
            image.height
        );
    }

    // MSRenderTarget's output texture is RGB8. Macroquad's Image container
    // allocates four bytes per pixel, but glReadPixels writes three bytes.
    let expected_rgb_size = (width * height * 3) as usize;
    if image.bytes.len() < expected_rgb_size {
        anyhow::bail!(
            "texture readback buffer too small: expected at least {}, got {}",
            expected_rgb_size,
            image.bytes.len()
        );
    }

    let y_size = (width * height) as usize;
    let uv_size = y_size / 4;
    let mut output = vec![0u8; y_size + uv_size * 2];

    for y in 0..height {
        for x in 0..width {
            let pixel = ((y * width + x) * 3) as usize;
            let r = image.bytes[pixel] as f32;
            let g = image.bytes[pixel + 1] as f32;
            let b = image.bytes[pixel + 2] as f32;
            output[(y * width + x) as usize] = yuv_component(0.299 * r + 0.587 * g + 0.114 * b);
        }
    }

    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let mut r = 0.0;
            let mut g = 0.0;
            let mut b = 0.0;
            for dy in 0..2 {
                for dx in 0..2 {
                    let pixel = (((y + dy) * width + x + dx) * 3) as usize;
                    r += image.bytes[pixel] as f32;
                    g += image.bytes[pixel + 1] as f32;
                    b += image.bytes[pixel + 2] as f32;
                }
            }
            r *= 0.25;
            g *= 0.25;
            b *= 0.25;
            let uv_index = ((y / 2) * (width / 2) + x / 2) as usize;
            output[y_size + uv_index] =
                yuv_component(-0.168736 * r - 0.331264 * g + 0.5 * b + 128.0);
            output[y_size + uv_size + uv_index] =
                yuv_component(0.5 * r - 0.418688 * g - 0.081312 * b + 128.0);
        }
    }

    Ok(output)
}

fn yuv_component(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}
