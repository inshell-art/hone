use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MarkdownError {
    #[error("Facet body contains heading level {level} at line {line}. Split this into another facet or rewrite it as non-heading prose.")]
    HeadingInFacet { level: u8, line: usize },
}

pub fn validate_facet_body(markdown: &str) -> Result<(), MarkdownError> {
    for (idx, line) in markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let count = trimmed.chars().take_while(|ch| *ch == '#').count();
            let after = trimmed.chars().nth(count);
            if (1..=6).contains(&count) && after.is_some_and(char::is_whitespace) {
                return Err(MarkdownError::HeadingInFacet {
                    level: count as u8,
                    line: idx + 1,
                });
            }
        }
    }
    Ok(())
}

pub fn markdown_to_text(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::ENABLE_TABLES);
    let mut out = String::new();
    for event in parser {
        match event {
            Event::Text(text) | Event::Code(text) => {
                if !ends_with_ws(&out) && !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&text);
            }
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            Event::Start(Tag::Paragraph | Tag::Heading { .. } | Tag::Item)
                if !ends_with_ws(&out) && !out.is_empty() =>
            {
                out.push(' ');
            }
            _ => {}
        }
    }
    normalize_ws(&out)
}

pub fn render_article(title: &str, rendered_segments: &[RenderedSegment]) -> String {
    let mut out = String::new();
    out.push_str("# ");
    out.push_str(title.trim());
    out.push_str("\n\n");
    for segment in rendered_segments {
        match segment {
            RenderedSegment::Prose(body) => {
                let body = body.trim();
                if !body.is_empty() {
                    out.push_str(body);
                    out.push_str("\n\n");
                }
            }
            RenderedSegment::Facet { title, body } => {
                out.push_str("## ");
                out.push_str(title.trim());
                out.push_str("\n\n");
                let body = body.trim();
                if !body.is_empty() {
                    out.push_str(body);
                    out.push_str("\n\n");
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
pub enum RenderedSegment {
    Prose(String),
    Facet { title: String, body: String },
}

pub fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

pub fn normalize_ws(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn ends_with_ws(input: &str) -> bool {
    input.chars().last().is_some_and(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_heading_in_facet_body() {
        let err = validate_facet_body("ok\n\n### bad").unwrap_err();
        assert!(err.to_string().contains("heading level 3"));
    }

    #[test]
    fn strips_markdown_to_plain_text() {
        assert_eq!(markdown_to_text("A **bold** `code`"), "A bold code");
    }
}
