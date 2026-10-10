//! Assignment window. F1 through F4, each with a thumbnail and a name.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui;
use serde_json::{Map, Value};

use crate::focus::as_bool;
use crate::i18n::{import_notice, tr};
use crate::listener::{AvatarOscListener, ExpressionListener};
use crate::paths::{log_line, AppPaths};
use crate::profile::{
    activate_avatar, active_profile, expression_slot, import_profile_pack, list_profiles, Slot,
};
use crate::settings::{
    avatar_listen_port, lang_of, load_settings, normalize_assign, resolve_config, save_settings,
};
use crate::tray::{Tray, TrayAction};
use crate::{DISPLAY_NAME, FKEY_NAMES};

const PINK: egui::Color32 = egui::Color32::from_rgb(255, 67, 151);
const THUMB: f32 = 56.0;

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
}

impl HotkeyApp {
    fn new(cc: &eframe::CreationContext<'_>, paths: AppPaths) -> Self {
        install_fonts(&cc.egui_ctx);
        install_theme(&cc.egui_ctx);
        let settings = load_settings(&paths);
        let lang = lang_of(&settings);
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
        };
        app.sync_avatar();
        app
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(255, 247, 250))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                self.header(ui);
                ui.add_space(8.0);
                self.options(ui);
                ui.add_space(6.0);
                self.status_line(ui);
                ui.add_space(8.0);
                self.rows(ui);
            });
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let icon = self.header_texture(ui.ctx());
        egui::Frame::NONE
            .fill(PINK)
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
                                .color(egui::Color32::from_rgb(255, 230, 240))
                                .size(13.0),
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
        ui.horizontal(|ui| {
            ui.label("Host:");
            let host = ui.add(egui::TextEdit::singleline(&mut self.host).desired_width(140.0));
            ui.label("Port:");
            let port = ui.add(egui::TextEdit::singleline(&mut self.port).desired_width(64.0));
            ui.label(tr(&lang, "listen"));
            let listen =
                ui.add(egui::TextEdit::singleline(&mut self.listen_port).desired_width(64.0));
            if host.changed() || port.changed() {
                self.save_osc();
            }
            if listen.changed() {
                self.save_listen();
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr(&lang, "avatar"));
            self.avatar_box(ui);
            if ui.button(tr(&lang, "import")).clicked() {
                self.import_profile();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("EN/JA").clicked() {
                    self.toggle_lang();
                }
            });
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
        let (labels, indexes) = choice_labels(&lang, &profile, &self.paths);
        let assign = normalize_assign(self.settings.get("fkey_assign"));
        for name in FKEY_NAMES {
            let chosen = assign.get(name).copied().unwrap_or(0);
            let slot = if chosen == 0 {
                None
            } else {
                expression_slot(chosen, &profile, &self.paths.config_dir)
            };
            let thumb = self.thumb_texture(ui.ctx(), slot.as_ref());
            let position = indexes
                .iter()
                .position(|index| *index == chosen)
                .unwrap_or(0);
            let shown = labels.get(position).cloned().unwrap_or_default();
            let mut picked = position;
            egui::Frame::NONE
                .fill(egui::Color32::WHITE)
                .inner_margin(egui::Margin::same(6))
                .corner_radius(8.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
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
                            .width(360.0)
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for (index, label) in labels.iter().enumerate() {
                                    ui.selectable_value(&mut picked, index, label);
                                }
                            });
                    });
                });
            ui.add_space(4.0);
            if picked != position {
                let index = indexes.get(picked).copied().unwrap_or(0);
                self.set_assign(name, index);
            }
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
                "-".to_string()
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

    fn save_osc(&mut self) {
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
        let host = self.host.trim();
        osc.insert(
            "host".into(),
            Value::String(if host.is_empty() {
                "127.0.0.1".into()
            } else {
                host.to_string()
            }),
        );
        let Ok(port) = self.port.trim().parse::<i64>() else {
            return;
        };
        osc.insert("port".into(), Value::from(port));
        let _ = save_settings(&self.paths, &self.settings);
    }

    fn save_listen(&mut self) {
        let Ok(port) = self.listen_port.trim().parse::<i64>() else {
            return;
        };
        if !(1..=65535).contains(&port) {
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

    fn toggle_lang(&mut self) {
        self.lang = if self.lang == "ja" {
            "en".into()
        } else {
            "ja".into()
        };
        if let Some(obj) = self.settings.as_object_mut() {
            obj.insert("lang".into(), Value::String(self.lang.clone()));
        }
        let _ = save_settings(&self.paths, &self.settings);
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
            Err(err) => self.notice = err,
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

    fn thumb_texture(
        &mut self,
        ctx: &egui::Context,
        slot: Option<&Slot>,
    ) -> Option<egui::TextureHandle> {
        let slot = slot?;
        if !slot.thumb.is_empty() {
            if let Some(texture) = self.textures.get(&slot.thumb) {
                return Some(texture.clone());
            }
            if let Ok(bytes) = std::fs::read(&slot.thumb) {
                if let Some(texture) = texture_from_png(ctx, &slot.thumb, &bytes) {
                    self.textures.insert(slot.thumb.clone(), texture.clone());
                    return Some(texture);
                }
            }
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
        self.poll_tray(ctx);
        self.poll_avatar();
        ctx.request_repaint_after(Duration::from_millis(200));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.contents(ui);
    }

    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        self.listener.stop();
        if let Some(avatar) = &self.avatar {
            avatar.stop();
        }
    }
}

fn choice_labels(
    lang: &str,
    profile: &crate::profile::Profile,
    paths: &AppPaths,
) -> (Vec<String>, Vec<i64>) {
    let mut labels = vec![tr(lang, "unassigned").to_string()];
    let mut indexes = vec![0];
    for expr in &profile.expressions {
        let slot = expression_slot(expr.index, profile, &paths.config_dir);
        let label = slot
            .as_ref()
            .map(|item| item.label.clone())
            .unwrap_or_else(|| format!("Expression{}", expr.index));
        labels.push(format!("{}  {label}", expr.index));
        indexes.push(expr.index);
    }
    (labels, indexes)
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "noto".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/NotoSansCJKjp-UI.otf"
        ))),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "noto".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.push("noto".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn install_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = egui::Color32::from_rgb(255, 247, 250);
    visuals.window_fill = visuals.panel_fill;
    visuals.selection.bg_fill = PINK;
    visuals.selection.stroke.color = PINK;
    visuals.hyperlink_color = PINK;
    ctx.set_visuals(visuals);
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
