use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::mpsc::{channel, Receiver, Sender},
};

use crate::{color_from_tag, link_text, readable_text, Deadline, Note, StorageMode};
use egui::{
    epaint::{ahash::HashSet, RectShape, Shadow, TextShape},
    global_dark_light_mode_buttons, vec2, Color32, FontData, FontFamily, FontId, Id, Layout, Pos2,
    Rect, Response, RichText, Rounding, SelectableLabel, Sense, Shape, Stroke, Ui, Vec2,
    WidgetText,
};
use egui_graphs::{Graph, GraphView};
use egui_notify::Toasts;
use petgraph::{stable_graph::StableGraph, Directed};

// use egui_commonmark::*;

#[derive(serde::Deserialize, serde::Serialize, Default, Debug, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Board,
    List,
    Graph,
}
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct UiState {
    #[serde(skip)]
    settings_enabled: bool,
    scratchpad_enabled: bool,
    tags_enabled: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct ScratchPad {
    sections: Vec<String>,
}

pub const GAMMA_MULT: f32 = 0.5;

pub type Notes = BTreeMap<u128, Note>;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct MeteoraApp {
    /// All notes
    notes: Notes,
    tags: Vec<String>,
    active_tags: HashSet<String>,
    active_note: Option<u128>,
    /// Authentication/encryption
    credentials: (String, String),
    /// The search filter
    filter: String,
    /// How notes are displayed
    viewmode: ViewMode,
    always_on_top: bool,
    /// How data is stored
    storage_mode: StorageMode,
    #[serde(skip)]
    toasts: Toasts,
    #[serde(skip)]
    channels: Channels,
    #[serde(skip)]
    graph: Option<Graph<String, (), Directed>>,
    ui_state: UiState,
    scratchpad: ScratchPad,
}

pub struct Channels {
    pub note_channel: (Sender<Notes>, Receiver<Notes>),
    pub id_channel: (Sender<String>, Receiver<String>),
    pub msg_channel: (Sender<Message>, Receiver<Message>),
}

impl Default for Channels {
    fn default() -> Self {
        Self {
            note_channel: channel(),
            id_channel: channel(),
            msg_channel: channel(),
        }
    }
}

pub enum Message {
    Info(String),
    Warn(String),
    Err(String),
}

impl Message {
    pub fn info(msg: &str) -> Self {
        Self::Info(msg.into())
    }
    pub fn warn(msg: &str) -> Self {
        Self::Warn(msg.into())
    }
    pub fn err(msg: &str) -> Self {
        Self::Err(msg.into())
    }
}

impl MeteoraApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "inter".to_owned(),
            FontData::from_static(include_bytes!("fonts/Inter-Regular.ttf")),
        );
        egui_extras::install_image_loaders(&cc.egui_ctx);

        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

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
                FontId::new(15.0, FontFamily::Proportional),
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

        cc.egui_ctx.set_style(style);

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
            if let Err(e) =
                self.storage_mode
                    .save_notes(&self.notes, &self.credentials, &self.channels)
            {
                eprintln!("{e}")
            }
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        use egui_phosphor::regular::*;

        if let Ok(id) = self.channels.id_channel.1.try_recv() {
            self.credentials.0 = id.clone();
            match &mut self.storage_mode {
                StorageMode::Local { .. } => {}
                StorageMode::JsonBin { bin_id, .. } => {
                    *bin_id = Some(id);
                    self.toasts.info("Registered JsonBin.".to_string());
                }
            }
        }

        if let Ok(notes) = self.channels.note_channel.1.try_recv() {
            self.toasts.info(format!("Loaded {} notes", notes.len()));
            self.notes = notes;
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            //    ui.allocate_exact_size(vec2(ui.available_width(), 30.), Sense::drag());
            ui.add_space(10.);

            ui.horizontal(|ui| {
                let not_settings = !self.ui_state.settings_enabled;
                ui.selectable_value(
                    &mut self.ui_state.settings_enabled,
                    not_settings,
                    egui::RichText::new(LIST).size(32.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .frame(false)
                        .hint_text("🔍 Search notes..."),
                );
                if !self.filter.is_empty() {
                    if ui.button(X).clicked() {
                        self.filter.clear();
                    }
                }
            });

            ui.add_space(10.);
        });

        if self.ui_state.settings_enabled {
            egui::SidePanel::left("side_panel_settings").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(SLIDERS);
                    ui.label("SETTINGS")
                });
                ui.separator();

                egui::ComboBox::from_label("Storage")
                    .selected_text(format!("{:?}", self.storage_mode))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.storage_mode,
                            StorageMode::Local {
                                path: PathBuf::from("backup.json"),
                            },
                            "Local",
                        );
                        // let k = env!("KEY", "");
                        ui.selectable_value(
                            &mut self.storage_mode,
                            StorageMode::JsonBin {
                                masterkey: include_str!("../key.jsonbin.master").into(),
                                bin_id: None,
                            },
                            "JsonBin",
                        );
                    });

                ui.horizontal(|ui| {
                    if ui.button("SAVE").clicked() {
                        if let Err(e) = self.storage_mode.save_notes(
                            &self.notes,
                            &self.credentials,
                            &self.channels,
                        ) {
                            self.toasts.error(format!("Error saving notes! {e}"));
                        } else {
                            self.toasts.info("Saved!".to_string());
                        }
                    }

                    if ui.button("RESTORE").clicked() {
                        match self
                            .storage_mode
                            .load_notes(&self.credentials, &self.channels)
                        {
                            Ok(_) => {
                                self.toasts.info("Loaded notes!");
                            }
                            Err(e) => {
                                self.toasts.error(format!("Error restoring notes! {e}"));
                            }
                        }
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.credentials.0)
                            .hint_text("Username")
                            .desired_width(150.),
                    );
                    ui.label("USER");
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.credentials.1)
                            .hint_text("Encryption Key")
                            .desired_width(150.)
                            .password(true),
                    );
                    ui.label("SECRET");
                });

                egui::ComboBox::from_label("View")
                    .selected_text(format!("{:?}", self.viewmode))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.viewmode, ViewMode::Board, "Board");
                        ui.selectable_value(&mut self.viewmode, ViewMode::List, "List");
                        ui.selectable_value(&mut self.viewmode, ViewMode::Graph, "Graph");
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

                            if ui.button("Restore from username").clicked() {
                                *bin_id = Some(self.credentials.0.clone());

                                _ = self
                                    .storage_mode
                                    .load_notes(&self.credentials, &self.channels);
                            }

                            if ui.button("Publish as new").clicked() {
                                if let Err(e) = self.storage_mode.save_notes(
                                    &self.notes,
                                    &self.credentials,
                                    &self.channels,
                                ) {
                                    self.toasts.error(format!("Error publishing notes! {e}"));
                                }
                            }
                        } else {
                            ui.label(format!("Bin ID: {}", bin_id.clone().unwrap_or_default()));
                            if ui.button("Copy to clipboard").clicked() {
                                ui.output_mut(|o| {
                                    o.copied_text = bin_id.clone().unwrap_or_default()
                                });
                            }
                        }
                    }
                }

                global_dark_light_mode_buttons(ui);

                ui.checkbox(&mut self.ui_state.scratchpad_enabled, "Scratchpad");
                ui.checkbox(&mut self.ui_state.tags_enabled, "Tags");
                ui.checkbox(&mut self.always_on_top, "Always on top");
            });
        }

        if self.ui_state.scratchpad_enabled {
            egui::SidePanel::left("right_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(NOTEPAD);
                    ui.label("SCRATCH")
                });
                ui.separator();
                if bare_button(FILE_PLUS, ui).clicked() {
                    self.scratchpad.sections.push("".into());
                }

                let mut remove: Option<usize> = None;

                for (i, section) in self.scratchpad.sections.iter_mut().enumerate() {
                    egui::CollapsingHeader::new(section.lines().next().unwrap_or("New scratch".into()))
                        .id_source(i)
                        .show_unindented(ui, |ui| {
                            ui.indent(i, |ui| {
                                ui.style_mut().visuals.selection.stroke = Stroke::NONE;
                                egui::TextEdit::multiline(section)
                                    .desired_width(f32::INFINITY)
                                    .hint_text("Enter some quick thoughts here!")
                                    .show(ui);
                                ui.horizontal(|ui| {
                                    if bare_button(NOTE, ui)
                                        .on_hover_text("Turn into note")
                                        .clicked()
                                    {
                                        let mut n = Note::new();
                                        n.text = section.clone();
                                        self.notes.insert(n.id, n);
                                        remove = Some(i);
                                    }

                                    if bare_button(TRASH, ui).on_hover_text("Delete").clicked() {
                                        remove = Some(i);
                                    }
                                });
                            });
                        });
                }

                if let Some(remove) = remove {
                    self.scratchpad.sections.remove(remove);
                }
            });
        }

        if self.ui_state.tags_enabled {
            egui::SidePanel::left("side_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(TAG);
                    ui.label("TAGS")
                });
                ui.separator();

                ui.vertical(|ui| {
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
                            color_from_tag(tag).gamma_multiply(GAMMA_MULT);
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
                    if !self.active_tags.is_empty() {
                        if ui.button("Show all").clicked() {
                            self.active_tags.clear();
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
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Offer restore fuctionality if local

            #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
            if self.notes.is_empty() && ui.button("load notes").clicked() {
                match self
                    .storage_mode
                    .load_notes(&self.credentials, &self.channels)
                {
                    Ok(_) => {
                        self.toasts.info("Loaded notes");
                    }
                    Err(e) => {
                        self.toasts.error(format!("Can't load notes: {e}"));
                    }
                }
            }

            if self.notes.is_empty() {
                let logo_size = 300.;
                let top = Pos2::new(250., 50.);
                egui::Image::new(egui::include_image!("../assets/maskable_icon_x512.png"))
                    .paint_at(ui, Rect::from_min_max(top, top + Vec2::splat(logo_size)));
            }

            match self.viewmode {
                ViewMode::Board => {
                    boardview(ui, self);
                }
                ViewMode::List => {
                    listview(ui, self);
                }
                ViewMode::Graph => {
                    ui.label("Work in progress!");

                    // add graph if not present
                    if self.graph.is_none() {
                        let mut g: StableGraph<String, ()> = StableGraph::new();
                        let mut added = vec![];
                        for note in self.notes.values() {
                            if !added.contains(&note.id) {
                                let a = g.add_node(note.get_title().into());
                                added.push(note.id);
                                for c in &note.depends {
                                    if let Some(depend) = self.notes.get(c) {
                                        let b = g.add_node(depend.get_title().into());
                                        g.add_edge(a, b, ());
                                        added.push(*c);
                                    }
                                }
                            }
                        }
                        self.graph = Some(Graph::from(&g));
                    }

                    if let Some(g) = self.graph.as_mut() {
                        ui.add(&mut GraphView::new(g).with_custom_node_draw(
                            |ctx, n, meta, _style, l| {
                                // lets draw a rect with label in the center for every node

                                // find node center location on the screen coordinates
                                let node_center_loc = n.screen_location(meta).to_pos2();

                                // find node radius accounting for current zoom level; we will use it as a reference for the rect and label sizes
                                let rad = n.screen_radius(meta);

                                // first create rect shape
                                let size = Vec2::new(rad * 1.5, rad * 1.5);
                                let rect = Rect::from_center_size(node_center_loc, size);
                                let shape_rect = Shape::rect_stroke(
                                    rect,
                                    Rounding::default(),
                                    Stroke::new(1., n.color(ctx)),
                                );

                                // then create shape for the label placing it in the center of the rect
                                let color = ctx.style().visuals.text_color();
                                let galley = ctx.fonts(|f| {
                                    f.layout_no_wrap(
                                        n.data().unwrap().clone(),
                                        FontId::new(rad, FontFamily::Monospace),
                                        color,
                                    )
                                });
                                // we need to offset a bit to place the label in the center of the rect
                                let label_loc = Pos2::new(
                                    node_center_loc.x - rad / 2.,
                                    node_center_loc.y - rad / 2.,
                                );
                                let shape_label = TextShape::new(label_loc, galley);

                                // add shapes to the drawing layers; the drawing process is happening in the widget lifecycle.
                                l.add(shape_rect);
                                l.add(shape_label);
                            },
                        ));
                    }
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
                    Rounding::ZERO,
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

            egui::Window::new("xxxxx")
                .collapsible(false)
                .title_bar(false)
                .fixed_rect(ctx.screen_rect().shrink(80.).translate(vec2(-10.0, 00.)))
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

fn edit_note(ui: &mut Ui, note_id: &u128, tags: &mut Vec<String>, notes: &mut Notes) {
    // make sure id is valid
    if notes.get(note_id).is_none() {
        ui.label("No such ID");
        return;
    }
    let immutable_notes = notes.clone();

    let note = notes.get_mut(note_id).unwrap();

    // ui.text_edit_multiline(&mut note.text);
    ui.add_sized(
        [ui.available_width(), 10.],
        egui::TextEdit::multiline(&mut note.text)
            .frame(false)
            .margin(vec2(20., 20.))
            .desired_rows(15),
    );

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
                ui.style_mut().visuals.selection.bg_fill =
                    color_from_tag(tag).gamma_multiply(GAMMA_MULT);
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

        ui.checkbox(&mut note.complete, "Finished");

        egui::ComboBox::from_id_source(format!("{}x", note.id))
            .selected_text("☞ depends on...".to_string())
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

        if ui.button("🗑 delete").clicked() {
            notes.remove(note_id);
        }
    });

    // ui.collapsing("RND", |ui| {
    // let mut cache = CommonMarkCache::default();
    //     CommonMarkViewer::new("viewer").show(ui, &mut cache, &note.text);

    // });
}

fn draw_note(ui: &mut Ui, note_id: &u128, notes: &Notes, active_note: &mut Option<u128>) {
    // make sure id is valid
    if notes.get(note_id).is_none() {
        ui.label("No such ID");
        return;
    }

    let note = notes.get(note_id).unwrap();

    let estimated_size =
        note.get_approx_height(ui.fonts(|r| r.row_height(&FontId::proportional(15.))) + 2.);

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
            color_from_tag(tag).gamma_multiply(GAMMA_MULT),
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
    //     let mut fcache = CommonMarkCache::default();
    //     CommonMarkViewer::new("viewer").show(&mut sub_ui, &mut cache, &note.get_clean_text());
    // } else {
    //     sub_ui.label(note.get_clean_text());
    // }

    sub_ui.add(
        egui::Label::new(
            RichText::new(&note.get_clean_text_truncated()).color(readable_text(
                &Color32::from_rgb(note.color[0], note.color[1], note.color[2]),
            )),
        )
        .truncate(true)
        .wrap(true),
    );

    // sub_ui.label(
    //     RichText::new(&note.get_clean_text())
    //     .color(readable_text(&Color32::from_rgb(
    //         note.color[0],
    //         note.color[1],
    //         note.color[2],
    //     ))
    // ), // .size(12.)
    // );

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

fn draw_list_note(ui: &mut Ui, note_id: &u128, notes: &Notes, active_note: &mut Option<u128>) {
    // make sure id is valid
    if notes.get(note_id).is_none() {
        ui.label("No such ID");
        return;
    }

    let note = notes.get(note_id).unwrap();

    let frame = egui::Frame {
        fill: note.get_color(),
        inner_margin: 5.0.into(),
        ..Default::default()
    };
    let inner = frame.show(ui, |ui| {
        ui.allocate_exact_size(vec2(ui.available_width(), 0.), Sense::click());
        ui.horizontal(|ui| {
            ui.label(note.get_title());
            ui.add(egui::Label::new(RichText::new(note.get_excerpt()).size(10.)).truncate(true));
        });
        for d in &note.depends {
            if let Some(dependent) = notes.get(d) {
                ui.collapsing(dependent.get_title(), |ui| {
                    draw_list_note(ui, d, notes, active_note);
                });
            }
        }
        if ui.ui_contains_pointer() {
            ui.painter().rect_filled(
                ui.min_rect().expand(5.),
                0.0,
                Color32::from_rgb_additive(11, 11, 11),
            );
        }
    });

    let resp = inner.response.interact(Sense::click());

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
    v.sort_by(|(_, a), (_, b)| b.get_final_prio().total_cmp(&a.get_final_prio()));

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
                                if note.complete {
                                    continue;
                                }

                                if !state.filter.is_empty()
                                    && !note
                                        .text
                                        .to_lowercase()
                                        .contains(&state.filter.to_lowercase())
                                {
                                    continue;
                                }
                                draw_note(ui, id, &state.notes, &mut state.active_note);
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
    v.sort_by(|(_, a), (_, b)| b.get_final_prio().total_cmp(&a.get_final_prio()));

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

                    draw_list_note(ui, id, &state.notes, &mut state.active_note);

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

pub fn bare_button(text: impl Into<String>, ui: &mut Ui) -> Response {
    ui.add(egui::Button::new(RichText::new(text).size(30.)).frame(false))
}
