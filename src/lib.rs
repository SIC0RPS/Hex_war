use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;

use js_sys::Math;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{window, CanvasRenderingContext2d, HtmlCanvasElement, Element};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Team { Black, White }

#[derive(Clone, Copy)]
struct Ball {
    x: f64, y: f64,
    vx: f64, vy: f64,
    team: Team,
    radius: f64,
    base_speed: f64,
    last_bounce_ts: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HexColor { Black, White }

struct Cell {
    col: usize, row: usize,
    cx: f64, cy: f64,
    color: HexColor,
}

struct Grid {
    cells: Vec<Cell>,
    cols: usize,
    rows: usize,
    r: f64,
    hex_h: f64, // vertical step (flat-top)
}

impl Ball {
    fn maintain_speed(&mut self) {
        let mag = (self.vx * self.vx + self.vy * self.vy).sqrt();
        if mag > 1e-6 {
            let scale = self.base_speed / mag;
            self.vx *= scale;
            self.vy *= scale;
        }
    }
}

const TEAM_BOOST: f64 = 1.12;
const MAX_BASE_SPEED: f64 = 520.0;

struct App {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    dpr: f64,
    css_w: f64, css_h: f64,

    grid: Grid,
    balls: Vec<Ball>,

    running: bool,
    last_ts: f64,
    speed_mul: f64,

    // Points (flip-based scoring)
    points_white: usize,
    points_black: usize,
    points_white_el: Option<Element>,
    points_black_el: Option<Element>,
    points_dirty: bool,

    raf_handle: Option<Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>>,
}

thread_local! { static APP: RefCell<Option<App>> = RefCell::new(None); }

fn js_err(msg: &str) -> JsValue { JsValue::from_str(msg) }
fn rand_range(min: f64, max: f64) -> f64 { min + (max - min) * Math::random() }

impl Grid {
    fn new(css_w: f64, css_h: f64, r: f64) -> Grid {
        let hex_h = (3.0f64).sqrt() * r;
        let step_x = 1.5 * r;

        // columns
        let mut cols = 0usize; let mut x = r;
        while x + r <= css_w - 1.0 { cols += 1; x += step_x; }
        if cols == 0 { cols = 1; }

        // rows (min of even/odd columns)
        let mut rows_even = 0usize; let mut y_even = hex_h / 2.0;
        while y_even + hex_h / 2.0 <= css_h - 1.0 { rows_even += 1; y_even += hex_h; }
        let mut rows_odd = 0usize; let mut y_odd = hex_h;
        while y_odd + hex_h / 2.0 <= css_h - 1.0 { rows_odd += 1; y_odd += hex_h; }
        let rows = rows_even.min(rows_odd).max(1);

        let mut cells = Vec::with_capacity(cols * rows);
        let mid_x = css_w * 0.5;
        for col in 0..cols {
            let cx = r + (col as f64) * step_x;
            let offset_y = if col % 2 == 0 { 0.0 } else { hex_h / 2.0 };
            for row in 0..rows {
                let cy = hex_h / 2.0 + offset_y + (row as f64) * hex_h;
                let color = if cx < mid_x { HexColor::White } else { HexColor::Black };
                cells.push(Cell { col, row, cx, cy, color });
            }
        }
        Grid { cells, cols, rows, r, hex_h }
    }

    #[inline]
    fn center_to_index(&self, x: f64, y: f64) -> Option<usize> {
        let step_x = 1.5 * self.r;
        let hex_h = self.hex_h;

        let col = ((x - self.r) / step_x).round() as isize;
        if col < 0 || col >= self.cols as isize { return None; }
        let col_us = col as usize;

        let offset = if col_us % 2 == 0 { 0.0 } else { hex_h / 2.0 };
        let row = ((y - hex_h / 2.0 - offset) / hex_h).round() as isize;
        if row < 0 || row >= self.rows as isize { return None; }
        let row_us = row as usize;

        Some(col_us * self.rows + row_us)
    }

    /// Set color at (x,y) to team color; return (old,new) if changed.
    fn flip_at(&mut self, x: f64, y: f64, team: Team) -> Option<(HexColor, HexColor)> {
        if let Some(i) = self.center_to_index(x, y) {
            let c = &mut self.cells[i];
            let new = match team { Team::Black => HexColor::Black, Team::White => HexColor::White };
            if c.color != new { let old = c.color; c.color = new; return Some((old, new)); }
        }
        None
    }

