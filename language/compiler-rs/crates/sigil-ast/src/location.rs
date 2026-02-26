//! Source location tracking for AST nodes

/// Position in source code (line, column, byte offset)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Position {
    /// 1-indexed line number
    pub line: usize,
    /// 1-indexed column number (in characters, not bytes)
    pub column: usize,
    /// 0-indexed byte offset from start of file
    pub offset: usize,
}

impl Position {
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self { line, column, offset }
    }
}

/// Source location span (start and end positions)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SourceLocation {
    pub start: Position,
    pub end: Position,
}

impl SourceLocation {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Create a location from explicit coordinates
    pub fn from_coords(
        start_line: usize,
        start_column: usize,
        start_offset: usize,
        end_line: usize,
        end_column: usize,
        end_offset: usize,
    ) -> Self {
        Self {
            start: Position::new(start_line, start_column, start_offset),
            end: Position::new(end_line, end_column, end_offset),
        }
    }

    /// Merge two source locations (from start of first to end of second)
    pub fn merge(start: SourceLocation, end: SourceLocation) -> Self {
        Self {
            start: start.start,
            end: end.end,
        }
    }
}
