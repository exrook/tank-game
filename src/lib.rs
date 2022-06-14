#![feature(min_type_alias_impl_trait)]
#![feature(array_chunks)]

use std::f32::consts::TAU;
use std::mem;

use serde::{Deserialize, Serialize};

use euclid::{Angle, Length, Scale};

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "server")]
mod server;

#[cfg(all(feature = "druid_backend", feature = "client"))]
pub use client::DruidEventLoop;
#[cfg(all(feature = "minifb_backend", feature = "client"))]
pub use client::MinifbEventLoop;
#[cfg(all(feature = "pathfinder_backend", feature = "client"))]
pub use client::PathfinderEventLoop;
#[cfg(all(feature = "pixels_backend", feature = "client"))]
pub use client::PixelsEventLoop;
#[cfg(feature = "client")]
pub use client::{run_client, NoopRenderer};
#[cfg(feature = "server")]
pub use server::run_server;

/// Gm = Game meter
pub enum Gm {}
/// 1 Pixel
pub type Pixel = euclid::UnknownUnit;

type Point2D<T = i64, U = Gm> = euclid::Point2D<T, U>;
type Size2D<T = i64, U = Gm> = euclid::Size2D<T, U>;
type Vector2D<T = i64, U = Gm> = euclid::Vector2D<T, U>;
type Box2D<T = i64, U = Gm> = euclid::Box2D<T, U>;
type Rotation2D<T = f32, Src = Gm, Dst = Gm> = euclid::Rotation2D<T, Src, Dst>;
type Transform2D<T = f32, Src = Gm, Dst = Gm> = euclid::Rotation2D<T, Src, Dst>;

