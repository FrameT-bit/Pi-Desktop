
use std::cell::Cell;

use gtk::prelude::*;

use crate::tokens as t;

const MAX_CHARS: i32 = 78;
const LINE_HEIGHT: &str = "1.65";

thread_local! {
    static WIDTH: Cell<i32> = const { Cell::new(MAX_CHARS) };
}

pub fn set_width(width: i32) {
    WIDTH.with(|slot| slot.set(width.clamp(40, 160)));
}

fn width() -> i32 {
    WIDTH.with(|slot| slot.get())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnAlign {
    Left,
    Center,
    Right,
}

impl ColumnAlign {
    fn xalign(self) -> f32 {
        match self {
            ColumnAlign::Left => 0.0,
            ColumnAlign::Center => 0.5,
            ColumnAlign::Right => 1.0,
        }
    }

    fn justify(self) -> gtk::Justification {
        match self {
            ColumnAlign::Left => gtk::Justification::Left,
            ColumnAlign::Center => gtk::Justification::Center,
            ColumnAlign::Right => gtk::Justification::Right,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub align: Vec<ColumnAlign>,
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn render(text: &str) -> Vec<gtk::Widget> {
    let mut out: Vec<gtk::Widget> = Vec::new();
    let lines: Vec<&str> = text.split('\n').collect();
    let mut para: Vec<String> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            flush_paragraph(&mut para, &mut out);
            let mut code: Vec<&str> = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            out.push(code_block(&code.join("\n")));
            continue;
        }

        if trimmed.contains('|') {
            if let Some((table, next)) = parse_table(&lines, i) {
                flush_paragraph(&mut para, &mut out);
                out.push(table_widget(&table));
                i = next;
                continue;
            }
        }

        if trimmed.is_empty() {
            flush_paragraph(&mut para, &mut out);
            i += 1;
            continue;
        }

        if trimmed.len() > 1 && trimmed.starts_with('#') {
            let hashes = trimmed.chars().take_while(|c| *c == '#').count();
            if hashes <= 6 && trimmed.chars().nth(hashes) == Some(' ') {
                flush_paragraph(&mut para, &mut out);
                let level = hashes;
                let content = trimmed[hashes + 1..].trim();
                out.push(heading(level, content));
                i += 1;
                continue;
            }
        }

        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            flush_paragraph(&mut para, &mut out);
            let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
            sep.add_css_class("md-hr");
            sep.set_margin_top(4);
            sep.set_margin_bottom(4);
            out.push(sep.upcast());
            i += 1;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("> ") {
            flush_paragraph(&mut para, &mut out);
            out.push(quote(rest));
            i += 1;
            continue;
        }

        if let Some((indent, marker, rest)) = list_item(trimmed) {
            flush_paragraph(&mut para, &mut out);
            out.push(bullet(indent, marker, rest));
            i += 1;
            continue;
        }

        para.push(line.to_string());
        i += 1;
    }

    flush_paragraph(&mut para, &mut out);
    out
}

fn flush_paragraph(para: &mut Vec<String>, out: &mut Vec<gtk::Widget>) {
    if para.is_empty() {
        return;
    }
    let joined = para.join(" ");
    para.clear();
    out.push(paragraph(&joined));
}

fn base_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.set_halign(gtk::Align::Start);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_selectable(true);
    label.set_max_width_chars(width());
    label
}

fn paragraph(text: &str) -> gtk::Widget {
    let label = base_label();
    label.add_css_class("md-p");
    let inline = inline(text);
    label.set_markup(&format!(
        "<span line_height='{LINE_HEIGHT}'>{}</span>",
        inline.markup
    ));
    attach_links(&label, &inline);
    label.upcast()
}

fn heading(level: usize, text: &str) -> gtk::Widget {
    let label = base_label();
    label.add_css_class(&format!("md-h{level}"));
    let inline = inline(text);
    label.set_markup(&inline.markup);
    attach_links(&label, &inline);
    label.upcast()
}

fn quote(text: &str) -> gtk::Widget {
    let label = base_label();
    label.add_css_class("md-quote");
    let inline = inline(text);
    label.set_markup(&format!(
        "<span line_height='{LINE_HEIGHT}'>{}</span>",
        inline.markup
    ));
    attach_links(&label, &inline);
    label.upcast()
}

fn bullet(indent: usize, marker: &str, text: &str) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_halign(gtk::Align::Start);
    if indent > 0 {
        row.set_margin_start((indent * 12) as i32);
    }

    let bullet = gtk::Label::new(None);
    bullet.add_css_class("md-bullet");
    bullet.set_text(marker);
    bullet.set_valign(gtk::Align::Start);
    bullet.set_width_chars(2);
    bullet.set_xalign(1.0);

    let label = base_label();
    label.add_css_class("md-p");
    label.set_max_width_chars(width() - 2);
    let inline = inline(text);
    label.set_markup(&format!(
        "<span line_height='{LINE_HEIGHT}'>{}</span>",
        inline.markup
    ));
    attach_links(&label, &inline);

    row.append(&bullet);
    row.append(&label);
    row.upcast()
}

