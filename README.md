# tauri-shot

A lightweight screenshot app built with Tauri + SvelteKit. Global shortcuts, regional capture, clipboard copy, and export to PNG, JPEG, or WebP.

## Installation

### macOS

1. Open the [Releases](../../releases) page and download the latest `tauri-shot_*.dmg`.
2. Open the dmg and drag `tauri-shot.app` into `Applications`.
3. On **macOS Sequoia (15) and later**, remove the quarantine flag before first launch (the app is not notarized):

```bash
sudo xattr -dr com.apple.quarantine /Applications/tauri-shot.app
```

4. In **System Settings → Privacy & Security**, grant **Screen Recording** and **Accessibility** so capture and global shortcuts work.
5. Launch `tauri-shot.app`. The app stays in the menu bar.

## Usage

- Default global shortcut: `Cmd + Shift + 2` (change it in the panel).
- After capture, drag to select a region; confirm to copy to the clipboard or save as PNG, JPEG, or WebP.
- Closing the main window does not quit the app; use the menu bar tray to reopen or quit.

## Development

Prerequisites: [Node.js](https://nodejs.org/) ≥ 20, [pnpm](https://pnpm.io/), [Rust](https://www.rust-lang.org/), and Tauri’s [system dependencies](https://tauri.app/start/prerequisites/).

```bash
pnpm install
pnpm tauri dev
```

Build:

```bash
pnpm tauri build
```

Bundles are emitted under `src-tauri/target/release/bundle/`.

## Stack

- [Tauri 2](https://tauri.app/)
- [SvelteKit 5](https://svelte.dev/) + TypeScript + Vite
- [`xcap`](https://crates.io/crates/xcap) capture, [`arboard`](https://crates.io/crates/arboard) clipboard, [`image`](https://crates.io/crates/image) encoding

## License

[MIT](LICENSE)
