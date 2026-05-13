use anyhow::Result;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Heading {
    pub level: u32,
    pub text: String,
    pub line_index: usize,
}

#[derive(Clone, Debug)]
pub struct Link {
    pub text: String,
    pub url: String,
    pub line_index: usize,
    pub col_index: usize,
}

#[derive(Clone)]
pub struct Document {
    pub path: PathBuf,
    pub content: String,
    pub lines: Vec<String>,
    pub headings: Vec<Heading>,
    pub links: Vec<Link>,
}

impl Document {
    pub async fn from_file(path: &Path) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        
        let (headings, links) = Self::parse_structure(&content);

        Ok(Self {
            path: path.to_path_buf(),
            content,
            lines,
            headings,
            links,
        })
    }

    fn parse_structure(content: &str) -> (Vec<Heading>, Vec<Link>) {
        let mut headings = Vec::new();
        let mut links = Vec::new();
        let parser = Parser::new(content);

        let mut current_line = 0;
        let mut heading_text = String::new();
        let mut in_heading = false;
        let mut heading_level = 0;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    heading_text.clear();
                    heading_level = level as u32;
                    in_heading = true;
                }
                Event::End(TagEnd::Heading(_)) => {
                    if in_heading {
                        headings.push(Heading {
                            level: heading_level,
                            text: heading_text.clone(),
                            line_index: current_line,
                        });
                        heading_text.clear();
                        in_heading = false;
                    }
                }
                Event::Text(text) => {
                    if in_heading {
                        if !heading_text.is_empty() {
                            heading_text.push(' ');
                        }
                        heading_text.push_str(&text);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    current_line += 1;
                }
                _ => {}
            }
        }

        (headings, links)
    }
}
