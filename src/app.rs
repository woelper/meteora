use std::{collections::BTreeMap, fs::File, path::Path};

use egui_extras::Size;
use egui_grid::GridBuilder;

use crate::{color_from_tag, link_text, Note};
use egui::{
    epaint::{ahash::HashSet, RectShape, Shadow},
    Align2, Color32, FontData, FontFamily, FontId, Label, Layout, Rect, Rounding, ScrollArea,
    SelectableLabel, Sense, Shape, Stroke, Style, TextStyle, Ui, Vec2, RichText, text::LayoutJob,
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
        let mut fonts = egui::FontDefinitions::default();
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
        let mut style = (*cc.egui_ctx.style()).clone();
        style.text_styles = [
            (
                egui::TextStyle::Heading,
                FontId::new(20.0, FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Body,
                FontId::new(14.0, FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Monospace,
                FontId::new(14.0, FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Button,
                FontId::new(14.0, FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Small,
                FontId::new(10.0, FontFamily::Proportional),
            ),
        ]
        .into();

        // style.visuals.faint_bg_color = Color32::BLUE;
        // style.visuals.widgets.noninteractive.bg_fill = Color32::BLUE;
        style.visuals.widgets.inactive.bg_fill = Color32::BLUE;
        style.visuals.panel_fill = Color32::from_gray(255);

        cc.egui_ctx.set_style(style);

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
        #[cfg(not(target_arch = "wasm32"))]
        let w = File::create("backup.json").unwrap();
        _ = serde_json::to_writer_pretty(w, &self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                let all_used_tags = self
                    .notes
                    .iter()
                    .map(|(_, n)| &n.tags)
                    .flatten()
                    .collect::<Vec<_>>();
                for tag in &self.tags {
                    // Hide tags that are unused.
                    if !all_used_tags.contains(&tag) {
                        continue;
                    }
                    let contained = self.active_tags.contains(tag);

                    ui.style_mut().visuals.selection.bg_fill = color_from_tag(tag);

                    if ui.add(SelectableLabel::new(contained, tag)).clicked() {
                        // if ui.selectable_label(contained, tag).clicked() {
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

                ui.vertical(|ui| {
                    ui.add_space(ui.available_height() - 250.);
                });

                ui.collapsing("Edit", |ui| {
                    if ui.button("Add tag").clicked() {
                        self.tags.push("New Tag".into());
                    }

                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        for tag in &mut self.tags {
                            ui.text_edit_singleline(tag);
                        }
                    });
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut v = Vec::from_iter(self.notes.clone());
            v.sort_by(|(_, a), (_, b)| b.priority.total_cmp(&a.priority));

            // egui::ScrollArea::horizontal().max_width(128.).show(ui, |ui| {
                // ui.layout().with_main_wrap(true).

                // ui.with_layout(Layout::top_down(egui::Align::LEFT).with_main_wrap(true), |ui| {

                // ui.allocate_ui_with_layout(Vec2::INFINITY, Layout::left_to_right(egui::Align::Center), |ui| {

            //     });

            ui.allocate_ui(
                Vec2::new(150., ui.available_size_before_wrap().y),
                |ui| ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT).with_main_wrap(true), |ui|{
                    for (id, note) in &v {
                        if self.active_tags.is_empty()
                            || note.tags.iter().any(|t| self.active_tags.contains(t))
                        {
                            // ui.label("dsds");
                            draw_note(ui, id, &mut self.notes, &mut self.active_note);
                        }
                    }
                }),
            );
         
       
         

            // ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT).with_main_wrap(true).with_cross_align(egui::Align::LEFT), |ui| {
            //     ui.label("world!");
            //     ui.label("Hello");
            // });

            // ui.horizontal_wrapped(|ui| {
            //     for (id, note) in &v {
            //         if self.active_tags.is_empty()
            //             || note.tags.iter().any(|t| self.active_tags.contains(t))
            //         {
            //             ui.label("dsds");
            //             // draw_note(ui, /id, &mut self.notes, &mut self.active_note);
            //         }
            //     }
            // });

            if ui.button("New Note").clicked() {
                let n = Note::new();
                self.notes.insert(n.id, n);
            }

            if let Some(id) = self.active_note {
                // Black background
                let rect_all = ui.allocate_rect(Rect::EVERYTHING, Sense::click());
                ui.painter_at(Rect::EVERYTHING).rect_filled(
                    Rect::EVERYTHING,
                    Rounding::none(),
                    Color32::from_rgba_premultiplied(0, 0, 0, 150),
                );

                if rect_all.clicked() {
                    self.active_note = None;
                }
            }
        });

        if let Some(id) = self.active_note {
            egui::Window::new("x")
                .collapsible(false)
                .movable(false)
                .title_bar(false)
                .anchor(Align2::CENTER_CENTER, Vec2::splat(0.))
                .default_height(800.)
                .min_height(800.)
                .min_width(300.)
                .show(ctx, |ui| {
                    edit_note(ui, &id, &self.tags, &mut self.notes);

                    if ui.button("close").clicked() {
                        self.active_note = None;
                    }
                });
        }

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

    let note = notes.get_mut(note_id).unwrap();
    ui.text_edit_multiline(&mut note.text);

    ui.add_space(200.);

    if note.tags.is_empty() {
        ui.color_edit_button_srgb(&mut note.color);
    }

    // ui.label(format!("{}", note.id));
    ui.add(egui::Slider::new(&mut note.priority, 0.0..=1.0).text("Priority"));
    // ui.label(format!("prio {}", note.priority));
    // ui.label(format!("prog {}", note.progress));

    ui.horizontal(|ui| {
        let note = notes.get_mut(note_id).unwrap();

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

        if ui.button("🗑").clicked() {
            notes.remove(note_id);
        }
    });

    // ui.collapsing("RND", |ui| {
    // let mut cache = CommonMarkCache::default();
    //     CommonMarkViewer::new("viewer").show(ui, &mut cache, &note.text);

    // });
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

    let estimated_size = note.get_approx_height(20.);

    let note_size = Vec2::new(150.,estimated_size.max(150.));


    let (rect, resp) = ui.allocate_exact_size(note_size, Sense::click());

    let stroke = if resp.hovered() {
        Stroke::new(2., Color32::GRAY)
    } else {
        Stroke::NONE
    };

    let frame_shape = Shape::Rect(RectShape {
        rect,
        rounding: 2.2.into(),
        fill: note.get_color(),
        stroke,
    });

    let shp = {
        let shadow = Shadow::small_light();
        let shadow = shadow.tessellate(rect, 5.0);
        let shadow = Shape::Mesh(shadow);
        Shape::Vec(vec![shadow, frame_shape])
    };

    ui.painter().add(shp);

    let mut sub_ui = ui.child_ui(
        rect.shrink(10.),
        Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
    );


    sub_ui.label(note.get_clean_text());
    // sub_ui.label(&note.text);
    // sub_ui.add_space(20.);

    for link in note.get_links() {
        // sub_ui.label(format!("l{link}"));
        sub_ui.hyperlink_to(link_text(link), link);
    }

    // ui.put(rect, egui::Label::new(note.get_title()));

    // });
    // let resp = r.response.interact(egui::Sense::click());
    if resp.clicked() {
        *active_note = Some(*note_id);
    }
}