    /// Claim every hex within `radius` of `(x,y)`; returns awarded points (white, black) and bounce normal.
    fn flip_disc(&mut self, x: f64, y: f64, radius: f64, team: Team) -> (usize, usize, Option<(f64, f64)>) {
        let target = match team { Team::Black => HexColor::Black, Team::White => HexColor::White };
        let mut white_pts = 0usize;
        let mut black_pts = 0usize;
        let r2 = radius * radius;
        let mut nx = 0.0;
        let mut ny = 0.0;
        let mut hits = 0usize;

        for cell in &mut self.cells {
            let dx = cell.cx - x;
            let dy = cell.cy - y;
            if dx * dx + dy * dy > r2 { continue; }
            if cell.color == target { continue; }

            let old = cell.color;
            cell.color = target;
            match (old, target) {
                (HexColor::Black, HexColor::White) => white_pts += 1,
                (HexColor::White, HexColor::Black) => black_pts += 1,
                _ => {}
            }

            let vx = x - cell.cx;
            let vy = y - cell.cy;
            let len = (vx * vx + vy * vy).sqrt();
            if len > 1e-6 {
                nx += vx / len;
                ny += vy / len;
                hits += 1;
            }
        }

        let normal = if hits > 0 {
            let len = (nx * nx + ny * ny).sqrt();
            if len > 1e-6 { Some((nx / len, ny / len)) } else { None }
        } else {
            None
        };

        (white_pts, black_pts, normal)
    }

    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        ctx.set_line_width(1.0);
        let _ = ctx.set_line_join("round");
        for cell in &self.cells {
            let (fill, stroke) = match cell.color {
                HexColor::Black => ("#000", "#fff"),
                HexColor::White => ("#fff", "#000"),
            };
            ctx.set_fill_style(&JsValue::from_str(fill));
            ctx.set_stroke_style(&JsValue::from_str(stroke));

            let r = self.r;
            ctx.begin_path();
            for i in 0..6 {
                let ang = (i as f64) * 60.0 * PI / 180.0;
                let vx = cell.cx + r * ang.cos();
                let vy = cell.cy + r * ang.sin();
                if i == 0 { ctx.move_to(vx, vy); } else { ctx.line_to(vx, vy); }
            }
            ctx.close_path();
            let _ = ctx.fill();
            let _ = ctx.stroke();
        }
    }
}

