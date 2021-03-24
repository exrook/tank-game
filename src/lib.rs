#![feature(type_alias_impl_trait)]

use std::f32::consts::TAU;
use std::mem;

use serde::{Deserialize, Serialize};

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "server")]
mod server;

pub use client::{run_client, NoopRenderer, PathfinderEventLoop};
pub use server::run_server;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tank {
    player: Idx<'static, Player>,
    position: (f32, f32),
    angle: f32,
    turret_angle: f32,
    health: i32,
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

impl Tank {
    fn tick(&self, state: &GameState, bullets: &[Bullet]) -> TankUpdate {
        let hp = match bullets.into_iter().try_fold(self.health, |hp, bullet| {
            let hp = hp - bullet.damage;
            if hp <= 0 {
                Err(bullet.player)
            } else {
                Ok(hp)
            }
        }) {
            Err(player) => return TankUpdate::Dead(player),
            Ok(hp) => hp,
        };
        let input = &match &state.players[self.player] {
            None => return TankUpdate::Dead(self.player),
            Some(s) => s,
        }
        .input;
        const TURN_RATE: f32 = TAU * (0.5 / 60.0);
        let angle = (self.angle
            + match input.rotate {
                Some(Turn::Left) => TURN_RATE,
                Some(Turn::Right) => -TURN_RATE,
                None => 0.0,
            })
            % TAU;
        let turret_angle = (self.turret_angle
            + match input.turret {
                Some(Turn::Left) => TURN_RATE,
                Some(Turn::Right) => -TURN_RATE,
                None => 0.0,
            })
            % TAU;
        let position = match input.drive {
            Some(Drive::Forward) => (self.position.0 + angle.cos(), self.position.1 + angle.sin()),
            Some(Drive::Reverse) => (self.position.0 - angle.cos(), self.position.1 - angle.sin()),
            None => self.position,
        };

        let position = if let Some(_) = state.collide(position) {
            self.position
        } else {
            position
        };
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
                    position: (
                        self.position.0 + turret_angle.cos(),
                        self.position.1 + turret_angle.sin(),
                    ),
                    angle: self.turret_angle,
                    damage: 10,
                    player: self.player,
                },
            ),
            false => TankUpdate::Alive(tank),
        }
    }
}

struct TankList {
    tanks: Vec<(Tank, Vec<Bullet>)>,
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
    type Item = (Idx<'static, E>, &'a E);
    type IntoIter = impl Iterator<Item = Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.list
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.as_ref().map(|t| (i, t)))
            .map(|(i, t)| (Idx(i, Default::default()), t))
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
    position: (f32, f32),
    angle: f32,
    damage: i32,
    player: Idx<'static, Player>,
}

impl Bullet {
    fn tick(&self, state: &GameState) -> BulletUpdate {
        let position = (
            self.position.0 + self.angle.cos(),
            self.position.1 + self.angle.sin(),
        );
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
    collision: CollisionMap<Collision>,
    time: Time,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Time(pub u64);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CollisionMap<T> {
    rtree: rstar::RTree<Hitbox<T>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Hitbox<T> {
    rect: rstar::primitives::Rectangle<[f32; 2]>,
    element: T,
}

impl<T> rstar::RTreeObject for Hitbox<T> {
    type Envelope = rstar::AABB<[f32; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.rect.envelope()
    }
}
impl<T> rstar::PointDistance for Hitbox<T> {
    fn distance_2(&self, point: &[f32; 2]) -> f32 {
        self.rect.distance_2(point)
    }
    fn contains_point(&self, point: &[f32; 2]) -> bool {
        self.rect.contains_point(point)
    }
    fn distance_2_if_less_or_equal(&self, point: &[f32; 2], max_distance_2: f32) -> Option<f32> {
        self.rect.distance_2_if_less_or_equal(point, max_distance_2)
    }
}

impl<T> CollisionMap<T> {
    fn add(&mut self, position: (f32, f32), size: (f32, f32), element: T) {
        let c1 = [position.0 + (size.0 / 2.0), position.1 + (size.1 / 2.0)];
        let c2 = [position.0 - (size.0 / 2.0), position.1 - (size.1 / 2.0)];
        self.rtree.insert(Hitbox {
            rect: rstar::primitives::Rectangle::from_corners(c1, c2),
            element,
        });
    }
    fn remove(&mut self, position: (f32, f32), size: (f32, f32), element: T) -> Option<T>
    where
        T: PartialEq,
    {
        let c1 = [position.0 + (size.0 / 2.0), position.1 + (size.1 / 2.0)];
        let c2 = [position.0 - (size.0 / 2.0), position.1 - (size.1 / 2.0)];
        let hitbox = Hitbox {
            rect: rstar::primitives::Rectangle::from_corners(c1, c2),
            element,
        };
        self.rtree.remove(&hitbox).map(|x| x.element)
    }
    fn collide(&self, position: (f32, f32)) -> Option<&T> {
        self.rtree
            .locate_at_point(&[position.0, position.1])
            .map(|h| &h.element)
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
            .map(|((tank_idx, tank), (_, bullets))| (tank_idx, tank.tick(&self, &bullets)))
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
                    if tank.position != self.tanks[tank_idx].as_ref().unwrap().position {
                        moved_tanks.push(tank_idx);
                    }
                    new_tanks[tank_idx] = Some(tank);
                }
                TankUpdate::Fire(tank, bullet) => {
                    if tank.position != self.tanks[tank_idx].as_ref().unwrap().position {
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
            collision.remove(
                self.tanks[tank].as_ref().unwrap().position,
                (2.0, 2.0),
                Collision::Tank(tank),
            );
        }
        for tank_idx in moved_tanks {
            collision.remove(
                self.tanks[tank_idx].as_ref().unwrap().position,
                (2.0, 2.0),
                Collision::Tank(tank_idx),
            );
            collision.add(
                new_tanks[tank_idx].as_ref().unwrap().position,
                (2.0, 2.0),
                Collision::Tank(tank_idx),
            );
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
    fn collide(&self, position: (f32, f32)) -> Option<Collision> {
        self.collision.collide(position).cloned()
    }
}

enum BulletUpdate {
    Hit(Idx<'static, Tank>), // hit tank
    Move(Bullet),            // otherwise move forward
    Dead,
}
