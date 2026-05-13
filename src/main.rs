use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{ImageBuffer, Rgba};
use macroquad::prelude::*;

const CANVAS_W: usize = 48;
const CANVAS_H: usize = 48;
const HISTORY_LIMIT: usize = 80;

fn window_conf() -> Conf {
    Conf {
        window_title: "Pixel Paint Studio".to_owned(),
        window_width: 1180,
        window_height: 780,
        high_dpi: true,
        sample_count: 4,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut app = App::new();

    loop {
        let layout = Layout::new();
        app.update(&layout);
        app.draw(&layout);
        next_frame().await;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Pixel {
    const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    fn color(self) -> Color {
        Color::new(
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Brush,
    Eraser,
    Fill,
    Picker,
    Line,
    Rect,
}

impl Tool {
    const ALL: [Tool; 6] = [
        Tool::Brush,
        Tool::Eraser,
        Tool::Fill,
        Tool::Picker,
        Tool::Line,
        Tool::Rect,
    ];

    fn label(self) -> &'static str {
        match self {
            Tool::Brush => "Brush",
            Tool::Eraser => "Eraser",
            Tool::Fill => "Fill",
            Tool::Picker => "Picker",
            Tool::Line => "Line",
            Tool::Rect => "Rect",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Tool::Brush => "B",
            Tool::Eraser => "E",
            Tool::Fill => "F",
            Tool::Picker => "I",
            Tool::Line => "L",
            Tool::Rect => "R",
        }
    }
}

#[derive(Clone, Copy)]
struct Layout {
    left: Rect,
    right: Rect,
    canvas: Rect,
    export_button: Rect,
    clear_button: Rect,
    cell: f32,
    ui_scale: f32,
}

impl Layout {
    fn new() -> Self {
        let sw = screen_width();
        let sh = screen_height();
        let ui_scale = (sw / 1180.0).min(sh / 780.0).clamp(0.72, 1.25);
        let margin = 22.0 * ui_scale;
        let top = 86.0 * ui_scale;
        let left_w = 150.0 * ui_scale;
        let right_w = 248.0 * ui_scale;
        let gap = 24.0 * ui_scale;
        let available_w = sw - margin * 2.0 - left_w - right_w - gap * 2.0;
        let available_h = sh - top - margin;
        let cell = (available_w / CANVAS_W as f32)
            .min(available_h / CANVAS_H as f32)
            .floor()
            .clamp(5.0, 18.0);
        let canvas_w = cell * CANVAS_W as f32;
        let canvas_h = cell * CANVAS_H as f32;
        let total_w = left_w + gap + canvas_w + gap + right_w;
        let start_x = (sw - total_w) * 0.5;
        let canvas_x = start_x + left_w + gap;
        let canvas_y = top + (available_h - canvas_h) * 0.5;
        let left = Rect::new(start_x, canvas_y, left_w, canvas_h);
        let right = Rect::new(canvas_x + canvas_w + gap, canvas_y, right_w, canvas_h);
        let export_button = Rect::new(
            right.x + 18.0 * ui_scale,
            right.y + right.h - 96.0 * ui_scale,
            right.w - 36.0 * ui_scale,
            38.0 * ui_scale,
        );
        let clear_button = Rect::new(
            right.x + 18.0 * ui_scale,
            right.y + right.h - 50.0 * ui_scale,
            right.w - 36.0 * ui_scale,
            32.0 * ui_scale,
        );

        Self {
            left,
            right,
            canvas: Rect::new(canvas_x, canvas_y, canvas_w, canvas_h),
            export_button,
            clear_button,
            cell,
            ui_scale,
        }
    }
}

struct App {
    pixels: Vec<Pixel>,
    undo: Vec<Vec<Pixel>>,
    redo: Vec<Vec<Pixel>>,
    palette: Vec<Pixel>,
    current: Pixel,
    tool: Tool,
    show_grid: bool,
    brush_size: usize,
    drawing_stroke: bool,
    last_cell: Option<(usize, usize)>,
    drag_start: Option<(usize, usize)>,
    status: String,
    status_until: f64,
}

impl App {
    fn new() -> Self {
        let palette = vec![
            Pixel::rgb(20, 20, 31),
            Pixel::rgb(255, 255, 255),
            Pixel::rgb(255, 216, 64),
            Pixel::rgb(255, 148, 64),
            Pixel::rgb(255, 72, 91),
            Pixel::rgb(189, 115, 255),
            Pixel::rgb(82, 144, 255),
            Pixel::rgb(88, 230, 255),
            Pixel::rgb(97, 242, 116),
            Pixel::rgb(24, 185, 109),
            Pixel::rgb(142, 93, 51),
            Pixel::rgb(92, 60, 43),
            Pixel::rgb(38, 46, 69),
            Pixel::rgb(87, 102, 139),
            Pixel::rgb(161, 171, 194),
            Pixel::rgb(252, 186, 211),
            Pixel::rgb(254, 118, 190),
            Pixel::rgb(124, 63, 88),
            Pixel::rgb(74, 32, 64),
            Pixel::rgb(34, 17, 43),
        ];

        let mut app = Self {
            pixels: vec![Pixel::TRANSPARENT; CANVAS_W * CANVAS_H],
            undo: Vec::new(),
            redo: Vec::new(),
            palette,
            current: Pixel::rgb(255, 216, 64),
            tool: Tool::Brush,
            show_grid: true,
            brush_size: 1,
            drawing_stroke: false,
            last_cell: None,
            drag_start: None,
            status: "Ready".to_owned(),
            status_until: get_time() + 2.0,
        };
        app.seed_demo_art();
        app
    }

    fn seed_demo_art(&mut self) {
        let yellow = Pixel::rgb(255, 216, 64);
        let cyan = Pixel::rgb(88, 230, 255);
        let pink = Pixel::rgb(255, 72, 91);
        let purple = Pixel::rgb(189, 115, 255);
        let green = Pixel::rgb(97, 242, 116);

        for y in 31..45 {
            for x in 14..18 {
                self.set_raw(x, y, yellow);
            }
        }
        for y in 34..45 {
            self.set_raw(18, y, pink);
            self.set_raw(19, y, pink);
        }
        for y in 28..45 {
            self.set_raw(27, y, cyan);
        }
        for x in 22..31 {
            self.set_raw(x, 42, purple);
            self.set_raw(x, 43, purple);
        }
        for x in 8..13 {
            self.set_raw(x, 38, green);
            self.set_raw(x, 39, green);
        }
    }

    fn update(&mut self, layout: &Layout) {
        self.handle_keyboard();

        let mouse = mouse_vec();
        if is_mouse_button_pressed(MouseButton::Left) {
            if self.handle_tool_click(layout, mouse) {
                return;
            }
            if self.handle_palette_click(layout, mouse) {
                return;
            }
            if layout.export_button.contains(mouse) {
                self.export_png();
                return;
            }
            if layout.clear_button.contains(mouse) {
                self.clear_canvas();
                return;
            }
        }

        let cell = canvas_cell(layout, mouse);

        if is_mouse_button_pressed(MouseButton::Left) {
            self.begin_canvas_action(cell);
        }

        if is_mouse_button_down(MouseButton::Left) {
            self.continue_canvas_action(cell);
        }

        if is_mouse_button_released(MouseButton::Left) {
            self.finish_canvas_action(cell);
        }
    }

    fn handle_keyboard(&mut self) {
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);

        if ctrl && is_key_pressed(KeyCode::Z) {
            if shift {
                self.redo();
            } else {
                self.undo();
            }
        }
        if ctrl && is_key_pressed(KeyCode::Y) {
            self.redo();
        }
        if ctrl && is_key_pressed(KeyCode::S) {
            self.export_png();
        }

        if is_key_pressed(KeyCode::B) {
            self.select_tool(Tool::Brush);
        }
        if is_key_pressed(KeyCode::E) {
            self.select_tool(Tool::Eraser);
        }
        if is_key_pressed(KeyCode::F) {
            self.select_tool(Tool::Fill);
        }
        if is_key_pressed(KeyCode::I) {
            self.select_tool(Tool::Picker);
        }
        if is_key_pressed(KeyCode::L) {
            self.select_tool(Tool::Line);
        }
        if is_key_pressed(KeyCode::R) {
            self.select_tool(Tool::Rect);
        }
        if is_key_pressed(KeyCode::G) {
            self.show_grid = !self.show_grid;
            self.flash(if self.show_grid {
                "Grid on"
            } else {
                "Grid off"
            });
        }
        if is_key_pressed(KeyCode::LeftBracket) {
            self.brush_size = self.brush_size.saturating_sub(1).max(1);
            self.flash(&format!("Brush {}px", self.brush_size));
        }
        if is_key_pressed(KeyCode::RightBracket) {
            self.brush_size = (self.brush_size + 1).min(5);
            self.flash(&format!("Brush {}px", self.brush_size));
        }
        if is_key_pressed(KeyCode::Delete) {
            self.clear_canvas();
        }
    }

    fn handle_tool_click(&mut self, layout: &Layout, mouse: Vec2) -> bool {
        for (index, tool) in Tool::ALL.iter().enumerate() {
            if tool_button_rect(layout, index).contains(mouse) {
                self.select_tool(*tool);
                return true;
            }
        }
        false
    }

    fn handle_palette_click(&mut self, layout: &Layout, mouse: Vec2) -> bool {
        for (index, color) in self.palette.iter().enumerate() {
            if palette_rect(layout, index).contains(mouse) {
                self.current = *color;
                self.tool = Tool::Brush;
                self.flash("Color selected");
                return true;
            }
        }
        false
    }

    fn begin_canvas_action(&mut self, cell: Option<(usize, usize)>) {
        let Some((x, y)) = cell else {
            return;
        };

        match self.tool {
            Tool::Brush | Tool::Eraser => {
                self.push_undo();
                self.drawing_stroke = true;
                self.last_cell = Some((x, y));
                self.paint_cell(x, y);
            }
            Tool::Fill => {
                self.push_undo();
                if !self.bucket_fill(x, y) {
                    self.undo.pop();
                }
            }
            Tool::Picker => {
                let picked = self.pixel(x, y);
                if picked.a > 0 {
                    self.current = picked;
                    self.flash("Picked color");
                }
            }
            Tool::Line | Tool::Rect => {
                self.drag_start = Some((x, y));
            }
        }
    }

    fn continue_canvas_action(&mut self, cell: Option<(usize, usize)>) {
        if !self.drawing_stroke {
            return;
        }
        let Some((x, y)) = cell else {
            return;
        };

        if let Some((last_x, last_y)) = self.last_cell {
            self.draw_line(last_x, last_y, x, y, self.tool_pixel());
        } else {
            self.paint_cell(x, y);
        }
        self.last_cell = Some((x, y));
    }

    fn finish_canvas_action(&mut self, cell: Option<(usize, usize)>) {
        self.drawing_stroke = false;
        self.last_cell = None;

        let Some(start) = self.drag_start.take() else {
            return;
        };
        let Some(end) = cell else {
            return;
        };

        self.push_undo();
        match self.tool {
            Tool::Line => {
                self.draw_line(start.0, start.1, end.0, end.1, self.current);
            }
            Tool::Rect => {
                let filled = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                self.draw_rect(start, end, self.current, filled);
            }
            _ => {}
        }
    }

    fn draw(&self, layout: &Layout) {
        draw_background();
        draw_title(layout.ui_scale);
        draw_panel(layout.left, Color::new(0.055, 0.052, 0.078, 0.88));
        draw_panel(layout.right, Color::new(0.055, 0.052, 0.078, 0.88));
        self.draw_toolbar(layout);
        self.draw_canvas(layout);
        self.draw_palette(layout);
        self.draw_status(layout);
    }

    fn draw_toolbar(&self, layout: &Layout) {
        let scale = layout.ui_scale;
        draw_text_ex(
            "TOOLS",
            layout.left.x + 18.0 * scale,
            layout.left.y + 30.0 * scale,
            TextParams {
                font_size: (18.0 * scale) as u16,
                color: Color::new(0.62, 0.91, 1.0, 1.0),
                ..Default::default()
            },
        );

        for (index, tool) in Tool::ALL.iter().enumerate() {
            let rect = tool_button_rect(layout, index);
            let active = *tool == self.tool;
            let accent = if active {
                Color::new(1.0, 0.82, 0.28, 1.0)
            } else {
                Color::new(0.2, 0.23, 0.31, 1.0)
            };
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                if active {
                    Color::new(0.14, 0.13, 0.1, 0.94)
                } else {
                    Color::new(0.025, 0.028, 0.042, 0.78)
                },
            );
            draw_rectangle(rect.x, rect.y, 4.0 * scale, rect.h, accent);
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, fade(accent, 0.55));
            draw_text_ex(
                tool.key(),
                rect.x + 14.0 * scale,
                rect.y + 25.0 * scale,
                TextParams {
                    font_size: (18.0 * scale) as u16,
                    color: Color::new(0.95, 0.9, 0.66, 1.0),
                    ..Default::default()
                },
            );
            draw_text_ex(
                tool.label(),
                rect.x + 42.0 * scale,
                rect.y + 25.0 * scale,
                TextParams {
                    font_size: (15.0 * scale) as u16,
                    color: Color::new(0.78, 0.87, 0.93, 1.0),
                    ..Default::default()
                },
            );
        }

        draw_text_ex(
            "BRUSH",
            layout.left.x + 18.0 * scale,
            layout.left.y + layout.left.h - 82.0 * scale,
            TextParams {
                font_size: (13.0 * scale) as u16,
                color: Color::new(0.55, 0.6, 0.68, 1.0),
                ..Default::default()
            },
        );
        draw_text_ex(
            &format!("{} px", self.brush_size),
            layout.left.x + 18.0 * scale,
            layout.left.y + layout.left.h - 48.0 * scale,
            TextParams {
                font_size: (26.0 * scale) as u16,
                color: Color::new(1.0, 0.86, 0.5, 1.0),
                ..Default::default()
            },
        );
    }

    fn draw_canvas(&self, layout: &Layout) {
        glow_rect(
            layout.canvas.x - 14.0,
            layout.canvas.y - 14.0,
            layout.canvas.w + 28.0,
            layout.canvas.h + 28.0,
            self.current.color(),
            0.09,
        );
        draw_rectangle(
            layout.canvas.x - 8.0,
            layout.canvas.y - 8.0,
            layout.canvas.w + 16.0,
            layout.canvas.h + 16.0,
            Color::new(0.02, 0.021, 0.032, 0.96),
        );
        draw_rectangle_lines(
            layout.canvas.x - 8.0,
            layout.canvas.y - 8.0,
            layout.canvas.w + 16.0,
            layout.canvas.h + 16.0,
            2.0,
            Color::new(0.92, 0.67, 0.28, 0.55),
        );

        for y in 0..CANVAS_H {
            for x in 0..CANVAS_W {
                let px = layout.canvas.x + x as f32 * layout.cell;
                let py = layout.canvas.y + y as f32 * layout.cell;
                let checker = if (x + y) % 2 == 0 {
                    Color::new(0.1, 0.11, 0.15, 1.0)
                } else {
                    Color::new(0.075, 0.08, 0.115, 1.0)
                };
                draw_rectangle(px, py, layout.cell, layout.cell, checker);

                let pixel = self.pixel(x, y);
                if pixel.a > 0 {
                    draw_rectangle(px, py, layout.cell, layout.cell, pixel.color());
                    draw_rectangle(
                        px + 1.0,
                        py + 1.0,
                        layout.cell - 2.0,
                        (layout.cell * 0.25).max(1.0),
                        fade(WHITE, 0.18),
                    );
                }
            }
        }

        if let Some((start, end)) = self.preview_drag(layout) {
            let cells = match self.tool {
                Tool::Line => line_cells(start, end),
                Tool::Rect => {
                    let filled =
                        is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                    rect_cells(start, end, filled)
                }
                _ => Vec::new(),
            };
            for (x, y) in cells {
                let px = layout.canvas.x + x as f32 * layout.cell;
                let py = layout.canvas.y + y as f32 * layout.cell;
                draw_rectangle(
                    px,
                    py,
                    layout.cell,
                    layout.cell,
                    fade(self.current.color(), 0.42),
                );
                draw_rectangle_lines(
                    px + 2.0,
                    py + 2.0,
                    layout.cell - 4.0,
                    layout.cell - 4.0,
                    1.5,
                    Color::new(1.0, 0.95, 0.58, 0.8),
                );
            }
        }

        if self.show_grid {
            for x in 0..=CANVAS_W {
                let px = layout.canvas.x + x as f32 * layout.cell;
                draw_line(
                    px,
                    layout.canvas.y,
                    px,
                    layout.canvas.y + layout.canvas.h,
                    1.0,
                    Color::new(0.18, 0.21, 0.28, 0.55),
                );
            }
            for y in 0..=CANVAS_H {
                let py = layout.canvas.y + y as f32 * layout.cell;
                draw_line(
                    layout.canvas.x,
                    py,
                    layout.canvas.x + layout.canvas.w,
                    py,
                    1.0,
                    Color::new(0.18, 0.21, 0.28, 0.55),
                );
            }
        }
    }

    fn draw_palette(&self, layout: &Layout) {
        let scale = layout.ui_scale;
        draw_text_ex(
            "PALETTE",
            layout.right.x + 18.0 * scale,
            layout.right.y + 30.0 * scale,
            TextParams {
                font_size: (18.0 * scale) as u16,
                color: Color::new(0.62, 0.91, 1.0, 1.0),
                ..Default::default()
            },
        );

        for (index, pixel) in self.palette.iter().enumerate() {
            let rect = palette_rect(layout, index);
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, pixel.color());
            draw_rectangle(rect.x, rect.y, rect.w, rect.h * 0.25, fade(WHITE, 0.22));
            let active = *pixel == self.current;
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                if active { 3.0 } else { 1.0 },
                if active {
                    Color::new(1.0, 0.9, 0.48, 1.0)
                } else {
                    Color::new(0.0, 0.0, 0.0, 0.55)
                },
            );
        }

        let current_rect = Rect::new(
            layout.right.x + 18.0 * scale,
            layout.right.y + 298.0 * scale,
            layout.right.w - 36.0 * scale,
            76.0 * scale,
        );
        draw_rectangle(
            current_rect.x,
            current_rect.y,
            current_rect.w,
            current_rect.h,
            Color::new(0.025, 0.028, 0.042, 0.78),
        );
        draw_rectangle_lines(
            current_rect.x,
            current_rect.y,
            current_rect.w,
            current_rect.h,
            1.0,
            Color::new(0.35, 0.9, 1.0, 0.28),
        );
        draw_text_ex(
            "CURRENT",
            current_rect.x + 14.0 * scale,
            current_rect.y + 22.0 * scale,
            TextParams {
                font_size: (12.0 * scale) as u16,
                color: Color::new(0.55, 0.6, 0.68, 1.0),
                ..Default::default()
            },
        );
        draw_rectangle(
            current_rect.x + current_rect.w - 58.0 * scale,
            current_rect.y + 16.0 * scale,
            42.0 * scale,
            42.0 * scale,
            self.current.color(),
        );
        draw_text_ex(
            &format!(
                "#{:02X}{:02X}{:02X}",
                self.current.r, self.current.g, self.current.b
            ),
            current_rect.x + 14.0 * scale,
            current_rect.y + 52.0 * scale,
            TextParams {
                font_size: (18.0 * scale) as u16,
                color: Color::new(0.96, 0.9, 0.72, 1.0),
                ..Default::default()
            },
        );

        draw_button(layout.export_button, "Export PNG", true, scale);
        draw_button(layout.clear_button, "Clear", false, scale);
    }

    fn draw_status(&self, layout: &Layout) {
        let scale = layout.ui_scale;
        let status = if get_time() < self.status_until {
            self.status.as_str()
        } else {
            "Ctrl+S export  Ctrl+Z undo  G grid  [ ] brush"
        };
        let y = (layout.canvas.y + layout.canvas.h + 34.0 * scale).min(screen_height() - 18.0);
        let dims = measure_text(status, None, (15.0 * scale) as u16, 1.0);
        draw_text_ex(
            status,
            screen_width() * 0.5 - dims.width * 0.5,
            y,
            TextParams {
                font_size: (15.0 * scale) as u16,
                color: Color::new(0.7, 0.84, 0.9, 0.9),
                ..Default::default()
            },
        );
    }

    fn preview_drag(&self, layout: &Layout) -> Option<((usize, usize), (usize, usize))> {
        let start = self.drag_start?;
        let end = canvas_cell(layout, mouse_vec())?;
        Some((start, end))
    }

    fn select_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.flash(tool.label());
    }

    fn push_undo(&mut self) {
        if self.undo.last() == Some(&self.pixels) {
            return;
        }
        self.undo.push(self.pixels.clone());
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(self.pixels.clone());
            self.pixels = previous;
            self.flash("Undo");
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.pixels.clone());
            self.pixels = next;
            self.flash("Redo");
        }
    }

    fn clear_canvas(&mut self) {
        self.push_undo();
        self.pixels.fill(Pixel::TRANSPARENT);
        self.flash("Canvas cleared");
    }

    fn export_png(&mut self) {
        if let Err(err) = fs::create_dir_all("exports") {
            self.flash(&format!("Export failed: {err}"));
            return;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let path = format!("exports/pixel-art-{timestamp}.png");
        let mut image = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(CANVAS_W as u32, CANVAS_H as u32);

        for y in 0..CANVAS_H {
            for x in 0..CANVAS_W {
                let pixel = self.pixel(x, y);
                image.put_pixel(
                    x as u32,
                    y as u32,
                    Rgba([pixel.r, pixel.g, pixel.b, pixel.a]),
                );
            }
        }

        match image.save(&path) {
            Ok(()) => self.flash(&format!("Saved {path}")),
            Err(err) => self.flash(&format!("Export failed: {err}")),
        }
    }

    fn paint_cell(&mut self, x: usize, y: usize) {
        let color = self.tool_pixel();
        let radius = self.brush_size.saturating_sub(1) as i32;
        let center_x = x as i32;
        let center_y = y as i32;

        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let nx = center_x + dx;
                let ny = center_y + dy;
                if nx < 0 || ny < 0 {
                    continue;
                }
                let nx = nx as usize;
                let ny = ny as usize;
                if nx < CANVAS_W && ny < CANVAS_H {
                    self.set_raw(nx, ny, color);
                }
            }
        }
    }

    fn tool_pixel(&self) -> Pixel {
        if self.tool == Tool::Eraser {
            Pixel::TRANSPARENT
        } else {
            self.current
        }
    }

    fn draw_line(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, color: Pixel) {
        for (x, y) in line_cells((x0, y0), (x1, y1)) {
            self.set_raw(x, y, color);
        }
    }

    fn draw_rect(
        &mut self,
        start: (usize, usize),
        end: (usize, usize),
        color: Pixel,
        filled: bool,
    ) {
        for (x, y) in rect_cells(start, end, filled) {
            self.set_raw(x, y, color);
        }
    }

    fn bucket_fill(&mut self, x: usize, y: usize) -> bool {
        let target = self.pixel(x, y);
        let replacement = self.current;
        if target == replacement {
            return false;
        }

        let mut stack = vec![(x, y)];
        let mut changed = false;
        while let Some((cx, cy)) = stack.pop() {
            if self.pixel(cx, cy) != target {
                continue;
            }

            self.set_raw(cx, cy, replacement);
            changed = true;

            if cx > 0 {
                stack.push((cx - 1, cy));
            }
            if cx + 1 < CANVAS_W {
                stack.push((cx + 1, cy));
            }
            if cy > 0 {
                stack.push((cx, cy - 1));
            }
            if cy + 1 < CANVAS_H {
                stack.push((cx, cy + 1));
            }
        }
        changed
    }

    fn pixel(&self, x: usize, y: usize) -> Pixel {
        self.pixels[index(x, y)]
    }

    fn set_raw(&mut self, x: usize, y: usize, color: Pixel) {
        self.pixels[index(x, y)] = color;
    }

    fn flash(&mut self, message: &str) {
        self.status = message.to_owned();
        self.status_until = get_time() + 2.2;
    }
}

