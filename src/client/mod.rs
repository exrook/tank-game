use futures::{try_join, FutureExt, Sink, SinkExt, Stream, StreamExt};
use futures::lock::BiLock;
use std::collections::VecDeque;
use std::future::Future;
use std::net::{SocketAddr, ToSocketAddrs};

use tokio::sync::{watch, RwLock};

use crate::{Bullet, GameState, Idx, Input, Player, Tank, Time};

use tokio_tungstenite::tungstenite;

use rmp_serde;

#[cfg(feature = "pathfinder_backend")]
mod pathfinder;
#[cfg(feature = "pathfinder_backend")]
pub use pathfinder::PathfinderEventLoop;
#[cfg(feature = "pixels_backend")]
mod pixels;
#[cfg(feature = "pixels_backend")]
pub use self::pixels::PixelsEventLoop;
#[cfg(feature = "minifb_backend")]
mod minifb;
#[cfg(feature = "minifb_backend")]
pub use self::minifb::MinifbEventLoop;
#[cfg(feature = "raqote_backend")]
mod raqote;
#[cfg(feature = "raqote_backend")]
pub use self::raqote::RaqoteRenderer;
#[cfg(feature = "druid_backend")]
mod druid;
#[cfg(feature = "druid_backend")]
pub use self::druid::DruidEventLoop;

pub fn run_client<EL: EventLoop>(host: Option<&str>) {
    let addr = host
        .and_then(|x| (x, 8999).to_socket_addrs().ok().and_then(|mut x| x.next()))
        .unwrap_or(([127, 0, 0, 1], 8999).into());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (input_send, input_recv) = watch::channel(Input::default());
    let (client_loop, recv_state) = rt.block_on(client_loop(addr, input_recv));
    let event_loop = EL::create();
    rt.spawn(client_loop);
    //rt.spawn_blocking(|| render_loop(make_renderer(), recv_state));
    event_loop.run_loop(rt, input_send, recv_state);
}

pub trait EventLoop {
    //    type Renderer: Renderer;
    //    type MakeRenderer: FnOnce() -> Self::Renderer + Send + 'static;
    fn run_loop(
        self,
        rt: tokio::runtime::Runtime,
        send_input: watch::Sender<Input>,
        recv_state: watch::Receiver<GameState>,
    );
    fn create() -> Self
    where
        Self: Sized;
    //    fn create() -> (Self, Self::MakeRenderer)
    //    where
    //        Self: Sized;
}

pub trait Renderer {
    fn draw_tank(&mut self, tank: &Tank);
    fn draw_bullet(&mut self, bullet: &Bullet);
    fn present_frame(&mut self);
}

pub struct NoopRenderer;

impl EventLoop for NoopRenderer {
    //type Renderer = Self;
    fn run_loop(
        self,
        rt: tokio::runtime::Runtime,
        send_input: watch::Sender<Input>,
        recv_state: watch::Receiver<GameState>,
    ) {
        loop {}
    }
    //    type MakeRenderer = impl FnOnce() -> Self::Renderer;
    //    fn create() -> (Self, Self::MakeRenderer)
    //    where
    //        Self: Sized,
    //    {
    //        (Self, || Self)
    //    }
    fn create() -> Self
    where
        Self: Sized,
    {
        Self
    }
}

impl Renderer for NoopRenderer {
    fn draw_tank(&mut self, _tank: &Tank) {}
    fn draw_bullet(&mut self, _bullet: &Bullet) {}
    fn present_frame(&mut self) {}
}

pub fn render_loop<R: Renderer>(mut renderer: R, mut recv_state: watch::Receiver<GameState>) {
    let mut state = recv_state.borrow().clone();
    loop {
        if let Some(res) = recv_state.changed().now_or_never() {
            if let Err(_) = res {
                break;
            }
            state = recv_state.borrow().clone();
        }

        draw_state(&state, &mut renderer);
        renderer.present_frame();
    }
    println!("Render loop ended");
}

/// Renders a frame if there is a new gamestate
pub fn render_frame<R: Renderer>(
    renderer: &mut R,
    recv_state: &mut watch::Receiver<GameState>,
) -> Result<(), ()> {
    if let Some(res) = recv_state.changed().now_or_never() {
        if let Err(_) = res {
            return Err(());
        }
    }
    let state = recv_state.borrow().clone();
    // Get current state
    draw_state(&state, renderer);
    renderer.present_frame();
    Ok(())
}

//struct Client {
//    server_state: GameState,
//    client_state: GameState,
//    input_history: VecDeque<(Time, Input)>,
//}

async fn client_loop(
    addr: SocketAddr,
    mut input_ui_recv: watch::Receiver<Input>,
) -> (impl Future<Output = ()> + Send, watch::Receiver<GameState>) {
    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{}/stream", addr))
        .await
        .unwrap();
    let (mut sink, mut stream) = socket.split();
    let player_id = parse_id(stream.next().await.unwrap().unwrap()).unwrap();
    let init_game_state = parse_state(stream.next().await.unwrap().unwrap()).unwrap();
    let (send_state, recv_state) = watch::channel(init_game_state);
    let (input_send, input_recv) = watch::channel(Default::default());
    (
        async move {
            let (history_write, history_read) = BiLock::new(VecDeque::<Input>::new());
            let input_loop = async {
                // need async type ascription to remove this
                if false {
                    return Ok::<(), ()>(());
                }
                let mut input_seq = 0;
                loop {
                    let sleep = tokio::time::sleep(Duration::from_secs(1) / 60);
                    input_ui_recv.changed().await;
                    let mut input = input_ui_recv.borrow().clone();
                    input.seq = input_seq;
                    input_seq += 1;
                    input_send.send(input).map_err(|_|())?;
                    input_history.write().await.push_back(input);

                    // limit speed
                    sleep.await;
                }
            };
            let input_send = async {
                // need async type ascription to remove this
                if false {
                    return Ok::<(), ()>(());
                }
                loop {
                    input_recv.changed().await;
                    let mut input = input_recv.borrow().clone();
                    sink.send(tungstenite::Message::Binary(
                        rmp_serde::to_vec(&input).map_err(|_| ())?,
                    ))
                    .await
                    .map_err(|_| ())?;
                }
            };
            let recv_loop = async {
                // need async block type ascription to remove this
                if false {
                    return Ok::<(), _>(());
                }
                loop {
                    let mut msg = stream.next().await.unwrap().map_err(|_| ())?;
                    // Attempt to drain any states that may be buffered
                    while let Some(next_msg) = stream.next().now_or_never() {
                        msg = next_msg.unwrap().unwrap()
                    }
                    let state = parse_state(msg).unwrap();
                }
            };
            let predict_loop = async {
                    let game_seq = match state.players[player_id].as_ref() {
                        None => continue,
                        Some(p) => p.input.seq,
                    };
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
                    println!("Predicted Frames: {:?}", input_history.len());
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
        if let Some(tank) = tank {
            r.draw_tank(tank)
        }
    }
    for (_i, bullet) in &state.bullets {
        r.draw_bullet(bullet)
    }
}
