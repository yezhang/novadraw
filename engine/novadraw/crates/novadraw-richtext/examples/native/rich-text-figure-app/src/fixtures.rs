//! Deterministic sample loads; Basic preserves the original resize control.
use super::{color, style};
use novadraw::{
    figure::PreparationBudget,
    text::{FontDescriptor, TextStyle, TextStyleSpan},
};
use novadraw_richtext::{ParagraphInput, RichTextDocumentBuilder, RichTextDocumentSnapshot};
use serde_json::{Value, json};

const LONG_PARAGRAPHS: usize = 128;
const REPEATS: usize = 16;
const GIANT_REPEATS: usize = 400;
const BODY: &str = "河流穿过山谷，文字沿着页面展开。每个段落保留自己的内容与样式，窗口变窄时重新换行。\
    A long document shares one canvas with diagrams and shapes. العربية 中文 日本語。 ";
const EMOJI: &str = "🙂 🚀 🎉 ❤️ 👍🏽 👩‍💻 👨‍👩‍👧‍👦 🇨🇳 1️⃣";
pub const EMOJI_FONT_PATH: &str = "/System/Library/Fonts/Apple Color Emoji.ttc";
const EMOJI_FAMILY: &str = "Apple Color Emoji";
const LARGE_WORK_LIMIT: u64 = 80_000_000;
const LARGE_ALLOCATION_LIMIT: u64 = 32_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fixture {
    Basic,
    Mixed,
    Large,
    Giant,
}

impl Fixture {
    pub fn from_args(args: &[String]) -> Result<Self, Box<dyn std::error::Error>> {
        match args.iter().position(|arg| arg == "--text-load") {
            None => Ok(Self::Large),
            Some(index) => match args.get(index + 1).map(String::as_str) {
                Some("basic") => Ok(Self::Basic),
                Some("mixed") => Ok(Self::Mixed),
                Some("large") => Ok(Self::Large),
                Some("giant") => Ok(Self::Giant),
                _ => Err("--text-load expects basic, mixed, large or giant".into()),
            },
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Mixed => "mixed",
            Self::Large => "large",
            Self::Giant => "giant",
        }
    }
    pub fn budget(self) -> PreparationBudget {
        match self {
            Self::Large | Self::Giant => {
                PreparationBudget::new(LARGE_WORK_LIMIT, LARGE_ALLOCATION_LIMIT)
            }
            _ => PreparationBudget::default(),
        }
    }
    pub fn document(self) -> RichTextDocumentSnapshot {
        if self == Self::Basic {
            return super::basic_document();
        }
        let mut builder = RichTextDocumentBuilder::default();
        push(
            &mut builder,
            format!("富文本压力测试 · {}", self.name()),
            28.0,
        );
        push(&mut builder, "14 px 小字 · 中文 English 日本語", 14.0);
        let mut mixed = ParagraphInput::new(
            "18 px 正文 · 28 px 标题 · 40 px 大字 · 64 px 超大字",
            style(18.0, "#334155"),
        );
        for (text, size) in [
            ("28 px 标题", 28.0),
            ("40 px 大字", 40.0),
            ("64 px 超大字", 64.0),
        ] {
            let start = mixed.text.find(text).unwrap();
            mixed.spans.push(TextStyleSpan {
                range: start..start + text.len(),
                style: style(size, "#176c68"),
            });
        }
        mixed.space_after = super::PARAGRAPH_SPACE;
        builder.push(mixed).unwrap();
        let mut emoji = ParagraphInput::new(
            EMOJI,
            TextStyle::new(
                FontDescriptor::new(EMOJI_FAMILY, 36.0).unwrap(),
                color("#334155"),
            ),
        );
        emoji.space_after = super::PARAGRAPH_SPACE;
        builder.push(emoji).unwrap();
        push(
            &mut builder,
            "Emoji：肤色、ZWJ 职业／家庭、旗帜与键帽；上行应显示彩色图形。",
            18.0,
        );
        match self {
            Self::Mixed => {
                for _ in 0..24 {
                    push(&mut builder, BODY, 18.0);
                }
            }
            Self::Large => {
                for index in 0..LONG_PARAGRAPHS {
                    push(
                        &mut builder,
                        format!("{index:03}  {}", BODY.repeat(REPEATS)),
                        18.0,
                    );
                }
                push(&mut builder, BODY.repeat(80), 18.0);
            }
            Self::Giant => push(&mut builder, BODY.repeat(GIANT_REPEATS), 18.0),
            Self::Basic => unreachable!(),
        }
        builder.build()
    }
    pub fn metadata(self, doc: &RichTextDocumentSnapshot) -> Value {
        let budget = self.budget();
        let chars: Vec<_> = doc
            .paragraphs()
            .iter()
            .map(|p| p.text().chars().count())
            .collect();
        json!({
            "fixture": self.name(), "paragraphs": chars.len(),
            "characters": chars.iter().sum::<usize>(), "bytes": doc.len_bytes(),
            "longest_paragraph_characters": chars.iter().max(),
            "style_spans": doc.paragraphs().iter().map(|p| p.spans().len()).sum::<usize>(),
            "showcase_sizes_px": if self == Self::Basic { vec![18, 28] } else { vec![14, 18, 28, 36, 40, 64] },
            "emoji_font": if self == Self::Basic { None } else { Some(EMOJI_FONT_PATH) },
            "preparation_budget": { "work": budget.work, "allocation": budget.allocation },
            "gpu_submission": false
        })
    }
}

fn push(builder: &mut RichTextDocumentBuilder, text: impl Into<String>, size: f32) {
    let mut paragraph = ParagraphInput::new(text, style(size, "#334155"));
    paragraph.space_after = super::PARAGRAPH_SPACE;
    builder.push(paragraph).unwrap();
}
