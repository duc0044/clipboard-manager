use std::{
    borrow::Cow,
    env,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use serde::{Deserialize, Serialize};
use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, Theme, WebviewWindow,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as AutostartExt};
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, GlobalShortcutExt, ShortcutState};

const HISTORY_LIMIT: usize = 100;
const HISTORY_EVENT: &str = "clipboard-history-updated";
const MANAGER_SHOWN_EVENT: &str = "manager-shown";
const SETTINGS_OPENED_EVENT: &str = "settings-opened";
const DEFAULT_SHORTCUT: &str = "Ctrl+Shift+V";
const DEFAULT_THEME: &str = "system";
const START_MINIMIZED_ARG: &str = "--hidden";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Clip {
    id: u64,
    clip_type: String,
    content: String,
    source: String,
    created_at: u64,
    pinned: bool,
    /// Absolute path to the PNG on disk for image clips, served lazily to the
    /// webview via the asset protocol. `None` for text clips. We deliberately
    /// never keep the decoded/base64 image in memory so an idle app with a long
    /// image history stays cheap in both the Rust and webview heaps.
    #[serde(skip_serializing_if = "Option::is_none")]
    image_path: Option<String>,
    /// Cheap fingerprint (dimensions + content hash) used only to de-duplicate
    /// images against existing history. Never sent to the frontend.
    #[serde(skip)]
    image_sig: Option<String>,
}

struct ClipboardState {
    history: Arc<Mutex<Vec<Clip>>>,
    storage_path: PathBuf,
}

struct SettingsState {
    shortcut: Mutex<String>,
    theme: Mutex<String>,
    storage_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AppSettings {
    shortcut: String,
    #[serde(default = "default_theme")]
    theme: String,
}

fn default_theme() -> String {
    DEFAULT_THEME.into()
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// A cheap Win32 counter that bumps on every clipboard change. The watcher
/// compares it each tick and only performs an expensive clipboard read when it
/// moves, so an idle app barely touches the CPU.
#[cfg(windows)]
fn clipboard_sequence() -> u32 {
    unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() }
}

#[cfg(not(windows))]
fn clipboard_sequence() -> u32 {
    0
}

fn classify_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        "link".into()
    } else if trimmed.contains('\n')
        && (trimmed.contains("const ")
            || trimmed.contains("let ")
            || trimmed.contains("fn ")
            || trimmed.contains("function ")
            || trimmed.contains("class ")
            || trimmed.contains("import ")
            || trimmed.contains("def ")
            || trimmed.contains("=>")
            || trimmed.contains("();")
            || trimmed.contains(") {"))
    {
        "code".into()
    } else {
        "text".into()
    }
}

/// On-disk representation of a clip. Images are stored as separate PNG files so
/// the history JSON stays small; `image_file` holds just the file name. The
/// legacy `image` field (an inline data URL) is still read for backwards
/// compatibility and migrated to a file on the next persist.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredClip {
    id: u64,
    clip_type: String,
    content: String,
    #[serde(default)]
    source: String,
    created_at: u64,
    pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image_sig: Option<String>,
}

fn image_dir(path: &Path) -> PathBuf {
    path.parent()
        .map(|parent| parent.join("images"))
        .unwrap_or_else(|| PathBuf::from("images"))
}

fn data_url_to_bytes(data_url: &str) -> Option<Vec<u8>> {
    let encoded = data_url.split_once(',').map(|(_, value)| value)?;
    STANDARD.decode(encoded).ok()
}

fn persist(path: &Path, history: &[Clip]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let dir = image_dir(path);
    let mut referenced = std::collections::HashSet::new();
    let mut stored = Vec::with_capacity(history.len());

    for clip in history {
        // Image bytes are written to disk when the clip is created (or migrated
        // on load), so here we only record the file name that points at them.
        let image_file = clip.image_path.as_ref().map(|_| {
            let file_name = format!("{}.png", clip.id);
            referenced.insert(file_name.clone());
            file_name
        });
        stored.push(StoredClip {
            id: clip.id,
            clip_type: clip.clip_type.clone(),
            content: clip.content.clone(),
            source: clip.source.clone(),
            created_at: clip.created_at,
            pinned: clip.pinned,
            image: None,
            image_file,
            image_sig: clip.image_sig.clone(),
        });
    }

    let json = serde_json::to_vec_pretty(&stored).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| error.to_string())?;

    // Drop image files that no longer belong to any clip (deleted / trimmed).
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let keep = entry
                .file_name()
                .to_str()
                .map(|name| referenced.contains(name))
                .unwrap_or(false);
            if !keep {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    Ok(())
}

