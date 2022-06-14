use std::any::Any;
use std::time::Duration;

use super::{render_frame, EventLoop, Renderer};
use crate::{Bullet, Drive, GameState, Input, Tank, Turn, GM_SCALE};

use tokio::sync::watch;

use druid_shell::kurbo::{Affine, Ellipse, Rect, Size};
use druid_shell::piet::{self, Color, Piet, RenderContext};
use druid_shell::{Application, Code, KeyEvent, Region, WinHandler, WindowBuilder, WindowHandle};

pub struct DruidEventLoop {
    app: Application,
}

impl EventLoop for DruidEventLoop {
    fn run_loop(
        mut self,
        rt: tokio::runtime::Runtime,
        send_input: watch::Sender<Input>,
        mut recv_state: watch::Receiver<GameState>,
    ) {
        struct WHandler {
            win: Option<WindowHandle>,
            recv_state: watch::Receiver<GameState>,
            input: Input,
            send_input: watch::Sender<Input>,
            size: Size,
        }
        impl WinHandler for WHandler {
            fn connect(&mut self, handle: &WindowHandle) {
                self.win = Some(handle.clone())
            }
            fn prepare_paint(&mut self) {
                if let Some(ref win) = self.win {
                    win.invalidate();
                }
            }
            fn paint(&mut self, piet: &mut Piet<'_>, _invalid: &Region) {
                piet.clear(Color::rgb8(0, 0, 0));
                piet.transform(Affine::scale_non_uniform(1.0, -1.0));
                piet.transform(Affine::translate((0.0, -self.size.height)));
                let mut r = PietRenderer { piet };
                if let Err(_) = self.send_input.send(self.input.clone()) {
                    self.request_close();
                    return;
                }
                if let Err(_) = render_frame(&mut r, &mut self.recv_state) {
                    self.request_close();
                }
            }
            fn size(&mut self, size: Size) {
                self.size = size
            }
            fn as_any(&mut self) -> &mut dyn Any {
                self as &mut dyn Any
            }
            fn key_down(&mut self, event: KeyEvent) -> bool {
                match event.code {
                    Code::KeyW => self.input.drive = Some(Drive::Forward),
                    Code::KeyS => self.input.drive = Some(Drive::Reverse),
                    Code::KeyA => self.input.rotate = Some(Turn::Left),
                    Code::KeyD => self.input.rotate = Some(Turn::Right),
                    Code::KeyJ => self.input.turret = Some(Turn::Left),
                    Code::KeyL => self.input.turret = Some(Turn::Right),
                    Code::Space => self.input.fire = true,
                    _ => {}
                };
                true
            }
            fn key_up(&mut self, event: KeyEvent) {
                match event.code {
                    Code::KeyW | Code::KeyS => self.input.drive = None,
                    Code::KeyA | Code::KeyD => self.input.rotate = None,
                    Code::KeyJ | Code::KeyL => self.input.turret = None,
                    Code::Space => self.input.fire = false,
                    _ => {}
                }
            }
            fn request_close(&mut self) {
                if let Some(ref win) = self.win {
                    win.close()
                }
                Application::global().quit();
            }
        }
        let size = Size::new(1920.0, 1080.0);
        let mut wb = WindowBuilder::new(self.app.clone());
        wb.set_size(size);
        wb.set_handler(Box::new(WHandler {
            win: None,
            recv_state,
            input: Input::default(),
            send_input,
            size,
        }));
        let win = wb.build().unwrap();
        win.show();
        self.app.run(None);
        rt.shutdown_timeout(Duration::from_secs(1));
    }
    fn create() -> Self
    where
        Self: Sized,
    {
        let app = Application::new().unwrap();
        Self { app }
    }
}

struct PietRenderer<'a, 'b> {
    piet: &'a mut Piet<'b>,
}

impl Renderer for PietRenderer<'_, '_> {
    fn draw_tank(&mut self, tank: &Tank) {
        self.piet.save().unwrap();
        let pos = (tank.position / GM_SCALE).to_f64();
        self.piet.transform(Affine::translate((pos.x, pos.y)));

        self.piet
            .with_save(|piet| {
                piet.transform(Affine::rotate(tank.angle.to_f64().radians));
                piet.fill(
                    Rect::from_center_size((0.0, 0.0), (40.0, 40.0)),
                    &Color::rgb8(0, 255, 0),
                );
                Ok(())
            })
            .unwrap();

        self.piet
            .with_save(|piet| {
                piet.transform(Affine::rotate(tank.turret_angle.to_f64().radians));
                piet.fill(
                    Rect::from_origin_size((0.0, -5.0), (40.0, 10.0)),
                    &Color::rgb8(0, 200, 0),
                );
                Ok(())
            })
            .unwrap();

        self.piet.fill(
            Rect::from_origin_size((-30.0, -45.0), (60.0, 5.0)),
            &Color::rgb8(255, 0, 0),
        );
        self.piet.fill(
            Rect::from_origin_size((-30.0, -45.0), (tank.health as f64 * (60.0 / 100.0), 5.0)),
            &Color::rgb8(0, 255, 0),
        );

        self.piet.restore().unwrap();
    }
    fn draw_bullet(&mut self, bullet: &Bullet) {
        //if !Box2D::new(
        //    Point2D::new(-40, -40),
        //    Point2D::new(self.raqote.width(), self.raqote.height()).to_i64(),
        //)
        //.contains(bullet.position / GM_SCALE)
        //{
        //    return;
        //}
        let pos = (bullet.position / GM_SCALE).to_f64();
        self.piet.fill(
            Ellipse::new((pos.x, pos.y), (20.0, 5.0), bullet.angle.to_f64().radians),
            &Color::rgb8(255, 0, 0),
        );
    }
    fn present_frame(&mut self) {}
}
