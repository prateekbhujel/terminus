#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrimaryColor {
    #[default]
    Emerald,
    Indigo,
    Violet,
    Amber,
    Rose,
    Cyan,
    Zinc,
}

impl PrimaryColor {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Emerald => "emerald",
            Self::Indigo => "indigo",
            Self::Violet => "violet",
            Self::Amber => "amber",
            Self::Rose => "rose",
            Self::Cyan => "cyan",
            Self::Zinc => "zinc",
        }
    }

    pub fn hex(&self) -> &'static str {
        match self {
            Self::Emerald => "#10b981",
            Self::Indigo => "#6366f1",
            Self::Violet => "#8b5cf6",
            Self::Amber => "#f59e0b",
            Self::Rose => "#f43f5e",
            Self::Cyan => "#06b6d4",
            Self::Zinc => "#71717a",
        }
    }

    pub fn hover_hex(&self) -> &'static str {
        match self {
            Self::Emerald => "#059669",
            Self::Indigo => "#4f46e5",
            Self::Violet => "#7c3aed",
            Self::Amber => "#d97706",
            Self::Rose => "#e11d48",
            Self::Cyan => "#0891b2",
            Self::Zinc => "#52525b",
        }
    }

    pub fn light_bg(&self) -> &'static str {
        match self {
            Self::Emerald => "rgba(16, 185, 129, 0.12)",
            Self::Indigo => "rgba(99, 102, 241, 0.12)",
            Self::Violet => "rgba(139, 92, 246, 0.12)",
            Self::Amber => "rgba(245, 158, 11, 0.12)",
            Self::Rose => "rgba(244, 63, 94, 0.12)",
            Self::Cyan => "rgba(6, 182, 212, 0.12)",
            Self::Zinc => "rgba(113, 113, 122, 0.12)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontFamily {
    #[default]
    Geist,
    Inter,
    Outfit,
    PlusJakartaSans,
}

impl FontFamily {
    pub fn css_url(&self) -> &'static str {
        match self {
            Self::Geist => "https://cdn.jsdelivr.net/npm/geist@1.3.0/dist/fonts/geist-sans/style.css",
            Self::Inter => "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap",
            Self::Outfit => "https://fonts.googleapis.com/css2?family=Outfit:wght@400;500;600;700&display=swap",
            Self::PlusJakartaSans => "https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700&display=swap",
        }
    }

    pub fn font_family_css(&self) -> &'static str {
        match self {
            Self::Geist => "'Geist Sans', system-ui, -apple-system, sans-serif",
            Self::Inter => "'Inter', system-ui, -apple-system, sans-serif",
            Self::Outfit => "'Outfit', system-ui, -apple-system, sans-serif",
            Self::PlusJakartaSans => "'Plus Jakarta Sans', system-ui, -apple-system, sans-serif",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub brand_name: String,
    pub brand_logo: Option<String>,
    pub primary_color: PrimaryColor,
    pub font_family: FontFamily,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            brand_name: "Terminus".to_string(),
            brand_logo: None,
            primary_color: PrimaryColor::Emerald,
            font_family: FontFamily::Geist,
        }
    }
}

impl ThemeConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn brand_name(mut self, name: impl Into<String>) -> Self {
        self.brand_name = name.into();
        self
    }

    pub fn brand_logo(mut self, logo_svg_or_html: impl Into<String>) -> Self {
        self.brand_logo = Some(logo_svg_or_html.into());
        self
    }

    pub fn primary_color(mut self, color: PrimaryColor) -> Self {
        self.primary_color = color;
        self
    }

    pub fn font_family(mut self, font: FontFamily) -> Self {
        self.font_family = font;
        self
    }
}
