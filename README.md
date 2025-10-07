# Hex War

Hex War is a browser-based hex grid arena where two teams of glossy balls collide, flip territory, and race for points. It is written in Rust, compiled to WebAssembly, and rendered through the HTML5 canvas API.

<img width="1229" height="1022" alt="image" src="https://github.com/user-attachments/assets/568e3114-26c7-49f5-a69b-918d232158e4" />

## Play the bundled build
1. From the repository root run a static file server, for example:
   ```bash
   python3 -m http.server 8080
   ```
2. Visit <http://localhost:8080/www/>. The precompiled Wasm bundle in `pkg/` sits alongside `www/`, so serving the repo root keeps the import path `../pkg/hex_war.js` working.

> ℹ️ Browsers require Wasm files to be served over HTTP(S), so opening the HTML file directly from the filesystem will not work.

If your static server only exposes a single directory (for example `python3 -m http.server --directory www 8080`), make sure `pkg/` is also inside that directory or adjust the import path in `www/index.html` accordingly.

## Build the Wasm bundle yourself
```bash
rustup target add wasm32-unknown-unknown
wasm-pack build --target web --release
```
The generated JavaScript bindings and `.wasm` binary land in `pkg/`. Reload the page in your browser to pick up the fresh build.

## Project layout
- `src/` – Rust source for the simulation and rendering logic.
- `www/index.html` – UI shell that wires up controls, canvas, and the Wasm module.
- `www/styles.css` – Standalone styling for the scoreboard, controls, and stage.
- `pkg/` – Prebuilt WebAssembly bundle produced by `wasm-pack` (ready to deploy).

## Customising or extending
- Tweak colours, layout, or typography in `www/styles.css`.
- Adjust gameplay parameters (grid size, speed curves, ball visuals) inside `src/lib.rs`.
- Rebuild with `wasm-pack build --target web --release` to ship your changes.

## Deploying elsewhere
Any static host (GitHub Pages, Netlify, Vercel, S3, etc.) can serve the `/www` and `/pkg` directories. Upload both directories as-is so `index.html` can resolve the `pkg/hex_war.js` loader and `hex_war_bg.wasm` binary.

