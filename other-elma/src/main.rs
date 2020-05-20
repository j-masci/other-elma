use crate::atlas::{Atlas, Pic};
use crate::render::PictureVertex;
use cgmath::{vec2, Vector2};
use elma::lev::Level;
use elma::rec::EventType;
use elma::Clip;
use elma_physics::{Control, Events, Moto, Object, Segments};
use gl::types::*;
use glutin::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
use glutin::event_loop::ControlFlow;
use std::time::{Duration, Instant};

mod atlas;
mod render;
mod triangulation;

mod gl {
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

/*
mod gles {
    include!(concat!(env!("OUT_DIR"), "/gles_bindings.rs"));
}*/

struct GameState {
    moto: Moto,
    level: Level,
    segments: Segments,
}

impl GameState {
    fn new(path: &str) -> GameState {
        let level = Level::load(path).unwrap();

        let player = level
            .objects
            .iter()
            .find(|object| object.is_player())
            .unwrap();

        let moto = Moto::new(vec2(player.position.x, player.position.y));
        let segments = Segments::new(&level.polygons);

        GameState {
            moto,
            level,
            segments,
        }
    }
}

struct E;
impl Events for E {
    fn event(&mut self, _kind: EventType) {
        //  dbg!(kind);
    }
}

struct Scene<V> {
    vertices: Vec<V>,
    indices: Vec<u32>,
}

fn vec_dir(i: i32) -> Vector2<f64> {
    match i {
        0 => vec2(0.0, 0.0),
        1 => vec2(1.0, 0.0),
        2 => vec2(1.0, 1.0),
        3 => vec2(0.0, 1.0),
        _ => unreachable!(),
    }
}

const PIXELS_PER_UNIT: f64 = 95.0 / 2.0; // FIXME: the exact coefficient isn't known

impl Scene<PictureVertex> {
    fn add_image(&mut self, pic: &Pic, position: Vector2<f64>, clip: Clip) -> usize {
        let v = self.vertices.len() as u32;

        for i in 0..4 {
            let v = vec_dir(i);
            let p = position + (1.0 / PIXELS_PER_UNIT) * vec2(v.x * pic.size.x, -v.y * pic.size.y);

            self.vertices.push(PictureVertex {
                position: [p.x as f32, p.y as f32],
                tex_coord: [v.x as f32, v.y as f32],
                tex_bounds: pic.bounds,
                clip: match clip {
                    Clip::Unclipped => 0.5,
                    Clip::Ground => 0.0,
                    Clip::Sky => 1.0,
                },
            });
        }

        self.indices
            .extend_from_slice(&[v, v + 1, v + 2, v, v + 2, v + 3]);

        v as usize
    }
}

/*
1st pass - render polygons with depth
2ns pass - render sorted pictures with depth test but no depth writing
*/

fn main() {
    let mut game_state = GameState::new("E:/d/games/ElastoMania/Lev/0lp26.lev");

    let mut texture = Atlas::new("E:/d/games/ElastoMania/lgr/default.lgr");
    let sky_texture = texture.get(&(game_state.level.sky.clone() + ".pcx"));
    let ground_texture = texture.get(&(game_state.level.ground.clone() + ".pcx"));
    let sky_size = sky_texture.size;
    let ground_size = ground_texture.size;

    let polygon_buffers = triangulation::triangulate(&game_state.level);

    let mut picture_scene = Scene {
        vertices: Vec::new(),
        indices: Vec::new(),
    };

    let sky = picture_scene.add_image(sky_texture, vec2(0.0, 0.0), Clip::Sky);
    let ground = picture_scene.add_image(ground_texture, vec2(0.0, 0.0), Clip::Ground);

    for pic in &game_state.level.pictures {
        if pic.name.is_empty() {
            continue;
        }

        let pic2 = texture.get(&(pic.name.clone() + ".pcx"));
        picture_scene.add_image(pic2, vec2(pic.position.x, pic.position.y), pic.clip);
    }

    let wheel_pic = texture.get("Q1WHEEL.pcx");
    let bike = picture_scene.add_image(wheel_pic, vec2(0.0, 0.0), Clip::Unclipped);
    let wheels = [
        picture_scene.add_image(wheel_pic, vec2(0.0, 0.0), Clip::Unclipped),
        picture_scene.add_image(wheel_pic, vec2(0.0, 0.0), Clip::Unclipped),
    ];

    let events_loop = glutin::event_loop::EventLoop::new();
    let window_builder = glutin::window::WindowBuilder::new()
        .with_title("Elastomania")
        .with_inner_size(glutin::dpi::LogicalSize::new(1024.0, 768.0));

    let windowed_context = glutin::ContextBuilder::new()
        .with_vsync(true)
        // .with_multisampling(0)
        .with_depth_buffer(8)
        .build_windowed(window_builder, &events_loop)
        .unwrap();

    let mut size = windowed_context.window().inner_size();
    let mut scale_factor = windowed_context.window().scale_factor();

    let windowed_context = unsafe { windowed_context.make_current().unwrap() };

    let gl = gl::Gl::load_with(|name| windowed_context.get_proc_address(name) as *const _);
    //  let _gles = gles::Gles2::load_with(|name| self.window.context().get_proc_address(name) as *const _);

    let mut renderer = unsafe { render::Renderer::new(&gl, &mut texture) };
    let time = Instant::now();
    let mut control = Control::default();
    let mut next_frame_time = Instant::now();

    events_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(next_frame_time);
        let now = Instant::now();
        if now > next_frame_time {
            windowed_context.window().request_redraw();
            next_frame_time = now + Duration::from_millis(20);
        }

        let mut close = false;
        let mut resize = false;

        let time = time.elapsed().as_secs_f64();
        game_state
            .moto
            .advance(control, time * 0.4368, &game_state.segments, &mut E);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                close = true;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(new_size),
                ..
            } => {
                size = new_size;
                resize = true;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::ScaleFactorChanged {
                        scale_factor: new_scale_factor,
                        new_inner_size,
                    },
                ..
            } => {
                scale_factor = new_scale_factor;
                size = *new_inner_size;
                resize = true;
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                if let Some(key) = input.virtual_keycode {
                    let state = input.state == ElementState::Pressed;
                    match key {
                        VirtualKeyCode::Left => control.rotate_left = state,
                        VirtualKeyCode::Right => control.rotate_right = state,
                        VirtualKeyCode::Up => control.throttle = state,
                        VirtualKeyCode::Down => control.brake = state,
                        VirtualKeyCode::Space if state => {
                            game_state.moto.direction = !game_state.moto.direction
                        }
                        _ => {}
                    }
                }
            }
            Event::WindowEvent { event: _event, .. } => {
                //    dbg!(event);
            }
            Event::RedrawRequested(_) => {
                // let width = size.width as f64 / scale_factor;
                // let height = size.height as f64 / scale_factor;

                unsafe {
                    gl.DepthMask(true as _);
                    gl.ClearColor(0.0, 0.0, 0.0, 1.0);
                    gl.ClearDepth(1.0);
                    gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
                }

                let viewport = render::Viewport::from_center_and_scale(
                    game_state.moto.bike.position,
                    15.0,
                    size,
                );

                for i in 0..2 {
                    object_to_vertices(
                        &game_state.moto.wheels[i],
                        &mut picture_scene.vertices[wheels[i]..],
                    );
                }
                object_to_vertices(&game_state.moto.bike, &mut picture_scene.vertices[bike..]);

                let sky = &mut picture_scene.vertices[sky..];
                //  let sky_width =
                //      sky_size.y as f64 / sky_size.x as f64 * viewport.size.x / viewport.size.y;
                //   let sky_offset = viewport.position.x;
                // FIXME: vertical inversion
                for i in 0..4 {
                    /*    let v = vec_dir(i);
                    let p = viewport.position + vec2(viewport.size.x * v.x, viewport.size.y * v.y);
                    sky[i as usize].position = [p.x as f32, p.y as f32];
                    if v.x > 0.5 {
                        sky[i as usize].tex_coord[0] = sky_width as f32;
                    } else {
                        sky[i as usize].tex_coord[0] = 0.0;
                    }

                    let tex_coord_a = PIXELS_PER_UNIT * viewport.position.x / sky_size.x;
                    //    let tex_coord = PIXELS_PER_UNIT * viewport.size.x / sky_size.x;
                    let tex_coord = 0.5 * tex_coord_a; // + tex_coord * v.x;
                    sky[i as usize].tex_coord[0] += tex_coord as f32;*/

                    let v = vec_dir(i);
                    let p = viewport.position + vec2(viewport.size.x * v.x, viewport.size.y * v.y);
                    sky[i as usize].position = [p.x as f32, p.y as f32];

                    let tex_coord_a = PIXELS_PER_UNIT * vec2(viewport.position.x / sky_size.x, 0.0);

                    let tex_coord = PIXELS_PER_UNIT
                        * vec2(viewport.size.x / sky_size.x, viewport.size.y / sky_size.y);
                    let tex_coord = 0.5 * tex_coord_a + vec2(tex_coord.x * v.x, tex_coord.y * v.y);
                    sky[i as usize].tex_coord = [tex_coord.x as f32, tex_coord.y as f32];
                }

                // FIXME: vertical inversion
                let ground = &mut picture_scene.vertices[ground..];
                for i in 0..4 {
                    let v = vec_dir(i);
                    let p = viewport.position + vec2(viewport.size.x * v.x, viewport.size.y * v.y);
                    ground[i as usize].position = [p.x as f32, p.y as f32];

                    let tex_coord_a = PIXELS_PER_UNIT
                        * vec2(
                            viewport.position.x / ground_size.x,
                            viewport.position.y / ground_size.y,
                        );

                    let tex_coord = PIXELS_PER_UNIT
                        * vec2(
                            viewport.size.x / ground_size.x,
                            viewport.size.y / ground_size.y,
                        );
                    let tex_coord = tex_coord_a + vec2(tex_coord.x * v.x, tex_coord.y * v.y);
                    ground[i as usize].tex_coord = [tex_coord.x as f32, tex_coord.y as f32];
                }

                unsafe {
                    renderer.draw_polygons(
                        &gl,
                        &polygon_buffers.vertices,
                        &polygon_buffers.indices,
                        viewport,
                    );
                    renderer.draw_pictures(
                        &gl,
                        &picture_scene.vertices,
                        &picture_scene.indices,
                        viewport,
                    );
                };

                windowed_context.swap_buffers().unwrap(); // FIXME: handle error
            }
            _ => {}
        };

        if resize {
            windowed_context.resize(size);
            unsafe { gl.Viewport(0, 0, size.width as GLsizei, size.height as GLsizei) };
        }

        if close {
            *control_flow = ControlFlow::Exit;

            // FIXME: move?
            unsafe {
                renderer.cleanup(&gl);
            }
            return;
        }
    });

    //  unsafe { renderer.cleanup(&gl) };
}

fn object_to_vertices(object: &Object, vertices: &mut [PictureVertex]) {
    let (sin, cos) = object.angular_position.sin_cos();
    let v = 0.4 * 2.0f64.sqrt() * vec2(cos, sin);
    let pos = [
        object.position - v,
        object.position + vec2(v.y, -v.x),
        object.position + v,
        object.position + vec2(-v.y, v.x),
    ];

    for i in 0..4 {
        vertices[i].position = [pos[i].x as f32, pos[i].y as f32];
    }
}