fn index(x: usize, y: usize) -> usize {
    y * CANVAS_W + x
}

fn mouse_vec() -> Vec2 {
    let (x, y) = mouse_position();
    vec2(x, y)
}

fn canvas_cell(layout: &Layout, mouse: Vec2) -> Option<(usize, usize)> {
    if !layout.canvas.contains(mouse) {
        return None;
    }
    let x = ((mouse.x - layout.canvas.x) / layout.cell).floor() as usize;
    let y = ((mouse.y - layout.canvas.y) / layout.cell).floor() as usize;
    (x < CANVAS_W && y < CANVAS_H).then_some((x, y))
}

fn tool_button_rect(layout: &Layout, index: usize) -> Rect {
    let scale = layout.ui_scale;
    Rect::new(
        layout.left.x + 16.0 * scale,
        layout.left.y + 54.0 * scale + index as f32 * 48.0 * scale,
        layout.left.w - 32.0 * scale,
        36.0 * scale,
    )
}

fn palette_rect(layout: &Layout, index: usize) -> Rect {
    let scale = layout.ui_scale;
    let swatch = 34.0 * scale;
    let gap = 9.0 * scale;
    let columns = 5;
    let col = index % columns;
    let row = index / columns;
    Rect::new(
        layout.right.x + 18.0 * scale + col as f32 * (swatch + gap),
        layout.right.y + 54.0 * scale + row as f32 * (swatch + gap),
        swatch,
        swatch,
    )
}

