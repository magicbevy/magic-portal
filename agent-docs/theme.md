# Theme

Magic Portal must use the same egui version and visual style as Magic Bevy.

## Versions

```toml
eframe = "0.36"
egui = "0.36"
egui-phosphor = "0.14"
```

Use `egui_dock = "0.21"` only if the portal needs docking.

## Setup

After creating the egui context:

```rust
let mut fonts = egui::FontDefinitions::default();
egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
ctx.set_fonts(fonts);

EngineTheme::apply(&ctx);
```

Call `EngineTheme::apply` before building the UI.

## Theme Code

The theme is implemented in `src/theme.rs`. Keep it in sync with Magic Bevy.

```rust
#![allow(dead_code)]

use egui::{Color32, CornerRadius, Margin, Stroke, Vec2};

pub struct EngineTheme;

impl EngineTheme {
    pub const BG_DEEP: Color32 = Color32::from_rgb(12, 13, 18);
    pub const BG_DARK: Color32 = Color32::from_rgb(16, 17, 22);
    pub const BG_PANEL: Color32 = Color32::from_rgb(22, 24, 31);
    pub const BG_BAR: Color32 = Color32::from_rgb(28, 31, 40);

    pub const ITEM_BG: Color32 = Color32::from_rgb(30, 33, 43);
    pub const ITEM_HOVER: Color32 = Color32::from_rgb(38, 43, 56);
    pub const ITEM_SELECTED: Color32 = Color32::from_rgb(42, 48, 75);

    pub const BORDER: Color32 = Color32::from_rgb(45, 50, 65);
    pub const BORDER_ACTIVE: Color32 = Color32::from_rgb(110, 86, 240);

    pub const ACCENT: Color32 = Color32::from_rgb(110, 86, 240);
    pub const ACCENT_CYAN: Color32 = Color32::from_rgb(56, 189, 248);

    pub const WARNING: Color32 = Color32::from_rgb(245, 185, 66);
    pub const DANGER: Color32 = Color32::from_rgb(239, 68, 68);

    pub const TEXT_MAIN: Color32 = Color32::from_rgb(238, 240, 246);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(145, 152, 170);

    pub fn apply(ctx: &egui::Context) {
        let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
        let mut visuals = egui::Visuals::dark();

        visuals.dark_mode = true;

        visuals.panel_fill = Self::BG_DARK;
        visuals.window_fill = Self::BG_PANEL;
        visuals.extreme_bg_color = Self::BG_DEEP;
        visuals.faint_bg_color = Color32::from_rgb(20, 22, 29);
        visuals.code_bg_color = Self::BG_DEEP;

        visuals.hyperlink_color = Self::ACCENT_CYAN;
        visuals.warn_fg_color = Self::WARNING;
        visuals.error_fg_color = Self::DANGER;
        visuals.selection.bg_fill = Self::ITEM_SELECTED;
        visuals.selection.stroke = Stroke::new(1.0, Self::ACCENT_CYAN);

        visuals.window_corner_radius = CornerRadius::same(8);
        visuals.window_stroke = Stroke::new(1.0, Self::BORDER);
        visuals.window_highlight_topmost = true;
        visuals.menu_corner_radius = CornerRadius::same(6);

        visuals.window_shadow = egui::epaint::Shadow {
            offset: [0, 8],
            blur: 20,
            spread: 2,
            color: Color32::from_black_alpha(140),
        };
        visuals.popup_shadow = egui::epaint::Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(120),
        };

        visuals.widgets.noninteractive.bg_fill = Self::BG_PANEL;
        visuals.widgets.noninteractive.weak_bg_fill = Self::BG_PANEL;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Self::BORDER);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Self::TEXT_MAIN);
        visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);

        visuals.widgets.inactive.bg_fill = Self::ITEM_BG;
        visuals.widgets.inactive.weak_bg_fill = Self::ITEM_BG;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Self::BORDER);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Self::TEXT_MAIN);
        visuals.widgets.inactive.corner_radius = CornerRadius::same(5);
        visuals.widgets.inactive.expansion = 0.0;

        visuals.widgets.hovered.bg_fill = Self::ITEM_HOVER;
        visuals.widgets.hovered.weak_bg_fill = Self::ITEM_HOVER;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Self::BORDER_ACTIVE);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
        visuals.widgets.hovered.corner_radius = CornerRadius::same(5);
        visuals.widgets.hovered.expansion = 1.0;

        visuals.widgets.active.bg_fill = Self::ITEM_SELECTED;
        visuals.widgets.active.weak_bg_fill = Self::ACCENT;
        visuals.widgets.active.bg_stroke = Stroke::new(1.5, Self::ACCENT_CYAN);
        visuals.widgets.active.fg_stroke = Stroke::new(2.0, Color32::WHITE);
        visuals.widgets.active.corner_radius = CornerRadius::same(5);
        visuals.widgets.active.expansion = 1.0;

        visuals.widgets.open.bg_fill = Self::BG_BAR;
        visuals.widgets.open.weak_bg_fill = Self::BG_BAR;
        visuals.widgets.open.bg_stroke = Stroke::new(1.0, Self::BORDER_ACTIVE);
        visuals.widgets.open.fg_stroke = Stroke::new(1.0, Self::ACCENT_CYAN);
        visuals.widgets.open.corner_radius = CornerRadius::same(5);
        visuals.widgets.open.expansion = 0.0;

        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.window_margin = Margin::same(10);
        style.spacing.menu_margin = Margin::same(6);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        style.spacing.indent = 18.0;
        style.spacing.interact_size = Vec2::new(40.0, 22.0);
        style.spacing.scroll.bar_width = 8.0;

        style.visuals = visuals;
        ctx.set_style_of(egui::Theme::Dark, style.clone());
        ctx.set_style_of(egui::Theme::Light, style);
    }
}
```

## Color Table

| Name | RGB | Usage |
|---|---|---|
| `BG_DEEP` | 12, 13, 18 | Deep background |
| `BG_DARK` | 16, 17, 22 | Panels |
| `BG_PANEL` | 22, 24, 31 | Cards and windows |
| `BG_BAR` | 28, 31, 40 | Bars |
| `ITEM_BG` | 30, 33, 43 | Normal widget |
| `ITEM_HOVER` | 38, 43, 56 | Hover |
| `ITEM_SELECTED` | 42, 48, 75 | Selection |
| `BORDER` | 45, 50, 65 | Normal border |
| `BORDER_ACTIVE` | 110, 86, 240 | Active border |
| `ACCENT` | 110, 86, 240 | Accent |
| `ACCENT_CYAN` | 56, 189, 248 | Secondary accent |
| `WARNING` | 245, 185, 66 | Warning |
| `DANGER` | 239, 68, 68 | Error |
| `TEXT_MAIN` | 238, 240, 246 | Main text |
| `TEXT_MUTED` | 145, 152, 170 | Muted text |

## Rules

- Do not change these colors without user approval.
- Do not use the default egui dark theme.
- Set both `Theme::Dark` and `Theme::Light` to the same style, exactly like Magic Bevy.
- Use `egui-phosphor` icons if the portal uses icons.
- Keep the theme in `src/theme.rs` so the UI and any future windows share it.
- Do not copy Magic Bevy UI code that depends on engine-only types.