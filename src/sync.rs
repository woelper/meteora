use anyhow::{bail, Context, Result};
use magic_crypt::{new_magic_crypt, MagicCryptTrait};
use reqwest::{blocking::ClientBuilder, header::CONTENT_TYPE};
use serde_json::json;
use std::{collections::BTreeMap, fs::write, path::PathBuf};

use crate::Note;

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum StorageMode {
    Local {
        path: PathBuf,
    },
    JsonBin {
        masterkey: String,
        bin_id: Option<String>,
    },
}

impl std::fmt::Debug for StorageMode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            StorageMode::Local { .. } => write!(f, "Local"),
            StorageMode::JsonBin { .. } => write!(f, "JsonBin"),
        }
    }
}



impl StorageMode {
    pub fn save_notes(
        &mut self,
        notes: &BTreeMap<u128, Note>,
        credentials: &(String, String),
    ) -> Result<()> {
        match self {
            StorageMode::Local { path } =>
            {
                #[cfg(not(target_arch = "wasm32"))]
                if let Ok(enc) = encrypt_notes(notes, credentials) {
                    _ = write(path, enc);
                }
            }
            StorageMode::JsonBin { masterkey, bin_id } => {
                let url = "https://api.jsonbin.io/v3/b";

                if bin_id.is_none() {
                    let client = reqwest::blocking::Client::new();

                    let res = client
                        .post(url)
                        .header("X-Master-Key", masterkey.clone())
                        .json(notes)
                        .send()?;

                    if !res.status().is_success() {
                        bail!("Error {:?}", res.text())
                    }

                    let val: serde_json::Value = res.json()?;

                    let id = val
                        .as_object()
                        .context("no object")?
                        .get("metadata")
                        .context("can't get meta")?
                        .as_object()
                        .context("no object")?
                        .get("id")
                        .context("can't get id")?
                        .as_str()
                        .context("can't get id string")?;
                    *bin_id = Some(id.into());

                    // dbg!(id);
                } else {
                    // safe, since we checked if the Option is Some
                    let bin_id = bin_id.clone().unwrap_or_default();
                    // rewrite bin url with bin id
                    let bin_url = format!("{url}/{bin_id}");
                    let client = reqwest::blocking::Client::new();

                    let res = client
                        .put(bin_url)
                        .header("X-Master-Key", masterkey.clone())
                        .json(notes)
                        .send()?;

                    if !res.status().is_success() {
                        bail!("Error {:?}", res.text())
                    }
                }
            }
        }
        Ok(())
    }

    pub fn load_notes(&self, credentials: &(String, String)) -> Result<BTreeMap<u128, Note>> {
        match self {
            StorageMode::Local { path } => {
                #[cfg(not(target_arch = "wasm32"))]
                if path.exists() {
                    if let Ok(encrypted_notes) = std::fs::read_to_string(path) {
                        let notes = decrypt_notes(&encrypted_notes, credentials)?;
                            dbg!("Decrypted notes");
                            return Ok(notes);
                        
                    } else {
                        // TODO: send toast
                        println!("Can't load notes from disk");
                    }
                }
            }
            StorageMode::JsonBin { masterkey, bin_id } => {

                let url = "https://api.jsonbin.io/v3/b";


                   
                    let bin_id = bin_id.clone().context("Bin id is needed for loading")?;
                    // rewrite bin url with bin id
                    let bin_url = format!("{url}/{bin_id}?meta=false");
                    let client = reqwest::blocking::Client::new();

                    let res = client
                        .get(bin_url)
                        .header("X-Master-Key", masterkey.clone())
                        .send()?;

                    if !res.status().is_success() {
                        bail!("Error {:?}", res.text())
                    }

                    let n :BTreeMap<u128, Note> = res.json()?;
                    return Ok(n);

            }
        }
        bail!("Could not load notes")
    }
}

impl Default for StorageMode {
    fn default() -> Self {
        StorageMode::Local {
            path: PathBuf::from("backup.json"),
        }
    }
}

pub fn decrypt_notes(
    raw_notes: &str,
    credentials: &(String, String),
) -> Result<BTreeMap<u128, Note>> {
    if credentials.1.is_empty() {
        // no encryption
        Ok(serde_json::from_str(raw_notes)?)
    } else {
        // encrypt using key
        let mc = new_magic_crypt!(&credentials.1, 256);
        let d = mc.decrypt_base64_to_string(raw_notes)?;
        dbg!("decrypted with ", credentials);
        Ok(serde_json::from_str(&d)?)
    }
}

pub fn encrypt_notes(
    notes: &BTreeMap<u128, Note>,
    credentials: &(String, String),
) -> Result<String> {
    if credentials.1.is_empty() {
        // no encryption
        Ok(serde_json::to_string_pretty(notes)?)
    } else {
        // encrypt using key
        let mc = new_magic_crypt!(&credentials.1, 256);
        Ok(mc.encrypt_str_to_base64(serde_json::to_string(notes)?))
    }
}
