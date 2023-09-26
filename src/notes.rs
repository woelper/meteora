use egui::Color32;
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;
use std::collections::BTreeSet;

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Clone, Default, Debug)]
pub enum Deadline {
    #[default]
    Eternal,
    Periodic {
        start: chrono::NaiveDate,
        days: u16,
    },
    Fixed(chrono::NaiveDate),
}

#[derive(serde::Deserialize, serde::Serialize, Default, PartialEq, Clone)]
#[serde(default)]
pub struct Note {
    pub text: String,
    pub tags: BTreeSet<String>,
    pub id: u128,
    pub depends: BTreeSet<u128>,
    pub color: [u8; 3],
    pub progress: f32,
    pub priority: f32,
    pub deadline: Deadline,
    pub complete: bool,
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
        self.text.split('\n').next().unwrap_or("Default")
    }
    pub fn get_clean_text(&self) -> String {
        let mut t = self
            .text
            .split(' ')
            .filter(|w| !w.contains("http"))
            .collect::<Vec<_>>()
            .join(" ");
        t.push('\n');
        t
    }
    pub fn get_color(&self) -> Color32 {
        if self.tags.is_empty() {
            Color32::from_rgb(self.color[0], self.color[1], self.color[2])
            .gamma_multiply(0.5)
        } else {
            let s = self.tags.clone().into_iter().collect::<Vec<String>>().join("");
            color_from_tag(&s)
            .gamma_multiply(0.5)
        }
    }
    pub fn get_links(&self) -> Vec<&str> {
        self.text
            .split(&[' ', '\n'])
            .filter(|t| t.contains("http"))
            .collect()
    }

    /// Calculate the approximate note height in px based on line height and chars per line
    pub fn get_approx_height(&self, line_height: f32) -> f32 {
        let chars_per_row = 15;
        let newlines = self.get_clean_text().lines().count();
        let breaks: usize = self
            .get_clean_text()
            .lines()
            .map(|l| l.chars().count() / chars_per_row)
            .sum();
        (newlines + breaks) as f32 * line_height
    }

    pub fn contains_markdown(&self) -> bool {
        self.text.contains("# ")
            || self.text.contains("* ")
            || self.text.contains("- ")
            || self.text.contains("1. ")
            || self.text.contains('[')
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
    // .linear_multiply(0.1)
}

pub fn link_text(raw_link: &str) -> &str {
    raw_link
        .split("//")
        .nth(1)
        .unwrap_or_default()
        .split('/')
        .next()
        .unwrap_or_default()
}

// determine the color's brigheness level and then invert it
pub fn readable_text(color: &Color32) -> Color32 {
    let brightness = color.r() as f32 * 0.299 + color.g() as f32 * 0.587 + color.b() as f32 * 0.114;
    if brightness > 60.0 {
        Color32::from_rgb(0, 0, 0)
    } else {
        Color32::from_rgb(255, 255, 255)
    }
}