fn line_cells(start: (usize, usize), end: (usize, usize)) -> Vec<(usize, usize)> {
    let (mut x0, mut y0) = (start.0 as i32, start.1 as i32);
    let (x1, y1) = (end.0 as i32, end.1 as i32);
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cells = Vec::new();

    loop {
        if x0 >= 0 && y0 >= 0 && x0 < CANVAS_W as i32 && y0 < CANVAS_H as i32 {
            cells.push((x0 as usize, y0 as usize));
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
    cells
}

fn rect_cells(start: (usize, usize), end: (usize, usize), filled: bool) -> Vec<(usize, usize)> {
    let min_x = start.0.min(end.0);
    let max_x = start.0.max(end.0);
    let min_y = start.1.min(end.1);
    let max_y = start.1.max(end.1);
    let mut cells = Vec::new();

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if filled || x == min_x || x == max_x || y == min_y || y == max_y {
                cells.push((x, y));
            }
        }
    }
    cells
}

fn draw_background() {
    let sw = screen_width();
    let sh = screen_height();
    for i in 0..72 {
        let t = i as f32 / 71.0;
        draw_rectangle(
            0.0,
            sh * t,
            sw,
            sh / 72.0 + 1.0,
            Color::new(0.017 + t * 0.035, 0.018 + t * 0.026, 0.032 + t * 0.047, 1.0),
        );
    }

    let drift = get_time() as f32;
    for i in 0..48 {
        let x = i as f32 * 52.0 + (drift * 10.0) % 52.0;
        draw_line(
            x,
            0.0,
            x - 260.0,
            sh,
            1.0,
            Color::new(0.28, 0.23, 0.42, 0.13),
        );
    }

    for i in 0..20 {
        let t = i as f32;
        let x = (t * 137.0 + drift * 7.0).rem_euclid(sw + 80.0) - 40.0;
        let y = (t * 89.0 + (drift * 0.8 + t).sin() * 18.0).rem_euclid(sh + 60.0) - 30.0;
        draw_circle(
            x,
            y,
            1.3 + (i % 4) as f32 * 0.55,
            Color::new(1.0, 0.75, 0.28, 0.13),
        );
    }
}