pub const GM_ONE_PIXEL: i64 = 10000;
pub const GM_SCALE: Scale<i64, Pixel, Gm> = Scale::new(GM_ONE_PIXEL);
const UPDATES_PER_SECOND: i64 = 60;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tank {
    player: Idx<'static, Player>,
    position: Point2D,
    angle: Angle<f32>,
    turret_angle: Angle<f32>,
    health: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum TankUpdate {
    Dead(Idx<'static, Player>),
    Alive(Tank),
    Fire(Tank, Bullet),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum Turn {
    Left,
    Right,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum Drive {
    Forward,
    Reverse,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Input {
    drive: Option<Drive>,
    rotate: Option<Turn>,
    turret: Option<Turn>,
    fire: bool,
    seq: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TankHitbox {
    center: Point2D,
    angle: Angle<f32>,
}

impl TankHitbox {
    const TANK_SIZE: i64 = 20 * GM_ONE_PIXEL;
    const ENVELOPE_LIMIT: i64 = (Self::TANK_SIZE as f64 * std::f64::consts::SQRT_2) as i64;
    fn distance_relative(&self, relative_vec: &Vector2D) -> i64 {
        let aabb = Box2D::<_, Gm>::zero().inflate(Self::TANK_SIZE, Self::TANK_SIZE);
        rstar::AABB::from_corners(aabb.min, aabb.max).distance_2(
            &Rotation2D::new(self.angle)
                .inverse()
                .transform_vector(relative_vec.to_f32())
                .to_i64()
                .to_point(),
        )
    }
    //pub fn box(&self) -> (Box2D, Rotation2D, Translation2D) {
    //    (Box2D::zero()
    //        .inflate(Self::ENVELOPE_LIMIT, Self::ENVELOPE_LIMIT),
    //    Rotation2D::new(self.angle),
    //    Translation2D::new(self.point.to_vector())
    //}
}

impl rstar::RTreeObject for TankHitbox {
    type Envelope = rstar::AABB<Point2D>;
    fn envelope(&self) -> Self::Envelope {
        let aabb = Box2D::zero()
            .translate(self.center.to_vector())
            .inflate(Self::ENVELOPE_LIMIT, Self::ENVELOPE_LIMIT);
        rstar::AABB::from_corners(aabb.min, aabb.max)
    }
}

impl rstar::PointDistance for TankHitbox {
    fn distance_2(&self, point: &Point2D) -> i64 {
        self.distance_relative(&(*point - self.center))
    }
    fn contains_point(&self, point: &Point2D) -> bool {
        let vec = *point - self.center;
        if vec.square_length() > (Self::ENVELOPE_LIMIT * Self::ENVELOPE_LIMIT * 2) {
            false
        } else {
            self.distance_relative(&vec) <= 0
        }
    }
}

impl Tank {
    fn tick(&self, state: &GameState, bullets: &[Bullet]) -> TankUpdate {
        let hp = match bullets.into_iter().try_fold(self.health, |hp, bullet| {
            let hp = hp - bullet.damage;
            //if hp <= 0 {
            //    Err(bullet.player)
            //} else {
            Ok(hp)
            //}
        }) {
            Err(player) => return TankUpdate::Dead(player),
            Ok(hp) => hp,
        };
        let input = &match &state.players[self.player] {
            None => return TankUpdate::Dead(self.player),
            Some(s) => s,
        }
        .input;
        const TURN_RATE: Angle<f32> = Angle {
            radians: TAU * (0.5 / UPDATES_PER_SECOND as f32),
        };
        let angle = (self.angle
            + match input.rotate {
                Some(Turn::Left) => TURN_RATE,
                Some(Turn::Right) => -TURN_RATE,
                None => Angle::zero(),
            })
        .positive();
        //% TAU;
        let turret_angle = (self.turret_angle
            + match input.turret {
                Some(Turn::Left) => TURN_RATE,
                Some(Turn::Right) => -TURN_RATE,
                None => Angle::zero(),
            })
        .positive();
        //% TAU;
        let position = self.position
            + match input.drive {
                Some(Drive::Forward) => {
                    (Vector2D::from_angle_and_length(angle, 280.0) * GM_SCALE.cast()).to_i64()
                        / UPDATES_PER_SECOND
                }
                Some(Drive::Reverse) => {
                    (-Vector2D::from_angle_and_length(angle, 280.0) * GM_SCALE.cast()).to_i64()
                        / UPDATES_PER_SECOND
                }
                None => Vector2D::zero(),
            };

        // TODO
        //let position = if let Some(_) = state.collide(position) {
        //    self.position
        //} else {
        //    position
        //};
        let tank = Tank {
            player: self.player,
            position,
            angle,
            turret_angle,
            health: hp,
        };
        match input.fire {
            true => TankUpdate::Fire(
                tank,
                Bullet {
                    position: self.position
                        + Vector2D::from_angle_and_length(turret_angle, 40.0).to_i64() * GM_SCALE,
                    angle: self.turret_angle,
                    damage: 10,
                    player: self.player,
                    birth: state.time,
                },
            ),
            false => TankUpdate::Alive(tank),
        }
    }
    fn hitbox(&self) -> TankHitbox {
        TankHitbox {
            center: self.position,
            angle: self.angle,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ElementList<E> {
    list: Vec<E>,
}

impl<E> ElementList<E> {
    fn len(&self) -> usize {
        self.list.len()
    }
}

impl<E> From<Vec<E>> for ElementList<E> {
    fn from(list: Vec<E>) -> Self {
        Self { list }
    }
}
#[derive(Debug, Serialize, Deserialize)]
struct Idx<'a, E>(usize, std::marker::PhantomData<&'a ElementList<E>>);

impl<'a, T> Clone for Idx<'a, T> {
    fn clone(&self) -> Self {
        Self(self.0, self.1.clone())
    }
}
impl<'a, T> Copy for Idx<'a, T> {}

impl<'a, E> PartialEq for Idx<'a, E> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<'a, E> Eq for Idx<'a, E> {}
impl<'a, E> std::hash::Hash for Idx<'a, E> {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.0.hash(hasher)
    }
}

impl<'a, E> std::iter::IntoIterator for &'a ElementList<E> {
    type Item = (Idx<'a, E>, &'a E);
    type IntoIter = impl Iterator<Item = Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.list
            .iter()
            .enumerate()
            .map(|(i, t)| (Idx(i, Default::default()), t))
    }
}

impl<'a, E> std::ops::Index<Idx<'a, E>> for ElementList<E> {
    type Output = E;
    fn index(&self, idx: Idx<'a, E>) -> &Self::Output {
        &self.list[idx.0]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StableList<E> {
    list: Vec<Option<E>>,
}

impl<E> StableList<E> {
    pub fn len(&self) -> usize {
        self.list.len()
    }
    pub fn push(&mut self, value: E) -> Idx<'static, E> {
        let i = if let Some((i, x)) = self.list.iter_mut().enumerate().find(|(i, x)| x.is_none()) {
            *x = Some(value);
            i
        } else {
            let i = self.list.len();
            self.list.push(Some(value));
            i
        };
        Idx(i, Default::default())
    }
    pub fn remove(&mut self, idx: &Idx<'static, E>) -> Option<E> {
        mem::take(&mut self.list[idx.0])
    }
}
impl<E> From<Vec<Option<E>>> for StableList<E> {
    fn from(list: Vec<Option<E>>) -> Self {
        Self { list }
    }
}

impl<'a, E: 'static> std::iter::IntoIterator for &'a StableList<E> {
    type Item = (Idx<'static, E>, Option<&'a E>);
    type IntoIter = impl Iterator<Item = Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.list
            .iter()
            .enumerate()
            //.filter_map(|(i, t)| t.as_ref().map(|t| (i, t)))
            .map(|(i, t)| (Idx(i, Default::default()), t.as_ref()))
    }
}

impl<E> std::ops::Index<Idx<'static, E>> for StableList<E> {
    type Output = Option<E>;
    fn index(&self, idx: Idx<'static, E>) -> &Self::Output {
        &self.list[idx.0]
    }
}
impl<E> std::ops::IndexMut<Idx<'static, E>> for StableList<E> {
    fn index_mut(&mut self, idx: Idx<'static, E>) -> &mut Self::Output {
        &mut self.list[idx.0]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bullet {
    position: Point2D,
    angle: Angle<f32>,
    damage: i64,
    birth: Time,
    player: Idx<'static, Player>,
}

impl Bullet {
    fn tick(&self, state: &GameState) -> BulletUpdate {
        let position = self.position
            + (Vector2D::from_angle_and_length(self.angle, 1000.0) * GM_SCALE.cast()).to_i64()
                / UPDATES_PER_SECOND;
        if state.time.0 - self.birth.0 > 60 * 10 {
            return BulletUpdate::Dead;
        }
        //if position.square_length() > (1000000 * 1000) {
        //    return BulletUpdate::Dead;
        //}
        match state.collide(position) {
            Some(Collision::Tank(tank)) => BulletUpdate::Hit(tank),
            Some(Collision::Arena) => BulletUpdate::Dead,
            None => BulletUpdate::Move(Self {
                position,
                ..self.clone()
            }),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Player {
    name: String,
    input: Input,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameState {
    pub(crate) players: StableList<Player>,
    pub(crate) tanks: StableList<Tank>,
    pub(crate) tank_bullets: StableList<Vec<Bullet>>,
    pub(crate) bullets: ElementList<Bullet>,
    collision: CollisionMap,
    time: Time,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Time(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Hitbox {
    Tank(TankHitbox, Idx<'static, Tank>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CollisionMap {
    rtree: rstar::RTree<Hitbox>,
}

impl rstar::RTreeObject for Hitbox {
    type Envelope = rstar::AABB<Point2D>;
    fn envelope(&self) -> Self::Envelope {
        match &self {
            Self::Tank(hitbox, idx) => hitbox.envelope(),
        }
    }
}
impl rstar::PointDistance for Hitbox {
    fn distance_2(&self, point: &Point2D) -> i64 {
        match &self {
            Self::Tank(hitbox, idx) => hitbox.distance_2(point),
        }
    }
    fn contains_point(&self, point: &Point2D) -> bool {
        match &self {
            Self::Tank(hitbox, idx) => hitbox.contains_point(point),
        }
    }
    fn distance_2_if_less_or_equal(&self, point: &Point2D, max_distance_2: i64) -> Option<i64> {
        match &self {
            Self::Tank(hitbox, idx) => hitbox.distance_2_if_less_or_equal(point, max_distance_2),
        }
    }
}

impl CollisionMap {
    fn add(&mut self, element: Hitbox) {
        self.rtree.insert(element);
    }
    fn remove(&mut self, element: Hitbox) -> Option<Hitbox> {
        self.rtree.remove(&element)
    }
    fn collide(&self, position: Point2D) -> Option<&Hitbox> {
        self.rtree.locate_at_point(&position)
    }
    fn new() -> Self {
        CollisionMap {
            rtree: rstar::RTree::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Collision {
    Tank(Idx<'static, Tank>),
    Arena,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            players: StableList::from(vec![]),
            tanks: StableList::from(vec![]),
            tank_bullets: StableList::from(vec![]),
            bullets: ElementList::from(vec![]),
            collision: CollisionMap::new(),
            time: Time(0),
        }
    }
    pub fn tick(&self) -> Self {
        let mut new_players = self.players.clone();
        let mut new_tanks = self.tanks.clone();
        let mut new_bullets = Vec::with_capacity(self.bullets.len());
        let mut collision = self.collision.clone();
        let mut removed_tanks = vec![];
        let mut moved_tanks = vec![];
        // tick objects
        let tank_updates: Vec<_> = self
            .tanks
            .into_iter()
            .zip(&self.tank_bullets)
            .filter_map(|((tank_idx, tank), (b_idx, b))| {
                tank.map(|tank| ((tank_idx, tank), (b_idx, b)))
            })
            .map(|((tank_idx, tank), (_, bullets))| {
                (
                    tank_idx,
                    tank.tick(&self, bullets.map(|b| b.as_ref()).unwrap_or(&[])),
                )
            })
            .collect();
        let bullet_updates: Vec<_> = self
            .bullets
            .into_iter()
            .map(|(i, bullet)| (i, bullet.tick(&self)))
            .collect();

        // process updates
        for (tank_idx, update) in tank_updates {
            match update {
                TankUpdate::Dead(player) => {
                    let tank = self.tanks[tank_idx].as_ref().unwrap();
                    new_tanks[tank_idx] = None;
                    removed_tanks.push(tank_idx);
                    println!(
                        "PLAYER {:?} KILLED {:?}'S TANK",
                        self.players[player], self.players[tank.player]
                    );
                }
                TankUpdate::Alive(tank) => {
                    if tank.hitbox() != self.tanks[tank_idx].as_ref().unwrap().hitbox() {
                        moved_tanks.push(tank_idx);
                    }
                    new_tanks[tank_idx] = Some(tank);
                }
                TankUpdate::Fire(tank, bullet) => {
                    if tank.hitbox() != self.tanks[tank_idx].as_ref().unwrap().hitbox() {
                        moved_tanks.push(tank_idx);
                    }
                    new_tanks[tank_idx] = Some(tank);
                    new_bullets.push(bullet);
                }
            }
        }

        let mut new_tank_bullets: Vec<Option<Vec<_>>> = vec![None; new_tanks.len()];
        for (idx, update) in bullet_updates {
            match update {
                BulletUpdate::Hit(tank) => match &mut new_tank_bullets[tank.0] {
                    &mut Some(ref mut v) => {
                        v.push(self.bullets[idx].clone());
                    }
                    x @ &mut None => {
                        *x = Some(vec![self.bullets[idx].clone()]);
                    }
                },
                BulletUpdate::Move(bullet) => new_bullets.push(bullet),
                BulletUpdate::Dead => {
                    // do nothing
                }
            }
        }

        // reduce
        for tank in removed_tanks {
            let a = collision.remove(Hitbox::Tank(
                self.tanks[tank].as_ref().unwrap().hitbox(),
                tank,
            ));
        }
        for tank_idx in moved_tanks {
            let a = collision.remove(Hitbox::Tank(
                self.tanks[tank_idx].as_ref().unwrap().hitbox(),
                tank_idx,
            ));
            collision.add(Hitbox::Tank(
                new_tanks[tank_idx].as_ref().unwrap().hitbox(),
                tank_idx,
            ));
        }
        Self {
            players: new_players,
            tanks: new_tanks,
            tank_bullets: new_tank_bullets.into(),
            bullets: ElementList { list: new_bullets },
            collision,
            time: Time(self.time.0.wrapping_add(1)),
        }
    }
    fn collide(&self, position: Point2D) -> Option<Collision> {
        if let Some(h) = self.collision.collide(position).cloned() {
            Some(match h {
                Hitbox::Tank(h, tank_idx) => Collision::Tank(tank_idx),
            })
        } else {
            None
        }
    }
}

enum BulletUpdate {
    Hit(Idx<'static, Tank>), // hit tank
    Move(Bullet),            // otherwise move forward
    Dead,
}
