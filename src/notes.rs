use chrono::NaiveDate;
use egui::Color32;
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;
use std::collections::BTreeSet;

use crate::app::GAMMA_MULT;

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
    pub created: NaiveDate,
}

impl Note {
    pub fn new() -> Self {
        // let mut rng = ChaCha20Rng::from_entropy();
        let mut n = Self::default();
        let time = chrono::Utc::now().timestamp_micros();
        n.id = time as u128;
        n.text = "".to_string();
        n.created = chrono::Utc::now().date_naive();
        // n.color = [
        //     rng.gen_range(0..255),
        //     rng.gen_range(0..255),
        //     rng.gen_range(0..255),
        // ];
        n.color = [200, 200, 200];
        n
    }

    pub fn get_final_prio(&self) -> f32 {
        match self.deadline {
            Deadline::Eternal => self.priority,
            Deadline::Periodic { start, days } => {
                // this is the alerting range - the hours in a work week. Anything later is not affecting prio.
                // TODO later this should be configurable

                // it's Monday, start was last Friday, days is 3.
                // remaining = 3
                // mod days: 0

                // it's Monday, start is next Friday, days is 4.
                // remaining: -4

                let panic_range = (24 * 5) as f32;
                let delta = start
                    .signed_duration_since(chrono::Utc::now().date_naive())
                    .num_days();

                let mut remaining_hours = 0.0;

                println!("days {delta}");

                for d in start.iter_days().step_by(days.max(1) as usize) {
                    if d > chrono::Utc::now().date_naive() {
                        println!("Next is {}", d);
                        remaining_hours = (d
                            .signed_duration_since(chrono::Utc::now().date_naive())
                            .num_hours() as f32).min(panic_range);
                        break;
                    }
                }

                // let delta = if delta.is_positive() {delta.abs()} else {
                //     if days == 0 {
                //         0
                //     } else {

                //         delta.abs() % days as i64
                //     }
                // } as f32;
                let weight = 1. - (remaining_hours / panic_range);
                // println!("weight {weight}, rem {delta}");

                // 96 / 120
                // println!("remainung minutes: {:?} {weight}", remaining);

                println!("weight {weight}, rem {remaining_hours}");
                println!("now {}", chrono::Utc::now().timestamp() % 80);
                self.priority + weight
            }
            Deadline::Fixed(date) => {
                // this is the alerting range - the hours in a work week. Anything later is not affecting prio.
                // TODO later this should be configurable
                let panic_range = (24 * 5) as f32;
                let remaining_hours = date
                    .signed_duration_since(chrono::Utc::now().date_naive())
                    .num_hours() as f32;
                // #[cfg(debug_assertions)]
                // let remaining_hours = ((chrono::Utc::now().timestamp_millis() / 200) % panic_range as i64)
                // as f32 ;
                let weight = 1. - (remaining_hours / panic_range);
                println!("weight {weight}, rem {remaining_hours}");
                println!("now {}", chrono::Utc::now().timestamp() % 80);
                // 96 / 120
                // println!("remainung minutes: {:?} {weight}", remaining);
                self.priority + weight
            }
        }
    }

    pub fn get_title(&self) -> &str {
        self.text.lines().next().unwrap_or("Default")
    }

    pub fn get_body(&self) -> String {
        self.text.lines().skip(0).collect::<Vec<_>>().join("\n")
    }

    pub fn get_excerpt(&self) -> String {
        self.get_clean_text()
            .lines()
            .skip(1)
            .collect::<Vec<_>>()
            .join(" ")
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

    pub fn get_clean_text_truncated(&self) -> String {
        let max = 200;
        if self.get_clean_text().chars().count() > max {
            format!(
                "{}...",
                self.get_clean_text().chars().take(max).collect::<String>()
            )
        } else {
            self.get_clean_text()
        }
    }

    pub fn get_color(&self) -> Color32 {
        if self.tags.is_empty() {
            Color32::from_rgb(self.color[0], self.color[1], self.color[2])
                .gamma_multiply(GAMMA_MULT)
        } else {
            let s = self
                .tags
                .clone()
                .into_iter()
                .collect::<Vec<String>>()
                .join("");
            color_from_tag(&s).gamma_multiply(GAMMA_MULT)
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
        let newlines = self.get_clean_text_truncated().lines().count();
        let breaks: usize = self
            .get_clean_text_truncated()
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
    // let g = colorgrad::rainbow();

    let g = colorgrad::CustomGradient::new()
        .html_colors(&["gold", "hotpink", "darkturquoise", "seagreen"])
        .build()
        .unwrap();

    let c = g.at(rng.gen_range(0.0..1.0));

    Color32::from_rgb((c.r * 255.) as u8, (c.g * 255.) as u8, (c.b * 255.) as u8)
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
    if brightness > 120.0 {
        Color32::from_rgb(40, 40, 40)
    } else {
        Color32::from_rgb(230, 230, 230)
    }
    // Color32::from_rgb(255-color.r(), 255-color.g(), 255-color.b())
}
