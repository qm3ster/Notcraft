#![feature(const_fn, trace_macros, nll, match_beginning_vert, optin_builtin_traits)]

extern crate gl;
extern crate glfw;
extern crate image;
#[macro_use] extern crate cgmath;
extern crate noise;
extern crate smallvec;
extern crate collision;
extern crate rayon;
#[macro_use] extern crate lazy_static;

#[macro_use] mod gl_api;
pub mod engine;
pub mod util;

use collision::algorithm::minkowski::GJK3;
use collision::primitive::Cuboid;
use collision::Discrete;
use collision::{Aabb3, Ray3, CollisionStrategy};
use engine::chunk::Voxel;
use cgmath::{MetricSpace, Matrix4};
use gl_api::shader::program::LinkedProgram;
use std::collections::HashSet;
use glfw::{Action, Context, Key, Window, MouseButton, WindowEvent, WindowHint};
use cgmath::{Deg, InnerSpace, Vector3};
use engine::chunk_manager::ChunkManager;
use engine::camera::Rotation;
use gl_api::shader::*;
use gl_api::misc;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Block {
    Air,
    Stone,
    Dirt,
    Grass,
    Water,
}

impl engine::chunk::Voxel for Block {
    fn has_transparency(&self) -> bool { *self == Block::Air }
    fn color(&self) -> Vector3<f32> {
        match *self {
            Block::Air => Vector3::new(0.0, 0.0, 0.0),
            Block::Stone => Vector3::new(0.545098039, 0.552941176, 0.478431373),
            Block::Dirt => Vector3::new(0.250980392, 0.160784314, 0.0196078431),
            Block::Grass => Vector3::new(0.376, 0.502, 0.220),
            Block::Water => Vector3::new(0.1, 0.2, 0.9),
        }
    }
}

use glfw::CursorMode;
use engine::camera::Camera;

struct Inputs {
    active_keys: HashSet<Key>,
}

impl Inputs {
    fn new() -> Self {
        Inputs { active_keys: HashSet::new() }
    }

    fn set_key(&mut self, key: Key, active: bool) {
        if active {
            self.active_keys.insert(key);
        } else {
            self.active_keys.remove(&key);
        }
    }

    fn is_down(&self, key: Key) -> bool {
        self.active_keys.contains(&key)
    }
}

struct Application {
    time: f32,
    wireframe: bool,
    mouse_capture: bool,
    debug_frames: bool,
    noclip: bool,
    camera: Camera,
    player_pos: Vector3<f32>,
    // prev_camera: Camera,
    previous_cursor_x: f32,
    previous_cursor_y: f32,
    frames: i32,

    jumping: bool,
    velocity: Vector3<f32>,
    max_speed: f32,
    cam_acceleration: f32,

    pipeline: LinkedProgram,
    debug_pipeline: LinkedProgram,
    chunk_manager: ChunkManager<Block>,
}

impl Application {
    fn new(mut pipeline: LinkedProgram, mut debug_pipeline: LinkedProgram) -> Self {
        unsafe { gl_call!(Viewport(0, 0, 600, 600)).expect("glViewport failed"); }
        let projection = cgmath::perspective(Deg(70.0), 600.0 / 600.0, 0.1, 1000.0f32);
        pipeline.set_uniform("u_Projection", &projection);
        debug_pipeline.set_uniform("projection", &projection);

        let mut poses = Vec::new();
        let mut colors = Vec::new();
        let mut attenuations = Vec::new();

        for i in 0..3 {
            poses.push(Vector3::new(i as f32 * 10.0, 0.5, 0.5));
            colors.push(Vector3::new(i as f32/3.0, 0.0, 0.0));
            attenuations.push(0.5f32);
        }

        pipeline.set_uniform("u_Light", &poses.as_slice());
        pipeline.set_uniform("u_LightColor", &colors.as_slice());
        pipeline.set_uniform("u_LightAttenuation", &attenuations.as_slice());
        pipeline.set_uniform("u_LightAmbient", &Vector3::<f32>::new(0.2, 0.25, 0.3));

        use engine::terrain::NoiseGenerator;

        let chunk_manager = ChunkManager::new(NoiseGenerator::new_default(
            noise::OpenSimplex::default(),
            |pos: Vector3<f64>, n| {
                if n-3.0 >= pos.y { Block::Stone }
                else if n-1.0 >= pos.y { Block::Dirt }
                else if n >= pos.y { Block::Grass }
                else if pos.y <= -35.0 { Block::Water }
                else { Block::Air }
            }
        ));

        Application {
            velocity: Vector3::new(0.0, 0.0, 0.0),
            max_speed: 24.0,
            cam_acceleration: 0.1,
            frames: 0,
            player_pos: Vector3::new(0.0, 0.0, 0.0),
            time: 0.0,
            wireframe: false,
            debug_frames: false,
            mouse_capture: false,
            noclip: false,
            jumping: false,
            camera: Camera::default(),
            // prev_camera: Camera::default(),
            previous_cursor_x: 0.0,
            previous_cursor_y: 0.0,
            pipeline,
            debug_pipeline,
            chunk_manager,
        }
    }

