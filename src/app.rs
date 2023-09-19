use std::{collections::BTreeMap, fs::File, path::Path};

use egui_extras::Size;
use egui_grid::GridBuilder;

use crate::Note;
use egui::{
    epaint::{ahash::HashSet, Shadow},
    Color32, FontData, FontFamily, Label, Layout, Rect, Sense, Stroke, Style, Ui, Vec2, TextStyle,
};
use egui_commonmark::*;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct MeteoraApp {
    notes: BTreeMap<u128, Note>,
    tags: Vec<String>,
    active_tags: HashSet<String>,
    active_note: Option<u128>,
}

impl MeteoraApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        let mut fonts = egui::FontDefinitions::default();
        // Install my own font (maybe supporting non-latin characters):

        fonts.font_data.insert(
            "inter".to_owned(),
            FontData::from_static(include_bytes!("fonts/Inter-Regular.ttf")),
        );

        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, "inter".to_owned());

        cc.egui_ctx.set_fonts(fonts);


        // let mut style= cc.egui_ctx.style();
        // style.text_styles.get_mut(&TextStyle::Body).unwrap().size = 100.;


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

        egui::SidePanel::right("edit_panel").show(ctx, |ui| {
            ui.heading("Edit");
            ui.vertical_centered_justified(|ui| {
                if let Some(id) = self.active_note {
                    edit_note(ui, &id, &self.tags, &mut self.notes);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let note_size = 100.;
            let width = ui.available_width();

            let num_columns = (width / note_size).max(1.0) as usize;
            ui.label(format!("Cols {num_columns}, notes {}", self.notes.len()));

            let mut num_notes = 0;
            // let ordered_notes = self.notes.keys().collect::<Vec<_>>();

            // ui.columns(num_columns, |columns| {

            //     for (id, note) in &self.notes.clone() {
            //         if self.active_tags.is_empty()
            //             || note.tags.iter().any(|t| self.active_tags.contains(t))
            //         {
            //             // draw_note(ui, id, &mut self.notes, &mut self.active_note);
            //         }
            //         let column = num_notes % num_columns;
            //             columns[column].
            //         label(format!("Col {column}"));
            //             draw_note(&mut columns[column], id, &mut self.notes, &mut self.active_note);

            //         num_notes += 1;
            //     }

            //     // for i in 0..num_columns {

            //     //     columns[i].label(format!("C {i}"));
            //     //     if num_notes < self.notes.len() {
            //     //         columns[i].group(|ui| {
            //     //             // draw_note(ui, ordered_notes[num_notes], &self.notes, &mut self.active_note)
            //     //         });

            //     //     }

            //     // }
            //     // columns[1].label("Second column");
            // });

            // egui::ScrollArea::horizontal().max_width(128.).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (id, note) in &self.notes.clone() {
                    if self.active_tags.is_empty()
                        || note.tags.iter().any(|t| self.active_tags.contains(t))
                    {
                        draw_note(ui, id, &mut self.notes, &mut self.active_note);
                    }
                }
            });

            if ui.button("New Note").clicked() {
                let n = Note::new();
                self.notes.insert(n.id, n);
            }
        });

        // });
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

        ui.color_edit_button_srgb(&mut note.color);

        ui.label(format!("{}", note.id));
        ui.add(egui::Slider::new(&mut note.priority, 0.0..=1.0).text("Priority"));
        ui.label(format!("prio {}", note.priority));
        ui.label(format!("prog {}", note.progress));

        egui::ComboBox::from_id_source(&note.id)
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

fn draw_note(
    ui: &mut Ui,
    note_id: &u128,
    notes: &BTreeMap<u128, Note>,
    active_note: &mut Option<u128>,
) {
    // make sure id is valid
    if notes.get(note_id).is_none() {
        ui.label("No such ID");
        return;
    }
    let note = notes.get(note_id).unwrap();

    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(100.), Sense::click());

    ui.painter().rect_filled(
        rect,
        2.2,
        Color32::from_rgb(note.color[0], note.color[1], note.color[2]),
    );
    // let mut cache = CommonMarkCache::default();
    // CommonMarkViewer::new("viewer").show(ui, &mut cache, &note.text);

    ui.put(rect, egui::Label::new(&note.text));

    // });
    // let resp = r.response.interact(egui::Sense::click());
    if resp.clicked() {
        *active_note = Some(*note_id);
    }
}
