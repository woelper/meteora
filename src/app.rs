use std::{collections::BTreeMap, path::PathBuf};

use crate::{color_from_tag, link_text, readable_text, Deadline, Note, StorageMode};
use egui::{
    epaint::{ahash::HashSet, RectShape, Shadow},
    global_dark_light_mode_buttons, vec2, Color32, FontData, FontFamily, FontId, Id, Layout, Pos2,
    Rect, Response, RichText, Rounding, SelectableLabel, Sense, Shape, Stroke, Ui, Vec2,
};
use egui_notify::Toasts;

// use egui_commonmark::*;

#[derive(serde::Deserialize, serde::Serialize, Default, Debug, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Board,
    List,
    Graph,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct MeteoraApp {
    /// All notes
    notes: BTreeMap<u128, Note>,
    tags: Vec<String>,
    active_tags: HashSet<String>,
    active_note: Option<u128>,
    /// Authentication/encryption
    credentials: (String, String),
    /// The search filter
    filter: String,
    /// How notes are displayed
    viewmode: ViewMode,
    /// How data is stored
    storage_mode: StorageMode,
    #[serde(skip)]
    toasts: Toasts,
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
        // style.visuals.widgets.inactive.bg_fill = Color32::BLUE;

        // style.visuals.panel_fill = Color32::from_gray(255);

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
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
        {
            if let Err(e) = self.storage_mode.save_notes(&self.notes, &self.credentials) {
                eprintln!("{e}")
            }
        }
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
                    global_dark_light_mode_buttons(ui);
                    if ui.button("Save").clicked() {
                        if let Err(e) = self.storage_mode.save_notes(&self.notes, &self.credentials)
                        {
                            self.toasts.error(format!("Error saving notes! {e}"));
                        } else {
                            self.toasts.info("Saved!".to_string());
                        }
                        ui.close_menu();
                    }

                    if ui.button("Restore").clicked() {
                        match self.storage_mode.load_notes(&self.credentials) {
                            Ok(notes) => {
                                self.notes = notes;
                                self.toasts.info("Loaded notes!");
                            }
                            Err(e) => {
                                self.toasts.error(format!("Error restoring notes! {e}"));
                            }
                        }
                        ui.close_menu();
                    }
                });
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(&mut self.filter);
            });

            ui.heading("Tags");

            ui.vertical_centered_justified(|ui| {
                if ui.button("Show all").clicked() {
                    self.active_tags.clear();
                }
                let all_used_tags = self
                    .notes
                    .iter()
                    .flat_map(|(_, n)| &n.tags)
                    .collect::<Vec<_>>();
                for tag in &self.tags {
                    // Hide tags that are unused.
                    if !all_used_tags.contains(&tag) {
                        continue;
                    }
                    let contained = self.active_tags.contains(tag);

                    ui.style_mut().visuals.selection.bg_fill =
                        color_from_tag(tag).gamma_multiply(0.5);
                    // ui.label(format!("{:?}",color_from_tag(tag)));
                    if ui.add(SelectableLabel::new(contained, tag)).clicked() {
                        // if ui.selectable_label(contained, tag).clicked() {
                        if contained {
                            self.active_tags.remove(tag);
                        } else {
                            self.active_tags.insert(tag.clone());
                        }
                    }
                }

                ui.collapsing("Edit", |ui| {
                    if ui.button("Add tag").clicked() {
                        self.tags.push("New Tag".into());
                    }

                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        let mut tag_index_to_delete: Option<usize> = None;

                        for (i, tag) in &mut self.tags.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                if ui
                                    .button("🗑")
                                    .on_hover_text("Delete this tag from list and all notes.")
                                    .clicked()
                                {
                                    for note in self.notes.values_mut() {
                                        note.tags.remove(tag);
                                    }
                                    tag_index_to_delete = Some(i);
                                }
                                let old_tag = tag.clone();
                                if ui.text_edit_singleline(tag).changed() {
                                    // If a tag is renamed, we need to rename it in all notes.
                                    for note in self.notes.values_mut() {
                                        if note.tags.contains(&old_tag) {
                                            note.tags.remove(&old_tag);
                                            note.tags.insert(tag.clone());

                                            // if let Some(i) =
                                            //     note.tags.iter().position(|x| x == &old_tag)
                                            // {
                                            //     note.tags[i] = tag.clone();
                                            // }
                                        }
                                    }
                                }
                            });
                        }

                        if let Some(i) = tag_index_to_delete {
                            self.tags.remove(i);
                        }
                    });
                });

                ui.collapsing("Settings", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.credentials.0).hint_text("Username"),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.credentials.1)
                            .hint_text("Encryption Key"),
                    );

                    egui::ComboBox::from_label("View mode")
                        .selected_text(format!("{:?}", self.viewmode))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.viewmode, ViewMode::Board, "Board");
                            ui.selectable_value(&mut self.viewmode, ViewMode::List, "List");
                            ui.selectable_value(&mut self.viewmode, ViewMode::Graph, "Graph");
                        });

                    egui::ComboBox::from_label("Storage mode")
                        .selected_text(format!("{:?}", self.storage_mode))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.storage_mode,
                                StorageMode::Local {
                                    path: PathBuf::from("backup.json"),
                                },
                                "Local",
                            );
                            ui.selectable_value(
                                &mut self.storage_mode,
                                StorageMode::JsonBin {
                                    masterkey: include_str!("../key.jsonbin.master").into(),
                                    bin_id: None,
                                },
                                "JsonBin",
                            );
                        });

                    match &mut self.storage_mode {
                        StorageMode::Local { path } => {
                            let mut s = path.to_string_lossy().to_string();
                            if ui.text_edit_singleline(&mut s).changed() {
                                *path = PathBuf::from(s);
                            }
                        }
                        StorageMode::JsonBin {
                            masterkey: _,
                            bin_id,
                        } => {
                            if bin_id.is_none() {
                                ui.label("Your data has never been published.");

                                if ui.button("Publish now").clicked() {
                                    if let Err(e) =
                                        self.storage_mode.save_notes(&self.notes, &self.credentials)
                                    {
                                        self.toasts.error(format!("Error publishing notes! {e}"));
                                    }
                                }
                            } else {
                                ui.label(format!("Bin ID: {}", bin_id.clone().unwrap_or_default()));
                            }
                        }
                    }

                    global_dark_light_mode_buttons(ui);
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // Offer restore fuctionality
            if self.notes.is_empty() {
                match self.storage_mode.load_notes(&self.credentials) {
                    Ok(notes) => {
                        self.notes = notes;
                    }
                    Err(e) => {
                        self.toasts.info(format!("Can't load notes: {e}"));
                    }
                }
            }

            match self.viewmode {
                ViewMode::Board => {
                    boardview(ui, self);
                }
                ViewMode::List => {
                    listview(ui, self);
                }
                ViewMode::Graph => {
                    ui.label("Not yet!");
                }
            }

            //create a round button at an absolute position

            if draw_note_add_button(ui).clicked() {
                let mut n = Note::new();
                n.tags = self.active_tags.iter().cloned().collect();
                self.active_note = Some(n.id);
                self.notes.insert(n.id, n);
            }

            // Draw black background if editing note
            if self.active_note.is_some() {
                let rect_all = ui.allocate_rect(Rect::EVERYTHING, Sense::click());
                ui.painter_at(Rect::EVERYTHING).rect_filled(
                    Rect::EVERYTHING,
                    Rounding::none(),
                    Color32::from_rgba_premultiplied(0, 0, 0, 150),
                );

                // if rect is clicked, close edit mode
                if rect_all.clicked() {
                    self.active_note = None;
                }
            }
        });

        if let Some(id) = self.active_note {
            // clean invalid id (because of deletion)
            if !self.notes.contains_key(&id) {
                self.active_note = None;
            }

            egui::Window::new("x")
                .collapsible(false)
                .title_bar(false)
                .fixed_rect(ctx.available_rect().shrink(0.).translate(vec2(-60.0, 20.)))
                .show(ctx, |ui| {
                    ui.vertical_centered_justified(|ui| {
                        edit_note(ui, &id, &mut self.tags, &mut self.notes);

                        if ui.button("Close").clicked() {
                            self.active_note = None;
                        }
                    });
                });
        }

        self.toasts.show(ctx);

        // });
    }
}

