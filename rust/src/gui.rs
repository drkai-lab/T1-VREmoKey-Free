//! Assignment window. F1 through F4, each with a thumbnail and a name.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui;
use serde_json::{Map, Value};

use crate::focus::as_bool;
use crate::i18n::{import_notice, present_error, tr};
use crate::keys::KeyProblem;
use crate::listener::{AvatarOscListener, ExpressionListener};
use crate::paths::{log_line, AppPaths};
use crate::profile::{
    activate_avatar, active_profile, expression_slot, import_profile_pack, list_profiles,
    stage_card, Slot,
};
use crate::settings::{
    avatar_listen_port, lang_of, load_settings, normalize_assign, resolve_config, save_settings,
    zoom_of,
};
use crate::tray::{Tray, TrayAction};
use crate::{DISPLAY_NAME, FKEY_NAMES};

const PINK: egui::Color32 = egui::Color32::from_rgb(255, 67, 151);
const HEADER: egui::Color32 = egui::Color32::from_rgb(156, 42, 122);
const PANEL: egui::Color32 = egui::Color32::from_rgb(255, 247, 250);
const OUTLINE: egui::Color32 = egui::Color32::from_rgb(90, 62, 74);
const SELECT_TEXT: egui::Color32 = egui::Color32::from_rgb(40, 10, 24);
const ERROR_FG: egui::Color32 = egui::Color32::from_rgb(122, 20, 48);
const ERROR_BG: egui::Color32 = egui::Color32::from_rgb(255, 236, 240);
const SUBTITLE: egui::Color32 = egui::Color32::from_rgb(255, 230, 240);
const THUMB: f32 = 56.0;
const ZOOM_CHOICES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

pub fn run(paths: &AppPaths) -> i32 {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([720.0, 640.0])
        .with_min_inner_size([560.0, 480.0])
        .with_title(DISPLAY_NAME)
        .with_app_id(crate::APP_NAME);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let paths = paths.clone();
    match eframe::run_native(
        DISPLAY_NAME,
        native_options,
        Box::new(move |cc| Ok(Box::new(HotkeyApp::new(cc, paths.clone())))),
    ) {
        Ok(()) => 0,
        Err(err) => {
            let paths = AppPaths::system();
            log_line(&paths, &format!("window failed: {err}"));
            eprintln!("The window could not open: {err}");
            1
        }
    }
}

struct HotkeyApp {
    paths: AppPaths,
    settings: Value,
    lang: String,
    enabled: bool,
    vrchat_only: bool,
    host: String,
    port: String,
    listen_port: String,
    listener: ExpressionListener,
    avatar_tx: Sender<Option<String>>,
    avatar_rx: Receiver<Option<String>>,
    avatar: Option<AvatarOscListener>,
    notice: String,
    textures: HashMap<String, egui::TextureHandle>,
    generic: Option<egui::TextureHandle>,
    header_icon: Option<egui::TextureHandle>,
    allow_exit: bool,
    tray: Option<Tray>,
    tray_failed: bool,
    shot: Option<std::path::PathBuf>,
    shot_wait: u8,
    stage_pin: Option<i64>,
    stage_dismissed: Option<String>,
    zoom: f32,
    zoom_ready: bool,
}

