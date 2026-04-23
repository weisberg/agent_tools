use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Story {
    #[default]
    Body,
    #[serde(rename = "header.default")]
    HeaderDefault,
    #[serde(rename = "header.first")]
    HeaderFirst,
    #[serde(rename = "header.even")]
    HeaderEven,
    #[serde(rename = "footer.default")]
    FooterDefault,
    #[serde(rename = "footer.first")]
    FooterFirst,
    #[serde(rename = "footer.even")]
    FooterEven,
    Footnotes,
    Endnotes,
    Comments,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Target {
    Paragraph {
        paragraph: usize,
        #[serde(default)]
        story: Story,
    },
    Paragraphs {
        paragraphs: Vec<usize>,
        #[serde(default)]
        story: Story,
    },
    Range {
        range: String,
        #[serde(default)]
        story: Story,
    },
    Heading {
        heading: String,
        #[serde(default)]
        offset: usize,
        #[serde(default)]
        story: Story,
    },
    HeadingPath {
        heading_path: String,
        #[serde(default)]
        offset: usize,
    },
    Table {
        table: usize,
    },
    Image {
        image: usize,
    },
    Style {
        style: String,
        #[serde(default)]
        story: Story,
    },
    Text {
        text: String,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        occurrence: Option<usize>,
        #[serde(default)]
        story: Story,
    },
    Bookmark {
        bookmark: String,
    },
    NodeId {
        node_id: String,
    },
    Contains {
        contains: String,
        occurrence: usize,
        #[serde(default)]
        story: Story,
    },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct Job {
    pub operations: Vec<Operation>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Operation {
    #[serde(rename = "edit.replace")]
    EditReplace { target: Target, content: String },
    #[serde(rename = "edit.insert")]
    EditInsert {
        target: Target,
        position: Position,
        content: Vec<ContentBlock>,
    },
    #[serde(rename = "edit.delete")]
    EditDelete { target: Target },
    #[serde(rename = "edit.find-replace")]
    EditFindReplace {
        find: String,
        replace: String,
        #[serde(default)]
        scope: Scope,
    },
    #[serde(rename = "edit.update-table")]
    EditUpdateTable {
        target: Target,
        cell: CellRef,
        content: String,
    },
    #[serde(rename = "edit.append-row")]
    EditAppendRow { target: Target, row: Vec<String> },
    #[serde(rename = "edit.replace-image")]
    EditReplaceImage {
        target: Target,
        path: String,
        width: Option<String>,
    },
    #[serde(rename = "edit.set-style")]
    EditSetStyle {
        target: Target,
        style: StyleOverride,
    },
    #[serde(rename = "edit.set-heading")]
    EditSetHeading { target: Target, level: u8 },
    #[serde(rename = "review.comment")]
    ReviewComment {
        target: Target,
        text: String,
        #[serde(default)]
        parent: Option<u64>,
    },
    #[serde(rename = "review.track-replace")]
    ReviewTrackReplace { target: Target, content: String },
    #[serde(rename = "review.track-insert")]
    ReviewTrackInsert {
        target: Target,
        position: Position,
        content: Vec<ContentBlock>,
    },
    #[serde(rename = "review.track-delete")]
    ReviewTrackDelete { target: Target },
    #[serde(rename = "finalize.accept")]
    FinalizeAccept {
        #[serde(default)]
        ids: Option<Vec<u64>>,
    },
    #[serde(rename = "finalize.reject")]
    FinalizeReject {
        #[serde(default)]
        ids: Option<Vec<u64>>,
    },
    #[serde(rename = "finalize.strip")]
    FinalizeStrip {},
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    Before,
    After,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    All,
    First,
    Section(String),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct CellRef {
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct StyleOverride {
    pub style_id: Option<String>,
    pub font: Option<FontSpec>,
    pub size: Option<String>,
    pub color: Option<String>,
    pub align: Option<String>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct FontSpec {
    pub name: Option<String>,
    pub size: Option<String>,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ContentBlock {
    Ref {
        #[serde(rename = "$ref")]
        ref_uri: String,
    },
    Heading1 {
        heading1: String,
    },
    Heading2 {
        heading2: String,
    },
    Heading3 {
        heading3: String,
    },
    Paragraph {
        paragraph: ParagraphContent,
    },
    Bullets {
        bullets: Vec<String>,
    },
    Numbers {
        numbers: Vec<String>,
    },
    Table {
        table: TableBlock,
    },
    Image {
        image: ImageBlock,
    },
    PageBreak {
        page_break: bool,
    },
    Toc {
        toc: TocBlock,
    },
    Columns {
        columns: ColumnsBlock,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ParagraphContent {
    Text(String),
    Block(ParagraphBlock),
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct ParagraphBlock {
    #[serde(default)]
    pub runs: Vec<InlineRun>,
    pub align: Option<String>,
    pub style: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(untagged)]
pub enum InlineRun {
    Text(TextRun),
    Footnote { footnote: String },
    Link { link: LinkBlock },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct TextRun {
    pub text: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    pub font: Option<FontSpec>,
    pub size: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct LinkBlock {
    pub text: String,
    pub url: String,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct TableBlock {
    #[serde(default)]
    pub headers: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    pub style: Option<String>,
    #[serde(default)]
    pub column_widths: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct ImageBlock {
    pub path: String,
    pub width: Option<String>,
    pub alt: Option<String>,
    pub caption: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct TocBlock {
    pub heading_range: Option<String>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct ColumnsBlock {
    pub count: usize,
    pub gap: Option<String>,
    #[serde(default)]
    pub content: Vec<ContentBlock>,
}