fn draw_title(scale: f32) {
    let title = "PIXEL PAINT STUDIO";
    let subtitle = "pixel-art editor with undo, palette and PNG export";
    let title_size = (34.0 * scale) as u16;
    let subtitle_size = (16.0 * scale) as u16;
    let title_dims = measure_text(title, None, title_size, 1.0);
    let subtitle_dims = measure_text(subtitle, None, subtitle_size, 1.0);
    let x = screen_width() * 0.5 - title_dims.width * 0.5;
    let y = 48.0 * scale;

    draw_text_ex(
        title,
        x,
        y,
        TextParams {
            font_size: title_size,
            color: Color::new(1.0, 0.86, 0.46, 1.0),
            ..Default::default()
        },
    );
    draw_text_ex(
        subtitle,
        screen_width() * 0.5 - subtitle_dims.width * 0.5,
        y + 26.0 * scale,
        TextParams {
            font_size: subtitle_size,
            color: Color::new(0.62, 0.91, 1.0, 0.95),
            ..Default::default()
        },
    );
}

fn draw_panel(rect: Rect, color: Color) {
    draw_rectangle(
        rect.x + 7.0,
        rect.y + 9.0,
        rect.w,
        rect.h,
        Color::new(0.0, 0.0, 0.0, 0.24),
    );
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        3.0,
        Color::new(0.98, 0.72, 0.28, 0.48),
    );
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        2.0,
        Color::new(0.82, 0.62, 0.3, 0.28),
    );
}

