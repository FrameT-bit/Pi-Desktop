#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Yellow,
    Brown,
    Red,
    Pink,
    Orange,
    Green,
    Blue,
    Purple,
    Neutral,
    Dark,
}

#[derive(Clone, Copy)]
pub struct ThemeSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub variant: u8,
    /// Paper color behind the conversation. Every flavor gets its own tint.
    pub page: &'static str,
    /// Ink/seed accent for controls, links, selections and highlights.
    pub accent: &'static str,
}

const fn t(
    id: &'static str,
    name: &'static str,
    category: Category,
    variant: u8,
    page: &'static str,
    accent: &'static str,
) -> ThemeSpec {
    ThemeSpec {
        id,
        name,
        category,
        variant,
        page,
        accent,
    }
}

impl Category {
    pub fn is_dark(self) -> bool {
        matches!(self, Category::Dark)
    }

    pub fn group(self) -> &'static str {
        match self {
            Category::Yellow => "Yellow / Cream",
            Category::Brown => "Brown / Earthy",
            Category::Red => "Red",
            Category::Pink => "Pink",
            Category::Orange => "Orange",
            Category::Green => "Green",
            Category::Blue => "Blue",
            Category::Purple => "Purple",
            Category::Neutral => "Neutral / Light",
            Category::Dark => "Dark",
        }
    }
}

pub const THEMES: &[ThemeSpec] = &[
    t("lemon-moussie", "Lemon Moussie", Category::Yellow, 0, "#fbf7c7", "#856300"),
    t("butter-croissant", "Butter Croissant", Category::Yellow, 1, "#fbf0d2", "#94620f"),
    t("custard-cream", "Custard Cream", Category::Yellow, 2, "#faf1d8", "#8f6715"),
    t("banana-pudding", "Banana Pudding", Category::Yellow, 3, "#f8f0cf", "#75630f"),
    t("honey-glaze", "Honey Glaze", Category::Yellow, 4, "#f7ecd5", "#8a5707"),
    t("brownie", "Brownie", Category::Brown, 0, "#eee1d4", "#5b3524"),
    t("mocha-latte", "Mocha Latte", Category::Brown, 1, "#f1e4d8", "#6d4630"),
    t("caramel-drizzle", "Caramel Drizzle", Category::Brown, 2, "#f8e9d5", "#8f4d0a"),
    t("hazelnut-praline", "Hazelnut Praline", Category::Brown, 3, "#f2e4d5", "#81512f"),
    t("cinnamon-roll", "Cinnamon Roll", Category::Brown, 4, "#f8e7d3", "#9b4e1d"),
    t("cherry-cola", "Cherry Cola", Category::Red, 0, "#f0dfe1", "#7d1d2d"),
    t("strawberry-coulis", "Strawberry Coulis", Category::Red, 1, "#ffe3e8", "#b8233c"),
    t("red-velvet", "Red Velvet", Category::Red, 2, "#f6dfdb", "#a11f2e"),
    t("raspberry-tart", "Raspberry Tart", Category::Red, 3, "#f7e1e9", "#a51d52"),
    t("tomato-bisque", "Tomato Bisque", Category::Red, 4, "#fae6da", "#b84024"),
    t("cotton-candy", "Cotton Candy", Category::Pink, 0, "#fbe7f3", "#b83a79"),
    t("peach-sorbet", "Peach Sorbet", Category::Pink, 1, "#fde9de", "#a84834"),
    t("watermelon-fizz", "Watermelon Fizz", Category::Pink, 2, "#fbe5e5", "#c42f4b"),
    t("rose-macaron", "Rose Macaron", Category::Pink, 3, "#f7e7ee", "#a94b68"),
    t("bubblegum-parfait", "Bubblegum Parfait", Category::Pink, 4, "#fbe4f0", "#b0206b"),
    t("apricot-jam", "Apricot Jam", Category::Orange, 0, "#fbe7d3", "#94480f"),
    t("mango-lassi", "Mango Lassi", Category::Orange, 1, "#fdf0cf", "#856000"),
    t("tangerine-zest", "Tangerine Zest", Category::Orange, 2, "#fce8d3", "#a13d08"),
    t("pumpkin-spice", "Pumpkin Spice", Category::Orange, 3, "#f7e3d1", "#93440f"),
    t("creamsicle", "Creamsicle", Category::Orange, 4, "#fff0d7", "#a84d0b"),
    t("pistachio-gelato", "Pistachio Gelato", Category::Green, 0, "#eef2d8", "#58752d"),
    t("matcha-mochi", "Matcha Mochi", Category::Green, 1, "#e9efd8", "#4f6b25"),
    t("key-lime-pie", "Key Lime Pie", Category::Green, 2, "#edf5d0", "#526b08"),
    t("avocado-toast", "Avocado Toast", Category::Green, 3, "#edf0d8", "#5a712f"),
    t("mint-chip", "Mint Chip", Category::Green, 4, "#e2f1e6", "#26704f"),
    t("blueberry-muffin", "Blueberry Muffin", Category::Blue, 0, "#e6e9f6", "#3f4f8c"),
    t("blue-raspberry-slush", "Blue Raspberry Slush", Category::Blue, 1, "#def1fb", "#176c9e"),
    t("cookies-and-cream", "Cookies & Cream", Category::Blue, 2, "#eceff0", "#4b5d68"),
    t("blue-curacao", "Blue Curacao", Category::Blue, 3, "#d8f3f7", "#006d84"),
    t("bluebell-gelato", "Bluebell Gelato", Category::Blue, 4, "#e6e9fb", "#4e5db0"),
    t("grape-soda", "Grape Soda", Category::Purple, 0, "#f1e3f4", "#75308d"),
    t("ube-latte", "Ube Latte", Category::Purple, 1, "#f0e7f2", "#814786"),
    t("blackberry-cobbler", "Blackberry Cobbler", Category::Purple, 2, "#eee2ef", "#5a2b62"),
    t("lavender-honey", "Lavender Honey", Category::Purple, 3, "#f3eef8", "#76589f"),
    t("plum-wine", "Plum Wine", Category::Purple, 4, "#f0e2e9", "#642b46"),
    t("vanilla-bean", "Vanilla Bean", Category::Neutral, 0, "#faf6ea", "#7d6634"),
    t("marshmallow", "Marshmallow", Category::Neutral, 1, "#fafafa", "#6e6962"),
    t("oat-milk", "Oat Milk", Category::Neutral, 2, "#f7f2e7", "#7a6547"),
    t("meringue", "Meringue", Category::Neutral, 3, "#fbf8ef", "#7f6c4a"),
    t("coconut-cream", "Coconut Cream", Category::Neutral, 4, "#f8f6ee", "#7c6d55"),
    t("black-sesame", "Black Sesame", Category::Dark, 0, "#141210", "#d8b25e"),
    t("espresso-shot", "Espresso Shot", Category::Dark, 1, "#251a14", "#c98a4b"),
    t("dark-chocolate-ganache", "Dark Chocolate Ganache", Category::Dark, 2, "#1e1314", "#b87858"),
    t("licorice-twist", "Licorice Twist", Category::Dark, 3, "#181419", "#b497d6"),
    t("midnight-fudge", "Midnight Fudge", Category::Dark, 4, "#0f1524", "#86a2de"),
];