    fn toggle_wireframe(&mut self) {
        self.wireframe = !self.wireframe;
        misc::polygon_mode(if self.wireframe { misc::PolygonMode::Line } else { misc::PolygonMode::Fill });
    }

    fn toggle_mouse_capture(&mut self, window: &mut Window) {
        self.mouse_capture = !self.mouse_capture;
        window.set_cursor_mode(if self.mouse_capture { CursorMode::Disabled } else { CursorMode::Normal });
    }

    fn toggle_debug_frames(&mut self) {
        self.debug_frames = !self.debug_frames;
    }

    fn update_camera_rotation(&mut self, x: f32, y: f32) {
        let dx = self.previous_cursor_x - x;
        let dy = self.previous_cursor_y - y;
        self.previous_cursor_x = x;
        self.previous_cursor_y = y;

        self.camera.rotate(Rotation::AboutY(Deg(-dx as f32/3.0)));
        self.camera.rotate(Rotation::AboutX(Deg(-dy as f32/3.0)));
    }

    fn set_viewport(&mut self, width: i32, height: i32) {
        unsafe { gl_call!(Viewport(0, 0, width, height)).expect("glViewport failed"); }

        let projection = cgmath::perspective(Deg(70.0), width as f32 / height as f32, 0.1, 1000.0);
        self.pipeline.set_uniform("u_Projection", &projection);
        self.debug_pipeline.set_uniform("projection", &projection);
    }

    fn handle_event(&mut self, window: &mut Window, event: WindowEvent) -> bool {
        match event {
            WindowEvent::CursorPos(x, y) => self.update_camera_rotation(x as f32, y as f32),
            WindowEvent::MouseButton(MouseButton::Button1, Action::Press, _) => self.destroy_looking_at(),
            // WindowEvent::MouseButton(MouseButton::Button2, Action::Press, _) => self.place_looking_at(),

            WindowEvent::Key(Key::Escape, _, Action::Press, _) => return true,
            WindowEvent::Key(Key::F1, _, Action::Press, _) => self.toggle_debug_frames(),
            WindowEvent::Key(Key::F2, _, Action::Press, _) => self.toggle_wireframe(),
            WindowEvent::Key(Key::F3, _, Action::Press, _) => self.toggle_mouse_capture(window),
            WindowEvent::Key(Key::F4, _, Action::Press, _) => self.noclip = !self.noclip,
            
            WindowEvent::Size(width, height) => self.set_viewport(width, height),
            _ => {}
        }
        false
    }

    fn handle_inputs(&mut self, inputs: &Inputs, dt: f64) {
        // println!("camera={:?}", self.camera);
        if inputs.is_down(Key::Right) { self.camera.rotate(Rotation::AboutY(Deg(1.0))); }
        if inputs.is_down(Key::Left) { self.camera.rotate(Rotation::AboutY(-Deg(1.0))); }
        if inputs.is_down(Key::Up) { self.camera.rotate(Rotation::AboutX(-Deg(1.0))); }
        if inputs.is_down(Key::Down) { self.camera.rotate(Rotation::AboutX(Deg(1.0))); }
        
        let accel = self.cam_acceleration;
        if inputs.is_down(Key::W) {
            let look_vec = accel * self.camera.get_spin_vecs().0;
            self.velocity.x = self.velocity.x - look_vec.x;
            self.velocity.z = self.velocity.z - look_vec.z;
        }

        if inputs.is_down(Key::S) {
            let look_vec = self.camera.get_spin_vecs().0;
            self.velocity.x = self.velocity.x + accel * look_vec.x;
            self.velocity.z = self.velocity.z + accel * look_vec.z;
        }

        if inputs.is_down(Key::A) {
            let look_vec = self.camera.get_spin_vecs().1;
            self.velocity.x = self.velocity.x - accel * look_vec.x;
            self.velocity.z = self.velocity.z - accel * look_vec.z;
        }

        if inputs.is_down(Key::D) {
            let look_vec = self.camera.get_spin_vecs().1;
            self.velocity.x = self.velocity.x + accel * look_vec.x;
            self.velocity.z = self.velocity.z + accel * look_vec.z;
        }

        if inputs.is_down(Key::Space) {
            if !self.jumping {
                self.jumping = true;
                self.velocity.y = 6.5;
            }
            // No need to multiply the cam accel by anything because we only ever travel
            // straight up and down the Y axis.
            // self.velocity.y = self.velocity.y + accel;
            // if !self.jumping {
                // self.velocity.y = 0.5;
                // self.jumping = true;
            // }
        }

        if inputs.is_down(Key::LeftShift) {
            // self.velocity.y = self.velocity.y - accel;
        }

        if inputs.is_down(Key::LeftControl) {
            self.cam_acceleration = 2.0;
        } else {
            self.cam_acceleration = 0.1;
        }
    }

