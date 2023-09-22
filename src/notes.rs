use egui::Color32;
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;
use std::collections::BTreeSet;

#[derive(serde::Deserialize, serde::Serialize, Default, PartialEq, Clone)]
#[serde(default)]
pub struct Note {
    pub text: String,
    pub tags: Vec<String>,
    pub id: u128,
    pub depends: BTreeSet<u128>,
    pub color: [u8; 3],
    pub progress: f32,
    pub priority: f32,
}

impl Note {
    pub fn new() -> Self {
        let mut rng = ChaCha20Rng::from_entropy();
        let mut n = Self::default();
        let time = chrono::Utc::now().timestamp_micros();
        n.id = time as u128;
        n.text = "Empty".to_string();
        n.color = [
            rng.gen_range(0..255),
            rng.gen_range(0..255),
            rng.gen_range(0..255),
        ];
        n
    }

    pub fn get_title(&self) -> &str {
        &self.text.split("\n").nth(0).unwrap_or("Default")
    }
    pub fn get_clean_text(&self) -> String {
        self.text
            .split(" ")
            .filter(|w| !w.contains("http"))
            .collect::<Vec<_>>()
            .join(" ")
    }
    pub fn get_color(&self) -> Color32 {
        if self.tags.is_empty() {
            Color32::from_rgb(self.color[0], self.color[1], self.color[2]).gamma_multiply(0.5)
        } else {
            color_from_tag(&self.tags.join(""))
        }
    }
    pub fn get_links(&self) -> Vec<&str> {
        self.text
            .split(&[' ', '\n'])
            .filter(|t| t.contains("http"))
            .collect()
    }

    pub fn get_approx_height(&self, line_height: f32) -> f32 {
        let newlines = self.get_clean_text().lines().count();
        let breaks = self
            .get_clean_text()
            .lines()
            .map(|l| l.chars().count() > 15)
            .count();
        (newlines + breaks) as f32 * line_height
    }
}

pub fn color_from_tag(tag: &str) -> Color32 {
    let x: i32 = tag.as_bytes().iter().map(|x| *x as i32).sum();
    let mut rng = ChaCha20Rng::seed_from_u64(x as u64);
    Color32::from_rgb(
        rng.gen_range(0..255),
        rng.gen_range(0..255),
        rng.gen_range(0..255),
    )
    .gamma_multiply(0.2)
}

pub fn link_text(raw_link: &str) -> &str {
    raw_link
        .split("//")
        .nth(1)
        .unwrap_or_default()
        .split("/")
        .nth(0)
        .unwrap_or_default()
}
