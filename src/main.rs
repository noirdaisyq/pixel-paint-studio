use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{imageops::FilterType, ImageBuffer, Rgba};
use macroquad::prelude::*;
use rfd::FileDialog;

const HISTORY_LIMIT: usize = 80;
const MIN_CANVAS_SIZE: usize = 8;
const MAX_CANVAS_SIZE: usize = 256;
const DEFAULT_CUSTOM_CANVAS_SIZE: usize = 64;
const REFERENCE_DEFAULT_OPACITY: f32 = 0.45;

const PRESETS: [CanvasPreset; 6] = [
    CanvasPreset {
        name: "Icon",
        width: 16,
        height: 16,
        note: "tiny app icons",
    },
    CanvasPreset {
        name: "Sprite",
        width: 32,
        height: 32,
        note: "game assets",
    },
    CanvasPreset {
        name: "Pixel Art",
        width: 48,
        height: 48,
        note: "balanced canvas",
    },
    CanvasPreset {
        name: "Avatar",
        width: 64,
        height: 64,
        note: "profile image",
    },
    CanvasPreset {
        name: "Banner",
        width: 96,
        height: 64,
        note: "wide artwork",
    },
    CanvasPreset {
        name: "Large",
        width: 128,
        height: 128,
        note: "detailed piece",
    },
];

fn window_conf() -> Conf {
    Conf {
        window_title: "Pixel Paint Studio".to_owned(),
        window_width: 1240,
        window_height: 820,
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
        app.update();
        app.draw();
        next_frame().await;
    }
}

#[derive(Clone, Copy, Debug)]
struct CanvasPreset {
    name: &'static str,
    width: usize,
    height: usize,
    note: &'static str,
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
enum CanvasBackground {
    Transparent,
    White,
    Dark,
}

impl CanvasBackground {
    const ALL: [CanvasBackground; 3] = [
        CanvasBackground::Transparent,
        CanvasBackground::White,
        CanvasBackground::Dark,
    ];

    fn label(self) -> &'static str {
        match self {
            CanvasBackground::Transparent => "Transparent",
            CanvasBackground::White => "White",
            CanvasBackground::Dark => "Dark",
        }
    }

    fn pixel(self) -> Pixel {
        match self {
            CanvasBackground::Transparent => Pixel::TRANSPARENT,
            CanvasBackground::White => Pixel::rgb(255, 255, 255),
            CanvasBackground::Dark => Pixel::rgb(20, 20, 31),
        }
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
    Ellipse,
    Spray,
    Mirror,
    Dither,
}

impl Tool {
    const ALL: [Tool; 10] = [
        Tool::Brush,
        Tool::Eraser,
        Tool::Fill,
        Tool::Picker,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Spray,
        Tool::Mirror,
        Tool::Dither,
    ];

