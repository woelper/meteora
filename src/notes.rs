#[derive(serde::Deserialize, serde::Serialize, Default, PartialEq, Eq, Hash)]

pub struct Note {
    pub text: String,
    pub tags: Vec<String>
}