fn load_history(path: &Path) -> Vec<Clip> {
    let dir = image_dir(path);
    let stored: Vec<StoredClip> = fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();

    stored
        .into_iter()
        .map(|clip| {
            // Resolve the on-disk PNG to an absolute path the webview can load
            // directly. Never read the bytes here — that is what kept the whole
            // image history resident in memory before.
            let image_path = match &clip.image_file {
                Some(file) => {
                    let path = dir.join(file);
                    path.exists().then(|| path.to_string_lossy().into_owned())
                }
                // Legacy histories stored the image inline as a base64 data URL.
                // Migrate it to a file once so newer code only deals with paths.
                None => clip.image.as_deref().and_then(|data_url| {
                    let bytes = data_url_to_bytes(data_url)?;
                    let path = dir.join(format!("{}.png", clip.id));
                    fs::create_dir_all(&dir).ok()?;
                    fs::write(&path, &bytes).ok()?;
                    Some(path.to_string_lossy().into_owned())
                }),
            };
            Clip {
                id: clip.id,
                clip_type: clip.clip_type,
                content: clip.content,
                source: clip.source,
                created_at: clip.created_at,
                pinned: clip.pinned,
                image_path,
                image_sig: clip.image_sig,
            }
        })
        .collect()
}

fn load_settings(path: &Path) -> AppSettings {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| AppSettings {
            shortcut: DEFAULT_SHORTCUT.into(),
            theme: DEFAULT_THEME.into(),
        })
}

fn theme_from_preference(preference: &str) -> Option<Theme> {
    match preference {
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None,
    }
}

fn apply_window_theme(app: &AppHandle, preference: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_theme(theme_from_preference(preference));
    }
}

fn persist_settings(path: &Path, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| error.to_string())
}

fn show_manager(app: &AppHandle, open_settings: bool) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit(
            if open_settings {
                SETTINGS_OPENED_EVENT
            } else {
                MANAGER_SHOWN_EVENT
            },
            (),
        );
    }
}

fn is_start_minimized_launch() -> bool {
    env::args().any(|arg| arg == START_MINIMIZED_ARG)
}

fn trim_history(history: &mut Vec<Clip>) {
    let mut unpinned_seen = 0;
    history.retain(|clip| {
        if clip.pinned {
            true
        } else {
            unpinned_seen += 1;
            unpinned_seen <= HISTORY_LIMIT
        }
    });
}

fn add_clip(
    app: &AppHandle,
    history: &Arc<Mutex<Vec<Clip>>>,
    storage_path: &Path,
    clip: Clip,
    image_bytes: Option<Vec<u8>>,
) {
    // Only the visible window renders history, so when it is hidden we skip both
    // cloning the snapshot and the IPC emit. The UI re-fetches the history when
    // it is shown again.
    let visible = app
        .get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);

    let is_dup = |item: &Clip| {
        item.clip_type == clip.clip_type
            && item.content == clip.content
            && item.image_sig == clip.image_sig
    };

    let snapshot = {
        let mut items = history.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if items.first().is_some_and(&is_dup) {
            return;
        }
        let existing = items.iter().position(&is_dup);
        let mut clip = clip;
        if let Some(index) = existing {
            clip.pinned = items[index].pinned;
            items.remove(index);
        }
        // Persist the image bytes to disk now that we know the clip is kept; the
        // in-memory clip only ever carries the resulting path.
        if let Some(bytes) = image_bytes {
            let dir = image_dir(storage_path);
            let path = dir.join(format!("{}.png", clip.id));
            if fs::create_dir_all(&dir).is_ok() && fs::write(&path, &bytes).is_ok() {
                clip.image_path = Some(path.to_string_lossy().into_owned());
            }
        }
        items.insert(0, clip);
        trim_history(&mut items);
        let _ = persist(storage_path, &items);
        if visible {
            Some(items.clone())
        } else {
            None
        }
    };

    if let Some(snapshot) = snapshot {
        let _ = app.emit(HISTORY_EVENT, snapshot);
    }
}

fn encode_image_png(image: ImageData<'_>) -> Result<Vec<u8>, String> {
    let buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(
        image.width as u32,
        image.height as u32,
        image.bytes.into_owned(),
    )
    .ok_or("Clipboard image has invalid RGBA data")?;
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(buffer)
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(png.into_inner())
}

fn decode_image_file(path: &str) -> Result<ImageData<'static>, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let image = image::load_from_memory(&bytes)
        .map_err(|error| error.to_string())?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Ok(ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(image.into_raw()),
    })
}

