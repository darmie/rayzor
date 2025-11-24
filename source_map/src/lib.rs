//! Source mapping utilities for multi-file compilation
//!
//! This library provides source file tracking and position mapping for compiler
//! pipelines that process multiple source files. It manages file identifiers,
//! source text storage, and efficient line/column calculation from byte offsets.

use std::collections::HashMap;
use std::fmt;

/// Represents a position in source code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
    pub byte_offset: usize,
}

impl SourcePosition {
    pub fn new(line: usize, column: usize, byte_offset: usize) -> Self {
        Self { line, column, byte_offset }
    }
}

/// Represents a span of source code
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
    pub file_id: FileId,
}

impl SourceSpan {
    pub fn new(start: SourcePosition, end: SourcePosition, file_id: FileId) -> Self {
        Self { start, end, file_id }
    }
    
    pub fn single_position(pos: SourcePosition, file_id: FileId) -> Self {
        Self {
            start: pos,
            end: SourcePosition::new(pos.line, pos.column + 1, pos.byte_offset + 1),
            file_id,
        }
    }
    
    /// Merge two spans (must be from the same file)
    pub fn merge(self, other: SourceSpan) -> SourceSpan {
        assert_eq!(self.file_id, other.file_id, "Cannot merge spans from different files");
        
        let start = if self.start.byte_offset <= other.start.byte_offset {
            self.start
        } else {
            other.start
        };
        
        let end = if self.end.byte_offset >= other.end.byte_offset {
            self.end
        } else {
            other.end
        };
        
        SourceSpan::new(start, end, self.file_id)
    }
}

/// Unique identifier for a source file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(usize);

impl FileId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }
    
    pub fn as_usize(self) -> usize {
        self.0
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileId({})", self.0)
    }
}

/// Information about a source file
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub content: String,
    pub line_starts: Vec<usize>,
}

impl SourceFile {
    /// Create a new source file with precomputed line starts
    pub fn new(name: String, content: String) -> Self {
        let line_starts = compute_line_starts(&content);
        Self {
            name,
            content,
            line_starts,
        }
    }
    
    /// Get a specific line from the source file (1-based line numbers)
    pub fn get_line(&self, line_number: usize) -> Option<&str> {
        if line_number == 0 || line_number > self.line_starts.len() {
            return None;
        }
        
        let start = self.line_starts[line_number - 1];
        let end = if line_number < self.line_starts.len() {
            self.line_starts[line_number]
        } else {
            self.content.len()
        };
        
        Some(&self.content[start..end].trim_end_matches(&['\n', '\r']))
    }
    
    /// Convert a byte offset to line and column (1-based)
    pub fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        // Binary search for the line
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        
        let line_start = self.line_starts.get(line_index).copied().unwrap_or(0);
        let column = offset - line_start + 1;
        let line = line_index + 1;
        
        (line, column)
    }
    
    /// Create a SourcePosition from a byte offset
    pub fn offset_to_position(&self, offset: usize) -> SourcePosition {
        let (line, column) = self.offset_to_line_col(offset);
        SourcePosition::new(line, column, offset)
    }
}

/// Manages source files and their content for multi-file compilation
#[derive(Debug, Clone)]
pub struct SourceMap {
    files: HashMap<FileId, SourceFile>,
    next_id: usize,
}

impl SourceMap {
    /// Create a new empty source map
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            next_id: 0,
        }
    }
    
    /// Add a source file and return its FileId
    pub fn add_file(&mut self, name: String, content: String) -> FileId {
        let file_id = FileId(self.next_id);
        self.next_id += 1;
        
        let source_file = SourceFile::new(name, content);
        self.files.insert(file_id, source_file);
        
        file_id
    }
    
    /// Get a source file by its FileId
    pub fn get_file(&self, file_id: FileId) -> Option<&SourceFile> {
        self.files.get(&file_id)
    }
    
    /// Get a mutable reference to a source file by its FileId
    pub fn get_file_mut(&mut self, file_id: FileId) -> Option<&mut SourceFile> {
        self.files.get_mut(&file_id)
    }
    
    /// Get a specific line from a file (1-based line numbers)
    pub fn get_line(&self, file_id: FileId, line_number: usize) -> Option<&str> {
        self.get_file(file_id)?.get_line(line_number)
    }
    
    /// Convert a byte offset to line and column for a specific file
    pub fn offset_to_line_col(&self, file_id: FileId, offset: usize) -> Option<(usize, usize)> {
        self.get_file(file_id).map(|file| file.offset_to_line_col(offset))
    }
    
    /// Create a SourcePosition from a file and byte offset
    pub fn offset_to_position(&self, file_id: FileId, offset: usize) -> Option<SourcePosition> {
        self.get_file(file_id).map(|file| file.offset_to_position(offset))
    }
    
    /// Create a SourceSpan from file, start offset, and end offset
    pub fn span_from_offsets(&self, file_id: FileId, start: usize, end: usize) -> Option<SourceSpan> {
        let file = self.get_file(file_id)?;
        let start_pos = file.offset_to_position(start);
        let end_pos = file.offset_to_position(end);
        Some(SourceSpan::new(start_pos, end_pos, file_id))
    }
    
    /// Get all file IDs in the source map
    pub fn file_ids(&self) -> impl Iterator<Item = FileId> + '_ {
        self.files.keys().copied()
    }
    
    /// Get the number of files in the source map
    pub fn len(&self) -> usize {
        self.files.len()
    }
    
    /// Check if the source map is empty
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

