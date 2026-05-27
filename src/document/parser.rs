// src/document/parser.rs

pub enum Block {
    Heading { level: u8, inlines: Vec<Inline> },
    Paragraph { inlines: Vec<Inline> },
    CodeBlock { lang: Option<String>, code: String },
    BlockQuote { blocks: Vec<Block> },
    List { ordered: bool, items: Vec<Vec<Block>> },
    Rule,
    Table { headers: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
}

pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Code(String),
    Link { text: Vec<Inline>, url: String },
    SoftBreak,
    HardBreak,
}

use pulldown_cmark::{Event, Parser as MarkdownParser, Tag, TagEnd};

pub fn parse(content: &str) -> Vec<Block> {
    let parser = MarkdownParser::new(content);
    let mut blocks = Vec::new();
    let mut event_stack = Vec::new();

    for event in parser {
        event_stack.push(event);
    }

    let mut i = 0;
    while i < event_stack.len() {
        if let Some(block) = parse_block(&event_stack, &mut i) {
            blocks.push(block);
        }
    }

    blocks
}

fn parse_block(events: &[Event], index: &mut usize) -> Option<Block> {
    if *index >= events.len() {
        return None;
    }

    match &events[*index] {
        Event::Start(Tag::Heading { level, .. }) => {
            *index += 1;
            let inlines = parse_inlines_until_end(events, index, TagEnd::Heading(*level));
            Some(Block::Heading {
                level: *level as u8,
                inlines,
            })
        }
        Event::Start(Tag::Paragraph) => {
            *index += 1;
            let inlines = parse_inlines_until_end(events, index, TagEnd::Paragraph);
            Some(Block::Paragraph { inlines })
        }
        Event::Start(Tag::CodeBlock(kind)) => {
            let lang = match kind {
                pulldown_cmark::CodeBlockKind::Fenced(l) => {
                    if l.is_empty() {
                        None
                    } else {
                        Some(l.to_string())
                    }
                }
                _ => None,
            };
            *index += 1;
            let mut code = String::new();
            while *index < events.len() {
                match &events[*index] {
                    Event::End(TagEnd::CodeBlock) => {
                        *index += 1;
                        break;
                    }
                    Event::Text(t) => code.push_str(t),
                    _ => {}
                }
                *index += 1;
            }
            Some(Block::CodeBlock { lang, code })
        }
        Event::Start(Tag::BlockQuote(_)) => {
            *index += 1;
            let mut inner_blocks = Vec::new();
            while *index < events.len() {
                match &events[*index] {
                    Event::End(TagEnd::BlockQuote) => {
                        *index += 1;
                        break;
                    }
                    _ => {
                        if let Some(block) = parse_block(events, index) {
                            inner_blocks.push(block);
                        }
                    }
                }
            }
            Some(Block::BlockQuote {
                blocks: inner_blocks,
            })
        }
        Event::Start(Tag::List(ordered)) => {
            let is_ordered = ordered.is_some();
            *index += 1;
            let mut items = Vec::new();
            while *index < events.len() {
                match &events[*index] {
                    Event::End(TagEnd::List(_)) => {
                        *index += 1;
                        break;
                    }
                    Event::Start(Tag::Item) => {
                        *index += 1;
                        let mut item_blocks = Vec::new();
                        while *index < events.len() {
                            match &events[*index] {
                                Event::End(TagEnd::Item) => {
                                    *index += 1;
                                    break;
                                }
                                _ => {
                                    if let Some(block) = parse_block(events, index) {
                                        item_blocks.push(block);
                                    }
                                }
                            }
                        }
                        items.push(item_blocks);
                    }
                    _ => {
                        *index += 1;
                    }
                }
            }
            Some(Block::List {
                ordered: is_ordered,
                items,
            })
        }
        Event::Rule => {
            *index += 1;
            Some(Block::Rule)
        }
        Event::Start(Tag::Table(_)) => {
            *index += 1;
            while *index < events.len() {
                match &events[*index] {
                    Event::End(TagEnd::Table) => {
                        *index += 1;
                        break;
                    }
                    _ => {}
                }
                *index += 1;
            }
            // Simplified table handling for MVP
            Some(Block::Table {
                headers: vec![],
                rows: vec![],
            })
        }
        _ => {
            *index += 1;
            None
        }
    }
}

