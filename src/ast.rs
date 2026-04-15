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
        activation: Option<ActivationModifier>,
        delay: Option<u8>,
    },
    Note {
        position: NotePosition,
        text: String,
    },
    Activate(String),
    Deactivate(String),
    Destroy(String),
    FrameOpen {
        kind: FrameKind,
        label: String,
    },
    FrameElse {
        label: String,
    },
    FrameEnd,
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

/// Activation modifier on a message arrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationModifier {
    /// `+` activate target actor
    Activate,
    /// `-` deactivate target actor
    Deactivate,
}

/// Kind of frame block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Alt,
    Opt,
    Loop,
    Par,
    Critical,
    Break,
}

impl std::fmt::Display for FrameKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameKind::Alt => write!(f, "alt"),
            FrameKind::Opt => write!(f, "opt"),
            FrameKind::Loop => write!(f, "loop"),
            FrameKind::Par => write!(f, "par"),
            FrameKind::Critical => write!(f, "critical"),
            FrameKind::Break => write!(f, "break"),
        }
    }
}

/// Placement of a note in the diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotePosition {
    LeftOf(String),
    RightOf(String),
    Over(String),
    OverBetween(String, String),
}
