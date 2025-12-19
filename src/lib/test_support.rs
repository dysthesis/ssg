use std::path::PathBuf;

use proptest::collection;
use proptest::prelude::*;
use proptest::sample::select;
use proptest::test_runner::TestCaseResult;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag, TagEnd};

pub const MAX_DEFAULT_BYTES: usize = 8_192;
pub const MAX_LARGE_BYTES: usize = 1_048_576;
pub const DEFAULT_CASES: u32 = 512;
pub const FILE_CASES: u32 = 128;
pub const LARGE_CASES: u32 = 32;

fn chars_with_byte_limit(
    max_bytes: usize,
    char_strategy: impl Strategy<Value = char>,
) -> impl Strategy<Value = String> {
    // Generate a vector of chars and truncate before exceeding the UTF-8 byte budget,
    // avoiding filter-map rejection storms while still covering the full byte range.
    collection::vec(char_strategy, 0..=max_bytes).prop_map(move |chars| {
        let mut out = String::new();
        let mut used = 0;
        for ch in chars {
            let len = ch.len_utf8();
            if used + len > max_bytes {
                break;
            }
            out.push(ch);
            used += len;
        }
        out
    })
}

pub fn gen_any_utf8() -> impl Strategy<Value = String> {
    chars_with_byte_limit(MAX_DEFAULT_BYTES, proptest::char::any())
}

pub fn gen_any_utf8_large() -> impl Strategy<Value = String> {
    chars_with_byte_limit(MAX_LARGE_BYTES, proptest::char::any())
}

fn is_safe_html_scalar(ch: char) -> bool {
    !matches!(ch, '&' | '<' | '>' | '"' | '\'')
}

pub fn gen_safe_html_text() -> impl Strategy<Value = String> {
    let safe_char =
        proptest::char::any().prop_map(|c| if is_safe_html_scalar(c) { c } else { '_' });
    chars_with_byte_limit(MAX_DEFAULT_BYTES, safe_char)
}

pub fn gen_special_dense() -> impl Strategy<Value = String> {
    let dense_segment = prop_oneof![
        Just("&".to_string()),
        Just("<".to_string()),
        Just(">".to_string()),
        Just("\"".to_string()),
        Just("'".to_string()),
        Just("&&&&".to_string()),
        Just("<<<<".to_string()),
        Just(">>>>".to_string()),
        Just("\"\"\"\"".to_string()),
        Just("''''".to_string()),
        collection::vec(
            prop_oneof![
                Just('&'),
                Just('<'),
                Just('>'),
                Just('"'),
                Just('\''),
                proptest::char::any()
            ],
            1..=6
        )
        .prop_map(|chars| chars.into_iter().collect::<String>()),
    ];

    collection::vec(dense_segment, 0..=64)
        .prop_map(|segments| segments.concat())
        .prop_filter("<= 8192 bytes", |s| s.len() <= MAX_DEFAULT_BYTES)
}

pub fn gen_language_token_adversarial() -> impl Strategy<Value = String> {
    let ascii_alnum = select(
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
            .chars()
            .collect::<Vec<_>>(),
    );
    let whitespace = select(vec![' ', '\t', '\n', '\r']);
    collection::vec(
        prop_oneof![
            ascii_alnum,
            whitespace,
            Just('"'),
            Just('\''),
            Just('<'),
            Just('>'),
            Just('='),
            proptest::num::u8::ANY.prop_map(char::from),
        ],
        0..=64,
    )
    .prop_map(|chars| chars.into_iter().collect::<String>())
}

pub fn gen_display_mode() -> impl Strategy<Value = bool> {
    proptest::bool::ANY
}

fn codeblock_inner_event() -> impl Strategy<Value = Event<'static>> {
    prop_oneof![
        gen_any_utf8().prop_map(|s| Event::Text(CowStr::from(s))),
        gen_any_utf8().prop_map(|s| Event::Code(CowStr::from(s))),
        gen_any_utf8().prop_map(|s| Event::Html(CowStr::from(s))),
        gen_any_utf8().prop_map(|s| Event::InlineHtml(CowStr::from(s))),
        Just(Event::SoftBreak),
        Just(Event::HardBreak),
    ]
}

fn neutral_event() -> impl Strategy<Value = Event<'static>> {
    prop_oneof![
        gen_any_utf8().prop_map(|s| Event::Text(CowStr::from(s))),
        gen_any_utf8().prop_map(|s| Event::Code(CowStr::from(s))),
        gen_any_utf8().prop_map(|s| Event::Html(CowStr::from(s))),
        gen_any_utf8().prop_map(|s| Event::InlineHtml(CowStr::from(s))),
        Just(Event::SoftBreak),
        Just(Event::HardBreak),
    ]
}

