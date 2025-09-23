use std::{cmp::min, collections::HashMap, time::SystemTime};

use serde::{Deserialize, Serialize};

use crate::network::gameserver::ClientConnection;

const TOPK_SCORE : usize = 100;


struct HighScoreFile {
    scores: Vec<HighScoreEntry>
}

impl HighScoreFile {
    pub fn new() -> Self {
        HighScoreFile { scores: vec![] }
    }
}

impl Serialize for HighScoreFile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        let mut bytes = Vec::<u8>::new();
        bytes.extend_from_slice(&(self.scores.len() as u16).to_be_bytes());
        for score in self.scores.iter() {
            bytes.extend_from_slice(&(score.client_id.len() as u16).to_be_bytes());
            bytes.extend_from_slice(&score.client_id.as_bytes());
            bytes.extend_from_slice(&score.timestamp.to_be_bytes());
            bytes.extend_from_slice(&(score.username.len() as u16).to_be_bytes());
            bytes.extend_from_slice(&score.username.as_bytes());
            bytes.extend_from_slice(&score.score.to_be_bytes());
        }
        
        serializer.serialize_bytes(&bytes)
    }
}

impl<'de> Deserialize<'de> for HighScoreFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {
        
        struct HighScoreVisitor;

        impl<'de> serde::de::Visitor<'de> for HighScoreVisitor {
            type Value = HighScoreFile;
            
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("binary highscore data")
            }
            
            fn visit_bytes<E>(self, bytes: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let mut cursor = 0;
                
                let count = u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]) as usize;
                cursor += 2;
                
                let mut scores = Vec::new();
                
                for _ in 0..count {
                    // client_id
                    let client_len = u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]) as usize;
                    cursor += 2;
                    let client_id = String::from_utf8_lossy(&bytes[cursor..cursor + client_len]).to_string();
                    cursor += client_len;
                    
                    // timestamp
                    let timestamp = u32::from_be_bytes([
                        bytes[cursor], bytes[cursor + 1], bytes[cursor + 2], bytes[cursor + 3]
                    ]);
                    cursor += 4;
                    
                    // username
                    let username_len = u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]) as usize;
                    cursor += 2;
                    let username = String::from_utf8_lossy(&bytes[cursor..cursor + username_len]).to_string();
                    cursor += username_len;
                    
                    // score
                    let score = u32::from_be_bytes([
                        bytes[cursor], bytes[cursor + 1], bytes[cursor + 2], bytes[cursor + 3]
                    ]);
                    cursor += 4;
                    
                    scores.push(HighScoreEntry {
                        client_id,
                        timestamp,
                        username,
                        score,
                    });
                }
                
                Ok(HighScoreFile { scores })
            }
        }
        
        deserializer.deserialize_bytes(HighScoreVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighScoreEntry {
    #[serde(skip_serializing)]
    pub client_id: String,

    pub timestamp: u32,
    pub username: String,
    pub score: u32,
}

pub fn store_highscore(
    client: &ClientConnection,
    score: u32,
) {
    // 1 - read file that saves highscore
    // 2 - check if new highscore is under top 100
    // 3 - if new highscore is not top100, just ignore, else, write new score leaderboard
    let file_content = std::fs::read("./highscores").unwrap_or([].to_vec());
    let mut highscore_file = bincode::deserialize::<HighScoreFile>(file_content.as_slice())
        .unwrap_or(HighScoreFile::new());
    
    let new_ts = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    highscore_file.scores.push(
        HighScoreEntry {
            client_id: client.id.clone(),
            timestamp: new_ts,
            username: client.username.as_deref().unwrap_or("").clone().to_string(),
            score: score,
        },
    );
    highscore_file.scores.sort_by(
        |a,b| b.score.cmp(&a.score)
    );
    if highscore_file.scores.len() > TOPK_SCORE {
        let last = highscore_file.scores.last().unwrap();
        if last.timestamp == new_ts && last.client_id == client.id {
            // ignore, it is not a new topk score
            return;
        } else {
            highscore_file.scores.pop();
        }
    }
    
    let new_vec = bincode::serialize::<HighScoreFile>(&highscore_file).unwrap();
    std::fs::write("./highscores", new_vec);    

}

pub fn retrieve_top_highscore() -> Vec<HighScoreEntry> {
    let file_content = std::fs::read("./highscores").unwrap_or([].to_vec());
    let highscore_file = bincode::deserialize::<HighScoreFile>(file_content.as_slice())
        .unwrap_or(HighScoreFile::new());
    return highscore_file.scores;
}