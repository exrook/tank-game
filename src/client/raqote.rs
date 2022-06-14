use std::f32::consts::TAU;

use crate::client::{EventLoop, Renderer};
use crate::{Bullet, Drive, Gm, Input, Tank, Turn, GM_SCALE};

use euclid::{Box2D, Point2D, Transform2D, Vector2D};

use tokio::sync::watch;

use raqote::{
    DrawOptions, DrawTarget, Path, PathBuilder, SolidSource, Source, StrokeStyle, Transform,
};

pub struct RaqoteRenderer {
    raqote: raqote::DrawTarget,
}

impl RaqoteRenderer {
    pub fn new(width: i32, height: i32) -> Self {
        let mut raqote = DrawTarget::new(width, height);
        raqote.set_transform(
            &Transform2D::create_scale(1.0, -1.0).post_translate(Vector2D::new(0.0, height as f32)),
        );
        Self { raqote }
    }
    pub fn get_data_u8(&self) -> &[u8] {
        self.raqote.get_data_u8()
    }
    pub fn get_data(&self) -> &[u32] {
        self.raqote.get_data()
    }
    pub fn width(&self) -> i32 {
        self.raqote.width()
    }
    pub fn height(&self) -> i32 {
        self.raqote.height()
    }
}
impl Renderer for RaqoteRenderer {
    fn draw_tank(&mut self, tank: &Tank) {
        let og_transform = self.raqote.get_transform().clone();
        let translate = og_transform.pre_translate((tank.position / GM_SCALE).to_vector().to_f32());
        if !Box2D::new(
            Point2D::new(-40, -40),
            Point2D::new(self.raqote.width(), self.raqote.height()).to_i64(),
        )
        .contains(tank.position / GM_SCALE)
        {
            println!(
                "B {:?}",
                Box2D::<_, euclid::UnknownUnit>::new(
                    Point2D::new(-40, -40),
                    Point2D::new(self.raqote.width(), self.raqote.height()).to_i64(),
                )
            );
            println!("T {:?}", tank.position);
            println!("T {:?}", tank.position / GM_SCALE);
            return;
        }
        self.raqote
            .set_transform(&translate.pre_rotate(-tank.angle));

        self.raqote.fill_rect(
            -20.0,
            -20.0,
            40.0,
            40.0,
            &Source::Solid(SolidSource::from_unpremultiplied_argb(255, 0, 255, 0)),
            &DrawOptions::default(),
        );
        self.raqote
            .set_transform(&translate.pre_rotate(-tank.turret_angle));
        self.raqote.fill_rect(
            0.0,
            -5.0,
            40.0,
            10.0,
            &Source::Solid(SolidSource::from_unpremultiplied_argb(255, 0, 200, 0)),
            &DrawOptions::default(),
        );
        self.raqote.set_transform(&translate);
        self.raqote.fill_rect(
            -30.0,
            -45.0,
            60.0,
            5.0,
            &Source::Solid(SolidSource::from_unpremultiplied_argb(255, 255, 0, 0)),
            &DrawOptions::default(),
        );
        self.raqote.fill_rect(
            -30.0,
            -45.0,
            60.0 * (tank.health as f32 / 100.0),
            5.0,
            &Source::Solid(SolidSource::from_unpremultiplied_argb(255, 0, 255, 0)),
            &DrawOptions::default(),
        );
        self.raqote.set_transform(&og_transform);
    }
    fn draw_bullet(&mut self, bullet: &Bullet) {
        if !Box2D::new(
            Point2D::new(-40, -40),
            Point2D::new(self.raqote.width(), self.raqote.height()).to_i64(),
        )
        .contains(bullet.position / GM_SCALE)
        {
            return;
        }
        let og_transform = self.raqote.get_transform().clone();
        self.raqote.set_transform(
            &og_transform
                .pre_translate((bullet.position / GM_SCALE).to_vector().to_f32())
                .pre_rotate(-bullet.angle)
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
        self.raqote
            .clear(SolidSource::from_unpremultiplied_argb(255, 0, 0, 0));
    }
}
