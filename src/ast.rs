/// A parsed sequence diagram document.
#[derive(Debug, Clone)]
pub struct Document {
    pub statements: Vec<Statement>,
}

/// A single statement in a sequence diagram.
#[derive(Debug, Clone)]
pub enum Statement {
    Title(String),
    Participant {
        actor: String,
        alias: Option<String>,
    },
    Message {
        from: String,
        to: String,
        arrow: Arrow,
        text: String,
    },
    Note {
        position: NotePosition,
        text: String,
    },
}

/// Arrow style combining line and head variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrow {
    pub line_style: LineStyle,
    pub head_style: HeadStyle,
}

/// Line style for arrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    /// `-` solid line
    Solid,
    /// `--` dashed line
    Dashed,
}

/// Arrowhead style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadStyle {
    /// `>` open arrowhead
    Open,
    /// `>>` closed/filled arrowhead
    Closed,
}

/// Placement of a note in the diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotePosition {
    LeftOf(String),
    RightOf(String),
    Over(String),
    OverBetween(String, String),
}
