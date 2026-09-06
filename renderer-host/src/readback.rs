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

pub struct FrameReadback {
    width: u32,
    yuv_height: u32,
    target: RenderTarget,
    material: Material,
    pbo: GLuint,
    output_byte_size: usize,
}

impl FrameReadback {
    pub fn new(renderer: &PreparedFrameRenderer) -> Result<Self> {
        let (width, height) = renderer.output_size();
        if width % 2 != 0 || height % 2 != 0 {
            anyhow::bail!("YUV420 readback requires even dimensions");
        }
        let yuv_height = (height * 3).div_ceil(8);
        let output_byte_size = (width * height * 3 / 2) as usize;
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
            yuv_height,
            target,
            material,
            pbo,
            output_byte_size,
        })
    }

    pub fn read_frame(&self, renderer: &PreparedFrameRenderer) -> Result<Vec<u8>> {
        set_camera(&Camera2D {
            zoom: vec2(1., 1.),
            render_target: Some(self.target.clone()),
            ..Default::default()
        });
        self.material
            .set_texture("screenTexture", renderer.output_texture());
        gl_use_material(&self.material);
        draw_rectangle(-1., -1., 2., 2., WHITE);
        gl_use_default_material();
        unsafe { get_internal_gl().flush() };

        let mut output = vec![0u8; self.output_byte_size];
        unsafe {
            gl::glBindFramebuffer(gl::GL_READ_FRAMEBUFFER, internal_id(self.target.clone()));
            gl::glBindBuffer(gl::GL_PIXEL_PACK_BUFFER, self.pbo);
            gl::glReadPixels(
                0,
                0,
                self.width as _,
                self.yuv_height as _,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE,
                std::ptr::null_mut(),
            );
            gl::glBindBuffer(gl::GL_PIXEL_PACK_BUFFER, self.pbo);
            let source = gl::glMapBuffer(gl::GL_PIXEL_PACK_BUFFER, 0x88B8 /* GL_READ_ONLY */);
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

impl Drop for FrameReadback {
    fn drop(&mut self) {
        unsafe {
            gl::glDeleteBuffers(1, &self.pbo);
        }
    }
}
