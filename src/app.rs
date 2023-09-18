use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::format,
    fs::File,
    path::Path,
};

use crate::Note;
use egui::{epaint::ahash::HashSet, Ui};
use egui_commonmark::*;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct MeteoraApp {
    notes: BTreeMap<u128, Note>,
    tags: Vec<String>,
    active_tags: HashSet<String>,
    active_note: Option<u128>
}

impl MeteoraApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl eframe::App for MeteoraApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
        let w = File::create("backup.json").unwrap();
        _ = serde_json::to_writer_pretty(w, &self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // let Self { label, value } = self;

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                    if ui.button("Restore").clicked() {
                        let p = Path::new("backup.json");
                        if p.exists() {
                            self.notes = serde_json::from_reader(File::open(p).unwrap()).unwrap();
                        }
                    }
                });
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Tags");

            ui.vertical_centered_justified(|ui| {
                if ui.button("All").clicked() {
                    self.active_tags.clear();
                }
                for tag in &self.tags {
                    let contained = self.active_tags.contains(tag);
                    if ui.selectable_label(contained, tag).clicked() {
                        if contained {
                            self.active_tags.remove(tag);
                        } else {
                            self.active_tags.insert(tag.clone());
                        }
                    }
                    // if ui.button(tag).clicked() {
                    //     self.active_tag.insert(tag.clone());
                    // }
                }

                ui.collapsing("Edit", |ui| {
                    if ui.button("Add tag").clicked() {
                        self.tags.push("New Tag".into());
                    }
                    for tag in &mut self.tags {
                        ui.text_edit_singleline(tag);
                    }
                });
            });
        });

        egui::SidePanel::left("edit_panel").show(ctx, |ui| {
            ui.heading("Edit");

            ui.vertical_centered_justified(|ui| {
               if let Some(id) = self.active_note {
                    draw_note(ui, &id, &mut self.notes);
               }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("New Note").clicked() {
                let n = Note::new();
                self.notes.insert(n.id, n);
            }

            // let all_notes = self.notes.clone();
            // for (id, note) in &self.notes.clone() {
            //     if self.active_tags.is_empty()
            //         || note.tags.iter().any(|t| self.active_tags.contains(t))
            //     {
            //         edit_note(ui, id, &self.tags, &mut self.notes);
            //     }
            // }

            for (id, note) in &self.notes.clone() {
                if self.active_tags.is_empty()
                    || note.tags.iter().any(|t| self.active_tags.contains(t))
                {
                    draw_note(ui, id, &mut self.notes);
                }
            }


        });
    }
}

fn edit_note(ui: &mut Ui, note_id: &u128, tags: &Vec<String>, notes: &mut BTreeMap<u128, Note>) {
    // make sure id is valid
    if notes.get(note_id).is_none() {
        ui.label("No such ID");
        return;
    }
    let immutable_notes = notes.clone();

    ui.group(|ui| {
        let note = notes.get_mut(note_id).unwrap();
        ui.text_edit_multiline(&mut note.text);

        egui::ComboBox::from_id_source(&note)
            .selected_text(format!("Select tag"))
            .show_ui(ui, |ui| {
                for tag in tags {
                    let contains = note.tags.contains(tag);
                    if ui.selectable_label(contains, tag).clicked() {
                        if contains {
                            let index = note.tags.iter().position(|x| x == tag).unwrap();
                            note.tags.remove(index);
                        } else {
                            note.tags.push(tag.clone())
                        }
                    }
                }
            });

        egui::ComboBox::from_id_source(format!("{}x", note.id))
            .selected_text(format!("Depends on..."))
            .show_ui(ui, |ui| {
                for (i, n) in immutable_notes.iter() {
                    let contains = note.depends.contains(&i);
                    if ui.selectable_label(contains, n.get_title()).clicked() {
                        if contains {
                            note.depends.remove(&i);
                        } else {
                            note.depends.insert(*i);
                        }
                    }
                }
            });

        // ui.collapsing("RND", |ui| {
        // let mut cache = CommonMarkCache::default();
        //     CommonMarkViewer::new("viewer").show(ui, &mut cache, &note.text);

        // });
    });
}

fn draw_note(ui: &mut Ui, note_id: &u128, notes: &mut BTreeMap<u128, Note>) -> Option<u128> {
    // make sure id is valid
    if notes.get(note_id).is_none() {
        ui.label("No such ID");
        return None;
    }

    let r = ui.group(|ui| {
        let note = notes.get_mut(note_id).unwrap();
        let mut cache = CommonMarkCache::default();
        CommonMarkViewer::new("viewer").show(ui, &mut cache, &note.text);
    });
    if r.response.clicked() {
        ui.label("dwds");
        return Some(*note_id)
    }
    None
}
