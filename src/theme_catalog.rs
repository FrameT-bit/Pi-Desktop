
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
}

impl Category {
    fn palette(self) -> (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str, bool) {
        match self {
            Category::Yellow => ("#f7f1dc", "#fcf7e8", "#fffdf4", "#2b2517", "#9a6b17", "#c09535", false),
            Category::Brown => ("#efe4d8", "#f8f0e7", "#fffaf5", "#2e2118", "#7a4f35", "#a9754f", false),
            Category::Red => ("#f7e7e4", "#fdf3f1", "#fffaf9", "#301b18", "#a83b3a", "#c95b55", false),
            Category::Pink => ("#f8e9ef", "#fdf4f8", "#fffafd", "#311c25", "#a94e70", "#ce7897", false),
            Category::Orange => ("#f9ecdf", "#fdf6ef", "#fffaf6", "#332016", "#b05a28", "#d48647", false),
            Category::Green => ("#e9f1e5", "#f5faf2", "#fbfff9", "#1e2b1b", "#4c7448", "#7da36a", false),
            Category::Blue => ("#e7eff5", "#f3f8fc", "#fbfdff", "#182632", "#3f6b92", "#6d94b5", false),
            Category::Purple => ("#f0e9f5", "#f9f5fc", "#fffbff", "#281d31", "#6f4f8c", "#9878b3", false),
            Category::Neutral => ("#efefef", "#f8f8f8", "#ffffff", "#242424", "#606060", "#8a8a8a", false),
            Category::Dark => ("#10100f", "#181816", "#22221f", "#f2ede6", "#c9a24a", "#e0c078", true),
        }
    }

