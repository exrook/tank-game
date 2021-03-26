use std::f32::consts::TAU;

use crate::client::{EventLoop, Renderer};
use crate::{Bullet, Drive, Input, Tank, Turn};

use tokio::sync::watch;

use pixels::{Error as PixelsError, Pixels, SurfaceTexture};
use raqote::{
    DrawOptions, DrawTarget, Path, PathBuilder, SolidSource, Source, StrokeStyle, Transform,
};

use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop as WinitEventLoop};
use winit::window::{Window, WindowBuilder};
use winit_input_helper::WinitInputHelper;

pub struct PixelsRenderer {
    pixels: Pixels,
    raqote: raqote::DrawTarget,
    window: Window,
}
pub struct PixelsEventLoop {
    event_loop: WinitEventLoop<()>,
}

impl EventLoop for PixelsEventLoop {
    type Renderer = PixelsRenderer;
    fn run_loop(self, send_input: watch::Sender<Input>) {
        let mut input = WinitInputHelper::new();
        self.event_loop.run(move |event, _, control_flow| {
            if input.update(&event) {
                let drive = match (
                    input.key_held(VirtualKeyCode::W),
                    input.key_held(VirtualKeyCode::S),
                ) {
                    (true, false) => Some(Drive::Forward),
                    (false, true) => Some(Drive::Reverse),
                    _ => None,
                };
                let rotate = match (
                    input.key_held(VirtualKeyCode::A),
                    input.key_held(VirtualKeyCode::D),
                ) {
                    (true, false) => Some(Turn::Left),
                    (false, true) => Some(Turn::Right),
                    _ => None,
                };
                let turret = match (
                    input.key_held(VirtualKeyCode::J),
                    input.key_held(VirtualKeyCode::L),
                ) {
                    (true, false) => Some(Turn::Left),
                    (false, true) => Some(Turn::Right),
                    _ => None,
                };
                let fire = input.key_held(VirtualKeyCode::Space);
                if let Err(_) = send_input.send(Input {
                    drive,
                    rotate,
                    turret,
                    fire,
                    seq: 0,
                }) {
                    *control_flow = ControlFlow::Exit;
                } else {
                    *control_flow = ControlFlow::Wait;
                }
            }
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                _ => {}
            }
        });
    }
    type MakeRenderer = impl FnOnce() -> Self::Renderer + Send;
    fn create() -> (Self, Self::MakeRenderer)
    where
        Self: Sized,
    {
        let event_loop = WinitEventLoop::new();
        let size = LogicalSize::new(1920.0, 1080.0);
        let window = {
            WindowBuilder::new()
                .with_title("tank game")
                .with_inner_size(size)
                .with_min_inner_size(size)
                .build(&event_loop)
                .unwrap()
        };

        (Self { event_loop }, move || {
            let pixels = {
                let window_size = window.inner_size();
                let surface_texture =
                    SurfaceTexture::new(window_size.width, window_size.height, &window);
                Pixels::new(1920, 1080, surface_texture).unwrap()
            };
            let raqote = DrawTarget::new(size.width as i32, size.height as i32);
            PixelsRenderer {
                pixels,
                raqote,
                window,
            }
        })
    }
}

impl Renderer for PixelsRenderer {
    fn draw_tank(&mut self, tank: &Tank) {
        let og_transform = self.raqote.get_transform().clone();
        self.raqote.set_transform(
            &og_transform
                .pre_translate(tank.position.into())
                .pre_rotate(euclid::Angle::radians(tank.angle)),
        );

        self.raqote.fill_rect(
            0.0,
            0.0,
            20.0,
            20.0,
            &Source::Solid(SolidSource::from_unpremultiplied_argb(255, 0, 255, 0)),
            &DrawOptions::default(),
        );
        self.raqote.set_transform(&og_transform);
    }
    fn draw_bullet(&mut self, bullet: &Bullet) {
        let og_transform = self.raqote.get_transform().clone();
        self.raqote.set_transform(
            &og_transform
                .pre_translate(bullet.position.into())
                .pre_rotate(euclid::Angle::radians(bullet.angle))
                .pre_scale(2.0, 0.5),
        );
        let mut path = PathBuilder::new();
        path.arc(0.0, 0.0, 10.0, 0.0, TAU);
        path.close();
        let path = path.finish();
        self.raqote.fill(
            &path,
            &Source::Solid(SolidSource::from_unpremultiplied_argb(255, 255, 0, 0)),
            &DrawOptions::default(),
        );
        self.raqote.set_transform(&og_transform);
    }
    fn present_frame(&mut self) {
        let frame = self.pixels.get_frame();
        let pixels = self.raqote.get_data_u8();
        for ([dr, dg, db, da], [sb, sg, sr, sa]) in
            frame.array_chunks_mut().zip(pixels.array_chunks())
        {
            *dr = *sr;
            *dg = *sg;
            *db = *sb;
            *da = *sa;
        }
        self.pixels.render().unwrap();
        self.raqote
            .clear(SolidSource::from_unpremultiplied_argb(255, 0, 0, 0));
    }
}