impl Default for SourceMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute line start offsets for a source text
fn compute_line_starts(source: &str) -> Vec<usize> {
    let mut line_starts = vec![0];
    
    for (i, ch) in source.char_indices() {
        if ch == '\n' {
            line_starts.push(i + 1);
        }
    }
    
    line_starts
}

/// Integration with parser Span types
pub mod parser_integration {
    use super::*;
    
    /// Simple span type that matches the parser's Span
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ParserSpan {
        pub start: usize,
        pub end: usize,
    }
    
    impl ParserSpan {
        pub fn new(start: usize, end: usize) -> Self {
            Self { start, end }
        }
        
        pub fn merge(self, other: ParserSpan) -> ParserSpan {
            ParserSpan::new(self.start.min(other.start), self.end.max(other.end))
        }
    }
    
    /// Extension trait for converting parser spans to source spans
    pub trait SpanConversion {
        fn to_source_span(&self, file_id: FileId, source_map: &SourceMap) -> Option<SourceSpan>;
    }
    
    impl SpanConversion for ParserSpan {
        fn to_source_span(&self, file_id: FileId, source_map: &SourceMap) -> Option<SourceSpan> {
            source_map.span_from_offsets(file_id, self.start, self.end)
        }
    }
    
    impl SpanConversion for Option<ParserSpan> {
        fn to_source_span(&self, file_id: FileId, source_map: &SourceMap) -> Option<SourceSpan> {
            match self {
                Some(span) => span.to_source_span(file_id, source_map),
                None => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_source_map_basic() {
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file("test.hx".to_string(), "line 1\nline 2\nline 3".to_string());
        
        assert_eq!(source_map.get_line(file_id, 1), Some("line 1"));
        assert_eq!(source_map.get_line(file_id, 2), Some("line 2"));
        assert_eq!(source_map.get_line(file_id, 3), Some("line 3"));
        assert_eq!(source_map.get_line(file_id, 4), None);
    }
    
    #[test]
    fn test_offset_to_line_col() {
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file("test.hx".to_string(), "hello\nworld\ntest".to_string());
        
        // First line
        assert_eq!(source_map.offset_to_line_col(file_id, 0), Some((1, 1))); // 'h'
        assert_eq!(source_map.offset_to_line_col(file_id, 4), Some((1, 5))); // 'o'
        
        // Second line
        assert_eq!(source_map.offset_to_line_col(file_id, 6), Some((2, 1))); // 'w'
        assert_eq!(source_map.offset_to_line_col(file_id, 10), Some((2, 5))); // 'd'
        
        // Third line  
        assert_eq!(source_map.offset_to_line_col(file_id, 12), Some((3, 1))); // 't'
    }
    
    #[test]
    fn test_source_span_merge() {
        let file_id = FileId::new(0);
        let span1 = SourceSpan::new(
            SourcePosition::new(1, 1, 0),
            SourcePosition::new(1, 5, 4),
            file_id,
        );
        let span2 = SourceSpan::new(
            SourcePosition::new(1, 3, 2),
            SourcePosition::new(1, 8, 7),
            file_id,
        );
        
        let merged = span1.merge(span2);
        assert_eq!(merged.start.byte_offset, 0);
        assert_eq!(merged.end.byte_offset, 7);
    }
    
    #[test]
    fn test_multiple_files() {
        let mut source_map = SourceMap::new();
        let file1 = source_map.add_file("file1.hx".to_string(), "content1".to_string());
        let file2 = source_map.add_file("file2.hx".to_string(), "content2".to_string());
        
        assert_eq!(source_map.len(), 2);
        assert_eq!(source_map.get_file(file1).unwrap().name, "file1.hx");
        assert_eq!(source_map.get_file(file2).unwrap().name, "file2.hx");
        assert_ne!(file1, file2);
    }
}