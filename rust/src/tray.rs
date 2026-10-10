//! System tray. Closing the window hides it; Quit on this menu exits.

use std::sync::mpsc;

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::i18n::tr;
use crate::DISPLAY_NAME;

pub enum TrayAction {
    Open,
    Toggle,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    open_item: MenuItem,
    toggle_item: MenuItem,
    quit_item: MenuItem,
    rx: mpsc::Receiver<TrayAction>,
}

impl Tray {
    pub fn install(ctx: egui::Context, lang: &str) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let ctx_for_menu = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let id = event.id().as_ref();
            let action = match id {
                "open" => TrayAction::Open,
                "toggle" => TrayAction::Toggle,
                "quit" => TrayAction::Quit,
                _ => return,
            };
            let _ = tx.send(action);
            ctx_for_menu.request_repaint();
        }));

        let open_item = MenuItem::with_id("open", tr(lang, "tray_open"), true, None);
        let toggle_item = MenuItem::with_id("toggle", tr(lang, "tray_stop"), true, None);
        let quit_item = MenuItem::with_id("quit", tr(lang, "tray_quit"), true, None);
        let menu = Menu::new();
        menu.append(&open_item).map_err(|err| err.to_string())?;
        menu.append(&toggle_item).map_err(|err| err.to_string())?;
        menu.append(&quit_item).map_err(|err| err.to_string())?;

        let icon = tray_icon_image()?;
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(DISPLAY_NAME)
            .with_icon(icon)
            .build()
            .map_err(|err| err.to_string())?;
        Ok(Self {
            _icon: tray_icon,
            open_item,
            toggle_item,
            quit_item,
            rx,
        })
    }

    pub fn poll(&self) -> Option<TrayAction> {
        self.rx.try_recv().ok()
    }

    pub fn set_labels(&self, lang: &str, listening: bool) {
        self.open_item.set_text(tr(lang, "tray_open"));
        let toggle = if listening { "tray_stop" } else { "tray_start" };
        self.toggle_item.set_text(tr(lang, toggle));
        self.quit_item.set_text(tr(lang, "tray_quit"));
    }
}

fn tray_icon_image() -> Result<Icon, String> {
    let (rgba, width, height) = decode_png(include_bytes!("../assets/tray-64.png"))?;
    Icon::from_rgba(rgba, width, height).map_err(|err| err.to_string())
}

pub fn decode_png(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let image = image::load_from_memory(bytes)
        .map_err(|err| err.to_string())?
        .to_rgba8();
    let (width, height) = image.dimensions();
    Ok((image.into_raw(), width, height))
}