impl App {
    fn new(canvas: HtmlCanvasElement, ctx: CanvasRenderingContext2d, css_w: f64, css_h: f64) -> Self {
        let dpr = window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0).max(1.0);
        canvas.set_width((css_w * dpr) as u32);
        canvas.set_height((css_h * dpr) as u32);
        let _ = ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);

        let short = css_w.min(css_h);
        let r = (short / 50.0).clamp(3.0, 14.0);
        let grid = Grid::new(css_w, css_h, r);

        let (pw_el, pb_el) = {
            if let Some(doc) = window().and_then(|w| w.document()) {
                (doc.get_element_by_id("points-white"), doc.get_element_by_id("points-black"))
            } else { (None, None) }
        };

        let mut app = App {
            canvas, ctx, dpr, css_w, css_h,
            grid, balls: vec![],
            running: false, last_ts: 0.0, speed_mul: 1.0,
            points_white: 0, points_black: 0,
            points_white_el: pw_el, points_black_el: pb_el, points_dirty: true,
            raf_handle: None,
        };
        app.update_points_dom(); // initialize scoreboard to 0/0
        app
    }

    fn update_points_dom(&mut self) {
        if !self.points_dirty { return; }
        if let Some(ref el) = self.points_white_el { el.set_inner_html(&self.points_white.to_string()); }
        if let Some(ref el) = self.points_black_el { el.set_inner_html(&self.points_black.to_string()); }
        self.points_dirty = false;
    }

    fn resize(&mut self, css_w: f64, css_h: f64) {
        self.css_w = css_w; self.css_h = css_h;
        self.dpr = window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0).max(1.0);
        self.canvas.set_width((css_w * self.dpr) as u32);
        self.canvas.set_height((css_h * self.dpr) as u32);
        let _ = self.ctx.set_transform(self.dpr, 0.0, 0.0, self.dpr, 0.0, 0.0);

        let short = css_w.min(css_h);
        let r = (short / 50.0).clamp(3.0, 14.0);
        self.grid = Grid::new(css_w, css_h, r);

        for b in &mut self.balls {
            b.x = b.x.clamp(b.radius, self.css_w - b.radius);
            b.y = b.y.clamp(b.radius, self.css_h - b.radius);
        }
        self.render();
    }

    fn set_speed(&mut self, mul: f64) { self.speed_mul = mul.clamp(0.0, 6.25); }

    /// PUBLIC: set balls per team (0..=5)
    fn set_balls_per_team(&mut self, per_team: u32) {
        let n = per_team.min(5);
        self.spawn_balls_per_team(n);
    }

    fn spawn_balls_per_team(&mut self, per_team: u32) {
        self.balls.clear();
        let per_team = per_team as usize;
        if per_team == 0 { return; }

        let r = (self.grid.r * 1.8).clamp(6.0, 22.0);
        let speed = (self.grid.r * 20.0).clamp(200.0, 480.0);

        // White: left side, right-ish
        for _ in 0..per_team {
            let x = rand_range(r + 1.0, self.css_w * 0.25);
            let y = rand_range(r + 1.0, self.css_h - r - 1.0);
            let ang = rand_range(-0.35 * PI, 0.35 * PI);
            self.balls.push(Ball {
                x, y,
                vx: ang.cos() * speed,
                vy: ang.sin() * speed,
                team: Team::White,
                radius: r,
                base_speed: speed,
                last_bounce_ts: -1.0,
            });
        }
        // Black: right side, left-ish
        for _ in 0..per_team {
            let x = rand_range(self.css_w * 0.75, self.css_w - r - 1.0);
            let y = rand_range(r + 1.0, self.css_h - r - 1.0);
            let ang = PI + rand_range(-0.35 * PI, 0.35 * PI);
            self.balls.push(Ball {
                x, y,
                vx: ang.cos() * speed,
                vy: ang.sin() * speed,
                team: Team::Black,
                radius: r,
                base_speed: speed,
                last_bounce_ts: -1.0,
            });
        }
    }

    fn start(&mut self) -> Result<(), JsValue> {
        if self.running { return Ok(()); }
        self.running = true;
        self.last_ts = performance_now();

        let handle: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
        let handle_for_loop = handle.clone();
        let win = window().ok_or_else(|| js_err("no window"))?;
        let win_loop = win.clone();

        let callback = Closure::wrap(Box::new(move |ts: f64| {
            let again = APP.with(|a| {
                if let Some(ref mut app) = *a.borrow_mut() {
                    if app.running { let _ = app.tick(ts); return true; }
                }
                false
            });
            if again {
                if let Some(ref cb) = *handle_for_loop.borrow() {
                    let _ = win_loop.request_animation_frame(cb.as_ref().unchecked_ref());
                }
            }
        }) as Box<dyn FnMut(f64)>);

        {
            let mut slot = handle.borrow_mut();
            *slot = Some(callback);
        }

        if let Some(ref cb) = *handle.borrow() {
            let _ = win.request_animation_frame(cb.as_ref().unchecked_ref());
        }
        self.raf_handle = Some(handle);
        Ok(())
    }

    fn stop(&mut self) {
        self.running = false;
        self.raf_handle = None;
    }

    fn reset_grid(&mut self) {
        let short = self.css_w.min(self.css_h);
        let r = (short / 50.0).clamp(3.0, 14.0);
        self.grid = Grid::new(self.css_w, self.css_h, r);
        self.points_white = 0;
        self.points_black = 0;
        self.points_dirty = true;
        self.update_points_dom();
        self.render();
    }

    fn tick(&mut self, ts: f64) -> Result<(), JsValue> {
        let dt = ((ts - self.last_ts) / 1000.0).clamp(0.0, 0.050);
        self.last_ts = ts;
        let mul = self.speed_mul;
        let (w, h) = (self.css_w, self.css_h);

        // --- Phase 1: integrate + wall bounces ---
        for b in &mut self.balls {
            b.x += b.vx * dt * mul;
            b.y += b.vy * dt * mul;

            if b.x - b.radius <= 0.0 { b.x = b.radius; b.vx =  b.vx.abs(); }
            else if b.x + b.radius >= w { b.x = w - b.radius; b.vx = -b.vx.abs(); }
            if b.y - b.radius <= 0.0 { b.y = b.radius; b.vy =  b.vy.abs(); }
            else if b.y + b.radius >= h { b.y = h - b.radius; b.vy = -b.vy.abs(); }
        }

        // --- Phase 2: ball-ball collisions ---
        self.resolve_collisions();

        // --- Phase 3: claim & scoring (flip-based) ---
        let mut points_changed = false;
        for i in 0..self.balls.len() {
            let (x, y, radius, team, last_bounce_ts) = {
                let b = self.balls[i];
                (b.x, b.y, b.radius, b.team, b.last_bounce_ts)
            };
            let (add_white, add_black, normal) = self.grid.flip_disc(x, y, radius, team);
            if add_white > 0 { self.points_white += add_white; points_changed = true; }
            if add_black > 0 { self.points_black += add_black; points_changed = true; }
            if let Some((nx, ny)) = normal {
                let now = self.last_ts;
                if last_bounce_ts < 0.0 || now - last_bounce_ts > 15.0 {
                    let b = &mut self.balls[i];
                    let dot = b.vx * nx + b.vy * ny;
                    if dot < 0.0 {
                        b.vx -= 2.0 * dot * nx;
                        b.vy -= 2.0 * dot * ny;
                        b.maintain_speed();
                        b.last_bounce_ts = now;
                    }
                }
            }
        }
        if points_changed { self.points_dirty = true; self.update_points_dom(); }

        self.render();
        Ok(())
    }

    fn resolve_collisions(&mut self) {
        let n = self.balls.len();
        if n < 2 { return; }

        // Elastic collision, equal masses, slight restitution for liveliness
        let restitution = 0.98;

        for i in 0..n {
            for j in (i + 1)..n {
                // Safe split to borrow two balls mutably
                let (left, right) = self.balls.split_at_mut(j);
                let bi = &mut left[i];
                let bj = &mut right[0];

                let dx = bj.x - bi.x;
                let dy = bj.y - bi.y;
                let rsum = bi.radius + bj.radius;
                let dist2 = dx * dx + dy * dy;
                if dist2 > rsum * rsum { continue; }

                let mut dist = dist2.sqrt();
                if dist == 0.0 {
                    // Rare exact overlap: poke in a random direction
                    let ang = rand_range(0.0, PI * 2.0);
                    dist = 1e-6;
                    bi.x -= ang.cos() * 0.001;
                    bi.y -= ang.sin() * 0.001;
                }
                // Collision normal
                let nx = dx / dist;
                let ny = dy / dist;

                // Positional correction (separate overlap)
                let penetration = rsum - dist;
                let corr = (penetration / 2.0) + 1e-4;
                bi.x -= nx * corr; bi.y -= ny * corr;
                bj.x += nx * corr; bj.y += ny * corr;

                // Relative velocity along normal
                let rvx = bj.vx - bi.vx;
                let rvy = bj.vy - bi.vy;
                let vn = rvx * nx + rvy * ny;
                if vn >= 0.0 { continue; } // moving apart

                // Impulse (m1=m2=1): j = -(1+e)*vn / (1/m1+1/m2) = -(1+e)*vn/2
                let j = -(1.0 + restitution) * vn * 0.5;
                let jx = j * nx;
                let jy = j * ny;

                bi.vx -= jx; bi.vy -= jy;
                bj.vx += jx; bj.vy += jy;

                bi.maintain_speed();
                bj.maintain_speed();

                if bi.team == bj.team {
                    bi.base_speed = (bi.base_speed * TEAM_BOOST).min(MAX_BASE_SPEED);
                    bj.base_speed = (bj.base_speed * TEAM_BOOST).min(MAX_BASE_SPEED);
                    bi.maintain_speed();
                    bj.maintain_speed();
                }
            }
        }
    }

    fn render(&self) {
        // BG
        self.ctx.set_fill_style(&JsValue::from_str("#111"));
        let _ = self.ctx.fill_rect(0.0, 0.0, self.css_w, self.css_h);

        // Hex grid
        self.grid.draw(&self.ctx);

        // Glossy balls
        for b in &self.balls {
            let r = b.radius;
            let gx = b.x - r * 0.4;
            let gy = b.y - r * 0.4;
            let grad = self.ctx.create_radial_gradient(gx, gy, r * 0.05, b.x, b.y, r).unwrap();
            match b.team {
                Team::White => {
                    let _ = grad.add_color_stop(0.0, "#ffffff");
                    let _ = grad.add_color_stop(0.5, "#e9e9e9");
                    let _ = grad.add_color_stop(1.0, "#cfcfcf");
                    self.ctx.set_stroke_style(&JsValue::from_str("#000"));
                }
                Team::Black => {
                    let _ = grad.add_color_stop(0.0, "#6b6b6b");
                    let _ = grad.add_color_stop(0.5, "#181818");
                    let _ = grad.add_color_stop(1.0, "#000000");
                    self.ctx.set_stroke_style(&JsValue::from_str("#fff"));
                }
            }
            self.ctx.set_fill_style(&grad);

            self.ctx.begin_path();
            let _ = self.ctx.arc(b.x, b.y, r, 0.0, PI * 2.0);
            let _ = self.ctx.fill();

            // specular dot
            self.ctx.set_global_alpha(0.55);
            self.ctx.set_fill_style(&JsValue::from_str("#ffffff"));
            self.ctx.begin_path();
            let dot_r = (r * 0.28).max(0.8);
            let _ = self.ctx.arc(b.x - r * 0.45, b.y - r * 0.45, dot_r, 0.0, PI * 2.0);
            let _ = self.ctx.fill();
            self.ctx.set_global_alpha(1.0);

            self.ctx.set_line_width(1.0);
            let _ = self.ctx.stroke();
        }
    }
}

