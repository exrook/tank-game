use std::time::Duration;

use super::{render_frame, EventLoop, RaqoteRenderer, Renderer};
use crate::{Bullet, Drive, GameState, Input, Tank, Turn};

use tokio::sync::watch;

use minifb::{Key, Window, WindowOptions};

pub struct MinifbEventLoop {
    window: Window,
    raqote: RaqoteRenderer,
    frame: Vec<u32>,
    width: usize,
    height: usize,
}

impl EventLoop for MinifbEventLoop {
    fn run_loop(
        mut self,
        rt: tokio::runtime::Runtime,
        send_input: watch::Sender<Input>,
        mut recv_state: watch::Receiver<GameState>,
    ) {
        self.window
            .limit_update_rate(Some(Duration::from_secs(1) / 60));
        while self.window.is_open() {
            let drive = match (
                self.window.is_key_down(Key::W),
                self.window.is_key_down(Key::S),
            ) {
                (true, false) => Some(Drive::Forward),
                (false, true) => Some(Drive::Reverse),
                _ => None,
            };
            let rotate = match (
                self.window.is_key_down(Key::A),
                self.window.is_key_down(Key::D),
            ) {
                (true, false) => Some(Turn::Left),
                (false, true) => Some(Turn::Right),
                _ => None,
            };
            let turret = match (
                self.window.is_key_down(Key::J),
                self.window.is_key_down(Key::L),
            ) {
                (true, false) => Some(Turn::Left),
                (false, true) => Some(Turn::Right),
                _ => None,
            };
            let fire = self.window.is_key_down(Key::Space);
            if let Err(_) = send_input.send(Input {
                drive,
                rotate,
                turret,
                fire,
                seq: 0,
            }) {
                break;
            }
            if let Err(_) = render_frame(&mut self, &mut recv_state) {
                break;
            }
        }
        rt.shutdown_timeout(Duration::from_secs(1));
    }
    fn create() -> Self
    where
        Self: Sized,
    {
        let (width, height) = (1920, 1080);
        let window = Window::new(
            "tank game",
            width,
            height,
            WindowOptions {
                resize: true,
                ..Default::default()
            },
        )
        .unwrap();
        let raqote = RaqoteRenderer::new(width as i32, height as i32);
        let frame = vec![0; width * height];
        Self {
            window,
            raqote,
            frame,
            width,
            height,
        }
    }
}

impl Renderer for MinifbEventLoop {
    fn draw_tank(&mut self, tank: &Tank) {
        self.raqote.draw_tank(tank);
    }
    fn draw_bullet(&mut self, bullet: &Bullet) {
        self.raqote.draw_bullet(bullet);
    }
    fn present_frame(&mut self) {
        let pixels = self.raqote.get_data_u8();
        for (dest, [sb, sg, sr, sa]) in self.frame.iter_mut().zip(pixels.array_chunks()) {
            *dest = (*sa as u32) << 24 | (*sr as u32) << 16 | (*sg as u32) << 8 | *sb as u32;
        }
        self.window
            .update_with_buffer(&self.frame, self.width, self.height)
            .unwrap();
        self.raqote.present_frame();
    }
}