fn code_block(code: &str) -> gtk::Widget {
    let label = gtk::Label::new(None);
    label.add_css_class("md-codeblock");
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_selectable(true);
    label.set_text(code);
    label.set_halign(gtk::Align::Fill);
    label.upcast()
}

fn list_item(line: &str) -> Option<(usize, &'static str, &str)> {
    let indent = line.chars().take_while(|c| *c == ' ').count() / 2;
    let body = line.trim_start();
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = body.strip_prefix(marker) {
            return Some((indent, "•", rest));
        }
    }
    let digits = body.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && body[digits..].starts_with(". ") {
        return Some((indent, "·", &body[digits + 2..]));
    }
    None
}


pub fn parse_table(lines: &[&str], start: usize) -> Option<(Table, usize)> {
    let header = split_row(lines.get(start)?)?;
    let delimiter = split_row(lines.get(start + 1)?)?;
    if header.len() != delimiter.len() {
        return None;
    }
    let align = parse_delimiter(&delimiter)?;

    let mut rows = Vec::new();
    let mut i = start + 2;
    while let Some(line) = lines.get(i) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("```") {
            break;
        }
        let Some(mut cells) = split_row(line) else {
            break;
        };
        cells.truncate(align.len());
        while cells.len() < align.len() {
            cells.push(String::new());
        }
        rows.push(cells);
        i += 1;
    }

    Some((
        Table {
            align,
            header,
            rows,
        },
        i,
    ))
}

fn split_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return None;
    }

    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = trimmed.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                current.push('|');
                chars.next();
            }
            '|' => {
                cells.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    cells.push(current.trim().to_string());

    if trimmed.starts_with('|') && !cells.is_empty() {
        cells.remove(0);
    }
    if closes_with_pipe(trimmed) && !cells.is_empty() {
        cells.pop();
    }

    if cells.is_empty() {
        None
    } else {
        Some(cells)
    }
}

fn closes_with_pipe(text: &str) -> bool {
    if !text.ends_with('|') {
        return false;
    }
    let backslashes = text[..text.len() - 1]
        .chars()
        .rev()
        .take_while(|c| *c == '\\')
        .count();
    backslashes % 2 == 0
}

fn parse_delimiter(cells: &[String]) -> Option<Vec<ColumnAlign>> {
    if cells.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(cells.len());
    for cell in cells {
        let cell = cell.trim();
        let left = cell.starts_with(':');
        let right = cell.ends_with(':');
        let core = cell.trim_start_matches(':').trim_end_matches(':');
        if core.len() < 3 || !core.chars().all(|c| c == '-') {
            return None;
        }
        out.push(match (left, right) {
            (true, true) => ColumnAlign::Center,
            (false, true) => ColumnAlign::Right,
            _ => ColumnAlign::Left,
        });
    }
    Some(out)
}

fn table_widget(table: &Table) -> gtk::Widget {
    let grid = gtk::Grid::new();
    grid.add_css_class("md-table");
    grid.set_column_spacing(0);
    grid.set_row_spacing(0);
    grid.set_halign(gtk::Align::Fill);
    grid.set_valign(gtk::Align::Start);
    grid.set_hexpand(true);

    let weights = column_weights(table);
    let total: i32 = weights.iter().sum();

    for (col, text) in table.header.iter().enumerate() {
        let expand = weights[col] * 3 >= total;
        let cell = table_cell(
            text,
            table.align[col],
            true,
            false,
            false,
            weights[col],
            expand,
        );
        grid.attach(&cell, col as i32, 0, 1, 1);
    }
    for (row_index, row) in table.rows.iter().enumerate() {
        let striped = row_index % 2 == 0;
        for col in 0..table.align.len() {
            let text = row.get(col).map(String::as_str).unwrap_or("");
            let expand = weights[col] * 3 >= total;
            let cell = table_cell(
                text,
                table.align[col],
                false,
                striped,
                true,
                weights[col],
                expand,
            );
            grid.attach(&cell, col as i32, (row_index + 1) as i32, 1, 1);
        }
    }

    grid.upcast()
}