fn performance_now() -> f64 {
    window().and_then(|w| w.performance()).map(|p| p.now()).unwrap_or(0.0)
}

#[wasm_bindgen]
pub fn init_app(canvas_id: &str, css_w: f64, css_h: f64, balls_per_team: u32, speed: f64) -> Result<(), JsValue> {
    let (canvas, ctx) = {
        let win = window().ok_or_else(|| js_err("no window"))?;
        let doc = win.document().ok_or_else(|| js_err("no document"))?;
        let canvas = doc.get_element_by_id(canvas_id).ok_or_else(|| js_err("canvas not found"))?
            .dyn_into::<HtmlCanvasElement>()?;
        let ctx = canvas.get_context("2d")?.ok_or_else(|| js_err("2d ctx"))?
            .dyn_into::<CanvasRenderingContext2d>()?;
        (canvas, ctx)
    };
    let mut app = App::new(canvas, ctx, css_w, css_h);
    app.set_speed(speed);
    app.set_balls_per_team(balls_per_team);
    app.render();
    APP.with(|a| *a.borrow_mut() = Some(app));
    Ok(())
}

#[wasm_bindgen] pub fn start() -> Result<(), JsValue> { APP.with(|a| if let Some(ref mut app) = *a.borrow_mut() { app.start() } else { Err(js_err("app not initialized")) }) }
#[wasm_bindgen] pub fn stop() { APP.with(|a| if let Some(ref mut app) = *a.borrow_mut() { app.stop(); }) }
#[wasm_bindgen] pub fn reset_grid() { APP.with(|a| if let Some(ref mut app) = *a.borrow_mut() { app.reset_grid(); }) }
#[wasm_bindgen] pub fn set_speed(multiplier: f64) { APP.with(|a| if let Some(ref mut app) = *a.borrow_mut() { app.set_speed(multiplier); }) }

#[wasm_bindgen] pub fn set_balls_per_team(n: u32) { APP.with(|a| if let Some(ref mut app) = *a.borrow_mut() { app.set_balls_per_team(n); }) }

#[wasm_bindgen] pub fn set_num_balls(n: u32) { set_balls_per_team(n); }

#[wasm_bindgen] pub fn resize(css_w: f64, css_h: f64) { APP.with(|a| if let Some(ref mut app) = *a.borrow_mut() { app.resize(css_w, css_h); }) }
