use eframe::egui::{Color32, Visuals};

/// Centralized theme colors derived from egui's visual style (dark/light mode).
/// This avoids scattering color literals across UI code and makes re-theming
/// a single point of change.
#[derive(Clone, Debug)]
pub struct AppTheme {
    pub brand_color: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub border_color: Color32,
    pub success_color: Color32,
    pub btn_bg: Color32,
    pub danger_bg: Color32,
    pub danger_border: Color32,
    pub muted_color: Color32,
    #[allow(dead_code)]
    pub is_dark: bool,
}

impl AppTheme {
    pub fn from_visuals(visuals: &Visuals) -> Self {
        let is_dark = visuals.dark_mode;
        Self {
            brand_color: Color32::from_rgb(0, 102, 255),
            text_primary: if is_dark {
                Color32::from_rgb(220, 220, 220)
            } else {
                Color32::from_rgb(51, 51, 51)
            },
            text_secondary: if is_dark {
                Color32::from_rgb(160, 160, 160)
            } else {
                Color32::from_rgb(100, 100, 100)
            },
            border_color: if is_dark {
                Color32::from_rgb(80, 80, 80)
            } else {
                Color32::from_rgb(193, 213, 225)
            },
            success_color: Color32::from_rgb(44, 203, 91),
            btn_bg: if is_dark {
                Color32::from_rgb(60, 60, 60)
            } else {
                Color32::from_rgb(229, 231, 235)
            },
            danger_bg: Color32::from_rgb(255, 200, 200),
            danger_border: Color32::from_rgb(200, 100, 100),
            muted_color: Color32::from_rgb(180, 180, 180),
            is_dark,
        }
    }
}