    fn label(self) -> &'static str {
        match self {
            Tool::Brush => "Brush",
            Tool::Eraser => "Eraser",
            Tool::Fill => "Fill",
            Tool::Picker => "Picker",
            Tool::Line => "Line",
            Tool::Rect => "Rect",
            Tool::Ellipse => "Ellipse",
            Tool::Spray => "Spray",
            Tool::Mirror => "Mirror",
            Tool::Dither => "Dither",
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
            Tool::Ellipse => "O",
            Tool::Spray => "S",
            Tool::Mirror => "M",
            Tool::Dither => "D",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppMode {
    Setup,
    Editor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CustomDimension {
    Width,
    Height,
}

#[derive(Clone, Debug)]
struct SetupState {
    preset_index: usize,
    use_custom: bool,
    custom_width_text: String,
    custom_height_text: String,
    focused_dimension: Option<CustomDimension>,
    background: CanvasBackground,
    demo_art: bool,
}

impl Default for SetupState {
    fn default() -> Self {
        Self {
            preset_index: 2,
            use_custom: false,
            custom_width_text: DEFAULT_CUSTOM_CANVAS_SIZE.to_string(),
            custom_height_text: DEFAULT_CUSTOM_CANVAS_SIZE.to_string(),
            focused_dimension: None,
            background: CanvasBackground::Transparent,
            demo_art: true,
        }
    }
}

#[derive(Clone, Debug)]
struct ReferenceLayer {
    name: String,
    width: usize,
    height: usize,
    pixels: Vec<Pixel>,
}

impl ReferenceLayer {
    fn pixel(&self, x: usize, y: usize) -> Pixel {
        self.pixels[y * self.width + x]
    }
}

#[derive(Clone, Copy)]
struct Layout {
    left: Rect,
    right: Rect,
    canvas: Rect,
    export_button: Rect,
    new_button: Rect,
    undo_button: Rect,
    redo_button: Rect,
    grid_button: Rect,
    clear_button: Rect,
    cell: f32,
    ui_scale: f32,
    canvas_w: usize,
    canvas_h: usize,
}

impl Layout {
    fn new(canvas_w: usize, canvas_h: usize) -> Self {
        let sw = screen_width();
        let sh = screen_height();
        let ui_scale = (sw / 1240.0).min(sh / 820.0).clamp(0.72, 1.22);
        let margin = 22.0 * ui_scale;
        let top = 88.0 * ui_scale;
        let left_w = 172.0 * ui_scale;
        let right_w = 272.0 * ui_scale;
        let gap = 24.0 * ui_scale;
        let available_w = sw - margin * 2.0 - left_w - right_w - gap * 2.0;
        let available_h = sh - top - margin - 26.0 * ui_scale;
        let cell = (available_w / canvas_w as f32)
            .min(available_h / canvas_h as f32)
            .floor()
            .clamp(2.0, 18.0);
        let canvas_px_w = cell * canvas_w as f32;
        let canvas_px_h = cell * canvas_h as f32;
        let total_w = left_w + gap + canvas_px_w + gap + right_w;
        let start_x = (sw - total_w) * 0.5;
        let canvas_x = start_x + left_w + gap;
        let canvas_y = top + (available_h - canvas_px_h) * 0.5;
        let panel_h = canvas_px_h.max(640.0 * ui_scale).min(available_h);
        let panel_y = top + (available_h - panel_h) * 0.5;
        let left = Rect::new(start_x, panel_y, left_w, panel_h);
        let right = Rect::new(canvas_x + canvas_px_w + gap, panel_y, right_w, panel_h);
        let action_x = right.x + 18.0 * ui_scale;
        let action_w = right.w - 36.0 * ui_scale;
        let row_w = (action_w - 10.0 * ui_scale) * 0.5;
        let bottom = right.y + right.h;

        Self {
            left,
            right,
            canvas: Rect::new(canvas_x, canvas_y, canvas_px_w, canvas_px_h),
            export_button: Rect::new(
                action_x,
                bottom - 184.0 * ui_scale,
                action_w,
                38.0 * ui_scale,
            ),
            new_button: Rect::new(
                action_x,
                bottom - 138.0 * ui_scale,
                action_w,
                34.0 * ui_scale,
            ),
            undo_button: Rect::new(action_x, bottom - 96.0 * ui_scale, row_w, 32.0 * ui_scale),
            redo_button: Rect::new(
                action_x + row_w + 10.0 * ui_scale,
                bottom - 96.0 * ui_scale,
                row_w,
                32.0 * ui_scale,
            ),
            grid_button: Rect::new(action_x, bottom - 56.0 * ui_scale, row_w, 32.0 * ui_scale),
            clear_button: Rect::new(
                action_x + row_w + 10.0 * ui_scale,
                bottom - 56.0 * ui_scale,
                row_w,
                32.0 * ui_scale,
            ),
            cell,
            ui_scale,
            canvas_w,
            canvas_h,
        }
    }
}

struct App {
    mode: AppMode,
    setup: SetupState,
    width: usize,
    height: usize,
    background: CanvasBackground,
    pixels: Vec<Pixel>,
    reference: Option<ReferenceLayer>,
    reference_opacity: f32,
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
    tick: u64,
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

        Self {
            mode: AppMode::Setup,
            setup: SetupState::default(),
            width: 48,
            height: 48,
            background: CanvasBackground::Transparent,
            pixels: vec![Pixel::TRANSPARENT; 48 * 48],
            reference: None,
            reference_opacity: REFERENCE_DEFAULT_OPACITY,
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
            tick: 0,
        }
    }

    fn update(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        match self.mode {
            AppMode::Setup => self.update_setup(),
            AppMode::Editor => {
                let layout = Layout::new(self.width, self.height);
                self.update_editor(&layout);
            }
        }
    }

    fn draw(&self) {
        draw_background();
        match self.mode {
            AppMode::Setup => self.draw_setup(),
            AppMode::Editor => {
                let layout = Layout::new(self.width, self.height);
                draw_title(
                    layout.ui_scale,
                    "PIXEL PAINT STUDIO",
                    "full pixel-art paint workspace",
                );
                draw_panel(layout.left, Color::new(0.055, 0.052, 0.078, 0.88));
                draw_panel(layout.right, Color::new(0.055, 0.052, 0.078, 0.88));
                self.draw_toolbar(&layout);
                self.draw_canvas(&layout);
                self.draw_palette(&layout);
                self.draw_status(&layout);
            }
        }
    }

    fn update_setup(&mut self) {
        if self.setup.focused_dimension.is_some() {
            self.edit_custom_canvas_text();
        } else {
            if is_key_pressed(KeyCode::Key1) {
                self.select_preset(0);
            }
            if is_key_pressed(KeyCode::Key2) {
                self.select_preset(1);
            }
            if is_key_pressed(KeyCode::Key3) {
                self.select_preset(2);
            }
            if is_key_pressed(KeyCode::Key4) {
                self.select_preset(3);
            }
            if is_key_pressed(KeyCode::Key5) {
                self.select_preset(4);
            }
            if is_key_pressed(KeyCode::Key6) {
                self.select_preset(5);
            }
            if is_key_pressed(KeyCode::Left) {
                self.select_preset(self.setup.preset_index.saturating_sub(1));
            }
            if is_key_pressed(KeyCode::Right) {
                self.select_preset((self.setup.preset_index + 1).min(PRESETS.len() - 1));
            }
            if is_key_pressed(KeyCode::Space) {
                self.setup.demo_art = !self.setup.demo_art;
            }
            if is_key_pressed(KeyCode::Enter) {
                self.create_project_from_setup();
            }
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            let mouse = mouse_vec();
            for index in 0..PRESETS.len() {
                if setup_preset_rect(index, self.setup_scale()).contains(mouse) {
                    self.select_preset(index);
                    return;
                }
            }
            if custom_width_minus_rect(self.setup_scale()).contains(mouse) {
                self.adjust_custom_canvas_size(CustomDimension::Width, -1);
                return;
            }
            if custom_width_plus_rect(self.setup_scale()).contains(mouse) {
                self.adjust_custom_canvas_size(CustomDimension::Width, 1);
                return;
            }
            if custom_height_minus_rect(self.setup_scale()).contains(mouse) {
                self.adjust_custom_canvas_size(CustomDimension::Height, -1);
                return;
            }
            if custom_height_plus_rect(self.setup_scale()).contains(mouse) {
                self.adjust_custom_canvas_size(CustomDimension::Height, 1);
                return;
            }
            if custom_width_field_rect(self.setup_scale()).contains(mouse) {
                self.focus_custom_dimension(CustomDimension::Width);
                return;
            }
            if custom_height_field_rect(self.setup_scale()).contains(mouse) {
                self.focus_custom_dimension(CustomDimension::Height);
                return;
            }
            if setup_custom_rect(self.setup_scale()).contains(mouse) {
                self.setup.use_custom = true;
                self.setup.focused_dimension = None;
                return;
            }
            for (index, background) in CanvasBackground::ALL.iter().enumerate() {
                if setup_background_rect(index, self.setup_scale()).contains(mouse) {
                    self.setup.background = *background;
                    self.setup.focused_dimension = None;
                    return;
                }
            }
            if setup_demo_rect(self.setup_scale()).contains(mouse) {
                self.setup.demo_art = !self.setup.demo_art;
                self.setup.focused_dimension = None;
                return;
            }
            if setup_create_rect(self.setup_scale()).contains(mouse) {
                self.create_project_from_setup();
            }
        }
    }

    fn draw_setup(&self) {
        let scale = self.setup_scale();
        draw_title(
            scale,
            "PIXEL PAINT STUDIO",
            "create a canvas, choose a background, start painting",
        );

        let panel = setup_panel_rect(scale);
        draw_panel(panel, Color::new(0.055, 0.052, 0.078, 0.91));
        draw_section_label(
            "NEW PROJECT",
            panel.x + 32.0 * scale,
            panel.y + 46.0 * scale,
            scale,
        );

        for (index, preset) in PRESETS.iter().enumerate() {
            let rect = setup_preset_rect(index, scale);
            let active = !self.setup.use_custom && index == self.setup.preset_index;
            let accent = if active {
                Color::new(1.0, 0.82, 0.28, 1.0)
            } else {
                Color::new(0.32, 0.38, 0.48, 1.0)
            };
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                if active {
                    Color::new(0.13, 0.115, 0.075, 0.92)
                } else {
                    Color::new(0.025, 0.028, 0.042, 0.78)
                },
            );
            draw_rectangle(rect.x, rect.y, 4.0 * scale, rect.h, accent);
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, fade(accent, 0.55));
            draw_text_line(
                preset.name,
                rect.x + 18.0 * scale,
                rect.y + 27.0 * scale,
                18.0 * scale,
                Color::new(0.95, 0.9, 0.66, 1.0),
            );
            draw_text_line(
                &format!("{} x {}", preset.width, preset.height),
                rect.x + 18.0 * scale,
                rect.y + 54.0 * scale,
                20.0 * scale,
                Color::new(0.78, 0.93, 1.0, 1.0),
            );
            draw_text_line(
                preset.note,
                rect.x + 18.0 * scale,
                rect.y + 78.0 * scale,
                12.0 * scale,
                Color::new(0.58, 0.64, 0.72, 1.0),
            );
        }

        self.draw_custom_canvas_controls(scale);

        let selected = self.selected_canvas_preset();
        draw_section_label(
            "BACKGROUND",
            panel.x + 32.0 * scale,
            panel.y + 376.0 * scale,
            scale,
        );
        for (index, background) in CanvasBackground::ALL.iter().enumerate() {
            let rect = setup_background_rect(index, scale);
            let active = *background == self.setup.background;
            draw_button(rect, background.label(), active, scale);
        }
        draw_button(
            setup_demo_rect(scale),
            if self.setup.demo_art {
                "Demo art: ON"
            } else {
                "Demo art: OFF"
            },
            self.setup.demo_art,
            scale,
        );

        let preview = Rect::new(
            panel.x + panel.w - 252.0 * scale,
            panel.y + 118.0 * scale,
            180.0 * scale,
            180.0 * scale,
        );
        self.draw_setup_preview(preview, selected, self.setup.background);
        draw_text_line(
            "Canvas preview",
            preview.x,
            preview.y - 14.0 * scale,
            13.0 * scale,
            Color::new(0.58, 0.64, 0.72, 1.0),
        );

        draw_button(setup_create_rect(scale), "Create Canvas", true, scale);
    }

