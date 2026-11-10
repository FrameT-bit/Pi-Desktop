
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    pub start: usize,
    pub end: usize,
    pub query: String,
}

pub fn mention_at(text: &str, cursor: usize) -> Option<Mention> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());

    let mut start = cursor;
    while start > 0 {
        if chars[start - 1].is_whitespace() {
            break;
        }
        start -= 1;
    }
    if start >= chars.len() || chars[start] != '@' {
        return None;
    }

    let mut end = start + 1;
    while end < chars.len() && !chars[end].is_whitespace() {
        end += 1;
    }
    if cursor < start || cursor > end {
        return None;
    }

    let query: String = chars[start + 1..end].iter().collect();
    Some(Mention { start, end, query })
}

pub fn replace_mention(
    text: &str,
    start: usize,
    end: usize,
    replacement: &str,
) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let start = start.min(chars.len());
    let end = end.clamp(start, chars.len());

    let mut out: String = chars[..start].iter().collect();
    out.push_str(replacement);
    let cursor = out.chars().count();
    out.extend(chars[end..].iter());
    (out, cursor)
}