fn column_weights(table: &Table) -> Vec<i32> {
    let mut weights = vec![4i32; table.align.len()];
    for (col, text) in table.header.iter().enumerate() {
        weights[col] += text.chars().count() as i32;
    }
    for row in &table.rows {
        for (col, text) in row.iter().enumerate().take(weights.len()) {
            weights[col] += text.chars().count() as i32;
        }
    }
    const BUDGET: i32 = 96;
    let total: i32 = weights.iter().sum();
    weights
        .into_iter()
        .map(|weight| ((weight * BUDGET) / total).clamp(4, 72))
        .collect()
}

fn table_cell(
    text: &str,
    align: ColumnAlign,
    head: bool,
    striped: bool,
    separator: bool,
    weight: i32,
    expand: bool,
) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("md-table-cell");
    let inline = inline(text);
    if head {
        label.add_css_class("md-table-head");
        label.set_markup(&format!("<b>{}</b>", inline.markup));
    } else {
        label.set_markup(&inline.markup);
    }
    label.set_xalign(align.xalign());
    label.set_justify(align.justify());
    label.set_halign(gtk::Align::Fill);
    label.set_valign(gtk::Align::Start);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_lines(2);
    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    label.set_width_chars(weight.max(4));
    label.set_max_width_chars(weight.max(4) + 2);
    label.set_hexpand(expand);
    label.set_selectable(true);
    if striped {
        label.add_css_class("md-table-alt");
    }
    if separator {
        label.add_css_class("md-table-sep");
    }
    attach_links(&label, &inline);
    if text.chars().count() > 24 {
        label.set_tooltip_text(Some(text));
    }
    label
}

fn attach_links(label: &gtk::Label, inline: &Inline) {
    if inline.links.is_empty() {
        return;
    }

    let tooltip = if inline.links.len() == 1 {
        inline.links[0].url.as_str()
    } else {
        "right-click a link to copy or open"
    };
    label.set_tooltip_text(Some(tooltip));

    label.connect_activate_link(|_, uri| {
        open_uri(uri);
        gtk::glib::Propagation::Stop
    });

    let links = inline.links.clone();
    let plain = inline.plain.clone();

    let menu_gesture = gtk::GestureClick::new();
    menu_gesture.set_button(gtk::gdk::BUTTON_SECONDARY);
    menu_gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
    let label_weak = label.downgrade();
    menu_gesture.connect_pressed(move |gesture, _, x, y| {
        let Some(label) = label_weak.upgrade() else {
            return;
        };
        let Some(url) = link_at(&label, x, y, &links, &plain)
            .map(|link| link.url)
            .or_else(|| label.current_uri().map(|uri| uri.to_string()))
        else {
            return;
        };
        gesture.set_state(gtk::EventSequenceState::Claimed);

        let copy_url = url.clone();
        let open_url = url;
        crate::ui::menu::show(
            &label,
            vec![
                (
                    "Copy link",
                    std::rc::Rc::new(move || copy_text(&copy_url)),
                ),
                (
                    "Open in browser",
                    std::rc::Rc::new(move || open_uri(&open_url)),
                ),
            ],
        );
    });
    label.add_controller(menu_gesture);
}

fn link_at(label: &gtk::Label, x: f64, y: f64, links: &[Link], plain: &str) -> Option<Link> {
    if plain.is_empty() {
        return None;
    }
    let (offset_x, offset_y) = label.layout_offsets();
    let (inside, index, _) = label
        .layout()
        .xy_to_index(x as i32 - offset_x, y as i32 - offset_y);
    if !inside {
        return None;
    }
    let index = index as usize;
    links
        .iter()
        .find(|link| index >= link.start && index < link.end)
        .cloned()
}

fn copy_text(text: &str) {
    if let Some(display) = gtk::gdk::Display::default() {
        display.clipboard().set_text(text);
    }
}