    fn draw_custom_canvas_controls(&self, scale: f32) {
        let rect = setup_custom_rect(scale);
        let active = self.setup.use_custom;
        let accent = if active {
            Color::new(1.0, 0.82, 0.28, 1.0)
        } else {
            Color::new(0.32, 0.38, 0.48, 1.0)
        };

        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            if active {
                Color::new(0.13, 0.115, 0.075, 0.92)
            } else {
                Color::new(0.025, 0.028, 0.042, 0.78)
            },
        );
        draw_rectangle(rect.x, rect.y, 4.0 * scale, rect.h, accent);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, fade(accent, 0.55));
        draw_text_line(
            "Custom",
            rect.x + 18.0 * scale,
            rect.y + 25.0 * scale,
            17.0 * scale,
            Color::new(0.95, 0.9, 0.66, 1.0),
        );
        draw_text_line(
            "canvas size",
            rect.x + 18.0 * scale,
            rect.y + 48.0 * scale,
            12.0 * scale,
            Color::new(0.58, 0.64, 0.72, 1.0),
        );

        self.draw_dimension_editor(CustomDimension::Width, "W", scale);
        self.draw_dimension_editor(CustomDimension::Height, "H", scale);
    }

    fn draw_dimension_editor(&self, dimension: CustomDimension, label: &str, scale: f32) {
        let field = custom_dimension_field_rect(dimension, scale);
        let minus = custom_dimension_minus_rect(dimension, scale);
        let plus = custom_dimension_plus_rect(dimension, scale);
        let focused = self.setup.focused_dimension == Some(dimension);
        let value = match dimension {
            CustomDimension::Width => self.setup.custom_width_text.as_str(),
            CustomDimension::Height => self.setup.custom_height_text.as_str(),
        };
        let border = if focused {
            Color::new(1.0, 0.86, 0.38, 1.0)
        } else if self.setup.use_custom {
            Color::new(0.35, 0.9, 1.0, 0.72)
        } else {
            Color::new(0.38, 0.44, 0.55, 0.55)
        };

        draw_text_line(
            label,
            minus.x - 22.0 * scale,
            field.y + 23.0 * scale,
            14.0 * scale,
            Color::new(0.62, 0.91, 1.0, 1.0),
        );
        draw_button(minus, "-", false, scale);
        draw_rectangle(
            field.x,
            field.y,
            field.w,
            field.h,
            Color::new(0.015, 0.018, 0.03, 0.9),
        );
        draw_rectangle_lines(field.x, field.y, field.w, field.h, 1.5, border);
        let display = if value.is_empty() { "0" } else { value };
        let size = (17.0 * scale) as u16;
        let dims = measure_text(display, None, size, 1.0);
        draw_text_ex(
            display,
            field.x + field.w * 0.5 - dims.width * 0.5,
            field.y + field.h * 0.5 + dims.height * 0.35,
            TextParams {
                font_size: size,
                color: Color::new(0.92, 0.97, 1.0, 1.0),
                ..Default::default()
            },
        );
        draw_button(plus, "+", true, scale);
    }

    fn draw_setup_preview(&self, rect: Rect, preset: CanvasPreset, background: CanvasBackground) {
        glow_rect(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::new(1.0, 0.82, 0.28, 1.0),
            0.065,
        );
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::new(0.02, 0.021, 0.032, 0.96),
        );
        let cell = (rect.w / preset.width as f32)
            .min(rect.h / preset.height as f32)
            .floor()
            .max(1.0);
        let w = cell * preset.width as f32;
        let h = cell * preset.height as f32;
        let x0 = rect.x + (rect.w - w) * 0.5;
        let y0 = rect.y + (rect.h - h) * 0.5;
        for y in 0..preset.height {
            for x in 0..preset.width {
                let px = x0 + x as f32 * cell;
                let py = y0 + y as f32 * cell;
                draw_rectangle(px, py, cell, cell, checker_color(x, y, background));
            }
        }
        draw_rectangle_lines(x0, y0, w, h, 2.0, Color::new(0.92, 0.67, 0.28, 0.55));
    }

    fn setup_scale(&self) -> f32 {
        (screen_width() / 1240.0)
            .min(screen_height() / 820.0)
            .clamp(0.72, 1.18)
    }

    fn select_preset(&mut self, index: usize) {
        self.setup.preset_index = index.min(PRESETS.len() - 1);
        self.setup.use_custom = false;
        self.setup.focused_dimension = None;
    }

    fn selected_canvas_preset(&self) -> CanvasPreset {
        if self.setup.use_custom {
            CanvasPreset {
                name: "Custom",
                width: parse_canvas_size(&self.setup.custom_width_text),
                height: parse_canvas_size(&self.setup.custom_height_text),
                note: "custom size",
            }
        } else {
            PRESETS[self.setup.preset_index]
        }
    }

    fn focus_custom_dimension(&mut self, dimension: CustomDimension) {
        self.setup.use_custom = true;
        self.setup.focused_dimension = Some(dimension);
    }

    fn edit_custom_canvas_text(&mut self) {
        let Some(dimension) = self.setup.focused_dimension else {
            return;
        };

        while let Some(ch) = get_char_pressed() {
            if ch.is_ascii_digit() {
                let text = self.custom_dimension_text_mut(dimension);
                if text.len() < 3 {
                    if text == "0" {
                        text.clear();
                    }
                    text.push(ch);
                }
            }
        }

        if is_key_pressed(KeyCode::Backspace) {
            self.custom_dimension_text_mut(dimension).pop();
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Escape) {
            self.normalize_custom_canvas_size();
            self.setup.focused_dimension = None;
        }
    }

    fn adjust_custom_canvas_size(&mut self, dimension: CustomDimension, delta: i32) {
        self.setup.use_custom = true;
        self.setup.focused_dimension = None;
        let current = match dimension {
            CustomDimension::Width => parse_canvas_size(&self.setup.custom_width_text),
            CustomDimension::Height => parse_canvas_size(&self.setup.custom_height_text),
        };
        let next = clamp_canvas_size((current as i32 + delta) as usize);
        *self.custom_dimension_text_mut(dimension) = next.to_string();
    }

    fn normalize_custom_canvas_size(&mut self) {
        let width = parse_canvas_size(&self.setup.custom_width_text);
        let height = parse_canvas_size(&self.setup.custom_height_text);
        self.setup.custom_width_text = width.to_string();
        self.setup.custom_height_text = height.to_string();
    }

    fn custom_dimension_text_mut(&mut self, dimension: CustomDimension) -> &mut String {
        match dimension {
            CustomDimension::Width => &mut self.setup.custom_width_text,
            CustomDimension::Height => &mut self.setup.custom_height_text,
        }
    }

    fn create_project_from_setup(&mut self) {
        self.normalize_custom_canvas_size();
        let preset = self.selected_canvas_preset();
        self.width = preset.width;
        self.height = preset.height;
        self.background = self.setup.background;
        self.pixels = vec![self.background.pixel(); self.width * self.height];
        self.reference = None;
        self.undo.clear();
        self.redo.clear();
        self.tool = Tool::Brush;
        self.show_grid = true;
        self.brush_size = 1;
        self.mode = AppMode::Editor;
        self.status = format!("Created {} x {} canvas", self.width, self.height);
        self.status_until = get_time() + 2.4;
        if self.setup.demo_art {
            self.seed_demo_art();
        }
    }

    fn seed_demo_art(&mut self) {
        if self.width < 16 || self.height < 16 {
            return;
        }

        const CAT: &[&str] = &[
            "........................................",
            "............xxx............xxx..........",
            "...........xppx..........xppx...........",
            "..........xpooxx........xxoopx..........",
            ".........xpooooxxxxxxxxxxoooopx.........",
            "........xpoooooooooooooooooooopx........",
            ".......xoooooooooooooooooooooooox.......",
            "......xoooosooooosooooosoooooooox.......",
            ".....xoooosoooooosoooooosoooooooox......",
            "....xooooooooooooooooooooooooooooox.....",
            "...xoooooooeeeoooooooeeeoooooooox.......",
            "...xooooooeeweoooooeeweooooooooox.......",
            "...xooooooeeeeooocooeeeeooooooooox......",
            "...xooooooooooocccccoooooooooooox.......",
            "xxxxxxooooopoooocpcccoooopooooxxxxxx....",
            "...xoooooopoooocccccoooopooooox.........",
            "....xoooooopoooocccoooopooooox..........",
            ".....xooooooopppppppppoooooox...........",
            "......xxooooooooooooooooooxx............",
            "........xxxxoooooooooxxxx...............",
            ".....xxxxxxoooooooooooxxxxxx............",
            "...xxoooooooooooooooooooooooxx..........",
            "..xoooooooooooocccoooooooooooox.........",
            ".xoooooooooooocccccccoooooooooox........",
            ".xooooooooooocccccccccooooooooox........",
            ".xoooooooosooocccccccooosoooooox........",
            "xoooooooosssoooooooooossssoooooox.......",
            "xooooooossssoooooooooossssoooooox.......",
            ".xoooooooooooooooooooooooooooooox.......",
            "..xoooooooooooooooooooooooooooox........",
            "...xxooooooxxxxxxxxxoooooooxx...........",
            ".....xxxxxx.........xxxxxx..............",
            "........................................",
        ];

        self.stamp_sprite(CAT);
    }

    fn update_editor(&mut self, layout: &Layout) {
        self.handle_keyboard();

        let mouse = mouse_vec();
        if is_mouse_button_pressed(MouseButton::Left) {
            if self.handle_tool_click(layout, mouse) {
                return;
            }
            if self.handle_palette_click(layout, mouse) {
                return;
            }
            if self.handle_action_click(layout, mouse) {
                return;
            }
        }
        if is_mouse_button_down(MouseButton::Left)
            && reference_opacity_slider_rect(layout).contains(mouse)
        {
            self.set_reference_opacity_from_mouse(layout, mouse.x);
            return;
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

        if ctrl && is_key_pressed(KeyCode::N) {
            self.mode = AppMode::Setup;
            return;
        }
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
        if ctrl && is_key_pressed(KeyCode::O) {
            self.import_reference_image();
            return;
        }

        if !ctrl && is_key_pressed(KeyCode::B) {
            self.select_tool(Tool::Brush);
        }
        if !ctrl && is_key_pressed(KeyCode::E) {
            self.select_tool(Tool::Eraser);
        }
        if !ctrl && is_key_pressed(KeyCode::F) {
            self.select_tool(Tool::Fill);
        }
        if !ctrl && is_key_pressed(KeyCode::I) {
            self.select_tool(Tool::Picker);
        }
        if !ctrl && is_key_pressed(KeyCode::L) {
            self.select_tool(Tool::Line);
        }
        if !ctrl && is_key_pressed(KeyCode::R) {
            self.select_tool(Tool::Rect);
        }
        if !ctrl && is_key_pressed(KeyCode::O) {
            self.select_tool(Tool::Ellipse);
        }
        if !ctrl && is_key_pressed(KeyCode::S) {
            self.select_tool(Tool::Spray);
        }
        if !ctrl && is_key_pressed(KeyCode::M) {
            self.select_tool(Tool::Mirror);
        }
        if !ctrl && is_key_pressed(KeyCode::D) {
            self.select_tool(Tool::Dither);
        }
        if is_key_pressed(KeyCode::G) {
            self.show_grid = !self.show_grid;
            self.flash(if self.show_grid {
                "Grid on"
            } else {
                "Grid off"
            });
        }
        if is_key_pressed(KeyCode::LeftBracket) || is_key_pressed(KeyCode::Minus) {
            self.change_brush_size(-1);
        }
        if is_key_pressed(KeyCode::RightBracket) || is_key_pressed(KeyCode::Equal) {
            self.change_brush_size(1);
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

    fn handle_action_click(&mut self, layout: &Layout, mouse: Vec2) -> bool {
        if reference_import_rect(layout).contains(mouse) {
            self.import_reference_image();
            return true;
        }
        if reference_clear_rect(layout).contains(mouse) {
            self.reference = None;
            self.flash("Reference cleared");
            return true;
        }
        if reference_opacity_slider_rect(layout).contains(mouse) {
            self.set_reference_opacity_from_mouse(layout, mouse.x);
            return true;
        }
        if brush_minus_rect(layout).contains(mouse) {
            self.change_brush_size(-1);
            return true;
        }
        if brush_plus_rect(layout).contains(mouse) {
            self.change_brush_size(1);
            return true;
        }
        if layout.export_button.contains(mouse) {
            self.export_png();
            return true;
        }
        if layout.new_button.contains(mouse) {
            self.mode = AppMode::Setup;
            return true;
        }
        if layout.undo_button.contains(mouse) {
            self.undo();
            return true;
        }
        if layout.redo_button.contains(mouse) {
            self.redo();
            return true;
        }
        if layout.grid_button.contains(mouse) {
            self.show_grid = !self.show_grid;
            self.flash(if self.show_grid {
                "Grid on"
            } else {
                "Grid off"
            });
            return true;
        }
        if layout.clear_button.contains(mouse) {
            self.clear_canvas();
            return true;
        }
        false
    }

    fn begin_canvas_action(&mut self, cell: Option<(usize, usize)>) {
        let Some((x, y)) = cell else {
            return;
        };

        match self.tool {
            Tool::Brush | Tool::Eraser | Tool::Mirror | Tool::Dither | Tool::Spray => {
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
            Tool::Line | Tool::Rect | Tool::Ellipse => {
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

        if self.tool == Tool::Spray {
            self.paint_cell(x, y);
            self.last_cell = Some((x, y));
            return;
        }

        if let Some(last) = self.last_cell {
            for (sx, sy) in line_cells(last, (x, y)) {
                self.paint_cell(sx, sy);
            }
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
        let filled = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        match self.tool {
            Tool::Line => self.draw_line(start, end, self.current),
            Tool::Rect => self.draw_rect(start, end, self.current, filled),
            Tool::Ellipse => self.draw_ellipse(start, end, self.current, filled),
            _ => {}
        }
    }

    fn draw_toolbar(&self, layout: &Layout) {
        let scale = layout.ui_scale;
        draw_section_label(
            "TOOLS",
            layout.left.x + 18.0 * scale,
            layout.left.y + 30.0 * scale,
            scale,
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
            draw_text_line(
                tool.key(),
                rect.x + 13.0 * scale,
                rect.y + 23.0 * scale,
                16.0 * scale,
                Color::new(0.95, 0.9, 0.66, 1.0),
            );
            draw_text_line(
                tool.label(),
                rect.x + 40.0 * scale,
                rect.y + 23.0 * scale,
                14.0 * scale,
                Color::new(0.78, 0.87, 0.93, 1.0),
            );
        }

        draw_text_line(
            "BRUSH",
            layout.left.x + 18.0 * scale,
            layout.left.y + layout.left.h - 82.0 * scale,
            13.0 * scale,
            Color::new(0.55, 0.6, 0.68, 1.0),
        );
        draw_button(brush_minus_rect(layout), "-", false, scale);
        draw_button(brush_plus_rect(layout), "+", true, scale);
        draw_text_line(
            &format!("{} px", self.brush_size),
            layout.left.x + 62.0 * scale,
            layout.left.y + layout.left.h - 48.0 * scale,
            24.0 * scale,
            Color::new(1.0, 0.86, 0.5, 1.0),
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

        for y in 0..self.height {
            for x in 0..self.width {
                let px = layout.canvas.x + x as f32 * layout.cell;
                let py = layout.canvas.y + y as f32 * layout.cell;
                draw_rectangle(
                    px,
                    py,
                    layout.cell,
                    layout.cell,
                    checker_color(x, y, self.background),
                );

                let pixel = self.pixel(x, y);
                if pixel.a > 0 {
                    draw_rectangle(px, py, layout.cell, layout.cell, pixel.color());
                    if layout.cell >= 6.0 {
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
        }

        self.draw_reference_overlay(layout);

        if let Some((start, end)) = self.preview_drag(layout) {
            let cells = match self.tool {
                Tool::Line => line_cells(start, end),
                Tool::Rect => {
                    let filled =
                        is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                    rect_cells(start, end, filled)
                }
                Tool::Ellipse => {
                    let filled =
                        is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                    ellipse_cells(start, end, filled)
                }
                _ => Vec::new(),
            };
            for (x, y) in cells {
                if !self.in_bounds(x, y) {
                    continue;
                }
                let px = layout.canvas.x + x as f32 * layout.cell;
                let py = layout.canvas.y + y as f32 * layout.cell;
                draw_rectangle(
                    px,
                    py,
                    layout.cell,
                    layout.cell,
                    fade(self.current.color(), 0.42),
                );
                if layout.cell >= 6.0 {
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
        }

        if self.show_grid && layout.cell >= 5.0 {
            for x in 0..=self.width {
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
            for y in 0..=self.height {
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

    fn draw_reference_overlay(&self, layout: &Layout) {
        let Some(reference) = &self.reference else {
            return;
        };
        if reference.width != self.width || reference.height != self.height {
            return;
        }

        for y in 0..self.height {
            for x in 0..self.width {
                let mut color = reference.pixel(x, y).color();
                color.a *= self.reference_opacity;
                if color.a <= 0.0 {
                    continue;
                }
                draw_rectangle(
                    layout.canvas.x + x as f32 * layout.cell,
                    layout.canvas.y + y as f32 * layout.cell,
                    layout.cell,
                    layout.cell,
                    color,
                );
            }
        }
    }

    fn draw_reference_controls(&self, layout: &Layout) {
        let scale = layout.ui_scale;
        let title_y = layout.right.y + 234.0 * scale;
        draw_section_label("REFERENCE", layout.right.x + 18.0 * scale, title_y, scale);

        let import_rect = reference_import_rect(layout);
        let clear_rect = reference_clear_rect(layout);
        draw_button(import_rect, "Import Photo", self.reference.is_some(), scale);
        draw_button(clear_rect, "Clear", false, scale);

        let name = self
            .reference
            .as_ref()
            .map(|reference| short_label(&reference.name, 24))
            .unwrap_or_else(|| "No photo loaded".to_owned());
        draw_text_line(
            &name,
            layout.right.x + 18.0 * scale,
            layout.right.y + 296.0 * scale,
            12.0 * scale,
            Color::new(0.58, 0.64, 0.72, 1.0),
        );

        let slider = reference_opacity_slider_rect(layout);
        let opacity = format!(
            "Opacity {}%",
            (self.reference_opacity * 100.0).round() as i32
        );
        draw_text_line(
            &opacity,
            slider.x,
            slider.y - 7.0 * scale,
            12.0 * scale,
            Color::new(0.72, 0.84, 0.91, 1.0),
        );
        draw_slider(
            slider,
            self.reference_opacity,
            self.reference.is_some(),
            scale,
        );
    }

    fn draw_palette(&self, layout: &Layout) {
        let scale = layout.ui_scale;
        draw_section_label(
            "PALETTE",
            layout.right.x + 18.0 * scale,
            layout.right.y + 30.0 * scale,
            scale,
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

        self.draw_reference_controls(layout);

        let current_rect = Rect::new(
            layout.right.x + 18.0 * scale,
            layout.right.y + 352.0 * scale,
            layout.right.w - 36.0 * scale,
            86.0 * scale,
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
        draw_text_line(
            "CURRENT",
            current_rect.x + 14.0 * scale,
            current_rect.y + 22.0 * scale,
            12.0 * scale,
            Color::new(0.55, 0.6, 0.68, 1.0),
        );
        draw_rectangle(
            current_rect.x + current_rect.w - 58.0 * scale,
            current_rect.y + 16.0 * scale,
            38.0 * scale,
            38.0 * scale,
            self.current.color(),
        );
        draw_text_line(
            &format!(
                "#{:02X}{:02X}{:02X}",
                self.current.r, self.current.g, self.current.b
            ),
            current_rect.x + 14.0 * scale,
            current_rect.y + 52.0 * scale,
            18.0 * scale,
            Color::new(0.96, 0.9, 0.72, 1.0),
        );
        draw_text_line(
            &format!("{} x {} px", self.width, self.height),
            current_rect.x + 14.0 * scale,
            current_rect.y + 74.0 * scale,
            14.0 * scale,
            Color::new(0.72, 0.84, 0.91, 1.0),
        );

        draw_button(layout.export_button, "Export PNG", true, scale);
        draw_button(layout.new_button, "New Project", false, scale);
        draw_button(layout.undo_button, "Undo", false, scale);
        draw_button(layout.redo_button, "Redo", false, scale);
        draw_button(
            layout.grid_button,
            if self.show_grid {
                "Grid On"
            } else {
                "Grid Off"
            },
            self.show_grid,
            scale,
        );
        draw_button(layout.clear_button, "Clear", false, scale);
    }

    fn draw_status(&self, layout: &Layout) {
        let scale = layout.ui_scale;
        let status = if get_time() < self.status_until {
            self.status.as_str()
        } else {
            "Ctrl+O photo  Ctrl+S export  Ctrl+N new  Ctrl+Z undo  [ ] brush"
        };
        let y = (layout.canvas.y + layout.canvas.h + 34.0 * scale).min(screen_height() - 18.0);
        let dims = measure_text(status, None, (15.0 * scale) as u16, 1.0);
        draw_text_line(
            status,
            screen_width() * 0.5 - dims.width * 0.5,
            y,
            15.0 * scale,
            Color::new(0.7, 0.84, 0.9, 0.9),
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
        self.pixels.fill(self.background.pixel());
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
        let mut image =
            ImageBuffer::<Rgba<u8>, Vec<u8>>::new(self.width as u32, self.height as u32);

        for y in 0..self.height {
            for x in 0..self.width {
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

    fn import_reference_image(&mut self) {
        let Some(path) = FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg"])
            .pick_file()
        else {
            self.flash("Reference import cancelled");
            return;
        };

        match load_reference_layer(&path, self.width, self.height) {
            Ok(reference) => {
                let name = reference.name.clone();
                self.reference = Some(reference);
                self.reference_opacity = self.reference_opacity.max(0.2);
                self.flash(&format!("Reference loaded: {name}"));
            }
            Err(err) => self.flash(&format!("Reference failed: {err}")),
        }
    }

    fn set_reference_opacity_from_mouse(&mut self, layout: &Layout, mouse_x: f32) {
        let rect = reference_opacity_slider_rect(layout);
        let value = ((mouse_x - rect.x) / rect.w).clamp(0.0, 1.0);
        self.reference_opacity = value;
        self.flash(&format!(
            "Reference opacity {}%",
            (value * 100.0).round() as i32
        ));
    }

    fn paint_cell(&mut self, x: usize, y: usize) {
        match self.tool {
            Tool::Spray => self.spray_cell(x, y),
            Tool::Mirror => {
                self.paint_brush(x, y, self.current, false);
                let mirror_x = self.width - 1 - x;
                self.paint_brush(mirror_x, y, self.current, false);
            }
            Tool::Dither => self.paint_brush(x, y, self.current, true),
            Tool::Eraser => self.paint_brush(x, y, Pixel::TRANSPARENT, false),
            _ => self.paint_brush(x, y, self.current, false),
        }
    }

    fn paint_brush(&mut self, x: usize, y: usize, color: Pixel, dither: bool) {
        let size = self.brush_size.max(1) as i32;
        let anchor = size / 2;
        let center_x = x as i32;
        let center_y = y as i32;

        for oy in 0..size {
            for ox in 0..size {
                let nx = center_x + ox - anchor;
                let ny = center_y + oy - anchor;
                if nx < 0 || ny < 0 {
                    continue;
                }
                let nx = nx as usize;
                let ny = ny as usize;
                if self.in_bounds(nx, ny) && (!dither || (nx + ny).is_multiple_of(2)) {
                    self.set_raw(nx, ny, color);
                }
            }
        }
    }

    fn spray_cell(&mut self, x: usize, y: usize) {
        let radius = self.brush_size.max(2) as i32 + 2;
        let density = (self.brush_size * 5).max(10);
        let seed = self.tick as i32 + x as i32 * 37 + y as i32 * 53;
        for i in 0..density {
            let h = pseudo_hash(seed + i as i32 * 97);
            let dx = h % (radius * 2 + 1) - radius;
            let dy = (h / 17) % (radius * 2 + 1) - radius;
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && ny >= 0 {
                let nx = nx as usize;
                let ny = ny as usize;
                if self.in_bounds(nx, ny) {
                    self.set_raw(nx, ny, self.current);
                }
            }
        }
    }

    fn draw_line(&mut self, start: (usize, usize), end: (usize, usize), color: Pixel) {
        for (x, y) in line_cells(start, end) {
            if self.in_bounds(x, y) {
                self.set_raw(x, y, color);
            }
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
            if self.in_bounds(x, y) {
                self.set_raw(x, y, color);
            }
        }
    }

    fn draw_ellipse(
        &mut self,
        start: (usize, usize),
        end: (usize, usize),
        color: Pixel,
        filled: bool,
    ) {
        for (x, y) in ellipse_cells(start, end, filled) {
            if self.in_bounds(x, y) {
                self.set_raw(x, y, color);
            }
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
            if cx + 1 < self.width {
                stack.push((cx + 1, cy));
            }
            if cy > 0 {
                stack.push((cx, cy - 1));
            }
            if cy + 1 < self.height {
                stack.push((cx, cy + 1));
            }
        }
        changed
    }

    fn pixel(&self, x: usize, y: usize) -> Pixel {
        self.pixels[self.index(x, y)]
    }

    fn set_raw(&mut self, x: usize, y: usize, color: Pixel) {
        if self.in_bounds(x, y) {
            let index = self.index(x, y);
            self.pixels[index] = color;
        }
    }

    fn index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    fn in_bounds(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height
    }

    fn flash(&mut self, message: &str) {
        self.status = message.to_owned();
        self.status_until = get_time() + 2.2;
    }

    fn change_brush_size(&mut self, delta: i32) {
        self.brush_size = (self.brush_size as i32 + delta).clamp(1, 8) as usize;
        self.flash(&format!("Brush {}px", self.brush_size));
    }

    fn stamp_sprite(&mut self, rows: &[&str]) {
        let sprite_w = rows.iter().map(|row| row.len()).max().unwrap_or(1);
        let sprite_h = rows.len().max(1);
        let scale = (self.width as f32 * 0.86 / sprite_w as f32)
            .min(self.height as f32 * 0.9 / sprite_h as f32)
            .max(0.35);
        let draw_w = sprite_w as f32 * scale;
        let draw_h = sprite_h as f32 * scale;
        let offset_x = (self.width as f32 - draw_w) * 0.5;
        let offset_y = (self.height as f32 - draw_h) * 0.5;

        for (row, line) in rows.iter().enumerate() {
            for (col, marker) in line.chars().enumerate() {
                let Some(color) = sprite_pixel(marker) else {
                    continue;
                };

                let x0 = (offset_x + col as f32 * scale).floor().max(0.0) as usize;
                let x1 = (offset_x + (col + 1) as f32 * scale)
                    .ceil()
                    .min(self.width as f32) as usize;
                let y0 = (offset_y + row as f32 * scale).floor().max(0.0) as usize;
                let y1 = (offset_y + (row + 1) as f32 * scale)
                    .ceil()
                    .min(self.height as f32) as usize;

                for y in y0..y1 {
                    for x in x0..x1 {
                        self.set_raw(x, y, color);
                    }
                }
            }
        }
    }
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
    (x < layout.canvas_w && y < layout.canvas_h).then_some((x, y))
}

fn tool_button_rect(layout: &Layout, index: usize) -> Rect {
    let scale = layout.ui_scale;
    Rect::new(
        layout.left.x + 16.0 * scale,
        layout.left.y + 54.0 * scale + index as f32 * 36.5 * scale,
        layout.left.w - 32.0 * scale,
        29.0 * scale,
    )
}

fn palette_rect(layout: &Layout, index: usize) -> Rect {
    let scale = layout.ui_scale;
    let swatch = 32.0 * scale;
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

fn brush_minus_rect(layout: &Layout) -> Rect {
    let scale = layout.ui_scale;
    Rect::new(
        layout.left.x + 18.0 * scale,
        layout.left.y + layout.left.h - 66.0 * scale,
        34.0 * scale,
        34.0 * scale,
    )
}

fn brush_plus_rect(layout: &Layout) -> Rect {
    let scale = layout.ui_scale;
    Rect::new(
        layout.left.x + layout.left.w - 52.0 * scale,
        layout.left.y + layout.left.h - 66.0 * scale,
        34.0 * scale,
        34.0 * scale,
    )
}

fn reference_import_rect(layout: &Layout) -> Rect {
    let scale = layout.ui_scale;
    Rect::new(
        layout.right.x + 18.0 * scale,
        layout.right.y + 248.0 * scale,
        144.0 * scale,
        30.0 * scale,
    )
}

fn reference_clear_rect(layout: &Layout) -> Rect {
    let scale = layout.ui_scale;
    Rect::new(
        layout.right.x + layout.right.w - 86.0 * scale,
        layout.right.y + 248.0 * scale,
        68.0 * scale,
        30.0 * scale,
    )
}

fn reference_opacity_slider_rect(layout: &Layout) -> Rect {
    let scale = layout.ui_scale;
    Rect::new(
        layout.right.x + 18.0 * scale,
        layout.right.y + 324.0 * scale,
        layout.right.w - 36.0 * scale,
        24.0 * scale,
    )
}

fn setup_panel_rect(scale: f32) -> Rect {
    let w = 900.0 * scale;
    let h = 560.0 * scale;
    Rect::new(
        screen_width() * 0.5 - w * 0.5,
        screen_height() * 0.52 - h * 0.5,
        w,
        h,
    )
}

fn setup_preset_rect(index: usize, scale: f32) -> Rect {
    let panel = setup_panel_rect(scale);
    let col = index % 3;
    let row = index / 3;
    let w = 178.0 * scale;
    let h = 92.0 * scale;
    Rect::new(
        panel.x + 32.0 * scale + col as f32 * 194.0 * scale,
        panel.y + 76.0 * scale + row as f32 * 112.0 * scale,
        w,
        h,
    )
}

fn setup_custom_rect(scale: f32) -> Rect {
    let panel = setup_panel_rect(scale);
    Rect::new(
        panel.x + 32.0 * scale,
        panel.y + 296.0 * scale,
        552.0 * scale,
        58.0 * scale,
    )
}

fn custom_dimension_field_rect(dimension: CustomDimension, scale: f32) -> Rect {
    let rect = setup_custom_rect(scale);
    let x = match dimension {
        CustomDimension::Width => rect.x + 188.0 * scale,
        CustomDimension::Height => rect.x + 382.0 * scale,
    };
    Rect::new(x, rect.y + 15.0 * scale, 58.0 * scale, 30.0 * scale)
}

fn custom_dimension_minus_rect(dimension: CustomDimension, scale: f32) -> Rect {
    let field = custom_dimension_field_rect(dimension, scale);
    Rect::new(field.x - 36.0 * scale, field.y, 30.0 * scale, field.h)
}

fn custom_dimension_plus_rect(dimension: CustomDimension, scale: f32) -> Rect {
    let field = custom_dimension_field_rect(dimension, scale);
    Rect::new(
        field.x + field.w + 6.0 * scale,
        field.y,
        30.0 * scale,
        field.h,
    )
}

fn custom_width_field_rect(scale: f32) -> Rect {
    custom_dimension_field_rect(CustomDimension::Width, scale)
}

fn custom_height_field_rect(scale: f32) -> Rect {
    custom_dimension_field_rect(CustomDimension::Height, scale)
}

fn custom_width_minus_rect(scale: f32) -> Rect {
    custom_dimension_minus_rect(CustomDimension::Width, scale)
}

fn custom_width_plus_rect(scale: f32) -> Rect {
    custom_dimension_plus_rect(CustomDimension::Width, scale)
}

fn custom_height_minus_rect(scale: f32) -> Rect {
    custom_dimension_minus_rect(CustomDimension::Height, scale)
}

fn custom_height_plus_rect(scale: f32) -> Rect {
    custom_dimension_plus_rect(CustomDimension::Height, scale)
}

fn setup_background_rect(index: usize, scale: f32) -> Rect {
    let panel = setup_panel_rect(scale);
    Rect::new(
        panel.x + 32.0 * scale + index as f32 * 148.0 * scale,
        panel.y + 398.0 * scale,
        132.0 * scale,
        36.0 * scale,
    )
}

fn setup_demo_rect(scale: f32) -> Rect {
    let panel = setup_panel_rect(scale);
    Rect::new(
        panel.x + 32.0 * scale,
        panel.y + 448.0 * scale,
        200.0 * scale,
        36.0 * scale,
    )
}

fn setup_create_rect(scale: f32) -> Rect {
    let panel = setup_panel_rect(scale);
    Rect::new(
        panel.x + panel.w - 252.0 * scale,
        panel.y + panel.h - 86.0 * scale,
        180.0 * scale,
        42.0 * scale,
    )
}

fn clamp_canvas_size(value: usize) -> usize {
    value.clamp(MIN_CANVAS_SIZE, MAX_CANVAS_SIZE)
}

fn parse_canvas_size(text: &str) -> usize {
    text.parse::<usize>()
        .map(clamp_canvas_size)
        .unwrap_or(DEFAULT_CUSTOM_CANVAS_SIZE)
}

fn load_reference_layer(
    path: &Path,
    canvas_width: usize,
    canvas_height: usize,
) -> Result<ReferenceLayer, String> {
    let image = image::open(path).map_err(|err| err.to_string())?;
    let scaled = image
        .resize_exact(
            canvas_width as u32,
            canvas_height as u32,
            FilterType::Triangle,
        )
        .to_rgba8();
    let pixels = scaled
        .pixels()
        .map(|pixel| Pixel::rgba(pixel[0], pixel[1], pixel[2], pixel[3]))
        .collect();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("reference image")
        .to_owned();

    Ok(ReferenceLayer {
        name,
        width: canvas_width,
        height: canvas_height,
        pixels,
    })
}

fn short_label(label: &str, max_chars: usize) -> String {
    if label.chars().count() <= max_chars {
        return label.to_owned();
    }

    let keep = max_chars.saturating_sub(3);
    let mut shortened: String = label.chars().take(keep).collect();
    shortened.push_str("...");
    shortened
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
        cells.push((x0 as usize, y0 as usize));
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

fn ellipse_cells(start: (usize, usize), end: (usize, usize), filled: bool) -> Vec<(usize, usize)> {
    let min_x = start.0.min(end.0);
    let max_x = start.0.max(end.0);
    let min_y = start.1.min(end.1);
    let max_y = start.1.max(end.1);
    let width = max_x - min_x + 1;
    let height = max_y - min_y + 1;
    if width <= 2 || height <= 2 {
        return rect_cells(start, end, filled);
    }

    let cx = (min_x + max_x) as f32 * 0.5;
    let cy = (min_y + max_y) as f32 * 0.5;
    let rx = (width as f32 - 1.0) * 0.5;
    let ry = (height as f32 - 1.0) * 0.5;
    let edge = (1.35 / rx.max(ry)).max(0.035);
    let mut cells = Vec::new();

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let nx = (x as f32 - cx) / rx;
            let ny = (y as f32 - cy) / ry;
            let v = nx * nx + ny * ny;
            if (filled && v <= 1.0) || (!filled && (v - 1.0).abs() <= edge) {
                cells.push((x, y));
            }
        }
    }
    cells
}

fn pseudo_hash(mut value: i32) -> i32 {
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    value.abs()
}

fn sprite_pixel(marker: char) -> Option<Pixel> {
    match marker {
        'x' => Some(Pixel::rgb(54, 34, 38)),
        'o' => Some(Pixel::rgb(244, 153, 76)),
        's' => Some(Pixel::rgb(202, 96, 48)),
        'c' => Some(Pixel::rgb(255, 224, 166)),
        'p' => Some(Pixel::rgb(255, 132, 160)),
        'e' => Some(Pixel::rgb(30, 28, 38)),
        'w' => Some(Pixel::rgb(255, 240, 190)),
        _ => None,
    }
}

fn checker_color(x: usize, y: usize, background: CanvasBackground) -> Color {
    match background {
        CanvasBackground::Transparent => {
            if (x + y).is_multiple_of(2) {
                Color::new(0.1, 0.11, 0.15, 1.0)
            } else {
                Color::new(0.075, 0.08, 0.115, 1.0)
            }
        }
        CanvasBackground::White => {
            if (x + y).is_multiple_of(2) {
                Color::new(0.86, 0.88, 0.92, 1.0)
            } else {
                Color::new(0.94, 0.95, 0.98, 1.0)
            }
        }
        CanvasBackground::Dark => {
            if (x + y).is_multiple_of(2) {
                Color::new(0.07, 0.075, 0.105, 1.0)
            } else {
                Color::new(0.095, 0.1, 0.14, 1.0)
            }
        }
    }
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

fn draw_title(scale: f32, title: &str, subtitle: &str) {
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

fn draw_section_label(label: &str, x: f32, y: f32, scale: f32) {
    draw_text_line(label, x, y, 18.0 * scale, Color::new(0.62, 0.91, 1.0, 1.0));
}

fn draw_text_line(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text_ex(
        text,
        x,
        y,
        TextParams {
            font_size: size as u16,
            color,
            ..Default::default()
        },
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
    let size = (14.5 * scale) as u16;
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

fn draw_slider(rect: Rect, value: f32, enabled: bool, scale: f32) {
    let value = value.clamp(0.0, 1.0);
    let track_h = (5.0 * scale).max(3.0);
    let track_y = rect.y + rect.h * 0.5 - track_h * 0.5;
    let accent = if enabled {
        Color::new(0.35, 0.9, 1.0, 1.0)
    } else {
        Color::new(0.35, 0.4, 0.5, 1.0)
    };

    draw_rectangle(
        rect.x,
        track_y,
        rect.w,
        track_h,
        Color::new(0.015, 0.018, 0.03, 0.95),
    );
    draw_rectangle(rect.x, track_y, rect.w * value, track_h, fade(accent, 0.82));
    draw_rectangle_lines(rect.x, track_y, rect.w, track_h, 1.0, fade(accent, 0.45));

    let knob_x = rect.x + rect.w * value;
    draw_circle(knob_x, rect.y + rect.h * 0.5, 7.0 * scale, accent);
    draw_circle_lines(
        knob_x,
        rect.y + rect.h * 0.5,
        7.0 * scale,
        1.0,
        fade(WHITE, 0.7),
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

    #[test]
    fn ellipse_has_center_when_filled() {
        let cells = ellipse_cells((1, 1), (5, 5), true);
        assert!(cells.contains(&(3, 3)));
    }
}
