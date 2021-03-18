#![feature(type_alias_impl_trait)]

use std::f32::consts::TAU;

mod server;


#[derive(Clone, Debug)]
struct Tank {
    player: Idx<'static, Player>,
    position: (f32, f32),
    angle: f32,
    turret_angle: f32,
    health: i32,
}

#[derive(Clone, Debug)]
enum TankUpdate {
    Dead(Idx<'static, Player>),
    Alive(Tank),
    Fire(Tank, Bullet)
}

#[derive(Clone, Debug)]
enum Turn {
    Left,
    Right,
}

#[derive(Clone, Debug)]
enum Drive {
    Forward,
    Reverse,
}

#[derive(Debug, Default)]
struct Input {
    drive: Option<Drive>,
    rotate: Option<Turn>,
    turret: Option<Turn>,
    fire: bool,
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
            Err(player) => {return TankUpdate::Dead(player)}
            Ok(hp) => hp
        };
        let input = &match &state.players[self.player] {
            None => return TankUpdate::Dead(self.player),
            Some(s) => s
        }.input;
        const TURN_RATE: f32 = TAU * (0.5/60.0);
        let angle = (self.angle + match input.rotate {
            Some(Turn::Left) => TURN_RATE,
            Some(Turn::Right) => -TURN_RATE,
            None => 0.0
        }) % TAU;
        let turret_angle = (self.turret_angle + match input.turret {
            Some(Turn::Left) => TURN_RATE,
            Some(Turn::Right) => -TURN_RATE,
            None => 0.0
        }) % TAU;
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
            health: hp
        };
        match input.fire {
            true => TankUpdate::Fire(tank, Bullet {
                position: (self.position.0 + turret_angle.cos(), self.position.1 + turret_angle.sin()),
                angle: self.turret_angle,
                damage: 10,
                player: self.player
            }),
            false => TankUpdate::Alive(tank),
        }
    }
}

struct TankList {
    tanks: Vec<(Tank, Vec<Bullet>)>
}

#[derive(Clone)]
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
        Self {
            list
        }
    }
}
#[derive(Debug)]
struct Idx<'a, E>(usize, std::marker::PhantomData<&'a ElementList<E>>);

impl<'a, T> Clone for Idx<'a, T> {
    fn clone(&self) -> Self {
        Self(self.0, self.1.clone())
    }
}
impl<'a, T> Copy for Idx<'a, T> { }

impl<'a, E> std::iter::IntoIterator for &'a ElementList<E> {
    type Item = (Idx<'a, E>, &'a E);
    type IntoIter = impl Iterator<Item = Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.list.iter().enumerate().map(|(i, t)|(Idx(i, Default::default()), t))
    }
}

impl<'a, E> std::ops::Index<Idx<'a, E>> for ElementList<E> {
    type Output = E;
    fn index(&self, idx: Idx<'a, E>) -> &Self::Output {
        &self.list[idx.0]
    }
}

#[derive(Clone)]
struct StableList<E> {
    list: Vec<Option<E>>,
}

impl<'a, E: 'static> std::iter::IntoIterator for &'a StableList<E> {
    type Item = (Idx<'static, E>, &'a E);
    type IntoIter = impl Iterator<Item = Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.list.iter().enumerate().filter_map(|(i, t)|t.as_ref().map(|t|(i,t))).map(|(i, t)|(Idx(i, Default::default()), t))
    }
}

impl<E> std::ops::Index<Idx<'static, E>> for StableList<E> {
    type Output = Option<E>;
    fn index(&self, idx: Idx<'static, E>) -> &Self::Output {
        &self.list[idx.0]
    }
}

#[derive(Clone, Debug)]
struct Bullet {
    position: (f32, f32),
    angle: f32,
    damage: i32,
    player: Idx<'static, Player>,
}

impl Bullet {
    fn tick<'a>(&self, state: &'a GameState) -> BulletUpdate<'a> {
        let position = (self.position.0 + self.angle.cos(), self.position.1 + self.angle.sin());
        match state.collide(position) {
            Some(Collision::Tank(tank)) => BulletUpdate::Hit(tank),
            Some(Collision::Arena) => BulletUpdate::Dead,
            None => BulletUpdate::Move(Self {
                position,
                ..self.clone()
            })
        }
    }
}

#[derive(Debug)]
struct Player {
    name: String,
    input: Input
}

pub struct GameState {
    players: StableList<Player>,
    tanks: ElementList<Tank>,
    tank_bullets: ElementList<Vec<Bullet>>,
    bullets: ElementList<Bullet>,
}

enum Collision<'a> {
    Tank(Idx<'a, Tank>),
    Arena
}

impl GameState {
    pub fn step(&self) -> Self {
        let mut new_tanks = Vec::with_capacity(self.tanks.len());
        let mut new_bullets = Vec::with_capacity(self.bullets.len());
        for ((i, tank), (_, bullets)) in self.tanks.into_iter().zip(&self.tank_bullets) {
            match tank.tick(&self, &bullets) {
                TankUpdate::Dead(player) => {
                    println!("PLAYER {:?} KILLED {:?}'S TANK", self.players[player], self.players[tank.player]);
                }
                TankUpdate::Alive(tank) => {
                    new_tanks.push(tank);
                    todo!()
                }
                TankUpdate::Fire(tank, bullet) => {
                    new_tanks.push(tank);
                    new_bullets.push(bullet);
                    todo!()
                }
            }
        }
        for (i, bullet) in &self.bullets {
            match bullet.tick(&self) {
                BulletUpdate::Hit(i) => {
                    todo!();
                }
                BulletUpdate::Move(bullet) => {
                    new_bullets.push(bullet)
                }
                BulletUpdate::Dead => {
                    todo!()
                }
            }
        }
        todo!()
    }
    fn collide(&self, position: (f32, f32)) -> Option<Collision> {
        todo!()
    }
}

enum BulletUpdate<'a> {
    Hit(Idx<'a, Tank>), // hit tank
    Move(Bullet), // otherwise move forward
    Dead
}