fn open_uri(uri: &str) {
    let _ = gtk::gio::AppInfo::launch_default_for_uri(
        uri,
        None::<&gtk::gio::AppLaunchContext>,
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub url: String,
    /// Byte offset of the first visible character in `plain`.
    pub start: usize,
    /// Byte offset one past the last visible character in `plain`.
    pub end: usize,
}

pub struct Inline {
    pub markup: String,
    /// Visible text of the label, without markup or Markdown syntax.
    pub plain: String,
    pub links: Vec<Link>,
}

pub fn inline(text: &str) -> Inline {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 16);
    let mut plain = String::with_capacity(text.len());
    let mut links = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '&' => {
                out.push_str("&amp;");
                plain.push('&');
                i += 1;
            }
            '<' => {
                out.push_str("&lt;");
                plain.push('<');
                i += 1;
            }
            '>' => {
                out.push_str("&gt;");
                plain.push('>');
                i += 1;
            }
            '"' => {
                out.push_str("&quot;");
                plain.push('"');
                i += 1;
            }
            '`' => {
                if let Some(end) = find_char(&chars, i + 1, '`') {
                    let code: String = chars[i + 1..end].iter().collect();
                    out.push_str(&format!(
                        "<span font_family='{}' size='9pt' foreground='{}' background='{}'>{}</span>",
                        crate::theme::mono_font(),
                        t::active("mdCode"),
                        t::active("cardBg"),
                        escape(&code)
                    ));
                    plain.push_str(&code);
                    i = end + 1;
                } else {
                    out.push('`');
                    plain.push('`');
                    i += 1;
                }
            }
            '*' => {
                if chars.get(i + 1) == Some(&'*') {
                    if let Some(end) = find_seq(&chars, i + 2, "**") {
                        let inner: String = chars[i + 2..end].iter().collect();
                        out.push_str(&format!("<b>{}</b>", escape(&inner)));
                        plain.push_str(&inner);
                        i = end + 2;
                        continue;
                    }
                }
                if let Some(end) = find_char(&chars, i + 1, '*') {
                    if end > i + 1 {
                        let inner: String = chars[i + 1..end].iter().collect();
                        out.push_str(&format!("<i>{}</i>", escape(&inner)));
                        plain.push_str(&inner);
                        i = end + 1;
                        continue;
                    }
                }
                out.push('*');
                plain.push('*');
                i += 1;
            }
            '[' => {
                if let Some((label, url, next)) = parse_link(&chars, i) {
                    let start = plain.len();
                    plain.push_str(&label);
                    let end = plain.len();
                    out.push_str(&format!(
                        "<a href=\"{}\" title=\"{}\">{}</a>",
                        escape(&url),
                        escape(&url),
                        escape(&label)
                    ));
                    links.push(Link { url, start, end });
                    i = next;
                } else {
                    out.push('[');
                    plain.push('[');
                    i += 1;
                }
            }
            _ => {
                out.push(c);
                plain.push(c);
                i += 1;
            }
        }
    }

    Inline {
        markup: out,
        plain,
        links,
    }
}

fn find_char(chars: &[char], from: usize, needle: char) -> Option<usize> {
    (from..chars.len()).find(|i| chars[*i] == needle)
}

fn find_seq(chars: &[char], from: usize, needle: &str) -> Option<usize> {
    let needle: Vec<char> = needle.chars().collect();
    if chars.len() < needle.len() {
        return None;
    }
    (from..=chars.len() - needle.len()).find(|i| chars[*i..*i + needle.len()] == needle[..])
}

fn parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    let close = find_char(chars, start + 1, ']')?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let url_end = find_char(chars, close + 2, ')')?;
    let label: String = chars[start + 1..close].iter().collect();
    let url: String = chars[close + 2..url_end].iter().collect();
    if url.is_empty() {
        return None;
    }
    Some((label, url, url_end + 1))
}

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_links_map_visible_text_to_urls() {
        let inline = inline("See [OpenAI](https://openai.com) and [Pi](https://pi.dev).");
        assert_eq!(inline.plain, "See OpenAI and Pi.");
        assert_eq!(inline.links.len(), 2);
        assert_eq!(&inline.plain[inline.links[0].start..inline.links[0].end], "OpenAI");
        assert_eq!(&inline.plain[inline.links[1].start..inline.links[1].end], "Pi");
        assert_eq!(inline.links[0].url, "https://openai.com");
        assert_eq!(inline.links[1].url, "https://pi.dev");
        assert!(inline.markup.contains("<a href=\"https://openai.com\" title="));
    }

    #[test]
    fn inline_links_keep_offsets_after_formatting() {
        let inline = inline("**Docs:** [Pi](https://pi.dev)");
        assert_eq!(inline.plain, "Docs: Pi");
        assert_eq!(&inline.plain[inline.links[0].start..inline.links[0].end], "Pi");
    }
}
