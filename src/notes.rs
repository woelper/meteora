use std::collections::BTreeSet;

#[derive(serde::Deserialize, serde::Serialize, Default, PartialEq, Eq, Hash, Clone)]

pub struct Note {
    pub text: String,
    pub tags: Vec<String>,
    pub id: u128,
    pub depends: BTreeSet<u128>,
}

impl Note {
    pub fn new() -> Self {
        let mut n = Self::default();
        n.id = std::time::UNIX_EPOCH
            .elapsed()
            .map(|t| t.as_micros())
            .unwrap_or_default();
        n
    }

    pub fn get_title(&self) -> &str {
        &self.text.split("\n").nth(0).unwrap_or("Default")
    }
}
