use std::collections::HashSet;
use std::f32::consts::TAU;
use std::mem;

use crate::client::{EventLoop, Renderer};
use crate::{Bullet, Drive, Input, Tank, Turn};

use tokio::sync::watch;

use pathfinder_canvas::{
    Canvas, CanvasFontContext, CanvasRenderingContext2D, ColorF, ColorU, FillRule, FillStyle,
    Path2D, RectF, Vector2F, Vector2I,
};
use pathfinder_gl::{GLDevice, GLVersion};
use pathfinder_renderer::concurrent::rayon::RayonExecutor;
use pathfinder_renderer::concurrent::scene_proxy::SceneProxy;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererOptions};
use pathfinder_renderer::gpu::renderer::Renderer as PfRenderer;
use pathfinder_renderer::options::BuildOptions;
use pathfinder_resources::embedded::EmbeddedResourceLoader;

pub struct PathfinderEventLoop {
    inner: glutin::event_loop::EventLoop<()>,
}

impl EventLoop for PathfinderEventLoop {
    type Renderer = PathfinderRenderer;
    fn run_loop(self, send_input: watch::Sender<Input>) {
        use glutin::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
        let mut keymap = HashSet::new();
        self.inner.run(move |e, _, control_flow| {
            match e {
                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { input, .. },
                    ..
                } => {
                    if let Some(key) = input.virtual_keycode {
                        match input.state {
                            ElementState::Released => keymap.remove(&key),
                            ElementState::Pressed => keymap.insert(key),
                        };
                    }
                    // todo
                }
                Event::MainEventsCleared => {
                    let drive = match (
                        keymap.contains(&VirtualKeyCode::W),
                        keymap.contains(&VirtualKeyCode::S),
                    ) {
                        (true, false) => Some(Drive::Forward),
                        (false, true) => Some(Drive::Reverse),
                        _ => None,
                    };
                    let rotate = match (
                        keymap.contains(&VirtualKeyCode::A),
                        keymap.contains(&VirtualKeyCode::D),
                    ) {
                        (true, false) => Some(Turn::Left),
                        (false, true) => Some(Turn::Right),
                        _ => None,
                    };
                    let turret = match (
                        keymap.contains(&VirtualKeyCode::J),
                        keymap.contains(&VirtualKeyCode::L),
                    ) {
                        (true, false) => Some(Turn::Left),
                        (false, true) => Some(Turn::Right),
                        _ => None,
                    };
                    let fire = keymap.contains(&VirtualKeyCode::Space);
                    send_input
                        .send(Input {
                            drive,
                            rotate,
                            turret,
                            fire,
                            seq: 0,
                        })
                        .unwrap();
                }
                _ => {}
            }
        })
    }
    type MakeRenderer = impl FnOnce() -> Self::Renderer;
    fn create() -> (Self, Self::MakeRenderer)
    where
        Self: Sized,
    {
        let event_loop = glutin::event_loop::EventLoop::new();
        let window_size = Vector2I::new(640, 480);
        let physical_window_size = glutin::dpi::PhysicalSize::new(window_size.x(), window_size.y());

        let window_builder =
            glutin::window::WindowBuilder::new().with_inner_size(physical_window_size);

        let gl_context = glutin::ContextBuilder::new()
            .with_gl(glutin::GlRequest::Latest)
            .with_gl_profile(glutin::GlProfile::Core)
            .with_pixel_format(24, 8)
            .build_windowed(window_builder, &event_loop)
            .unwrap();
        (Self { inner: event_loop }, || {
            PathfinderRenderer::new(gl_context)
        })
    }
}

pub struct PathfinderRenderer {
    context: CanvasRenderingContext2D,
    gl_context: glutin::WindowedContext<glutin::PossiblyCurrent>, // a
    renderer: PfRenderer<GLDevice>,
}

impl PathfinderRenderer {
    fn new(gl_context: glutin::WindowedContext<glutin::NotCurrent>) -> Self {
        let size = Vector2F::new(10.0, 10.0);
        let scale = gl_context.window().scale_factor();
        let window_size = gl_context.window().inner_size().to_logical(scale);
        let window_size = Vector2I::new(window_size.width, window_size.height);

        let gl_context = unsafe { gl_context.make_current().unwrap() };
        gl::load_with(|name| gl_context.get_proc_address(name) as *const _);

        let device = GLDevice::new(GLVersion::GL3, 0);
        let options = RendererOptions {
            background_color: Some(ColorF::white()),
            ..RendererOptions::default()
        };
        let renderer = PfRenderer::new(
            device,
            &EmbeddedResourceLoader,
            DestFramebuffer::full_window(window_size),
            options,
        );
        Self {
            context: Canvas::new(size).get_context_2d(CanvasFontContext::from_system_source()), // a
            gl_context,
            renderer,
        }
    }
}

impl Renderer for PathfinderRenderer {
    fn draw_tank(&mut self, tank: &Tank) {
        let rect = RectF::new(
            Vector2F::new(tank.position.0, tank.position.1),
            Vector2F::new(2.0, 2.0),
        );
        self.context.set_fill_style(ColorU::new(0, 255, 0, 255));
        self.context.fill_rect(rect);
    }
    fn draw_bullet(&mut self, bullet: &Bullet) {
        let mut path = Path2D::new();
        path.ellipse(
            Vector2F::new(bullet.position.0, bullet.position.1),
            Vector2F::new(1.0, 0.5),
            bullet.angle,
            0.0,
            TAU,
        );
        self.context.set_fill_style(ColorU::new(255, 0, 0, 255));
        self.context.fill_path(path, FillRule::Winding);
    }
    fn present_frame(&mut self) {
        let size = self.context.canvas().size().to_f32();
        let font_bruh = CanvasFontContext::from_system_source();
        let context = mem::replace(
            &mut self.context,
            Canvas::new(size).get_context_2d(font_bruh),
        );
        let scene = SceneProxy::from_scene(context.into_canvas().into_scene(), RayonExecutor);
        scene.build_and_render(&mut self.renderer, BuildOptions::default());
        self.gl_context.swap_buffers().unwrap();
    }
}
