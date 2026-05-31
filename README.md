# picselect 🎬

A high-performance, cinematic desktop application for rapid photo and video ingestion, sorting, and organization. Built with **Tauri v2**, **Rust**, and **Vanilla HTML/CSS/JS**.

---

## ✨ Features

- **Smart Caching (L1 & L2)**: Predictive lookahead preloader reads files into RAM and pre-decodes complex images in the background, keeping navigation instant (0ms transitions) with automatic thread-cancellation ("drop reads") on rapid skipping.
- **Cross-Platform HEIC Scheme**: Custom scheme `picselect-asset://` decodes iPhone `.HEIC` files on-the-fly using pure Rust (streaming as compressed JPEGs on Linux, and native hardware-accelerated HEIC on macOS).
- **Live Photo Pairing**: Automatically groups and plays matched `HEIC`/`JPG` + `MOV` pairs. Hold **Space** to trigger seamless cinematic playback with audio overlay.
- **Visual Operations**:
  - **Keep / Discard**: Press **Enter** to keep or **Delete/D** to discard. Staged files are moved safely to destination folders.
  - **Undo Log**: Full in-memory LIFO transaction undo stack (`CMD+Z` or `U`) to reverse file moves instantly.
  - **Lossless EXIF Rotation**: Rotates media instantly in the UI (`R` / `Shift+R`) and commits orientation metadata losslessly to disk (`W`).
  - **Collision-Safe Renaming**: Premium overlay dialog (`T`) with integrated filename collision checking.
- **Cinematic UI**: Stunning dark-mode visual system complete with ambient color-rich blurred backdrops, glassmorphic panel alignments, and real-time preloading indicators.

---

## 🚀 Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust & Cargo](https://www.rust-lang.org/tools/install)

### Setup & Installation

1. Install frontend dependencies:
   ```bash
   npm install
   ```

2. Start the application in development (dev-server) mode:
   ```bash
   npm run tauri dev
   ```

3. Build the production package:
   ```bash
   npm run tauri build
   ```

---

## ⌨️ Keyboard Shortcuts

- `Arrow Right` / `Down` / `J` — Next media item
- `Arrow Left` / `Up` / `K` — Previous media item
- `Space` *(Hold)* — Play/Loop Live Photo video
- `Enter` — **Keep** (moves file to selected folder)
- `Delete` / `Backspace` / `D` — **Discard** (moves file to deleted folder)
- `CMD + Z` / `U` — **Undo** last keep/discard action
- `R` — Rotate 90° clockwise visually
- `Shift + R` — Rotate 90° counter-clockwise visually
- `W` — Save rotation losslessly to EXIF metadata
- `T` — Rename file safely (collision-checked)
- `,` *(Comma)* — Toggle Project Settings
