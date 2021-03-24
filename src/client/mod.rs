use futures::{try_join, FutureExt, Sink, SinkExt, Stream, StreamExt};
use std::collections::VecDeque;
use std::future::Future;

use tokio::sync::{watch, RwLock};

use crate::{Bullet, GameState, Idx, Input, Player, Tank, Time};

use tokio_tungstenite::tungstenite;

use rmp_serde;

mod pathfinder;
pub use pathfinder::PathfinderEventLoop;

pub fn run_client<EL: EventLoop>() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (input_send, input_recv) = watch::channel(Input::default());
    let (client_loop, recv_state) = rt.block_on(client_loop(input_recv));
    let (event_loop, make_renderer) = EL::create();
    rt.spawn(client_loop);
    rt.spawn_blocking(|| render_loop(make_renderer(), recv_state));
    event_loop.run_loop(input_send);
}

pub trait EventLoop {
    type Renderer: Renderer;
    type MakeRenderer: FnOnce() -> Self::Renderer + Send + 'static;
    fn run_loop(self, send_input: watch::Sender<Input>);
    fn create() -> (Self, Self::MakeRenderer)
    where
        Self: Sized;
}

pub trait Renderer {
    fn draw_tank(&mut self, tank: &Tank);
    fn draw_bullet(&mut self, bullet: &Bullet);
    fn present_frame(&mut self);
}

pub struct NoopRenderer;

impl EventLoop for NoopRenderer {
    type Renderer = Self;
    fn run_loop(self, _send_input: watch::Sender<Input>) {
        loop {}
    }
    type MakeRenderer = impl FnOnce() -> Self::Renderer;
    fn create() -> (Self, Self::MakeRenderer)
    where
        Self: Sized,
    {
        (Self, || Self)
    }
}

impl Renderer for NoopRenderer {
    fn draw_tank(&mut self, _tank: &Tank) {}
    fn draw_bullet(&mut self, _bullet: &Bullet) {}
    fn present_frame(&mut self) {}
}

fn render_loop<R: Renderer>(mut renderer: R, mut recv_state: watch::Receiver<GameState>) {
    let mut state = recv_state.borrow().clone();
    loop {
        if let Some(res) = recv_state.changed().now_or_never() {
            res.unwrap();
            state = recv_state.borrow().clone();
        }
        // Get current state
        draw_state(&state, &mut renderer);
        renderer.present_frame();
    }
}

//struct Client {
//    server_state: GameState,
//    client_state: GameState,
//    input_history: VecDeque<(Time, Input)>,
//}

async fn client_loop(
    mut input_recv: watch::Receiver<Input>,
) -> (impl Future<Output = ()> + Send, watch::Receiver<GameState>) {
    let (socket, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:8998/stream")
        .await
        .unwrap();
    let (mut sink, mut stream) = socket.split();
    let player_id = parse_id(stream.next().await.unwrap().unwrap()).unwrap();
    let init_game_state = parse_state(stream.next().await.unwrap().unwrap()).unwrap();
    let (send_state, recv_state) = watch::channel(init_game_state);
    (
        async move {
            let input_history = RwLock::new(VecDeque::<Input>::new());
            let input_loop = async {
                // need async type ascription to remove this
                if false {
                    return Ok::<(), ()>(());
                }
                let mut input_seq = 0;
                loop {
                    input_recv.changed().await;
                    let mut input = input_recv.borrow().clone();
                    input.seq = input_seq;
                    input_seq += 1;
                    sink.send(tungstenite::Message::Binary(
                        rmp_serde::to_vec(&input).map_err(|_| ())?,
                    ))
                    .await
                    .map_err(|_| ())?;
                    input_history.write().await.push_back(input);
                    // TODO: synchronize / delay
                }
            };
            let recv_loop = async {
                // need async block type ascription to remove this
                if false {
                    return Ok::<(), ()>(());
                }
                loop {
                    let mut msg = stream.next().await.unwrap().unwrap();
                    // Attempt to drain any states that may be buffered
                    while let Some(next_msg) = stream.next().now_or_never() {
                        msg = next_msg.unwrap().unwrap()
                    }
                    let state = parse_state(msg).unwrap();
                    let game_seq = state.players[player_id].as_ref().unwrap().input.seq;
                    let mut input_history = input_history.write().await;
                    // remove inputs already included
                    while input_history
                        .front()
                        .map(|i| i.seq <= game_seq)
                        .unwrap_or(false)
                    {
                        input_history.pop_front();
                    }
                    let input_history = input_history.downgrade();
                    // broadcast state
                    let predicted_state =
                        input_history
                            .iter()
                            .fold(state.clone(), |mut state, input| {
                                state.players[player_id].as_mut().unwrap().input = input.clone();
                                state.tick()
                            });
                    send_state.send(predicted_state).unwrap();
                }
            };
            let _ = try_join!(input_loop, recv_loop);
        },
        recv_state,
    )
}

fn parse_id(msg: tungstenite::Message) -> Option<Idx<'static, Player>> {
    rmp_serde::from_read_ref(&msg.into_data()).ok()
}
fn parse_state(msg: tungstenite::Message) -> Option<GameState> {
    rmp_serde::from_read_ref(&msg.into_data()).ok()
}

fn get_input(seq: u64) -> Input {
    todo!();
}

fn draw_state(state: &GameState, r: &mut impl Renderer) {
    for (_i, tank) in &state.tanks {
        r.draw_tank(tank)
    }
    for (_i, bullet) in &state.bullets {
        r.draw_bullet(bullet)
    }
}
