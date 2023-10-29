use anyhow::{anyhow, Context, Result};

use ehttp::headers;
use log::info;
use magic_crypt::{new_magic_crypt, MagicCryptTrait};
use serde_json::json;
use std::{collections::BTreeMap, fs::write, path::PathBuf};

use crate::{
    app::{Channels, Message, Notes, UserData},
    Note,
};

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
    pub fn save_userdata(
        &mut self,
        userdata: &UserData,
        credentials: &(String, String),
        channels: &Channels,
        manual_save: bool,
    ) -> Result<()> {
        let id_sender = channels.id_channel.0.clone();
        let msg_sender = channels.msg_channel.0.clone();
        match self {
            StorageMode::Local { path } => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Ok(enc) = encrypt_userdata(&userdata, credentials) {
                    _ = write(path, enc);
                    if manual_save {
                        _ = msg_sender.send(Message::Info("Saved notes!".into()));
                    }
                }
            }
            StorageMode::JsonBin { masterkey, bin_id } => {
                // rewrite notes so we can encrypt them
                let notes = json!({
                    "encrypted": encrypt_userdata(&userdata, credentials)?
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
                                info!("Saved");
                                if manual_save {
                                    _ = msg_sender.send(Message::Info("Saved notes!".into()));
                                }
                            }
                            Err(e) => {
                                _ = msg_sender.send(Message::err(&e.to_string()));
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
                            Ok(_id) => {
                                if manual_save {
                                    _ = msg_sender.send(Message::Info("Saved notes!".into()));
                                }
                            }
                            Err(e) => {
                                _ = msg_sender.send(Message::err(&e.to_string()));
                            }
                        },
                    );
                }
            }
        }
        Ok(())
    }

    pub fn load_userdata(&self, credentials: &(String, String), channels: &Channels) -> Result<()> {
        let userdata_sender = channels.userdata_channel.0.clone();
        let msg_sender = channels.msg_channel.0.clone();
        match self {
            // Disk mode
            StorageMode::Local { path } => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let decrypted_userdata = std::fs::read_to_string(path)?;
                    let userdata = decrypt_notes(&decrypted_userdata, credentials)?;
                    _ = msg_sender.send(Message::Info(format!(
                        "Loaded {} notes",
                        userdata.notes.len()
                    )));
                    _ = userdata_sender.send(userdata);
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
                    match notes_from_response(result, &credentials) {
                        Ok(userdata) => {
                            // let n = decrypt_notes(&String::from_utf8_lossy(&resp.bytes), &credentials).unwrap();
                            _ = msg_sender
                                .send(Message::Info(format!("Loaded {} notes", userdata.notes.len())));
                            _ = userdata_sender.send(userdata);
                        }
                        Err(e) => {
                            _ = msg_sender.send(Message::err(&e.to_string()));
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

pub fn decrypt_notes(raw_notes: &str, credentials: &(String, String)) -> Result<UserData> {
    // encrypt using key
    let mc = new_magic_crypt!(&credentials.1, 256);
    let d = mc.decrypt_base64_to_string(raw_notes)?;
    dbg!("decrypted with ", credentials);
    Ok(serde_json::from_str(&d)?)
}

pub fn encrypt_userdata(userdata: &UserData, credentials: &(String, String)) -> Result<String> {
    // encrypt using key
    let mc = new_magic_crypt!(&credentials.1, 256);
    Ok(mc.encrypt_str_to_base64(serde_json::to_string(userdata)?))
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

fn notes_from_response(
    result: ehttp::Result<ehttp::Response>,
    credentials: &(String, String),
) -> Result<UserData> {
    let resp = result.map_err(|e| anyhow!(e))?;
    // println!("res {}", res.status_text);

    let n: serde_json::Value = serde_json::from_slice(&resp.bytes)?;
    let decrypted_notes = decrypt_notes(
        n.as_object()
            .context("notes must be obj")?
            .get("encrypted")
            .context("There must be an 'encrypted' key")?
            .as_str()
            .context("The value must be string")?,
        credentials,
    )?;

    Ok(decrypted_notes)
}