impl HotkeyApp {
    fn new(cc: &eframe::CreationContext<'_>, paths: AppPaths) -> Self {
        install_fonts(&cc.egui_ctx);
        install_theme(&cc.egui_ctx);
        let settings = load_settings(&paths);
        let zoom = zoom_of(&settings);
        let shot = ui_shot_path();
        // Screenshot mode previews Japanese without writing that choice to settings.
        let lang = if shot.is_some() {
            "ja".to_string()
        } else {
            lang_of(&settings)
        };
        let enabled = as_bool(settings.get("enabled"), true);
        let target = resolve_config(&settings, &paths.config_dir).target;
        let osc = settings.get("osc").cloned().unwrap_or(Value::Null);
        let host = osc
            .get("host")
            .and_then(Value::as_str)
            .unwrap_or("127.0.0.1")
            .to_string();
        let port = osc
            .get("port")
            .and_then(Value::as_i64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| crate::OSC_PORT.to_string());
        let listen_port = avatar_listen_port(&settings).to_string();
        let listener = ExpressionListener::new(paths.clone());
        if enabled {
            listener.start();
        }
        let (avatar_tx, avatar_rx) = mpsc::channel();
        let mut app = Self {
            paths,
            settings,
            lang,
            enabled,
            vrchat_only: target.enabled,
            host,
            port,
            listen_port,
            listener,
            avatar_tx,
            avatar_rx,
            avatar: None,
            notice: String::new(),
            textures: HashMap::new(),
            generic: None,
            header_icon: None,
            allow_exit: false,
            tray: None,
            tray_failed: false,
            shot,
            shot_wait: 0,
            stage_pin: None,
            stage_dismissed: None,
            zoom,
            zoom_ready: false,
        };
        app.sync_avatar();
        app
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .fill(PANEL)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                self.header(ui);
                ui.add_space(8.0);
                self.options(ui);
                ui.add_space(6.0);
                self.status_line(ui);
                self.key_warning(ui);
                ui.add_space(8.0);
                self.rows(ui);
            });
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let icon = self.header_texture(ui.ctx());
        egui::Frame::NONE
            .fill(HEADER)
            .inner_margin(egui::Margin::same(10))
            .corner_radius(10.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(icon) = icon {
                        ui.add(egui::Image::new(&icon).fit_to_exact_size(egui::vec2(40.0, 40.0)));
                    }
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(DISPLAY_NAME)
                                .color(egui::Color32::WHITE)
                                .strong()
                                .size(20.0),
                        );
                        ui.label(
                            egui::RichText::new(tr(&self.lang, "subtitle"))
                                .color(SUBTITLE)
                                .size(14.0),
                        );
                    });
                });
            });
    }

    fn options(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang.clone();
        if ui
            .checkbox(&mut self.enabled, tr(&lang, "enable"))
            .changed()
        {
            self.save_flags();
        }
        if ui
            .checkbox(&mut self.vrchat_only, tr(&lang, "filter"))
            .changed()
        {
            self.save_flags();
        }
        let mut host_done = false;
        let mut port_done = false;
        let mut listen_done = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(tr(&lang, "host"));
            let host = ui.add(egui::TextEdit::singleline(&mut self.host).desired_width(140.0));
            ui.label(tr(&lang, "port"));
            let port = ui.add(egui::TextEdit::singleline(&mut self.port).desired_width(64.0));
            ui.label(tr(&lang, "listen"));
            let listen =
                ui.add(egui::TextEdit::singleline(&mut self.listen_port).desired_width(64.0));
            host_done = host.lost_focus();
            port_done = port.lost_focus();
            listen_done = listen.lost_focus();
        });
        if host_done || port_done {
            self.commit_osc();
        }
        if listen_done {
            self.commit_listen();
        }
        if !valid_host(&self.host) {
            error_line(ui, tr(&lang, "host_invalid"));
        }
        if valid_port(&self.port).is_none() || valid_port(&self.listen_port).is_none() {
            error_line(ui, tr(&lang, "port_invalid"));
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(tr(&lang, "avatar"));
            self.avatar_box(ui);
            if ui.button(tr(&lang, "import")).clicked() {
                self.import_profile();
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(tr(&lang, "language"));
            self.language_box(ui);
            ui.label(tr(&lang, "zoom"));
            self.zoom_box(ui);
        });
    }

    fn avatar_box(&mut self, ui: &mut egui::Ui) {
        let profiles = list_profiles(&self.paths.config_dir);
        let mut ids = Vec::new();
        let mut labels = Vec::new();
        for item in &profiles {
            ids.push(item.id.clone());
            if item.id == crate::DEFAULT_AVATAR_ID {
                labels.push(item.name.clone());
            } else {
                labels.push(format!("{} ({})", item.name, item.id));
            }
        }
        let current = self
            .settings
            .get("active_avatar")
            .and_then(Value::as_str)
            .unwrap_or(crate::DEFAULT_AVATAR_ID);
        let selected = ids.iter().position(|id| id == current).unwrap_or(0);
        let shown = labels.get(selected).cloned().unwrap_or_default();
        let mut picked = selected;
        egui::ComboBox::from_id_salt("avatar")
            .width(280.0)
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for (index, label) in labels.iter().enumerate() {
                    ui.selectable_value(&mut picked, index, label);
                }
            });
        if picked != selected {
            if let Some(id) = ids.get(picked) {
                activate_avatar(&mut self.settings, id, &self.paths.config_dir);
                let _ = save_settings(&self.paths, &self.settings);
            }
        }
    }

    fn rows(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang.clone();
        let profile = active_profile(&self.settings, &self.paths.config_dir);
        let choices = choice_labels(&lang, &profile, &self.paths);
        let assign = normalize_assign(self.settings.get("fkey_assign"));
        let mut pinned = None;
        for name in FKEY_NAMES {
            let chosen = assign.get(name).copied().unwrap_or(0);
            let slot = if chosen == 0 {
                None
            } else {
                expression_slot(chosen, &profile, &self.paths.config_dir)
            };
            let thumb = self.thumb_texture(ui.ctx(), slot.as_ref());
            let position = choices
                .iter()
                .position(|choice| choice.index == chosen)
                .unwrap_or(0);
            let shown = choices
                .get(position)
                .map(|choice| choice.label.clone())
                .unwrap_or_default();
            let mut picked = position;
            let icons: Vec<Option<egui::TextureHandle>> = choices
                .iter()
                .map(|choice| self.choice_icon(ui.ctx(), choice))
                .collect();
            egui::Frame::NONE
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(1.0, OUTLINE))
                .inner_margin(egui::Margin::same(6))
                .corner_radius(8.0)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.add_sized(
                            [40.0, THUMB],
                            egui::Label::new(egui::RichText::new(name).strong().size(16.0)),
                        );
                        if let Some(thumb) = thumb {
                            ui.add(
                                egui::Image::new(&thumb)
                                    .fit_to_exact_size(egui::vec2(THUMB, THUMB))
                                    .corner_radius(6.0),
                            );
                        } else {
                            ui.add_space(THUMB);
                        }
                        egui::ComboBox::from_id_salt(name)
                            .width(280.0)
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for (index, choice) in choices.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        if let Some(icon) =
                                            icons.get(index).and_then(Option::as_ref)
                                        {
                                            ui.add(
                                                egui::Image::new(icon)
                                                    .fit_to_exact_size(egui::vec2(28.0, 28.0)),
                                            );
                                        } else {
                                            ui.add_space(28.0);
                                        }
                                        ui.selectable_value(&mut picked, index, &choice.label);
                                    });
                                }
                            });
                        if chosen != 0 && ui.button(tr(&lang, "preview")).clicked() {
                            pinned = Some(chosen);
                        }
                    });
                });
            ui.add_space(4.0);
            if picked != position {
                let index = choices.get(picked).map(|choice| choice.index).unwrap_or(0);
                self.set_assign(name, index);
            }
        }
        if pinned.is_some() {
            self.stage_pin = pinned;
            self.stage_dismissed = None;
        }
    }

    fn status_line(&self, ui: &mut egui::Ui) {
        let state = if self.listener.is_running() {
            tr(&self.lang, "running")
        } else {
            tr(&self.lang, "stopped")
        };
        let last = {
            let last = self.listener.last();
            if last.is_empty() {
                tr(&self.lang, "none").to_string()
            } else {
                last
            }
        };
        let port_state = match &self.avatar {
            Some(avatar) if avatar.ok() => tr(&self.lang, "ok"),
            Some(avatar) if !avatar.error().is_empty() => tr(&self.lang, "busy"),
            _ => "",
        };
        let mut text = format!("{state} | {}: {last} {port_state}", tr(&self.lang, "last"));
        if !self.notice.is_empty() {
            text.push('\n');
            text.push_str(&self.notice);
        }
        ui.label(egui::RichText::new(text).size(13.0));
    }

    fn save_flags(&mut self) {
        let Some(obj) = self.settings.as_object_mut() else {
            return;
        };
        obj.insert("enabled".into(), Value::Bool(self.enabled));
        let target = obj
            .entry("target")
            .or_insert_with(|| Value::Object(Map::new()));
        if !target.is_object() {
            *target = Value::Object(Map::new());
        }
        if let Some(target) = target.as_object_mut() {
            target.insert("enabled".into(), Value::Bool(self.vrchat_only));
        }
        let _ = save_settings(&self.paths, &self.settings);
        self.sync_listener();
    }

    fn commit_osc(&mut self) {
        let host = valid_host(&self.host).then(|| self.host.trim().to_string());
        let port = valid_port(&self.port);
        if host.is_none() && port.is_none() {
            return;
        }
        if let Some(host) = host.as_ref() {
            self.host.clone_from(host);
        }
        if let Some(port) = port {
            self.port = port.to_string();
        }
        let Some(obj) = self.settings.as_object_mut() else {
            return;
        };
        let osc = obj
            .entry("osc")
            .or_insert_with(|| Value::Object(Map::new()));
        if !osc.is_object() {
            *osc = Value::Object(Map::new());
        }
        let Some(osc) = osc.as_object_mut() else {
            return;
        };
        let mut changed = false;
        if let Some(host) = host {
            let current = osc.get("host").and_then(Value::as_str).unwrap_or("");
            if current != host {
                osc.insert("host".into(), Value::String(host));
                changed = true;
            }
        }
        if let Some(port) = port {
            let current = osc.get("port").and_then(Value::as_i64);
            if current != Some(i64::from(port)) {
                osc.insert("port".into(), Value::from(port));
                changed = true;
            }
        }
        if changed {
            let _ = save_settings(&self.paths, &self.settings);
        }
    }

    fn commit_listen(&mut self) {
        let Some(port) = valid_port(&self.listen_port) else {
            return;
        };
        self.listen_port = port.to_string();
        if avatar_listen_port(&self.settings) == port {
            return;
        }
        let Some(obj) = self.settings.as_object_mut() else {
            return;
        };
        let block = obj
            .entry("avatar_osc")
            .or_insert_with(|| Value::Object(Map::new()));
        if !block.is_object() {
            *block = Value::Object(Map::new());
        }
        if let Some(block) = block.as_object_mut() {
            block.insert("port".into(), Value::from(port));
        }
        let _ = save_settings(&self.paths, &self.settings);
        self.sync_avatar();
    }

    fn set_assign(&mut self, name: &str, index: i64) {
        let Some(obj) = self.settings.as_object_mut() else {
            return;
        };
        let assign = obj
            .entry("fkey_assign")
            .or_insert_with(|| Value::Object(Map::new()));
        if !assign.is_object() {
            *assign = Value::Object(Map::new());
        }
        if let Some(assign) = assign.as_object_mut() {
            assign.insert(name.to_string(), Value::from(index));
        }
        let _ = save_settings(&self.paths, &self.settings);
    }

    fn sync_listener(&self) {
        if self.enabled {
            self.listener.start();
        } else {
            self.listener.stop();
        }
    }

    fn sync_avatar(&mut self) {
        let port = avatar_listen_port(&self.settings);
        if self
            .avatar
            .as_ref()
            .is_some_and(|current| current.port() == port && current.running())
        {
            return;
        }
        if let Some(current) = &self.avatar {
            current.stop();
        }
        let tx = self.avatar_tx.clone();
        let paths = self.paths.clone();
        let listener = AvatarOscListener::new(port, "127.0.0.1", move |avatar_id| {
            let switched = crate::profile::note_avatar_change(&paths, &avatar_id);
            let _ = tx.send(switched.map(|profile| profile.name));
        });
        if !listener.start() {
            log_line(
                &self.paths,
                &format!("avatar OSC port {port} is busy: {}", listener.error()),
            );
        }
        self.avatar = Some(listener);
    }

    fn language_box(&mut self, ui: &mut egui::Ui) {
        let mut code = self.lang.clone();
        let shown = if self.lang == "ja" {
            "日本語"
        } else {
            "English"
        };
        egui::ComboBox::from_id_salt("ui-lang")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut code, "ja".to_string(), "日本語");
                ui.selectable_value(&mut code, "en".to_string(), "English");
            });
        if code != self.lang {
            self.lang = code;
            if let Some(obj) = self.settings.as_object_mut() {
                obj.insert("lang".into(), Value::String(self.lang.clone()));
            }
            let _ = save_settings(&self.paths, &self.settings);
        }
    }

    fn zoom_box(&mut self, ui: &mut egui::Ui) {
        let mut chosen = self.zoom;
        egui::ComboBox::from_id_salt("ui-zoom")
            .selected_text(zoom_label(self.zoom))
            .show_ui(ui, |ui| {
                for zoom in ZOOM_CHOICES {
                    ui.selectable_value(&mut chosen, zoom, zoom_label(zoom));
                }
            });
        if (chosen - self.zoom).abs() > 0.01 {
            self.zoom = chosen;
            ui.ctx().set_zoom_factor(self.zoom);
            self.store_zoom();
        }
    }

    fn sync_zoom(&mut self, ctx: &egui::Context) {
        if !self.zoom_ready {
            ctx.set_zoom_factor(self.zoom);
            self.zoom_ready = true;
            return;
        }
        let current = ctx.zoom_factor().clamp(0.75, 3.0);
        if (current - self.zoom).abs() > 0.01 {
            self.zoom = current;
            if (ctx.zoom_factor() - current).abs() > 0.001 {
                ctx.set_zoom_factor(current);
            }
            self.store_zoom();
        }
    }

    fn store_zoom(&mut self) {
        let stored = (self.zoom * 100.0).round() / 100.0;
        if let Some(obj) = self.settings.as_object_mut() {
            obj.insert("ui_zoom".into(), Value::from(stored));
        }
        let _ = save_settings(&self.paths, &self.settings);
    }

    fn key_warning(&self, ui: &mut egui::Ui) {
        if !self.enabled {
            return;
        }
        let key = match self.listener.key_problem() {
            KeyProblem::Ok => return,
            KeyProblem::PermissionDenied => "key_permission",
            KeyProblem::NoKeyboard => "key_none",
            KeyProblem::Unavailable => "key_unsupported",
        };
        error_line(ui, tr(&self.lang, key));
    }

    fn import_profile(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Profile", &["zip", "json"])
            .add_filter("All", &["*"])
            .pick_file()
        else {
            return;
        };
        match import_profile_pack(&path, &self.paths) {
            Ok(imported) => {
                activate_avatar(
                    &mut self.settings,
                    &imported.profile.id,
                    &self.paths.config_dir,
                );
                let _ = save_settings(&self.paths, &self.settings);
                self.notice = if imported.skipped.is_empty() {
                    imported.profile.name.clone()
                } else {
                    import_notice(&imported.skipped, &self.lang)
                };
            }
            Err(err) => self.notice = present_error(&self.lang, &err),
        }
    }

    fn poll_avatar(&mut self) {
        let mut refresh = false;
        while let Ok(name) = self.avatar_rx.try_recv() {
            self.settings = load_settings(&self.paths);
            self.enabled = as_bool(self.settings.get("enabled"), true);
            self.vrchat_only = resolve_config(&self.settings, &self.paths.config_dir)
                .target
                .enabled;
            if let Some(name) = name {
                self.notice = name;
            }
            refresh = true;
        }
        if refresh {
            self.sync_listener();
        }
    }

    fn poll_tray(&mut self, ctx: &egui::Context) {
        let mut actions = Vec::new();
        if let Some(tray) = &self.tray {
            while let Some(action) = tray.poll() {
                actions.push(action);
            }
        }
        for action in actions {
            match action {
                TrayAction::Open => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayAction::Toggle => {
                    self.enabled = !self.enabled;
                    self.save_flags();
                }
                TrayAction::Quit => {
                    self.allow_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        if let Some(tray) = &self.tray {
            tray.set_labels(&self.lang, self.enabled && self.listener.is_running());
        }
    }

    fn sync_stage_dismiss(&mut self) {
        let held = self.listener.held();
        if self.stage_dismissed.is_some() && self.stage_dismissed != held {
            self.stage_dismissed = None;
        }
    }

    fn stage_index(&self) -> Option<i64> {
        let assign = normalize_assign(self.settings.get("fkey_assign"));
        let held = self.listener.held();
        let from_key = held.as_deref().and_then(|name| {
            if self.stage_dismissed.as_deref() == Some(name) {
                return None;
            }
            let index = assign.get(name).copied().unwrap_or(0);
            if (1..=crate::SLOT_COUNT).contains(&index) {
                Some(index)
            } else {
                None
            }
        });
        from_key.or(self
            .stage_pin
            .filter(|index| (1..=crate::SLOT_COUNT).contains(index)))
    }

    fn show_stage(&mut self, ctx: &egui::Context) {
        let Some(index) = self.stage_index() else {
            return;
        };
        let profile = active_profile(&self.settings, &self.paths.config_dir);
        let Some(card) = stage_card(index, &profile, &self.paths.config_dir) else {
            return;
        };
        let texture = match self.texture_for_path(ctx, &card.thumb) {
            Some(texture) => texture,
            None => self.generic_texture(ctx),
        };
        let name = card.name;
        let lang = self.lang.clone();
        let mut dismiss = false;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("expression-stage"),
            egui::ViewportBuilder::default()
                .with_title(&name)
                .with_fullscreen(true),
            |ctx, _class| {
                if ctx.input(|input| input.viewport().close_requested()) {
                    dismiss = true;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(24.0);
                        ui.label(egui::RichText::new(&name).strong().size(42.0).color(HEADER));
                        ui.add_space(16.0);
                        let side = ui.available_width().clamp(240.0, 640.0);
                        ui.add(
                            egui::Image::new(&texture)
                                .fit_to_exact_size(egui::vec2(side, side))
                                .corner_radius(12.0),
                        );
                        ui.add_space(16.0);
                        if ui.button(tr(&lang, "stage_close")).clicked() {
                            dismiss = true;
                        }
                    });
                });
            },
        );
        if dismiss {
            self.stage_pin = None;
            self.stage_dismissed = self.listener.held();
        }
    }

    fn choice_icon(&mut self, ctx: &egui::Context, choice: &Choice) -> Option<egui::TextureHandle> {
        if !choice.thumb.is_empty() {
            return self.texture_for_path(ctx, &choice.thumb);
        }
        if choice.index == 0 {
            None
        } else {
            Some(self.generic_texture(ctx))
        }
    }

    fn texture_for_path(&mut self, ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
        if path.is_empty() {
            return None;
        }
        if let Some(texture) = self.textures.get(path) {
            return Some(texture.clone());
        }
        let bytes = std::fs::read(path).ok()?;
        let texture = texture_from_png(ctx, path, &bytes)?;
        self.textures.insert(path.to_string(), texture.clone());
        Some(texture)
    }

    fn thumb_texture(
        &mut self,
        ctx: &egui::Context,
        slot: Option<&Slot>,
    ) -> Option<egui::TextureHandle> {
        let slot = slot?;
        if let Some(texture) = self.texture_for_path(ctx, &slot.thumb) {
            return Some(texture);
        }
        Some(self.generic_texture(ctx))
    }

    fn generic_texture(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        if let Some(texture) = &self.generic {
            return texture.clone();
        }
        let texture = ctx.load_texture(
            "generic-expression",
            generic_image(THUMB as usize),
            egui::TextureOptions::LINEAR,
        );
        self.generic = Some(texture.clone());
        texture
    }

    fn capture_ui_shot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.shot.clone() else {
            return;
        };
        let image = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        let Some(image) = image else {
            return;
        };
        if let Err(err) = save_color_image(&path, &image) {
            log_line(&self.paths, &format!("screenshot failed: {err}"));
            std::process::exit(1);
        }
        self.shot = None;
        self.allow_exit = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn note_ui_shot(&mut self, ctx: &egui::Context) {
        if self.shot.is_none() {
            return;
        }
        self.shot_wait = self.shot_wait.saturating_add(1);
        if self.shot_wait == 3 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
    }

    fn header_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.header_icon.is_none() {
            self.header_icon =
                texture_from_png(ctx, "app-header", include_bytes!("../assets/app-48.png"));
        }
        self.header_icon.clone()
    }
}