pub fn find(value: &str) -> Option<usize> {
    let value = value.trim().to_ascii_lowercase().replace(' ', "-");
    THEMES
        .iter()
        .position(|theme| theme.id == value || theme.name.to_ascii_lowercase() == value.replace('-', " "))
}

pub fn at(index: usize) -> Option<&'static ThemeSpec> {
    THEMES.get(index)
}

pub fn tokens(index: usize) -> Vec<(String, String)> {
    let Some(theme) = at(index) else {
        return Vec::new();
    };
    let dark = theme.category.is_dark();
    let page = theme.page.to_string();
    let accent = theme.accent.to_string();
    // Each flavor owns its paper color. Canvas and cards are derived from that
    // flavor instead of from one shared palette for the whole category.
    let canvas = if dark {
        mix_hex(&page, "#ffffff", 0.045)
    } else {
        mix_hex(&page, "#ffffff", 0.38)
    };
    let card = if dark {
        mix_hex(&page, "#ffffff", 0.085)
    } else {
        mix_hex(&page, "#ffffff", 0.73)
    };
    let ink = if dark { "#f5efe7" } else { "#211b18" };
    let text_base = mix_hex(ink, &page, 0.18);
    let muted = mix_hex(ink, &page, 0.36);
    let dim = mix_hex(ink, &page, 0.62);
    let border = mix_hex(ink, &page, 0.20);
    let border_muted = mix_hex(ink, &page, 0.12);
    let panel = mix_hex(ink, &page, 0.36);
    let selected = mix_hex(&card, &accent, if dark { 0.30 } else { 0.22 });
    let user_bg = mix_hex(&card, &accent, if dark { 0.10 } else { 0.085 });
    let user_border = mix_hex(&card, &accent, if dark { 0.34 } else { 0.30 });
    let tool_success = mix_hex(&card, "#35704a", if dark { 0.22 } else { 0.10 });
    let tool_error = mix_hex(&card, "#a34428", if dark { 0.22 } else { 0.10 });
    let out = vec![
        ("accent".into(), accent.clone()),
        ("pageBg".into(), page.clone()),
        ("canvas".into(), canvas.clone()),
        ("cardBg".into(), card.clone()),
        ("cardBgSoft".into(), mix_hex(&card, &accent, 0.09)),
        ("infoBg".into(), mix_hex(&card, &accent, 0.06)),
        ("userMessageBg".into(), user_bg),
        ("userMessageBorder".into(), user_border),
        ("customMessageBg".into(), mix_hex(&card, &accent, 0.07)),
        ("toolPendingBg".into(), card.clone()),
        ("toolSuccessBg".into(), tool_success),
        ("toolErrorBg".into(), tool_error),
        ("toolTitle".into(), ink.into()),
        ("toolOutput".into(), text_base.clone()),
        ("mdHeading".into(), ink.into()),
        ("mdLink".into(), accent.clone()),
        ("mdCode".into(), ink.into()),
        // mdCodeBlock is foreground ink, not a code-card background.
        ("mdCodeBlock".into(), text_base.clone()),
        ("mdCodeBlockBorder".into(), border_muted.clone()),
        ("mdQuote".into(), muted.clone()),
        ("mdQuoteBorder".into(), border_muted.clone()),
        ("mdHr".into(), border_muted.clone()),
        ("mdListBullet".into(), accent.clone()),
        ("toolDiffContext".into(), muted.clone()),
        ("text".into(), ink.into()),
        ("textBase".into(), text_base.clone()),
        ("muted".into(), muted.clone()),
        ("dim".into(), dim),
        ("selectedBg".into(), selected),
        ("scrollbarTrack".into(), page.clone()),
        ("scrollbarThumb".into(), border.clone()),
        ("searchMatchBg".into(), with_alpha(&accent, 0x26)),
        ("searchMatchText".into(), ink.into()),
        ("userMessageText".into(), ink.into()),
        ("customMessageText".into(), text_base.clone()),
        ("customMessageLabel".into(), accent.clone()),
        ("thinkingText".into(), muted.clone()),
        ("thinkingMedium".into(), accent.clone()),
        ("border".into(), border),
        ("borderAccent".into(), accent.clone()),
        ("borderMuted".into(), border_muted),
        ("panelBorder".into(), panel),
        ("threadBlue".into(), accent),
    ];
    out
}