fn start_clipboard_watcher(
    app: AppHandle,
    history: Arc<Mutex<Vec<Clip>>>,
    next_id: Arc<AtomicU64>,
    storage_path: PathBuf,
) {
    thread::spawn(move || {
        let Ok(mut clipboard) = Clipboard::new() else {
            return;
        };
        let mut last_seen = String::new();
        // Start one behind the current value so the first tick captures whatever
        // is already on the clipboard, matching the old eager behaviour.
        #[cfg(windows)]
        let mut last_sequence = clipboard_sequence().wrapping_sub(1);

        loop {
            #[cfg(windows)]
            {
                let sequence = clipboard_sequence();
                if sequence == last_sequence {
                    thread::sleep(Duration::from_millis(150));
                    continue;
                }
                last_sequence = sequence;
            }

            if let Ok(text) = clipboard.get_text() {
                let signature = format!("text:{text}");
                if !text.trim().is_empty() && signature != last_seen {
                    last_seen = signature;
                    add_clip(
                        &app,
                        &history,
                        &storage_path,
                        Clip {
                            id: next_id.fetch_add(1, Ordering::Relaxed),
                            clip_type: classify_text(&text),
                            content: text,
                            source: "Clipboard hệ thống".into(),
                            created_at: timestamp_ms(),
                            pinned: false,
                            image_path: None,
                            image_sig: None,
                        },
                        None,
                    );
                }
            } else if let Ok(image) = clipboard.get_image() {
                let signature = (
                    image.width,
                    image.height,
                    image.bytes.iter().take(4096).fold(0_u64, |hash, byte| {
                        hash.wrapping_mul(31).wrapping_add(*byte as u64)
                    }),
                );
                let signature = format!("image:{}:{}:{}", signature.0, signature.1, signature.2);
                if signature != last_seen {
                    last_seen = signature.clone();
                    if let Ok(png) = encode_image_png(image) {
                        add_clip(
                            &app,
                            &history,
                            &storage_path,
                            Clip {
                                id: next_id.fetch_add(1, Ordering::Relaxed),
                                clip_type: "image".into(),
                                content: "Hình ảnh từ clipboard".into(),
                                source: "Clipboard hệ thống".into(),
                                created_at: timestamp_ms(),
                                pinned: false,
                                image_path: None,
                                image_sig: Some(signature),
                            },
                            Some(png),
                        );
                    }
                }
            }
            #[cfg(windows)]
            thread::sleep(Duration::from_millis(150));
            #[cfg(not(windows))]
            thread::sleep(Duration::from_millis(500));
        }
    });
}

