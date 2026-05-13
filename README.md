# Pixel Paint Studio

<p align="center">
  <img src="assets/preview.svg" width="860" alt="Pixel Paint Studio preview">
</p>

<p align="center">
  <a href="https://github.com/noirdaisyq/pixel-paint-studio/releases/latest"><img alt="release" src="https://img.shields.io/github/v/release/noirdaisyq/pixel-paint-studio?label=release"></a>
  <img alt="rust" src="https://img.shields.io/badge/Rust-2021-f97316">
  <img alt="macroquad" src="https://img.shields.io/badge/macroquad-0.4-38bdf8">
  <img alt="export" src="https://img.shields.io/badge/export-PNG-facc15">
</p>

Pixel Paint Studio is a small portfolio-ready pixel-art editor built with Rust and macroquad. It focuses on a polished desktop-tool feel: responsive layout, palette workflow, history stack, shape tools and PNG export.

## Features

- 48x48 pixel canvas with optional grid.
- Brush, eraser, bucket fill, color picker, line and rectangle tools.
- Palette with current-color preview.
- Undo/redo history.
- PNG export to the local `exports/` folder.
- Responsive windowed UI with a neon tool-studio style.

## Run

```powershell
cargo run --release
```

## Controls

```text
B / E / F      brush, eraser, fill
I / L / R      picker, line, rectangle
Ctrl+Z / Y     undo, redo
Ctrl+S         export PNG
G              toggle grid
[ / ]          brush size
Delete         clear canvas
Shift+Rect     filled rectangle
```

## Build

```powershell
cargo build --release
```

The release executable is created at `target/release/pixel-paint-studio.exe`.
