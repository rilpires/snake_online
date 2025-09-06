use std::{cmp::min, collections::{HashMap, HashSet}};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighScoreEntry {
    pub username: String,
    pub score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighScores {
    pub highscores: HashMap<String, HighScoreEntry>
}
impl HighScores {
    pub fn from_vec(value: &mut Vec<HighScoreEntry>) -> Self {
        let mut ret = HashMap::new();
        value.sort_by(
            |a, b| {b.score.cmp(&a.score)}
        );
        for i in 0..min(10, value.len()) {
            ret.insert(
                format!("{}", i+1),
                value.get(i).unwrap().clone(),
            );
        }
        HighScores{
            highscores: ret
        }
    }
}