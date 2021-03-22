use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use std::net::{SocketAddr};

use futures::{SinkExt, StreamExt, try_join};

use parking_lot::Mutex;

use tokio::sync::{oneshot, watch};

use warp::ws::{self, WebSocket};
use warp::Filter;

use crate::{GameState, Idx, Input, Player};

struct SerializedGameState {
        bytes: Vec<u8>
}

impl SerializedGameState {
    fn into_message(&self) -> ws::Message {
        ws::Message::binary(self.bytes.clone())
    }
}

struct Server {
    last_state: GameState,
}

impl Server {
    fn new() -> Self {
        let state = GameState::new();
        Self { last_state: state }
    }
    fn tick<I: Iterator<Item = (Idx<'static, Player>, Input)>>(&mut self, inputs: I) {
        // take player inputs
        for (player, input) in inputs {
            self.last_state.players[player].as_mut().unwrap().input = input;
        }

        // tick gamestate
        let state = self.last_state.tick();
        self.last_state = state;
    }
}

fn serialize(state: &GameState) -> SerializedGameState {
    let bytes = rmp_serde::to_vec(state).unwrap();
    SerializedGameState {
        bytes
    }
}

#[derive(Default)]
pub struct PlayerInput {
    new_connections: Vec<oneshot::Sender<Idx<'static, Player>>>,
    disconnections: Vec<Idx<'static, Player>>,
    inputs: HashMap<Idx<'static, Player>, Input>,
}

pub fn run_server() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut server = Server::new();
    let (send, recv) = watch::channel(Arc::new(serialize(&server.last_state)));
    let inputs = Arc::new(Mutex::new(PlayerInput::default()));
    let server_input = inputs.clone();
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(16));
    rt.spawn(ws_server(SocketAddr::from(([0, 0, 0, 0], 8998)), server_input.clone(), recv));
    loop {
        // get inputs
        let inputs = mem::take(&mut *server_input.lock());
        server.tick(inputs.inputs.into_iter());
        let ser = Arc::new(serialize(&server.last_state));
        while let Err(_) = send.send(ser.clone()) {}
        // delay to 60 ups
        rt.block_on(interval.tick());
    }
}


async fn ws_server(addr: SocketAddr, server_input: Arc<Mutex<PlayerInput>>, watch: watch::Receiver<Arc<SerializedGameState>>) {
    let routes = warp::path("stream")
        .and(warp::ws())
        .map({
            move |ws: warp::ws::Ws| {
                let server_input = server_input.clone();
                let watch = watch.clone();
                ws.on_upgrade(move |websocket| handle_client(websocket, server_input, watch))
            }
        });
    warp::serve(routes).run(addr).await;
}

async fn handle_client(
    socket: WebSocket,
    global_input: Arc<Mutex<PlayerInput>>,
    mut watch: watch::Receiver<Arc<SerializedGameState>>,
) {
    let (mut sink, mut stream) = socket.split();
    let (send, recv) = oneshot::channel();
    global_input.lock().new_connections.push(send);
    let player_idx = recv.await.unwrap();
    // process player input
    let recv_input = async {
        while let Some(Ok(msg)) = stream.next().await {
            if let Some(input) = parse_input_message(&msg) {
                global_input.lock().inputs.insert(player_idx, input);
            }
        }
        Err::<(),()>(())
    };
    //
    // send gamestate updates
    let send_state = async {
        while let Ok(()) = {
            watch.changed().await.map_err(|_|())?;
            let state = watch.borrow().clone().into_message();
            sink.send(state).await
        } {}
        Err::<(),()>(())
    };
    let _ = try_join!(recv_input, send_state);
    global_input.lock().disconnections.push(player_idx);
}

fn parse_input_message(msg: &ws::Message) -> Option<Input> {
     rmp_serde::from_read_ref(msg.as_bytes()).ok()
}
