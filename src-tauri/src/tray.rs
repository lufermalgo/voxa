// Tray icon construction and menu event handling.

use tauri::menu::{Menu, Submenu, MenuItem, PredefinedMenuItem, CheckMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};
use sys_locale::get_locale;

use crate::audio;
use crate::db::{self, DbState, SettingsCache};

pub fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let db_state = app.state::<DbState>();
    let (profiles, settings) = {
        let conn_guard = db_state.conn.lock().unwrap();
        let p = db::get_profiles(&conn_guard).map_err(|e| tauri::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        let s = db::get_settings(&conn_guard).map_err(|e| tauri::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        (p, s)
    };

    let sys_lang = get_locale()
        .unwrap_or_else(|| "en".to_string())
        .split('-')
        .next()
        .unwrap_or("en")
        .to_string();
    let is_es = sys_lang == "es";

    let active_profile_id = settings.get("active_profile_id").cloned().unwrap_or_else(|| "1".to_string());
    let current_language  = settings.get("language").cloned().unwrap_or_else(|| "es".to_string());
    let current_mic       = settings.get("mic_id").cloned().unwrap_or_else(|| "auto".to_string());

    let profiles_label = if is_es { "Perfiles" } else { "Profiles" };
    let profiles_menu  = Submenu::with_id(app, "profiles_menu", profiles_label, true)?;
    for profile in profiles {
        let is_checked = profile.id.to_string() == active_profile_id;
        let item = CheckMenuItem::with_id(
            app, format!("profile_{}", profile.id), &profile.name, true, is_checked, None::<&str>,
        )?;
        profiles_menu.append(&item)?;
    }

    let lang_label    = if is_es { "Idioma" } else { "Language" };
    let language_menu = Submenu::with_id(app, "language_menu", lang_label, true)?;
    let lang_es = CheckMenuItem::with_id(app, "lang_es", "Español", true, current_language == "es", None::<&str>)?;
    let lang_en = CheckMenuItem::with_id(app, "lang_en", "English",  true, current_language == "en", None::<&str>)?;
    language_menu.append(&lang_es)?;
    language_menu.append(&lang_en)?;

    let mic_label = if is_es { "Micrófono" } else { "Microphone" };
    let mic_menu  = Submenu::with_id(app, "mic_menu", mic_label, true)?;
    let default_mic_name = audio::get_default_input_device_name().unwrap_or_else(|| "Unknown".to_string());
    let auto_mic_label = if is_es {
        format!("Auto-detectar ({})", default_mic_name)
    } else {
        format!("Auto-detect ({})", default_mic_name)
    };
    let auto_mic_item = CheckMenuItem::with_id(
        app, "mic_auto", &auto_mic_label, true, current_mic == "auto", None::<&str>,
    )?;
    mic_menu.append(&auto_mic_item)?;
    mic_menu.append(&PredefinedMenuItem::separator(app)?)?;
    let mics = audio::get_input_devices()
        .map_err(|e| tauri::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    for mic in mics {
        let item = CheckMenuItem::with_id(
            app, format!("mic_{}", mic.name), &mic.name, true, current_mic == mic.name, None::<&str>,
        )?;
        mic_menu.append(&item)?;
    }

    let icon_bytes = include_bytes!("../icons/tray-icon.png");
    let tray_icon  = tauri::image::Image::from_bytes(icon_bytes)?;
    let settings_label = if is_es { "Configuración..." } else { "Settings..." };
    let quit_label     = if is_es { "Salir de Voxa" }    else { "Quit Voxa" };

    let tray_menu = Menu::with_items(app, &[
        &profiles_menu,
        &mic_menu,
        &language_menu,
        &PredefinedMenuItem::separator(app)?,
        &MenuItem::with_id(app, "settings", settings_label, true, None::<&str>)?,
        &PredefinedMenuItem::separator(app)?,
        &PredefinedMenuItem::quit(app, Some(quit_label))?,
    ])?;

    let profiles_menu_c = profiles_menu.clone();
    let mic_menu_c      = mic_menu.clone();
    let language_menu_c = language_menu.clone();

    let tray = TrayIconBuilder::with_id("main")
        .icon(tray_icon)
        .menu(&tray_menu)
        .on_menu_event(move |app, event| {
            let id = event.id.as_ref();
            if id == "settings" {
                let _ = crate::commands::show_settings(app.clone(), None);
            } else if id.starts_with("profile_") {
                let profile_id = id.replace("profile_", "");
                let db_state = app.state::<DbState>();
                let cache    = app.state::<SettingsCache>();
                if let Ok(conn) = db_state.conn.lock() {
                    let _ = db::update_setting(&conn, "active_profile_id", &profile_id);
                    cache.invalidate("active_profile_id", &profile_id);
                    let _ = app.emit("settings-updated", ());
                }
                if let Ok(items) = profiles_menu_c.items() {
                    for item in items {
                        if let Some(cmi) = item.as_check_menuitem() {
                            let _ = cmi.set_checked(item.id().as_ref() == id);
                        }
                    }
                }
            } else if id.starts_with("mic_") {
                let mic_id   = if id == "mic_auto" { "auto".to_string() } else { id.replace("mic_", "") };
                let db_state = app.state::<DbState>();
                let cache    = app.state::<SettingsCache>();
                if let Ok(conn) = db_state.conn.lock() {
                    let _ = db::update_setting(&conn, "mic_id", &mic_id);
                    cache.invalidate("mic_id", &mic_id);
                    let _ = app.emit("settings-updated", ());
                }
                if let Ok(items) = mic_menu_c.items() {
                    for item in items {
                        if let Some(cmi) = item.as_check_menuitem() {
                            let _ = cmi.set_checked(item.id().as_ref() == id);
                        }
                    }
                }
            } else if id == "lang_es" || id == "lang_en" {
                let lang     = if id == "lang_es" { "es" } else { "en" };
                let db_state = app.state::<DbState>();
                let cache    = app.state::<SettingsCache>();
                if let Ok(conn) = db_state.conn.lock() {
                    let _ = db::update_setting(&conn, "language", lang);
                    cache.invalidate("language", lang);
                    let _ = app.emit("settings-updated", ());
                }
                if let Ok(items) = language_menu_c.items() {
                    for item in items {
                        if let Some(cmi) = item.as_check_menuitem() {
                            let _ = cmi.set_checked(item.id().as_ref() == id);
                        }
                    }
                }
            }
        })
        .build(app)?;

    app.manage(tray);
    Ok(())
}
