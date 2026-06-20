# Clipboard Manager v0.1.0

Release date: June 6, 2026

## Overview

This build focuses on a faster Windows clipboard workflow: the app runs quietly from the system tray, the clipboard list is more compact, copy-back uses a familiar shortcut, and clipboard items can be reviewed with a double click.

## What's New

- Startup with Windows now launches the app directly to the system tray without opening the main window.
- The clipboard list is more compact, similar to Clipdiary: each row shows only a content-type icon and the main clipboard content.
- Double click a clipboard row to review the full content.
- The review dialog is view-only and no longer includes extra action buttons.
- Long review content uses a smaller scrollbar and gives more space to the content.
- Copy-back now uses `Ctrl+C` instead of `Enter`.
- Light and dark mode now follow the Windows system theme automatically.
- The default global shortcut remains `Ctrl+Shift+V` for showing or hiding Clipboard Manager.

## Shortcuts

- `Ctrl+Shift+V`: show or hide Clipboard Manager.
- `Arrow Up` / `Arrow Down`: select an item in clipboard history.
- `Ctrl+C`: copy the selected item back to the clipboard and hide the window.
- `Double click`: review the full item content.
- `Esc`: close the active dialog or hide the window.

## Upgrade Note

If "Start with Windows" was enabled in an older build, disable and enable it once after installing this build. This refreshes the Windows startup entry so the app receives the new startup argument and opens in the tray only.

## App Update Setup

The app now has an updater control in Settings. Before publishing a release, configure the updater backend:

1. Generate a Tauri updater signing key:

```bash
npm run tauri signer generate
```

2. Add the public key and endpoint to `src-tauri/tauri.conf.json`:

```json
"plugins": {
  "updater": {
    "pubkey": "YOUR_TAURI_UPDATER_PUBLIC_KEY",
    "endpoints": [
      "https://github.com/ngthanhvu/clipboard-manager/releases/latest/download/latest.json"
    ],
    "windows": {
      "installMode": "passive"
    }
  }
}
```

3. Keep the private key outside the repository and expose it only during release builds with `TAURI_SIGNING_PRIVATE_KEY`.
4. Upload the generated installer and `latest.json` to the GitHub release.

## Build Check

Verified successfully:

```bash
npm run build
```