pub fn gen_events_well_formed_codeblock() -> impl Strategy<Value = Vec<Event<'static>>> {
    (
        prop_oneof![Just(None), gen_language_token_adversarial().prop_map(Some),],
        collection::vec(codeblock_inner_event(), 0..=12),
        collection::vec(neutral_event(), 0..=4),
        collection::vec(neutral_event(), 0..=4),
    )
        .prop_map(|(language, inner, mut prefix, mut suffix)| {
            let mut events = Vec::new();
            events.append(&mut prefix);
            let kind = match language.clone() {
                Some(lang) => CodeBlockKind::Fenced(CowStr::from(lang)),
                None => CodeBlockKind::Indented,
            };
            events.push(Event::Start(Tag::CodeBlock(kind)));
            events.extend(inner);
            events.push(Event::End(TagEnd::CodeBlock));
            events.append(&mut suffix);
            events
        })
}

pub fn gen_events_general_finite() -> impl Strategy<Value = Vec<Event<'static>>> {
    collection::vec(
        prop_oneof![
            neutral_event().prop_map(|e| vec![e]),
            gen_any_utf8().prop_map(|s| {
                vec![
                    Event::Start(Tag::Paragraph),
                    Event::Text(CowStr::from(s)),
                    Event::End(TagEnd::Paragraph),
                ]
            }),
        ],
        0..=16,
    )
    .prop_map(|blocks| blocks.into_iter().flatten().collect())
}

pub fn ref_escape_html(raw: &str) -> String {
    raw.chars()
        .flat_map(|ch| match ch {
            '&' => "&amp;".chars().collect::<Vec<char>>(),
            '<' => "&lt;".chars().collect(),
            '>' => "&gt;".chars().collect(),
            '"' => "&quot;".chars().collect(),
            '\'' => "&#x27;".chars().collect(),
            _ => vec![ch],
        })
        .collect()
}

pub fn counts(s: &str) -> (usize, usize, usize, usize, usize) {
    let mut amp = 0;
    let mut lt = 0;
    let mut gt = 0;
    let mut dq = 0;
    let mut sq = 0;
    for ch in s.chars() {
        match ch {
            '&' => amp += 1,
            '<' => lt += 1,
            '>' => gt += 1,
            '"' => dq += 1,
            '\'' => sq += 1,
            _ => {}
        }
    }
    (amp, lt, gt, dq, sq)
}

pub fn assert_ampersands_are_only_known_entities(out: &str) -> TestCaseResult {
    let valid = ["&amp;", "&lt;", "&gt;", "&quot;", "&#x27;"];
    let mut i = 0;
    while let Some(pos) = out[i..].find('&') {
        let start = i + pos;
        let tail = &out[start..];
        let mut matched = false;
        for entity in valid {
            if tail.starts_with(entity) {
                matched = true;
                i = start + entity.len();
                break;
            }
        }
        if !matched {
            return Err(proptest::test_runner::TestCaseError::fail(format!(
                "ampersand at byte {start} is not a known entity"
            )));
        }
    }
    Ok(())
}

pub fn decode_five_entities(out: &str) -> String {
    let mut decoded = String::with_capacity(out.len());
    let mut i = 0;
    let bytes = out.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'&' {
            if out[i..].starts_with("&amp;") {
                decoded.push('&');
                i += 5;
                continue;
            }
            if out[i..].starts_with("&lt;") {
                decoded.push('<');
                i += 4;
                continue;
            }
            if out[i..].starts_with("&gt;") {
                decoded.push('>');
                i += 4;
                continue;
            }
            if out[i..].starts_with("&quot;") {
                decoded.push('"');
                i += 6;
                continue;
            }
            if out[i..].starts_with("&#39;") {
                decoded.push('\'');
                i += 5;
                continue;
            }
            if out[i..].starts_with("&#x27;") {
                decoded.push('\'');
                i += 6;
                continue;
            }
        }
        if let Some(ch) = out[i..].chars().next() {
            decoded.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    decoded
}

pub fn strip_tags_lossy(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

pub fn gen_path() -> impl Strategy<Value = PathBuf> {
    let allowed_path_chars: Vec<char> =
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-."
            .chars()
            .collect();
    let segment = collection::vec(select(allowed_path_chars.clone()), 1..=12)
        .prop_map(|chars| chars.into_iter().collect::<String>())
        .prop_filter("segment cannot be '.' or '..'", |s| s != "." && s != "..");

    let rel = collection::vec(segment.clone(), 1..=4).prop_map(|parts| {
        let mut path = PathBuf::new();
        for part in parts {
            path.push(part);
        }
        path
    });

    prop_oneof![rel.clone(), rel.prop_map(|p| PathBuf::from("/").join(p)),]
}