fn mix_hex(a: &str, b: &str, t: f32) -> String {
    let parse = |hex: &str| {
        let digits = hex.trim_start_matches('#');
        let channel = |index: usize| u8::from_str_radix(&digits[index..index + 2], 16).unwrap_or(0) as f32;
        (channel(0), channel(2), channel(4))
    };
    let (ar, ag, ab) = parse(a);
    let (br, bg, bb) = parse(b);
    let t = t.clamp(0.0, 1.0);
    let c = |a: f32, b: f32| ((a + (b - a) * t).round() as u8).min(255);
    format!("#{:02x}{:02x}{:02x}", c(ar, br), c(ag, bg), c(ab, bb))
}

fn with_alpha(hex: &str, alpha: u8) -> String {
    let digits = hex.trim_start_matches('#');
    let channel = |index: usize| u8::from_str_radix(&digits[index..index + 2], 16).unwrap_or(0);
    format!(
        "rgba({}, {}, {}, {:.3})",
        channel(0),
        channel(2),
        channel(4),
        alpha as f32 / 255.0
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn luminance(hex: &str) -> f32 {
        let digits = hex.trim_start_matches('#');
        let channel = |index: usize| {
            let value = u8::from_str_radix(&digits[index..index + 2], 16).unwrap_or(0) as f32 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4)
    }

    fn contrast(a: &str, b: &str) -> f32 {
        let (light, dark) = {
            let (a, b) = (luminance(a), luminance(b));
            if a > b { (a, b) } else { (b, a) }
        };
        (light + 0.05) / (dark + 0.05)
    }

    #[test]
    fn every_flavor_has_its_own_surface_and_accent() {
        let pages: HashSet<_> = THEMES.iter().map(|theme| theme.page).collect();
        let accents: HashSet<_> = THEMES.iter().map(|theme| theme.accent).collect();
        assert_eq!(pages.len(), THEMES.len(), "duplicate surface color");
        assert_eq!(accents.len(), THEMES.len(), "duplicate accent color");
    }

    #[test]
    fn accents_stay_readable_on_their_paper() {
        for theme in THEMES {
            let ratio = contrast(theme.accent, theme.page);
            assert!(
                ratio >= 4.5,
                "{} has only {:.2}:1 accent contrast",
                theme.name,
                ratio
            );
        }
    }

    #[test]
    fn card_content_has_readable_contrast_in_every_theme() {
        for index in 0..THEMES.len() {
            let colors: std::collections::HashMap<_, _> = tokens(index).into_iter().collect();
            for (foreground, background) in [
                ("mdCodeBlock", "cardBg"), ("toolOutput", "cardBg"),
                ("customMessageText", "customMessageBg"), ("userMessageText", "userMessageBg"),
                ("thinkingText", "cardBg"),
            ] {
                let ratio = contrast(&colors[foreground], &colors[background]);
                assert!(ratio >= 4.5, "{} {foreground}/{background}: {ratio:.2}:1", THEMES[index].id);
            }
        }
    }
}
