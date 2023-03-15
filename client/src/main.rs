use json;
use serialport::SerialPort;
use std::net::TcpStream;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;
use tungstenite::connect;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::WebSocket;
use url::Url;

const BAUD_RATE: u32 = 115200;
const PORT: &'static str = "/dev/ttyACM0";
const ADDRESS: &'static str = "ws://localhost:24050/ws";

#[derive(PartialEq, Debug)]
enum GameState {
    Unknown,
    Gameplay { k1: bool, k2: bool, hp: f32 },
}

#[derive(Debug)]
enum Message {
    OsuMode(u8),
    OsuKeyState(bool, bool),
    OsuHp(u8),
    OsuHit(i8),
    OsuEnd,
}

fn get_game_state(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> GameState {
    let msg = socket.read_message().expect("Error reading message");
    let msg = msg.into_text().unwrap();
    let parsed = json::parse(&msg).unwrap();
    let state = parsed["menu"]["state"].as_u8().unwrap();
    match state {
        2 => GameState::Gameplay {
            k1: parsed["gameplay"]["keyOverlay"]["k1"]["isPressed"]
                .as_bool()
                .unwrap(),
            k2: parsed["gameplay"]["keyOverlay"]["k2"]["isPressed"]
                .as_bool()
                .unwrap(),
            hp: parsed["gameplay"]["hp"]["smooth"].as_f32().unwrap(),
        },
        _ => GameState::Unknown,
    }
}

fn dispatch(port: &mut Box<dyn SerialPort>, message: Message) {
    match message {
        Message::OsuMode(mode) => port.write(&[0, mode]),
        Message::OsuKeyState(k1, k2) => port.write(&[1, k1 as u8, k2 as u8]),
        Message::OsuHp(hp) => port.write(&[2, hp]),
        Message::OsuHit(offset) => port.write(&[3, offset as u8]),
        Message::OsuEnd => port.write(&[4]),
    }
    .unwrap();
}

fn main() {
    // Websocket thread
    let (gamestate_tx, gamestate_rx) = channel();
    thread::spawn(move || loop {
        let connection = connect(Url::parse(ADDRESS).unwrap());
        match connection {
            Ok((mut socket, _)) => {
                println!("Websocket connected.");
                loop {
                    let game_state = get_game_state(&mut socket);
                    gamestate_tx.send(game_state).unwrap();
                }
            }
            Err(err) => {
                println!("Error connecting to websocket ({})", err);
                println!("Retrying in 5 seconds...");
                thread::sleep(Duration::from_secs(5));
            }
        }
    });

    // Serial thread
    let (serial_tx, serial_rx) = channel::<Message>();
    thread::spawn(move || loop {
        let port = serialport::new(PORT, BAUD_RATE)
            .timeout(Duration::from_millis(50))
            .open();

        match port {
            Ok(mut port) => {
                println!("Serial connected.");
                while let Ok(msg) = serial_rx.recv() {
                    println!("Serial thread recieved {:?}", msg);
                    dispatch(&mut port, msg);
                }
            }
            Err(err) => {
                println!("Error opening port: {}", err);
                println!("Retrying in 5 seconds...");
                thread::sleep(Duration::from_secs(5));
            }
        }
    });

    // Main loop
    let mut last_state = GameState::Unknown;
    while let Ok(state) = gamestate_rx.recv() {
        match (&last_state, &state) {
            // Constanty poll for game state changes
            (
                GameState::Gameplay {
                    k1: last_k1,
                    k2: last_k2,
                    hp: _,
                },
                GameState::Gameplay { k1, k2, .. },
            ) => {
                if (k1, k2) != (last_k1, last_k2) {
                    serial_tx.send(Message::OsuKeyState(*k1, *k2)).unwrap();
                }
            }
            (_, GameState::Gameplay { .. }) => {
                serial_tx.send(Message::OsuMode(2)).unwrap();
            }

            // Only notify state change
            (GameState::Unknown, GameState::Unknown) => {}
            (_, GameState::Unknown) => {
                serial_tx.send(Message::OsuEnd).unwrap();
            }
        }
        last_state = state;
    }
}
