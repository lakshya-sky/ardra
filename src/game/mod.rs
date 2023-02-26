use std::sync::Arc;

use anyhow::Result;
use tokio::io::{self, AsyncBufReadExt};
use tokio::sync::{mpsc, Mutex};

mod game_event;

pub struct Game {
    receiver: mpsc::UnboundedReceiver<Vec<u8>>,
    sender: mpsc::UnboundedSender<Vec<u8>>,
    cancel: tokio::sync::oneshot::Receiver<()>,
    event_queue: Arc<Mutex<Vec<game_event::GameEvent>>>,
}

impl Game {
    pub fn new(
        receiver: mpsc::UnboundedReceiver<Vec<u8>>,
        sender: mpsc::UnboundedSender<Vec<u8>>,
        cancel: tokio::sync::oneshot::Receiver<()>,
    ) -> Self {
        let event_queue = Arc::new(Mutex::new(Vec::new()));
        Game {
            receiver,
            sender,
            cancel,
            event_queue,
        }
    }

    fn send_event(&mut self, event: game_event::GameEvent) -> Result<()> {
        let bytes = game_event::game_event_to_bytes(&event)?;
        self.sender.send(bytes)?;
        Ok(())
    }

    async fn run(mut self) -> Result<()> {
        let mut stdin = io::BufReader::new(io::stdin()).lines();
        loop {
            tokio::select! {
                line = stdin.next_line() => {
                    let line = line?.unwrap();
                    if line == "start" {
                        self.send_event(game_event::GameEvent::Start)?;
                    }
                },
                Some(bytes) = self.receiver.recv() => {
                    let event = game_event::bytes_to_game_event(bytes)?;
                    handle_game_event(&mut self, event);
                },

                cancel = &mut self.cancel => {
                    if cancel.is_ok() {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

pub async fn run(game: Game) -> Result<()> {
    dbg!("Game running");
    game.run().await
}

fn handle_game_event(game: &mut Game, event: game_event::GameEvent) {
    match event {
        game_event::GameEvent::Start => {
            dbg!("Game started");
            game.send_event(game_event::GameEvent::Test("Hello".to_string())).unwrap();
        }
        game_event::GameEvent::Test(s) => {
            dbg!("Test event received: {}", s);
        }
    }
}