impl eframe::App for HotkeyApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_zoom(ctx);
        if self.tray.is_none() && !self.tray_failed {
            match Tray::install(ctx.clone(), &self.lang) {
                Ok(tray) => {
                    tray.set_labels(&self.lang, self.enabled);
                    self.tray = Some(tray);
                }
                Err(err) => {
                    self.tray_failed = true;
                    log_line(&self.paths, &format!("tray icon unavailable: {err}"));
                }
            }
        }
        if ctx.input(|input| input.viewport().close_requested())
            && self.tray.is_some()
            && !self.allow_exit
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        self.capture_ui_shot(ctx);
        self.poll_tray(ctx);
        self.poll_avatar();
        self.sync_stage_dismiss();
        if self.shot.is_none() {
            self.show_stage(ctx);
        }
        if self.shot.is_some() {
            ctx.request_repaint();
        }
        let quick = self.stage_index().is_some() || self.listener.held().is_some();
        let wait = if quick { 50 } else { 200 };
        ctx.request_repaint_after(Duration::from_millis(wait));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.painter().rect_filled(ui.max_rect(), 0.0, PANEL);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.contents(ui);
            });
        self.note_ui_shot(ui.ctx());
    }

    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        self.commit_osc();
        self.commit_listen();
        self.listener.stop();
        if let Some(avatar) = &self.avatar {
            avatar.stop();
        }
    }
}

