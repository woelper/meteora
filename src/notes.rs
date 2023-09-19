use std::collections::BTreeSet;
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;

#[derive(serde::Deserialize, serde::Serialize, Default, PartialEq, Clone)]
#[serde(default)]
pub struct Note {
    pub text: String,
    pub tags: Vec<String>,
    pub id: u128,
    pub depends: BTreeSet<u128>,
    pub color: [u8;3],
    pub progress: f32,
    pub priority: f32,
}

impl Note {
    pub fn new() -> Self {
        let mut rng = ChaCha20Rng::from_entropy();
        let mut n = Self::default();
        n.id = std::time::UNIX_EPOCH
            .elapsed()
            .map(|t| t.as_micros())
            .unwrap_or_default();
        n.text = "Empty".to_string();
        n.color = [rng.gen_range(0..255),rng.gen_range(0..255),rng.gen_range(0..255)];
        n
    }

    pub fn get_title(&self) -> &str {
        &self.text.split("\n").nth(0).unwrap_or("Default")
    }
}
