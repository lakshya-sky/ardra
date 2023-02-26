use anyhow::{Ok, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    Start,
    Test(String),
}

pub fn bytes_to_game_event(data: Vec<u8>) -> Result<GameEvent> {
    let event: GameEvent = bincode::deserialize(&data).unwrap();
    println!("Deserialized event: {:?}", event);
    Ok(event)
}

pub fn game_event_to_bytes(event: &GameEvent) -> Result<Vec<u8>> {
    println!("Serializing event: {:?}", event);
    let event = bincode::serialize(event).unwrap();
    Ok(event)
}

//Receive bytes from Libp2p event handler and deserialize to GameEvent
//