struct Choice {
    label: String,
    index: i64,
    thumb: String,
}

fn choice_labels(lang: &str, profile: &crate::profile::Profile, paths: &AppPaths) -> Vec<Choice> {
    let mut choices = vec![Choice {
        label: tr(lang, "unassigned").to_string(),
        index: 0,
        thumb: String::new(),
    }];
    for expr in &profile.expressions {
        let slot = expression_slot(expr.index, profile, &paths.config_dir);
        let (label, thumb) = slot
            .as_ref()
            .map(|item| (item.label.clone(), item.thumb.clone()))
            .unwrap_or_else(|| (format!("Expression{}", expr.index), String::new()));
        choices.push(Choice {
            label: format!("{}  {label}", expr.index),
            index: expr.index,
            thumb,
        });
    }
    choices
}

/// egui paints only the fonts registered here. It does not consult Meiryo,
/// Yu Gothic, MS Gothic, or Hiragino, so Japanese has to live in this file.
fn ui_font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "noto-sans-jp".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/NotoSansJP-Regular.ttf"
        ))),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "noto-sans-jp".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.push("noto-sans-jp".to_owned());
    }
    fonts
}

fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(ui_font_definitions());
}

fn ui_shot_path() -> Option<std::path::PathBuf> {
    let path = std::env::var_os("VREMOKEY_UI_SHOT")?;
    if path.is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(path))
    }
}