fn parse_inlines_until_end(events: &[Event], index: &mut usize, end_tag: TagEnd) -> Vec<Inline> {
    let mut inlines = Vec::new();

    while *index < events.len() {
        match &events[*index] {
            Event::End(tag) if std::mem::discriminant(tag) == std::mem::discriminant(&end_tag) => {
                *index += 1;
                break;
            }
            Event::Text(text) => {
                inlines.push(Inline::Text(text.to_string()));
                *index += 1;
            }
            Event::Code(code) => {
                inlines.push(Inline::Code(code.to_string()));
                *index += 1;
            }
            Event::SoftBreak => {
                inlines.push(Inline::SoftBreak);
                *index += 1;
            }
            Event::HardBreak => {
                inlines.push(Inline::HardBreak);
                *index += 1;
            }
            Event::Start(Tag::Strong) => {
                *index += 1;
                let inner = parse_inlines_until_end(events, index, TagEnd::Strong);
                inlines.push(Inline::Bold(inner));
            }
            Event::Start(Tag::Emphasis) => {
                *index += 1;
                let inner = parse_inlines_until_end(events, index, TagEnd::Emphasis);
                inlines.push(Inline::Italic(inner));
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let url = dest_url.to_string();
                *index += 1;
                let text = parse_inlines_until_end(events, index, TagEnd::Link);
                inlines.push(Inline::Link { text, url });
            }
            _ => {
                *index += 1;
            }
        }
    }

    inlines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_heading() {
        let blocks = parse("# Hello");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Heading { level, inlines } => {
                assert_eq!(*level, 1);
                assert_eq!(inlines.len(), 1);
            }
            _ => panic!("Expected Heading"),
        }
    }

    #[test]
    fn parses_bold_text() {
        let blocks = parse("**bold**");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Paragraph { inlines } => {
                assert_eq!(inlines.len(), 1);
                match &inlines[0] {
                    Inline::Bold(inner) => {
                        assert_eq!(inner.len(), 1);
                        match &inner[0] {
                            Inline::Text(s) => assert_eq!(s, "bold"),
                            _ => panic!("Expected Text in Bold"),
                        }
                    }
                    _ => panic!("Expected Bold"),
                }
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn parses_link() {
        let blocks = parse("[link](./foo.md)");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Paragraph { inlines } => {
                assert_eq!(inlines.len(), 1);
                match &inlines[0] {
                    Inline::Link { text, url } => {
                        assert_eq!(url, "./foo.md");
                        assert_eq!(text.len(), 1);
                    }
                    _ => panic!("Expected Link"),
                }
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn parses_code_block() {
        let blocks = parse("```rust\nfn main() {}\n```");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::CodeBlock { lang, code } => {
                assert_eq!(lang, &Some("rust".to_string()));
                assert!(code.contains("fn main"));
            }
            _ => panic!("Expected CodeBlock"),
        }
    }

    #[test]
    fn parses_list() {
        let blocks = parse("- item1\n- item2");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::List { ordered, items } => {
                assert!(!ordered);
                assert_eq!(items.len(), 2);
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn parses_italic_text() {
        let blocks = parse("*italic*");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Paragraph { inlines } => {
                assert_eq!(inlines.len(), 1);
                match &inlines[0] {
                    Inline::Italic(inner) => {
                        assert_eq!(inner.len(), 1);
                        match &inner[0] {
                            Inline::Text(s) => assert_eq!(s, "italic"),
                            _ => panic!("Expected Text in Italic"),
                        }
                    }
                    _ => panic!("Expected Italic"),
                }
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn parses_code_inline() {
        let blocks = parse("`code`");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Paragraph { inlines } => {
                assert_eq!(inlines.len(), 1);
                match &inlines[0] {
                    Inline::Code(s) => assert_eq!(s, "code"),
                    _ => panic!("Expected Code"),
                }
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn parses_blockquote() {
        let blocks = parse("> quote");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::BlockQuote { blocks: inner } => {
                assert!(inner.len() > 0);
            }
            _ => panic!("Expected BlockQuote"),
        }
    }

    #[test]
    fn parses_rule() {
        let blocks = parse("---");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Rule => {}
            _ => panic!("Expected Rule"),
        }
    }
}