    fn destroy_looking_at(&mut self) {
        if let Some(pos) = self.get_look_pos() {
            self.chunk_manager.set_voxel(pos, Block::Air);
        }
    }

    fn apply_motion(&mut self, dt: f64) {
        let substeps = 3;
        let timestep = dt as f32 / (substeps as f32);
        for _ in 0..substeps {
            self.player_pos += self.velocity * timestep;

            let world = self.chunk_manager.world();
            let feet = Vector3::new(
                self.player_pos.x.floor() as i32,
                self.player_pos.y.floor() as i32,
                self.player_pos.z.floor() as i32);
            let gjk = GJK3::new();
            let around = world.around_voxel(feet, 3, |pos, voxel| if voxel.has_transparency() { None } else { Some(pos) });

            const PLAYER_WIDTH: f32 = 0.45;
            const PLAYER_HEIGHT: f32 = 1.8;

            for block_pos in around {
                self.frame_at_voxel(block_pos.cast().unwrap(), Vector3::new(0.0, 1.0, 1.0), 0.003);
                let block_tfm = Matrix4::from_translation(
                    block_pos.cast().unwrap() + Vector3::new(0.5, 0.5, 0.5),
                );

                let player_tfm = Matrix4::from_translation(
                    self.player_pos + Vector3::new(0.0, PLAYER_HEIGHT / 2.0, 0.0),
                );

                // NOTE: non-transparent blocks were filtered out
                if let Some(contact) = gjk.intersection(
                    &CollisionStrategy::FullResolution,
                    &Cuboid::new(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH),
                    &player_tfm,
                    &Cuboid::new(1.0, 1.0, 1.0),
                    &block_tfm
                ) {
                    let resolution = -1.0 * contact.penetration_depth * contact.normal;

                    // We check two cuboids here, so normals should be axis-aligned. If any of
                    // the components are not zero, that means we've had a collision on that
                    // face and should cancel velocity in that direction. Alternatively, you
                    // could multiply the component by something like -0.8 and have a lot of fun!
                    if resolution.x.abs() > 0.0 { self.velocity.x = 0.0; }
                    if resolution.y.abs() > 0.0 { self.velocity.y = 0.0; self.jumping = false; }
                    if resolution.z.abs() > 0.0 { self.velocity.z = 0.0; }
                    self.player_pos += resolution;
                }
            }
        }
    }

    fn update(&mut self, dt: f64) {
        let view = self.camera.transform_matrix();
        self.pipeline.set_uniform("u_Time", &self.time);
        self.pipeline.set_uniform("u_CameraPosition", &::util::to_vector(self.camera.position));
        self.pipeline.set_uniform("u_View", &view);
        self.debug_pipeline.set_uniform("view", &view);

        const FRICTION: f32 = 0.02;

        self.velocity.x *= FRICTION.powf(dt as f32);
        self.velocity.z *= FRICTION.powf(dt as f32);

        if self.noclip {
            self.velocity.y *= FRICTION.powf(dt as f32);
            self.player_pos += self.velocity * dt as f32;
        } else {
            self.velocity.y -= 14.0 * dt as f32;
            self.apply_motion(dt);
        }
        self.camera.position = ::util::to_point(self.player_pos + Vector3::new(0.0, 1.8 - 0.45, 0.0));

        self.chunk_manager.update_player_position(self.player_pos);
        self.chunk_manager.tick();
        self.time += 0.007;
    }