fn draw_button(rect: Rect, label: &str, primary: bool, scale: f32) {
    let accent = if primary {
        Color::new(0.35, 0.9, 1.0, 1.0)
    } else {
        Color::new(0.62, 0.66, 0.72, 1.0)
    };
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if primary {
            Color::new(0.02, 0.12, 0.16, 0.78)
        } else {
            Color::new(0.025, 0.028, 0.042, 0.78)
        },
    );
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, fade(accent, 0.65));
    let size = (15.0 * scale) as u16;
    let dims = measure_text(label, None, size, 1.0);
    draw_text_ex(
        label,
        rect.x + rect.w * 0.5 - dims.width * 0.5,
        rect.y + rect.h * 0.5 + dims.height * 0.35,
        TextParams {
            font_size: size,
            color: if primary {
                Color::new(0.82, 0.97, 1.0, 1.0)
            } else {
                Color::new(0.78, 0.82, 0.88, 1.0)
            },
            ..Default::default()
        },
    );
}

fn glow_rect(x: f32, y: f32, w: f32, h: f32, color: Color, alpha: f32) {
    for layer in 0..5 {
        let spread = 8.0 + layer as f32 * 8.0;
        draw_rectangle(
            x - spread,
            y - spread,
            w + spread * 2.0,
            h + spread * 2.0,
            fade(color, alpha / (layer as f32 + 1.2)),
        );
    }
}

fn fade(color: Color, alpha: f32) -> Color {
    Color::new(color.r, color.g, color.b, color.a * alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_cells_include_start_and_end() {
        let cells = line_cells((2, 3), (8, 9));
        assert_eq!(cells.first(), Some(&(2, 3)));
        assert_eq!(cells.last(), Some(&(8, 9)));
    }

    #[test]
    fn rect_outline_skips_center() {
        let cells = rect_cells((1, 1), (3, 3), false);
        assert!(cells.contains(&(1, 1)));
        assert!(cells.contains(&(3, 3)));
        assert!(!cells.contains(&(2, 2)));
    }

    #[test]
    fn filled_rect_contains_center() {
        let cells = rect_cells((1, 1), (3, 3), true);
        assert!(cells.contains(&(2, 2)));
    }
}