    pub fn is_dark(self) -> bool {
        self.palette().6
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

    fn accent(self, variant: u8) -> String {
        let (_, _, _, _, a, b, _) = self.palette();
        let t = variant.min(4) as f32 / 4.0;
        mix_hex(a, b, t)
    }
}

pub const THEMES: &[ThemeSpec] = &[
    ThemeSpec { id: "lemon-moussie", name: "Lemon Moussie", category: Category::Yellow, variant: 0 },
    ThemeSpec { id: "butter-croissant", name: "Butter Croissant", category: Category::Yellow, variant: 1 },
    ThemeSpec { id: "custard-cream", name: "Custard Cream", category: Category::Yellow, variant: 2 },
    ThemeSpec { id: "banana-pudding", name: "Banana Pudding", category: Category::Yellow, variant: 3 },
    ThemeSpec { id: "honey-glaze", name: "Honey Glaze", category: Category::Yellow, variant: 4 },
    ThemeSpec { id: "brownie", name: "Brownie", category: Category::Brown, variant: 0 },
    ThemeSpec { id: "mocha-latte", name: "Mocha Latte", category: Category::Brown, variant: 1 },
    ThemeSpec { id: "caramel-drizzle", name: "Caramel Drizzle", category: Category::Brown, variant: 2 },
    ThemeSpec { id: "hazelnut-praline", name: "Hazelnut Praline", category: Category::Brown, variant: 3 },
    ThemeSpec { id: "cinnamon-roll", name: "Cinnamon Roll", category: Category::Brown, variant: 4 },
    ThemeSpec { id: "cherry-cola", name: "Cherry Cola", category: Category::Red, variant: 0 },
    ThemeSpec { id: "strawberry-coulis", name: "Strawberry Coulis", category: Category::Red, variant: 1 },
    ThemeSpec { id: "red-velvet", name: "Red Velvet", category: Category::Red, variant: 2 },
    ThemeSpec { id: "raspberry-tart", name: "Raspberry Tart", category: Category::Red, variant: 3 },
    ThemeSpec { id: "tomato-bisque", name: "Tomato Bisque", category: Category::Red, variant: 4 },
    ThemeSpec { id: "cotton-candy", name: "Cotton Candy", category: Category::Pink, variant: 0 },
    ThemeSpec { id: "peach-sorbet", name: "Peach Sorbet", category: Category::Pink, variant: 1 },
    ThemeSpec { id: "watermelon-fizz", name: "Watermelon Fizz", category: Category::Pink, variant: 2 },
    ThemeSpec { id: "rose-macaron", name: "Rose Macaron", category: Category::Pink, variant: 3 },
    ThemeSpec { id: "bubblegum-parfait", name: "Bubblegum Parfait", category: Category::Pink, variant: 4 },
    ThemeSpec { id: "apricot-jam", name: "Apricot Jam", category: Category::Orange, variant: 0 },
    ThemeSpec { id: "mango-lassi", name: "Mango Lassi", category: Category::Orange, variant: 1 },
    ThemeSpec { id: "tangerine-zest", name: "Tangerine Zest", category: Category::Orange, variant: 2 },
    ThemeSpec { id: "pumpkin-spice", name: "Pumpkin Spice", category: Category::Orange, variant: 3 },
    ThemeSpec { id: "creamsicle", name: "Creamsicle", category: Category::Orange, variant: 4 },
    ThemeSpec { id: "pistachio-gelato", name: "Pistachio Gelato", category: Category::Green, variant: 0 },
    ThemeSpec { id: "matcha-mochi", name: "Matcha Mochi", category: Category::Green, variant: 1 },
    ThemeSpec { id: "key-lime-pie", name: "Key Lime Pie", category: Category::Green, variant: 2 },
    ThemeSpec { id: "avocado-toast", name: "Avocado Toast", category: Category::Green, variant: 3 },
    ThemeSpec { id: "mint-chip", name: "Mint Chip", category: Category::Green, variant: 4 },
    ThemeSpec { id: "blueberry-muffin", name: "Blueberry Muffin", category: Category::Blue, variant: 0 },
    ThemeSpec { id: "blue-raspberry-slush", name: "Blue Raspberry Slush", category: Category::Blue, variant: 1 },
    ThemeSpec { id: "cookies-and-cream", name: "Cookies & Cream", category: Category::Blue, variant: 2 },
    ThemeSpec { id: "blue-curacao", name: "Blue Curacao", category: Category::Blue, variant: 3 },
    ThemeSpec { id: "bluebell-gelato", name: "Bluebell Gelato", category: Category::Blue, variant: 4 },
    ThemeSpec { id: "grape-soda", name: "Grape Soda", category: Category::Purple, variant: 0 },
    ThemeSpec { id: "ube-latte", name: "Ube Latte", category: Category::Purple, variant: 1 },
    ThemeSpec { id: "blackberry-cobbler", name: "Blackberry Cobbler", category: Category::Purple, variant: 2 },
    ThemeSpec { id: "lavender-honey", name: "Lavender Honey", category: Category::Purple, variant: 3 },
    ThemeSpec { id: "plum-wine", name: "Plum Wine", category: Category::Purple, variant: 4 },
    ThemeSpec { id: "vanilla-bean", name: "Vanilla Bean", category: Category::Neutral, variant: 0 },
    ThemeSpec { id: "marshmallow", name: "Marshmallow", category: Category::Neutral, variant: 1 },
    ThemeSpec { id: "oat-milk", name: "Oat Milk", category: Category::Neutral, variant: 2 },
    ThemeSpec { id: "meringue", name: "Meringue", category: Category::Neutral, variant: 3 },
    ThemeSpec { id: "coconut-cream", name: "Coconut Cream", category: Category::Neutral, variant: 4 },
    ThemeSpec { id: "black-sesame", name: "Black Sesame", category: Category::Dark, variant: 0 },
    ThemeSpec { id: "espresso-shot", name: "Espresso Shot", category: Category::Dark, variant: 1 },
    ThemeSpec { id: "dark-chocolate-ganache", name: "Dark Chocolate Ganache", category: Category::Dark, variant: 2 },
    ThemeSpec { id: "licorice-twist", name: "Licorice Twist", category: Category::Dark, variant: 3 },
    ThemeSpec { id: "midnight-fudge", name: "Midnight Fudge", category: Category::Dark, variant: 4 },
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
    let (page, canvas, card, ink, _, _, dark) = theme.category.palette();
    let accent = theme.category.accent(theme.variant);
    let variant = theme.variant as f32;
    let (page_strength, card_strength) = if dark {
        (0.04 + 0.035 * variant, 0.03 + 0.03 * variant)
    } else {
        (0.025 + 0.025 * variant, 0.015 + 0.025 * variant)
    };
    let page = mix_hex(page, &accent, page_strength);
    let canvas = mix_hex(canvas, &accent, page_strength * 0.72);
    let card = mix_hex(&card, &accent, card_strength);
    let text_base = mix_hex(ink, &page, 0.18);
    let muted = mix_hex(ink, &page, 0.44);
    let dim = mix_hex(ink, &page, 0.62);
    let border = mix_hex(ink, &page, 0.20);
    let border_muted = mix_hex(ink, &page, 0.12);
    let panel = mix_hex(ink, &page, 0.36);
    let selected = mix_hex(&card, &accent, if dark { 0.26 } else { 0.16 });
    let user_bg = mix_hex(&card, &accent, if dark { 0.06 } else { 0.035 });
    let user_border = mix_hex(&card, &accent, if dark { 0.28 } else { 0.18 });
    let tool_success = mix_hex(&card, "#35704a", if dark { 0.22 } else { 0.10 });
    let tool_error = mix_hex(&card, "#a34428", if dark { 0.22 } else { 0.10 });
    let code_block = mix_hex(&card, ink, if dark { 0.14 } else { 0.06 });
    let mut out = vec![
        ("accent".into(), accent.clone()),
        ("pageBg".into(), page.clone()),
        ("canvas".into(), canvas.clone()),
        ("cardBg".into(), card.clone()),
        ("cardBgSoft".into(), mix_hex(&card, &accent, 0.06)),
        ("infoBg".into(), mix_hex(&card, &accent, 0.04)),
        ("userMessageBg".into(), user_bg),
        ("userMessageBorder".into(), user_border),
        ("customMessageBg".into(), mix_hex(&card, &accent, 0.045)),
        ("toolPendingBg".into(), card.clone()),
        ("toolSuccessBg".into(), tool_success),
        ("toolErrorBg".into(), tool_error),
        ("toolTitle".into(), ink.into()),
        ("toolOutput".into(), muted.clone()),
        ("mdHeading".into(), ink.into()),
        ("mdLink".into(), accent.clone()),
        ("mdCode".into(), ink.into()),
        ("mdCodeBlock".into(), mix_hex(&card, ink, 0.08)),
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
    if let Some(entry) = out.iter_mut().find(|(key, _)| key == "mdCodeBlock") {
        entry.1 = code_block;
    }
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


