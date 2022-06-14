use std::f32::consts::TAU;

use super::{render_loop, EventLoop, RaqoteRenderer, Renderer};
use crate::{Bullet, Drive, GameState, Input, Tank, Turn};

use tokio::sync::watch;

use pixels::{Error as PixelsError, Pixels, SurfaceTexture};

use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop as WinitEventLoop};
use winit::window::{Window, WindowBuilder};
use winit_input_helper::WinitInputHelper;

pub struct PixelsRenderer {
    pixels: Pixels,
    raqote: RaqoteRenderer,
    window: Window,
}
pub struct PixelsEventLoop {
    event_loop: WinitEventLoop<()>,
}

impl EventLoop for PixelsEventLoop {
    //type Renderer = PixelsRenderer;
    fn run_loop(
        self,
        rt: tokio::runtime::Runtime,
        send_input: watch::Sender<Input>,
        recv_state: watch::Receiver<GameState>,
    ) {
        let size = LogicalSize::new(1920.0, 1080.0);
        let window = {
            WindowBuilder::new()
                .with_title("tank game")
                .with_inner_size(size)
                .build(&self.event_loop)
                .unwrap()
        };
        let make_renderer = move || {
            let pixels = {
                let window_size = window.inner_size();
                let surface_texture =
                    SurfaceTexture::new(window_size.width, window_size.height, &window);
                Pixels::new(1920, 1080, surface_texture).unwrap()
            };
            let raqote = RaqoteRenderer::new(size.width as i32, size.height as i32);
            PixelsRenderer {
                pixels,
                raqote,
                window,
            }
        };
        rt.spawn_blocking(|| render_loop(make_renderer(), recv_state));
        let mut input = WinitInputHelper::new();
        self.event_loop.run(move |event, _, control_flow| {
            println!("{:?}", event);
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
    //type MakeRenderer = impl FnOnce() -> Self::Renderer + Send;
    //fn create() -> (Self, Self::MakeRenderer)
    //where
    //    Self: Sized,
    //{
    //    let event_loop = WinitEventLoop::new();
    //    let size = LogicalSize::new(1920.0, 1080.0);
    //    let window = {
    //        WindowBuilder::new()
    //            .with_title("tank game")
    //            .with_inner_size(size)
    //            .build(&event_loop)
    //            .unwrap()
    //    };

    //    (Self { event_loop }, move || {
    //        let pixels = {
    //            let window_size = window.inner_size();
    //            let surface_texture =
    //                SurfaceTexture::new(window_size.width, window_size.height, &window);
    //            Pixels::new(1920, 1080, surface_texture).unwrap()
    //        };
    //        let mut raqote = RaqoteRenderer::new(size.width as i32, size.height as i32);
    //        PixelsRenderer {
    //            pixels,
    //            raqote,
    //            window,
    //        }
    //    })
    //}
    fn create() -> Self
    where
        Self: Sized,
    {
        let event_loop = WinitEventLoop::new();
        Self { event_loop }
    }
}

impl Renderer for PixelsRenderer {
    fn draw_tank(&mut self, tank: &Tank) {
        self.raqote.draw_tank(tank);
    }
    fn draw_bullet(&mut self, bullet: &Bullet) {
        self.raqote.draw_bullet(bullet);
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
        self.raqote.present_frame();
    }
}