fn save_color_image(path: &std::path::Path, image: &egui::ColorImage) -> Result<(), String> {
    let mut rgba = Vec::with_capacity(image.pixels.len() * 4);
    for pixel in &image.pixels {
        rgba.extend(pixel.to_array());
    }
    save_rgba_png(path, image.width() as u32, image.height() as u32, &rgba)
}

fn save_rgba_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    image::save_buffer(path, rgba, width, height, image::ColorType::Rgba8)
        .map_err(|err| err.to_string())
}

fn install_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = egui::Color32::WHITE;
    visuals.error_fg_color = ERROR_FG;
    visuals.warn_fg_color = ERROR_FG;
    let outline = egui::Stroke::new(1.0, OUTLINE);
    for widgets in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widgets.bg_stroke = outline;
    }
    visuals.selection.bg_fill = PINK;
    visuals.selection.stroke = egui::Stroke::new(2.0, SELECT_TEXT);
    visuals.hyperlink_color = HEADER;
    ctx.set_visuals(visuals);
}

fn error_line(ui: &mut egui::Ui, text: &str) {
    egui::Frame::NONE
        .fill(ERROR_BG)
        .stroke(egui::Stroke::new(1.0, ERROR_FG))
        .inner_margin(egui::Margin::same(8))
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new(text).color(ERROR_FG).size(14.0));
        });
}