    fn get_look_pos(&self) -> Option<Vector3<i32>> {
        use std::cmp::Ordering;
        let cam_pos = self.camera.position;
        let cam_pos_int = Vector3::new(cam_pos.x as i32, cam_pos.y as i32, cam_pos.z as i32);
        let look_vec = -self.camera.get_look_vec();
        let ray = Ray3::new(self.camera.position, look_vec);

        let colliders = self.chunk_manager.world().around_voxel(cam_pos_int, 9, |pos, voxel| {
            if !voxel.has_transparency() {
                Some(Aabb3::new(
                    ::util::to_point(pos.cast().unwrap()),
                    ::util::to_point(pos.cast().unwrap() + Vector3::new(1.0, 1.0, 1.0)),
                ))
            } else { None }
        });

        let mut colliders: Vec<_> = colliders.iter()
            .filter(|aabb| ray.intersects(&aabb)).collect();
        
        colliders.sort_by(|a, b| a.min.distance2(cam_pos)
            .partial_cmp(&b.min.distance2(cam_pos))
            .unwrap_or(Ordering::Equal));
        
        colliders.get(0).map(|aabb| {
            let fv = ::util::to_vector(aabb.min);
            Vector3::new(fv.x as i32, fv.y as i32, fv.z as i32)
        })
    }

    fn draw(&mut self, _dt: f64) {
        // Draw frame around the block we're looking at
        if let Some(look) = self.get_look_pos() {
            self.frame_at_voxel(Vector3::new(look.x as f32, look.y as f32, look.z as f32), Vector3::new(0.2, 0.2, 0.2), 0.02);
        }

        self.draw_frame(Aabb3::new(
            ::util::to_point(::util::to_vector(self.camera.position) - Vector3::new(9.0, 9.0, 9.0)),
            ::util::to_point(::util::to_vector(self.camera.position) + Vector3::new(9.0, 9.0, 9.0)),
        ), Vector3::new(0.0, 1.0, 0.0), 0.02);

        self.chunk_manager.draw(&mut self.pipeline).expect("Drawing chunks failed");
        self.frames += 1;
    }

    fn draw_frame(&mut self, aabb: Aabb3<f32>, color: Vector3<f32>, thickness: f32) {
        if self.debug_frames {
            ::util::draw_frame(&mut self.debug_pipeline, aabb, color, thickness);
        }
    }

    fn frame_at_voxel(&mut self, pos: Vector3<f32>, color: Vector3<f32>, thickness: f32) {
        self.draw_frame(Aabb3::new(
            ::util::to_point(pos),
            ::util::to_point(pos + Vector3::new(1.0, 1.0, 1.0)),
        ), color, thickness);
    }
}

use glfw::SwapInterval;

fn main() {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    println!("GLFW init");
    
    glfw.window_hint(WindowHint::ContextVersion(4, 5));
    glfw.window_hint(WindowHint::DepthBits(Some(24)));

    let (mut window, events) = glfw.create_window(600, 600, "Not Minecraft", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");
    println!("Window created");

    window.set_all_polling(true);
    window.make_current();
    glfw.set_swap_interval(SwapInterval::Adaptive);

    // Load OpenGL function pointers.
    // good *god* this function takes a long time fo compile
    gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);

    let program = simple_pipeline("resources/terrain.vs", "resources/terrain.fs")
        .expect("Pipeline creation failure");
    let debug_program = simple_pipeline("resources/debug.vs", "resources/debug.fs")
        .expect("Pipeline creation failure");

    let mut application = Application::new(program, debug_program);
    let mut inputs = Inputs::new();

    unsafe {
        gl_call!(Enable(gl::DEPTH_TEST)).expect("glEnable failed");
        gl_call!(DepthFunc(gl::LESS)).expect("glDepthFunc failed");
        gl_call!(Enable(gl::CULL_FACE)).expect("glEnable failed");
        gl_call!(FrontFace(gl::CW)).expect("glFrontFace failed");
        gl_call!(CullFace(gl::BACK)).expect("glCullFace failed");
        gl_call!(Viewport(0, 0, 600, 600)).expect("glViewport failed");
    }

    application.set_viewport(600, 600);

    let mut prev_time = glfw.get_time();

    while !window.should_close() {
        misc::clear(misc::ClearMode::Color(0.529411765, 0.807843137, 0.921568627, 1.0));
        misc::clear(misc::ClearMode::Depth(1.0));
        
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            if let WindowEvent::Key(key, _, action, _) = event {
                inputs.set_key(key, match action {
                    Action::Press | Action::Repeat => true,
                    _ => false,
                })
            }

            if application.handle_event(&mut window, event) {
                window.set_should_close(true);
            }
        }

        let now = glfw.get_time();
        let dt = now - prev_time;
        prev_time = now;

        application.handle_inputs(&inputs, dt);
        application.update(dt);
        application.draw(dt);

        window.swap_buffers();
    }
}