#[tauri::command]
fn get_history(state: State<'_, ClipboardState>) -> Vec<Clip> {
    state
        .history
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

#[tauri::command]
fn copy_clip(id: u64, state: State<'_, ClipboardState>) -> Result<(), String> {
    let clip = state
        .history
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .find(|clip| clip.id == id)
        .cloned()
        .ok_or("Không tìm thấy mục clipboard")?;

    let image = match clip.image_path.as_deref() {
        Some(path) => Some(decode_image_file(path)?),
        None => None,
    };

    // On Windows the clipboard can only be held by one thread at a time, so
    // arboard intermittently fails to open it when our own watcher thread (or
    // another app) holds it for a moment (os error 1418). Retry briefly before
    // giving up instead of failing the whole copy.
    let mut last_error = String::from("Không mở được clipboard");
    for _ in 0..12 {
        match Clipboard::new() {
            Ok(mut clipboard) => {
                let result = match &image {
                    Some(img) => clipboard.set_image(ImageData {
                        width: img.width,
                        height: img.height,
                        bytes: Cow::Borrowed(img.bytes.as_ref()),
                    }),
                    None => clipboard.set_text(clip.content.clone()),
                };
                match result {
                    Ok(()) => return Ok(()),
                    Err(error) => last_error = error.to_string(),
                }
            }
            Err(error) => last_error = error.to_string(),
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(last_error)
}

fn update_history<F>(state: &ClipboardState, update: F) -> Result<Vec<Clip>, String>
where
    F: FnOnce(&mut Vec<Clip>),
{
    let mut history = state.history.lock().map_err(|error| error.to_string())?;
    update(&mut history);
    persist(&state.storage_path, &history)?;
    Ok(history.clone())
}

#[tauri::command]
fn toggle_pin(id: u64, state: State<'_, ClipboardState>) -> Result<Vec<Clip>, String> {
    update_history(&state, |history| {
        if let Some(clip) = history.iter_mut().find(|clip| clip.id == id) {
            clip.pinned = !clip.pinned;
        }
    })
}

#[tauri::command]
fn delete_clip(id: u64, state: State<'_, ClipboardState>) -> Result<Vec<Clip>, String> {
    update_history(&state, |history| history.retain(|clip| clip.id != id))
}

#[tauri::command]
fn clear_unpinned(state: State<'_, ClipboardState>) -> Result<Vec<Clip>, String> {
    update_history(&state, |history| history.retain(|clip| clip.pinned))
}

#[tauri::command]
fn hide_window(window: WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn get_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable().map_err(|error| error.to_string())?;
    } else {
        autostart.disable().map_err(|error| error.to_string())?;
    }
    autostart.is_enabled().map_err(|error| error.to_string())
}

#[tauri::command]
fn get_shortcut(state: State<'_, SettingsState>) -> String {
    state
        .shortcut
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

#[tauri::command]
fn set_shortcut(
    shortcut: String,
    app: AppHandle,
    state: State<'_, SettingsState>,
) -> Result<(), String> {
    let shortcut = shortcut.trim().to_string();
    if shortcut.is_empty() {
        return Err("Phím tắt không được để trống".into());
    }
    if !shortcut.contains('+') {
        return Err("Phím tắt phải gồm ít nhất một phím bổ trợ".into());
    }

    let mut current = state.shortcut.lock().map_err(|error| error.to_string())?;
    if *current == shortcut {
        return Ok(());
    }

    app.global_shortcut()
        .register(shortcut.as_str())
        .map_err(|error| format!("Không thể đăng ký phím tắt: {error}"))?;
    let _ = app.global_shortcut().unregister(current.as_str());

    let theme = state.theme.lock().map_err(|error| error.to_string())?.clone();
    let settings = AppSettings {
        shortcut: shortcut.clone(),
        theme,
    };
    persist_settings(&state.storage_path, &settings)?;
    *current = shortcut;
    Ok(())
}

#[tauri::command]
fn get_theme(state: State<'_, SettingsState>) -> String {
    state
        .theme
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

#[tauri::command]
fn set_theme(
    theme: String,
    app: AppHandle,
    state: State<'_, SettingsState>,
) -> Result<String, String> {
    let theme = theme.trim().to_string();
    if !matches!(theme.as_str(), "system" | "light" | "dark") {
        return Err("Chế độ giao diện không hợp lệ".into());
    }

    let shortcut = state.shortcut.lock().map_err(|error| error.to_string())?.clone();
    let settings = AppSettings {
        shortcut,
        theme: theme.clone(),
    };
    persist_settings(&state.storage_path, &settings)?;

    let mut current = state.theme.lock().map_err(|error| error.to_string())?;
    *current = theme.clone();
    apply_window_theme(&app, &theme);
    Ok(theme)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![START_MINIMIZED_ARG]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            ShortcutBuilder::new()
                .with_handler(|app, _, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            show_manager(app, false);
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let storage_path = app_data.join("clipboard-history.json");
            let history = Arc::new(Mutex::new(load_history(&storage_path)));
            let next_id = Arc::new(AtomicU64::new(
                history
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .iter()
                    .map(|clip| clip.id)
                    .max()
                    .map_or(1, |max| max + 1),
            ));
            app.manage(ClipboardState {
                history: history.clone(),
                storage_path: storage_path.clone(),
            });
            start_clipboard_watcher(app.handle().clone(), history, next_id, storage_path);

            let settings_path = app_data.join("settings.json");
            let settings = load_settings(&settings_path);
            let shortcut = if app
                .global_shortcut()
                .register(settings.shortcut.as_str())
                .is_ok()
            {
                settings.shortcut
            } else {
                app.global_shortcut().register(DEFAULT_SHORTCUT)?;
                DEFAULT_SHORTCUT.into()
            };
            apply_window_theme(app.handle(), &settings.theme);
            app.manage(SettingsState {
                shortcut: Mutex::new(shortcut),
                theme: Mutex::new(settings.theme),
                storage_path: settings_path,
            });

            let menu = MenuBuilder::new(app)
                .text("show", "Mở Clipboard")
                .text("settings", "Cài đặt phím tắt")
                .separator()
                .text("quit", "Thoát")
                .build()?;
            TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .expect("application icon missing"),
                )
                .tooltip("Clipboard Manager")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_manager(app, false),
                    "settings" => show_manager(app, true),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_manager(tray.app_handle(), false);
                    }
                })
                .build(app)?;
            if !is_start_minimized_launch() {
                show_manager(app.handle(), false);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_history,
            copy_clip,
            toggle_pin,
            delete_clip,
            clear_unpinned,
            hide_window,
            get_autostart_enabled,
            set_autostart_enabled,
            get_shortcut,
            set_shortcut,
            get_theme,
            set_theme
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