fn edit_note(
    ui: &mut Ui,
    note_id: &u128,
    tags: &mut Vec<String>,
    notes: &mut BTreeMap<u128, Note>,
) {
    // make sure id is valid
    if notes.get(note_id).is_none() {
        ui.label("No such ID");
        return;
    }
    let immutable_notes = notes.clone();

    let note = notes.get_mut(note_id).unwrap();

    // ui.text_edit_multiline(&mut note.text);
    ui.add(egui::TextEdit::multiline(&mut note.text).desired_rows(15));
    // ui.add_sized(ui.available_size()/2., egui::TextEdit::multiline(&mut note.text));

    ui.horizontal(|ui| {
        ui.label("Base Priority");
        ui.add(egui::Slider::new(&mut note.priority, 0.0..=1.0));
    });

    ui.horizontal(|ui| {
        ui.label("Deadline");

        egui::ComboBox::from_id_source(format!("{}xx", note.id))
            .selected_text(format!("{:?}", note.deadline))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut note.deadline, Deadline::Eternal, "Eternal");

                ui.selectable_value(
                    &mut note.deadline,
                    Deadline::Fixed(chrono::Utc::now().date_naive()),
                    "Date",
                );
                ui.selectable_value(
                    &mut note.deadline,
                    Deadline::Periodic {
                        start: chrono::Utc::now().date_naive(),
                        days: 0,
                    },
                    "Repeating",
                );
            });
    });

    match &mut note.deadline {
        crate::Deadline::Eternal => {}
        crate::Deadline::Periodic { start, days } => {
            ui.add(egui_extras::DatePickerButton::new(start));
            ui.add(egui::Slider::new(days, 0..=10000).text("days offset"));
        }
        crate::Deadline::Fixed(date) => {
            ui.add(egui_extras::DatePickerButton::new(date));
            // egui_extras::DatePickerButton::new(date);
        }
    }

    // Color comes from tags, so only show selector if there are no tags.
    if note.tags.is_empty() {
        ui.color_edit_button_srgb(&mut note.color);
    }

    ui.group(|ui| {
        ui.allocate_space(vec2(ui.available_width(), 0.));
        ui.horizontal(|ui| {
            ui.label("Tags:");
            for tag in tags.iter() {
                let contains = note.tags.contains(tag);
                ui.style_mut().visuals.selection.bg_fill = color_from_tag(tag).gamma_multiply(0.5);
                if ui.selectable_label(contains, tag.to_string()).clicked() {
                    if contains {
                        note.tags.remove(tag);
                    } else {
                        note.tags.insert(tag.clone());
                    }
                }
            }
        });

        let id = Id::new("newtag");
        if ui.button("Add tag...").clicked() {
            ui.ctx()
                .memory_mut(|w| w.data.insert_temp(id, "New Tag".to_string()));
        }

        let newtag = ui.ctx().memory_mut(|w| w.data.get_temp::<String>(id));
        if let Some(tag) = newtag {
            let mut tag = tag;
            if ui.text_edit_singleline(&mut tag).changed() {
                ui.ctx().memory_mut(|w| w.data.insert_temp(id, tag.clone()));
            }
            if ui.button("Save").clicked() {
                tags.push(tag.clone());
                ui.ctx().memory_mut(|w| w.data.clear());
            }
        }
    });

    ui.horizontal(|ui| {
        let note = notes.get_mut(note_id).unwrap();

        egui::ComboBox::from_id_source(note.id)
            .selected_text("Select tag".to_string())
            .show_ui(ui, |ui| {
                for tag in tags.iter() {
                    let contains = note.tags.contains(tag);
                    if ui.selectable_label(contains, tag.to_string()).clicked() {
                        if contains {
                            note.tags.remove(tag);
                        } else {
                            note.tags.insert(tag.clone());
                        }
                    }
                }
            });

        egui::ComboBox::from_id_source(format!("{}x", note.id))
            .selected_text("Depends on...".to_string())
            .show_ui(ui, |ui| {
                for (i, n) in immutable_notes.iter() {
                    let contains = note.depends.contains(i);
                    if ui.selectable_label(contains, n.get_title()).clicked() {
                        if contains {
                            note.depends.remove(i);
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

    let estimated_size =
        note.get_approx_height(ui.fonts(|r| r.row_height(&FontId::proportional(14.))) + 2.);

    let note_size = Vec2::new(150., estimated_size.max(150.));

    let (rect, resp) = ui.allocate_exact_size(note_size, Sense::click());

    let stroke = if resp.hovered() {
        Stroke::new(3., Color32::GRAY)
    } else {
        Stroke::NONE
    };

    let frame_shape = Shape::Rect(RectShape::new(rect, 5.0, note.get_color(), stroke));

    let mut shapes_to_draw = vec![frame_shape];

    for (i, tag) in note.tags.iter().enumerate().skip(1) {
        let offset = 20.;

        let r = Rect::from_min_max(
            rect.left_top(),
            Pos2::new(rect.left_top().x + offset, rect.left_top().y + offset),
        )
        .translate(vec2(offset * i as f32, 0.0))
        .translate(vec2(-offset + 2., note_size.y - offset - 2.));

        let tag_shape = Shape::Rect(RectShape::new(
            r,
            10.0,
            color_from_tag(tag).gamma_multiply(0.5),
            Stroke::NONE,
        ));
        shapes_to_draw.push(tag_shape)
    }

    let shp = {
        let shadow = Shadow::small_light();
        let shadow = shadow.tessellate(rect, 5.0);
        let shadow = Shape::Mesh(shadow);
        shapes_to_draw.push(shadow);
        Shape::Vec(shapes_to_draw)
    };

    ui.painter().add(shp);

    let mut sub_ui = ui.child_ui(
        rect.shrink(10.),
        Layout::left_to_right(egui::Align::TOP).with_main_wrap(true),
    );

    // if note.contains_markdown() {
    //     let mut cache = CommonMarkCache::default();
    //     CommonMarkViewer::new("viewer").show(&mut sub_ui, &mut cache, &note.get_clean_text());
    // } else {
    //     sub_ui.label(note.get_clean_text());
    // }

    sub_ui.label(
        RichText::new(&note.get_clean_text()).color(readable_text(&Color32::from_rgb(
            note.color[0],
            note.color[1],
            note.color[2],
        ))), // .size(12.)
    );

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

fn draw_list_note(
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

    // let r = ui.allocate_at_least(desired_size, sense)

    let inner = ui.group(|ui| {
        // ui.allocate_space(ui.available_size());
        ui.allocate_exact_size(vec2(ui.available_width(), 0.), Sense::click());
        ui.label(
            RichText::new(note.get_title()).color(readable_text(&Color32::from_rgb(
                note.color[0],
                note.color[1],
                note.color[2],
            ))), // .size(12.)
        );
    });

    let resp = inner.response.interact(Sense::click());

    // let mut shapes_to_draw = vec![];

    // for (i, tag) in note.tags.iter().enumerate().skip(1) {
    //     let offset = 20.;

    //     let r = Rect::from_min_max(
    //         rect.left_top(),
    //         Pos2::new(rect.left_top().x + offset, rect.left_top().y + offset),
    //     )
    //     .translate(vec2(offset * i as f32, 0.0))
    //     .translate(vec2(-offset + 2., note_size.y - offset - 2.));

    //     let tag_shape = Shape::Rect(RectShape {
    //         rect: r,
    //         rounding: 10.0.into(),
    //         fill: color_from_tag(tag).gamma_multiply(0.5),
    //         stroke: Stroke::NONE,
    //     });
    //     shapes_to_draw.push(tag_shape)
    // }

    // let shp = {
    //     let shadow = Shadow::small_light();
    //     let shadow = shadow.tessellate(rect, 5.0);
    //     let shadow = Shape::Mesh(shadow);
    //     shapes_to_draw.push(shadow);
    //     Shape::Vec(shapes_to_draw)
    // };

    // ui.painter().add(shp);

    // sub_ui.label(&note.text);
    // sub_ui.add_space(20.);

    // ui.put(rect, egui::Label::new(note.get_title()));

    // });
    // let resp = r.response.interact(egui::Sense::click());
    if resp.clicked() {
        *active_note = Some(*note_id);
    }
}

/// The lower-right plus-button
fn draw_note_add_button(ui: &mut Ui) -> Response {
    let button_size = Vec2::splat(60.);
    let margin = 10.;
    let pos = Pos2::new(
        ui.ctx().screen_rect().right() - button_size.x / 2. - margin,
        ui.ctx().screen_rect().bottom() - button_size.y / 2. - margin,
    );
    let rect = Rect::from_center_size(pos, button_size);
    ui.put(
        rect,
        egui::widgets::Button::new(RichText::new("✚").heading())
            .rounding(100.)
            .fill(Color32::from_rgba_premultiplied(50, 50, 50, 100)),
    )
}

fn boardview(ui: &mut Ui, state: &mut MeteoraApp) {
    let mut v = Vec::from_iter(state.notes.clone());
    v.sort_by(|(_, a), (_, b)| b.priority.total_cmp(&a.priority));

    egui::ScrollArea::horizontal()
        // .auto_shrink([false,false])
        .hscroll(true)
        .min_scrolled_width(ui.available_width())
        .show(ui, |ui| {
            ui.allocate_ui(Vec2::new(150., ui.available_size_before_wrap().y), |ui| {
                ui.with_layout(
                    egui::Layout::top_down(egui::Align::RIGHT).with_main_wrap(true),
                    |ui| {
                        // Add a lot of widgets here.
                        for (id, note) in &v {
                            if state.active_tags.is_empty()
                                || note.tags.iter().any(|t| state.active_tags.contains(t))
                            {
                                if !state.filter.is_empty()
                                    && !note
                                        .text
                                        .to_lowercase()
                                        .contains(&state.filter.to_lowercase())
                                {
                                    continue;
                                }
                                draw_note(ui, id, &mut state.notes, &mut state.active_note);
                                // Safety: if note has an unknown tag, add it.
                                for tag in &note.tags {
                                    if !state.tags.contains(tag) {
                                        state.tags.push(tag.clone());
                                    }
                                }
                            }
                        }
                    },
                )
            });
        });
}

fn listview(ui: &mut Ui, state: &mut MeteoraApp) {
    let mut v = Vec::from_iter(state.notes.clone());
    v.sort_by(|(_, a), (_, b)| b.priority.total_cmp(&a.priority));

    egui::ScrollArea::vertical()
        // .auto_shrink([false,false])
        // .min_scrolled_width(ui.available_width())
        .show(ui, |ui| {
            for (id, note) in &v {
                if state.active_tags.is_empty()
                    || note.tags.iter().any(|t| state.active_tags.contains(t))
                {
                    if !state.filter.is_empty()
                        && !note
                            .text
                            .to_lowercase()
                            .contains(&state.filter.to_lowercase())
                    {
                        continue;
                    }

                    draw_list_note(ui, id, &mut state.notes, &mut state.active_note);

                    // Safety: if note has an unknown tag, add it.
                    for tag in &note.tags {
                        if !state.tags.contains(tag) {
                            state.tags.push(tag.clone());
                        }
                    }
                }
            }
        });
}
