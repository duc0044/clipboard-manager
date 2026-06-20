# Clipboard Manager

A small Windows-style clipboard history app built with Tauri, React, TypeScript, and Fluent UI 2.

The app runs as a compact desktop window, keeps listening in the background, stores copied text and images locally, and can be opened quickly with a global shortcut or from the system tray.

## Features

- Clipboard history for text, links, code snippets, and images
- Local persistence across app restarts
- Compact fixed-size window designed for quick access
- Windows system tray icon with quick actions
- Global shortcut to show or hide the manager
- Keyboard navigation for fast copy-back
- Pin, delete, and clear unpinned clipboard items
- Light and dark theme toggle
- Optional startup with Windows
- Fluent UI 2 interface

## Shortcuts

Default global shortcut:

```text
Ctrl + Shift + V
```

Inside the manager:

```text
Arrow Up / Arrow Down  Select an item
Ctrl + C               Copy the selected item and hide the window
Double click           Review the full item content
Esc                    Hide the window or close the active dialog
```

The global shortcut can be changed from the settings dialog.

## System Tray

The app keeps running in the background when the window is closed.

Tray actions:

- Left click: open Clipboard Manager
- Right click: open the tray menu
- Tray menu: open manager, open settings, or quit the app

## Settings

The settings dialog supports:

- Changing the global shortcut
- Enabling or disabling startup with Windows

Startup with Windows is handled through the Tauri autostart plugin.

## Tech Stack

- Tauri 2
- React 19
- TypeScript
- Vite
- Fluent UI 2
- Rust backend
- `arboard` for clipboard access
- `tauri-plugin-global-shortcut`
- `tauri-plugin-autostart`

## Development

Install dependencies:

```bash
npm install
```

Run the Tauri app in development mode:

```bash
npm run tauri dev
```

Build the frontend:

```bash
npm run build
```

Check the Rust backend:

```bash
cd src-tauri
cargo check
```

Run Rust tests:

```bash
cd src-tauri
cargo test
```

## Build

Create a production desktop build:

```bash
npm run tauri build
```

## Project Structure

```text
src/
  App.tsx        React UI and app interactions
  App.css        Fluent-style layout and compact window styling
  main.tsx       React entry point

src-tauri/
  src/lib.rs     Clipboard watcher, commands, tray, shortcuts, autostart
  src/main.rs    Tauri app entry point
  icons/         App and tray icon assets
  tauri.conf.json
```

## Notes

- Clipboard history is stored in the app data directory as JSON.
- Images are stored as separate PNG files under an `images` folder next to the history JSON; the JSON only references the file name. Older histories that still embed inline data URLs are migrated automatically on first save.
- The window is intentionally fixed at `400x650` and cannot be maximized.
- In development mode, enabling autostart registers the current debug executable. In an installed build, it registers the installed app executable.