fn zoom_label(zoom: f32) -> String {
    format!("{}%", (zoom * 100.0).round() as i32)
}

fn valid_host(host: &str) -> bool {
    let host = host.trim();
    if host.is_empty() || host.len() > 253 || host.starts_with('.') || host.ends_with('.') {
        return false;
    }
    host.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':' | b'[' | b']')
    })
}

fn valid_port(text: &str) -> Option<u16> {
    let text = text.trim();
    if text.is_empty() || (text.len() > 1 && text.starts_with('0')) {
        return None;
    }
    let port: u16 = text.parse().ok()?;
    (1..=65535).contains(&port).then_some(port)
}

fn window_icon() -> Option<egui::IconData> {
    let (rgba, width, height) =
        crate::tray::decode_png(include_bytes!("../assets/app-64.png")).ok()?;
    Some(egui::IconData {
        rgba,
        width,
        height,
    })
}

fn texture_from_png(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture(name, color, egui::TextureOptions::LINEAR))
}

fn generic_image(size: usize) -> egui::ColorImage {
    let mut image =
        egui::ColorImage::filled([size, size], egui::Color32::from_rgb(0x3d, 0x4c, 0x5c));
    let margin = (size / 8).max(2);
    let inner = egui::Color32::from_rgb(0xd7, 0xc4, 0xa3);
    for y in margin..size.saturating_sub(margin) {
        for x in margin..size.saturating_sub(margin) {
            image.pixels[y * size + x] = inner;
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use super::{save_rgba_png, tr, ui_font_definitions, valid_host, valid_port};
    use crate::i18n::{import_notice, present_error};
    use crate::settings::zoom_of;
    use eframe::egui;

    fn preview_lines() -> Vec<String> {
        let mut lines = Vec::new();
        for key in [
            "enable",
            "filter",
            "avatar",
            "import",
            "listen",
            "unassigned",
            "busy",
            "ok",
            "running",
            "stopped",
            "last",
            "tray_open",
            "tray_start",
            "tray_stop",
            "tray_quit",
            "preview",
            "stage_close",
            "subtitle",
            "host",
            "port",
            "language",
            "zoom",
            "port_invalid",
            "host_invalid",
            "key_permission",
            "key_none",
            "key_unsupported",
            "none",
            "import_bad_pack",
            "import_unsafe",
            "import_missing",
        ] {
            lines.push(tr("ja", key).to_string());
        }
        lines.push(import_notice(&[5, 6, 12], "ja"));
        lines.push("日本語アバター".to_string());
        lines.push("1  笑顔".to_string());
        lines.push("2  ウィンク".to_string());
        lines.push("3  驚き".to_string());
        lines.push("4  あいうえお".to_string());
        lines
    }

    #[test]
    fn japanese_copy_is_translated_and_ports_are_checked() {
        assert_ne!(tr("ja", "subtitle"), tr("en", "subtitle"));
        assert!(tr("ja", "subtitle").contains('表'));
        assert_eq!(tr("ja", "host"), "ホスト");
        assert_eq!(tr("ja", "port"), "ポート");
        assert!(tr("ja", "key_permission").contains("usermod"));
        assert!(present_error("ja", "unsafe path in profile pack").contains("安全"));
        assert_eq!(
            present_error("en", "profile.json not found"),
            "profile.json was not found."
        );
        assert_eq!(valid_port("9000"), Some(9000));
        assert_eq!(valid_port("0"), None);
        assert_eq!(valid_port("65536"), None);
        assert_eq!(valid_port("abc"), None);
        assert_eq!(valid_port("09"), None);
        assert!(valid_host("127.0.0.1"));
        assert!(valid_host("localhost"));
        assert!(!valid_host(""));
        assert!(!valid_host("not a host"));
        let mut settings = serde_json::json!({"ui_zoom": 1.5});
        assert!((zoom_of(&settings) - 1.5).abs() < 0.001);
        settings = serde_json::json!({"ui_zoom": 9.0});
        assert!((zoom_of(&settings) - 1.0).abs() < 0.001);
    }

    #[test]
    fn japanese_ui_is_not_tofu() {
        let mut fonts = egui::epaint::text::Fonts::new(
            egui::epaint::text::TextOptions::default(),
            ui_font_definitions(),
        );
        let font_id = egui::FontId::proportional(18.0);
        let ppp = 2.0;
        let lines = preview_lines();
        let (galleys, atlas) = {
            let mut view = fonts.with_pixels_per_point(ppp);
            for line in &lines {
                assert!(view.has_glyphs(&font_id, line), "missing glyph in {line}");
            }
            let kana: String = (0x3041..0x3097)
                .chain(0x30A1..0x30F7)
                .filter_map(char::from_u32)
                .collect();
            assert!(view.has_glyphs(&font_id, &kana), "kana is missing");
            assert!(view.has_glyphs(&font_id, "日本語"));

            let mut galleys = Vec::new();
            for line in lines {
                galleys.push(view.layout_no_wrap(line, font_id.clone(), egui::Color32::BLACK));
            }
            let atlas = view.image();
            (galleys, atlas)
        };

        let (width, height, rgba) = paint_lines(&galleys, &atlas, ppp);
        let dark = rgba.chunks(4).filter(|px| px[0] < 200).count();
        assert!(
            dark > 800,
            "rendered Japanese is blank ({dark} dark pixels)"
        );

        let license = include_str!("../assets/OFL");
        assert!(license.contains("SIL OPEN FONT LICENSE"));
        assert!(license.contains("Noto Sans JP"));

        if let Ok(path) = std::env::var("VREMOKEY_FONT_SHOT") {
            save_rgba_png(std::path::Path::new(&path), width, height, &rgba)
                .unwrap_or_else(|err| panic!("write {path}: {err}"));
        }
    }

    fn paint_lines(
        galleys: &[std::sync::Arc<egui::Galley>],
        atlas: &egui::ColorImage,
        ppp: f32,
    ) -> (u32, u32, Vec<u8>) {
        let margin = 16.0;
        let gap = 8.0;
        let mut width = 360.0_f32;
        let mut y = margin;
        for galley in galleys {
            width = width.max(galley.rect.width() + margin * 2.0);
            y += galley.rect.height() + gap;
        }
        let px_w = (width * ppp).ceil() as u32;
        let px_h = ((y + margin) * ppp).ceil() as u32;
        let mut canvas = Canvas {
            rgba: vec![255_u8; (px_w * px_h * 4) as usize],
            px_w,
            px_h,
            atlas,
            ppp,
        };
        let mut origin_y = margin;
        for galley in galleys {
            canvas.blit_galley(galley, margin, origin_y);
            origin_y += galley.rect.height() + gap;
        }
        (px_w, px_h, canvas.rgba)
    }

    struct Canvas<'a> {
        rgba: Vec<u8>,
        px_w: u32,
        px_h: u32,
        atlas: &'a egui::ColorImage,
        ppp: f32,
    }

    impl Canvas<'_> {
        fn blit_galley(&mut self, galley: &egui::Galley, origin_x: f32, origin_y: f32) {
            for placed in &galley.rows {
                for glyph in &placed.glyphs {
                    let uv = glyph.uv_rect;
                    if uv.is_nothing() {
                        continue;
                    }
                    let left = origin_x + placed.pos.x + glyph.pos.x + uv.offset.x;
                    let top = origin_y + placed.pos.y + glyph.pos.y + uv.offset.y;
                    let dest_x = (left * self.ppp).round() as i32;
                    let dest_y = (top * self.ppp).round() as i32;
                    let src_w = i32::from(uv.max[0]) - i32::from(uv.min[0]);
                    let src_h = i32::from(uv.max[1]) - i32::from(uv.min[1]);
                    for row in 0..src_h {
                        for col in 0..src_w {
                            self.copy_coverage(
                                dest_x + col,
                                dest_y + row,
                                i32::from(uv.min[0]) + col,
                                i32::from(uv.min[1]) + row,
                            );
                        }
                    }
                }
            }
        }

        fn copy_coverage(&mut self, dest_x: i32, dest_y: i32, src_x: i32, src_y: i32) {
            if src_x < 0 || src_y < 0 || dest_x < 0 || dest_y < 0 {
                return;
            }
            let (src_x, src_y) = (src_x as usize, src_y as usize);
            let (dest_x, dest_y) = (dest_x as usize, dest_y as usize);
            if src_x >= self.atlas.width()
                || src_y >= self.atlas.height()
                || dest_x >= self.px_w as usize
                || dest_y >= self.px_h as usize
            {
                return;
            }
            let coverage = self.atlas[(src_x, src_y)].a();
            let ink = 255 - coverage;
            let index = (dest_y * self.px_w as usize + dest_x) * 4;
            self.rgba[index] = ink;
            self.rgba[index + 1] = ink;
            self.rgba[index + 2] = ink;
            self.rgba[index + 3] = 255;
        }
    }
}
