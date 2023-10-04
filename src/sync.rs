use anyhow::{bail, Context, Result};

use ehttp::headers;
use magic_crypt::{new_magic_crypt, MagicCryptTrait};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::write,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
};

use crate::{app::Notes, Note};

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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
        note_sender: Sender<Notes>,
        id_sender: Sender<String>,
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
                // rewrite notes so we can encrypt them
                let notes = json!({
                    "encrypted": encrypt_notes(notes, credentials)?
                });

                let url = "https://api.jsonbin.io/v3/b";

                // no bin configured, we need to ask for one
                if bin_id.is_none() {
                    let request = ehttp::Request {
                        method: "POST".into(),
                        url: url.into(),
                        body: notes.to_string().into_bytes(),
                        headers: headers(&[
                            ("Accept", "*/*"),
                            ("Content-Type", "application/json; charset=utf-8"),
                            ("X-Master-Key", masterkey),
                        ]),
                    };
                    ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
                        match id_from_response(result) {
                            Ok(id) => {
                                _ = id_sender.send(id);
                            }
                            Err(e) => {
                                println!("{e}")
                            }
                        }
                    });
                } else {
                    // safe, since we checked if the Option is Some
                    let bin_id = bin_id.clone().unwrap_or_default();
                    // rewrite bin url with bin id
                    let bin_url = format!("{url}/{bin_id}");

                    let request = ehttp::Request {
                        method: "PUT".into(),
                        url: bin_url,
                        body: notes.to_string().into_bytes(),
                        headers: headers(&[
                            ("Accept", "*/*"),
                            ("Content-Type", "application/json; charset=utf-8"),
                            ("X-Master-Key", masterkey),
                        ]),
                    };
                    ehttp::fetch(
                        request,
                        move |result: ehttp::Result<ehttp::Response>| match result {
                            Ok(_id) => {}
                            Err(e) => {
                                println!("{e}")
                            }
                        },
                    );
                }
            }
        }
        Ok(())
    }

    pub fn load_notes(&self, credentials: &(String, String), sender: Sender<Notes>) -> Result<()> {
        match self {
            // Disk mode
            StorageMode::Local { path } => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let encrypted_notes = std::fs::read_to_string(path)?;
                    let notes = decrypt_notes(&encrypted_notes, credentials)?;
                    _ = sender.send(notes);
                    Ok(())
                }
                #[cfg(target_arch = "wasm32")]
                {
                    // wasm should err here
                    bail!("Could not load notes")
                }
            }
            // JsonBin
            StorageMode::JsonBin { masterkey, bin_id } => {
                let url = "https://api.jsonbin.io/v3/b";
                let bin_id = bin_id.clone().context("Bin ID is needed for loading!")?;
                // rewrite bin url with bin id
                let bin_url = format!("{url}/{bin_id}?meta=false");

                let request = ehttp::Request {
                    method: "GET".into(),
                    url: bin_url,
                    body: vec![],
                    headers: headers(&[
                        ("Accept", "*/*"),
                        ("Content-Type", "application/json; charset=utf-8"),
                        ("X-Master-Key", masterkey),
                    ]),
                };
                // closure takes ownership, clone to move
                let credentials = credentials.clone();
                ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
                    match result {
                        Ok(resp) => {
                            let n: serde_json::Value = serde_json::from_slice(&resp.bytes).unwrap();
                            let decrypted_notes = decrypt_notes(
                                n.as_object()
                                    .context("notes must be obj")
                                    .unwrap()
                                    .get("encrypted")
                                    .context("There must be an 'encrypted' key")
                                    .unwrap()
                                    .as_str()
                                    .context("The value must be string")
                                    .unwrap(),
                                &credentials,
                            )
                            .unwrap();

                            // let n = decrypt_notes(&String::from_utf8_lossy(&resp.bytes), &credentials).unwrap();
                            _ = sender.send(decrypted_notes);
                        }
                        Err(e) => {
                            println!("{e}")
                        }
                    }
                });

                // let client = reqwest::blocking::Client::new();
                // let res = client
                //     .get(bin_url)
                //     .header("X-Master-Key", masterkey.clone())
                //     .send()?;
                // if !res.status().is_success() {
                //     bail!("Error {:?}", res.text())
                // }
                // let n: serde_json::Value = res.json()?;
                // let decrypted_notes = decrypt_notes(
                //     n.as_object()
                //         .context("notes must be obj")?
                //         .get("encrypted")
                //         .context("There must be an 'encrypted' key")?
                //         .as_str()
                //         .context("The value must be string")?,
                //     credentials,
                // )?;

                // let n: BTreeMap<u128, Note> = serde_json::from_value(n)?;
                Ok(())
            }
        }
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
    // encrypt using key
    let mc = new_magic_crypt!(&credentials.1, 256);
    let d = mc.decrypt_base64_to_string(raw_notes)?;
    dbg!("decrypted with ", credentials);
    Ok(serde_json::from_str(&d)?)
}

pub fn encrypt_notes(
    notes: &BTreeMap<u128, Note>,
    credentials: &(String, String),
) -> Result<String> {
    // encrypt using key
    let mc = new_magic_crypt!(&credentials.1, 256);
    Ok(mc.encrypt_str_to_base64(serde_json::to_string(notes)?))
}

fn id_from_response(result: ehttp::Result<ehttp::Response>) -> Result<String> {
    let res = result.unwrap();
    println!("res {}", res.status_text);

    let val: serde_json::Value = serde_json::from_slice(res.bytes.as_slice())?;

    // We only need the ID of the bin...
    // let val: serde_json::Value = res.json()?;
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
    Ok(id.to_string())
}
