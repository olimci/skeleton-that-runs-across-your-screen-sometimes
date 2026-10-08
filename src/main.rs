#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use glfw::{Context, OpenGlProfileHint, WindowHint as H, WindowMode};
use glow::HasContext;
use include_dir::{include_dir, Dir};
use std::{
    env,
    io::Cursor,
    thread,
    time::{Duration, Instant},
};

const FPS: f32 = 24.0;
static FRAMES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/animations");
static SOUND: &[u8] = include_bytes!("../assets/audio/sound.mp3");

struct Frame {
    image: image::RgbaImage,
    visible_x: Option<(u32, u32)>,
}

fn config(key: &str, default: f32) -> f32 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let min_delay = config("MIN_DELAY", 30.0);
    let max_delay = config("MAX_DELAY", 180.0).max(min_delay);
    let height = config("HEIGHT", 300.0).max(1.0);

    let clips: Vec<Vec<Frame>> = FRAMES
        .dirs()
        .map(|dir| {
            let mut files: Vec<_> = dir.files().collect();
            files.sort_by_key(|file| file.path());
            files
                .into_iter()
                .map(|file| {
                    let image = image::load_from_memory(file.contents()).unwrap().to_rgba8();
                    let visible_x = image
                        .enumerate_pixels()
                        .filter(|(_, _, pixel)| pixel[3] > 128)
                        .map(|(x, _, _)| x)
                        .fold(None, |bounds: Option<(u32, u32)>, x| {
                            Some(match bounds {
                                Some((left, right)) => (left.min(x), right.max(x)),
                                None => (x, x),
                            })
                        });
                    Frame { image, visible_x }
                })
                .collect()
        })
        .collect();

    assert!(!clips.is_empty(), "No embedded clips");
    let audio = rodio::DeviceSinkBuilder::open_default_sink().unwrap();

    let mut glfw = glfw::init(glfw::fail_on_errors).unwrap();

    for hint in [
        H::ContextVersion(3, 3),
        H::OpenGlForwardCompat(true),
        H::OpenGlProfile(OpenGlProfileHint::Core),
        H::Decorated(false),
        H::Floating(true),
        H::Focused(false),
        H::FocusOnShow(false),
        H::MousePassthrough(true),
        H::TransparentFramebuffer(true),
        H::CocoaRetinaFramebuffer(false),
    ] {
        glfw.window_hint(hint);
    }

    let (pos, sw, sh) = glfw.with_primary_monitor(|_, monitor| {
        let monitor = monitor.unwrap();
        let mode = monitor.get_video_mode().unwrap();
        (monitor.get_pos(), mode.width, mode.height)
    });

    let (mut window, _) = glfw
        .create_window(sw, sh, "skeleton", WindowMode::Windowed)
        .unwrap();

    window.set_pos(pos.0, pos.1);
    window.make_current();
    glfw.set_swap_interval(glfw::SwapInterval::None);

    assert!(
        window.is_framebuffer_transparent(),
        "Transparent framebuffer unsupported"
    );

    let gl = unsafe {
        glow::Context::from_loader_function(|s| {
            glfw.get_proc_address_raw(s)
                .map_or(std::ptr::null(), |p| p as *const _)
        })
    };

    let vertex = r#"#version 330 core
        uniform vec2 screen, pos, size;
        out vec2 uv;

        void main() {
            vec2 p = vec2(
                (gl_VertexID == 1 || gl_VertexID == 3) ? 1.0 : 0.0,
                (gl_VertexID >= 2) ? 1.0 : 0.0
            );
            uv = p;
            gl_Position = vec4(
                (pos + p * size) / screen * vec2(2.0, -2.0)
                    + vec2(-1.0, 1.0),
                0.0, 1.0
            );
        }
    "#;

    let fragment = r#"#version 330 core
        in vec2 uv;
        out vec4 color;
        uniform sampler2D tex;
        uniform bool flip;

        void main() {
            color = texture(tex, vec2(flip ? 1.0 - uv.x : uv.x, uv.y));
        }
    "#;

    unsafe {
        let program = gl.create_program().unwrap();

        for (kind, source) in [
            (glow::VERTEX_SHADER, vertex),
            (glow::FRAGMENT_SHADER, fragment),
        ] {
            let shader = gl.create_shader(kind).unwrap();
            gl.shader_source(shader, source);
            gl.compile_shader(shader);
            assert!(
                gl.get_shader_compile_status(shader),
                "{}",
                gl.get_shader_info_log(shader)
            );
            gl.attach_shader(program, shader);
            gl.delete_shader(shader);
        }

        gl.link_program(program);
        assert!(
            gl.get_program_link_status(program),
            "{}",
            gl.get_program_info_log(program)
        );
        gl.use_program(Some(program));

        let vao = gl.create_vertex_array().unwrap();
        gl.bind_vertex_array(Some(vao));

        let texture = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

        let uniform = |name| gl.get_uniform_location(program, name).unwrap();
        let screen_u = uniform("screen");
        let pos_u = uniform("pos");
        let size_u = uniform("size");
        let flip_u = uniform("flip");

        let (fw, fh) = window.get_framebuffer_size();
        gl.viewport(0, 0, fw, fh);
        gl.uniform_2_f32(Some(&screen_u), sw as f32, sh as f32);

        gl.disable(glow::DEPTH_TEST);

        let delay =
            || Duration::from_secs_f32(fastrand::f32() * (max_delay - min_delay) + min_delay);

        let mut next = Instant::now() + delay();

        while !window.should_close() {
            glfw.poll_events();

            if Instant::now() < next {
                thread::sleep(Duration::from_millis(100));
                continue;
            }

            let clip = &clips[fastrand::usize(..clips.len())];
            let w = clip[0].image.width() as f32 * height / clip[0].image.height() as f32;
            let max_y = (sh as f32 - height).max(0.0);
            let start_y = fastrand::f32() * max_y;
            let end_y = (start_y + if fastrand::bool() { 100.0 } else { -100.0 }).clamp(0.0, max_y);
            let reverse = fastrand::bool();
            gl.uniform_1_i32(Some(&flip_u), i32::from(!reverse));
            let start = Instant::now();
            let mut sound_started = false;

            loop {
                glfw.poll_events();

                let elapsed = start.elapsed().as_secs_f32();
                let index = (elapsed * FPS) as usize;

                if index >= clip.len() || window.should_close() {
                    break;
                }

                let progress = elapsed * FPS / clip.len() as f32;
                let travel = progress * (sw as f32 + w);
                let y = start_y + progress * (end_y - start_y);
                let x = if reverse {
                    sw as f32 - travel
                } else {
                    -w + travel
                };

                let frame = &clip[index];
                if !sound_started {
                    if let Some((left, right)) = frame.visible_x {
                        let (left, right) = if reverse {
                            (left, right)
                        } else {
                            (
                                frame.image.width() - 1 - right,
                                frame.image.width() - 1 - left,
                            )
                        };
                        let scale = height / frame.image.height() as f32;
                        if x + right as f32 * scale >= 0.0 && x + left as f32 * scale < sw as f32 {
                            audio
                                .mixer()
                                .add(rodio::Decoder::try_from(Cursor::new(SOUND)).unwrap());
                            sound_started = true;
                        }
                    }
                }

                // Upload the current frame.
                let frame = &frame.image;

                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    frame.width() as i32,
                    frame.height() as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(frame.as_raw())),
                );

                let (fw, fh) = window.get_framebuffer_size();
                gl.viewport(0, 0, fw, fh);

                gl.clear_color(0.0, 0.0, 0.0, 0.0);
                gl.clear(glow::COLOR_BUFFER_BIT);

                gl.uniform_2_f32(Some(&pos_u), x, y);
                gl.uniform_2_f32(Some(&size_u), w, height);

                gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
                window.swap_buffers();

                thread::sleep(Duration::from_millis(5));
            }

            // Clear the last frame.
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            window.swap_buffers();

            next = Instant::now() + delay();
        }
    }
}
