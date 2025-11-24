//! Utilities for converting parser spans to TAST source locations
//!
//! This module provides conversion between the parser's Span type and
//! the TAST's SourceLocation type, handling file tracking and position mapping.

use parser::Span;
use crate::tast::symbols::SourceLocation;

/// Context for converting spans to source locations
pub struct SpanConverter {
    /// File ID for the current file being processed
    file_id: u32,
    /// Original source text for line/column calculation
    source_text: String,
    /// Precomputed line starts for efficiency
    line_starts: Vec<usize>,
}

impl SpanConverter {
    /// Create a new span converter for a file
    pub fn new(file_id: u32, source_text: String) -> Self {
        let line_starts = compute_line_starts(&source_text);
        Self {
            file_id,
            source_text,
            line_starts,
        }
    }
    
    /// Convert a parser span to a TAST source location
    pub fn convert_span(&self, span: Span) -> SourceLocation {
        let (line, column) = self.offset_to_line_col(span.start);
        SourceLocation {
            file_id: self.file_id,
            line: line as u32,
            column: column as u32,
            byte_offset: span.start as u32,
        }
    }
    
    /// Convert a byte offset to line and column (1-based)
    fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
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
    
    /// Create a source location for unknown/synthetic nodes
    pub fn unknown_location(&self) -> SourceLocation {
        SourceLocation::unknown()
    }
    
    /// Merge two spans and convert to source location
    pub fn merge_spans(&self, span1: Span, span2: Span) -> SourceLocation {
        let merged = span1.merge(span2);
        self.convert_span(merged)
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

/// Extension trait for optional span conversion
pub trait SpanConversion {
    fn to_source_location(&self, converter: &SpanConverter) -> SourceLocation;
}

impl SpanConversion for Span {
    fn to_source_location(&self, converter: &SpanConverter) -> SourceLocation {
        converter.convert_span(*self)
    }
}

impl SpanConversion for Option<Span> {
    fn to_source_location(&self, converter: &SpanConverter) -> SourceLocation {
        match self {
            Some(span) => converter.convert_span(*span),
            None => converter.unknown_location(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_span_conversion() {
        let source = "line1\nline2\nline3";
        let converter = SpanConverter::new(1, source.to_string());
        
        // Test first line
        let span1 = Span::new(0, 5);
        let loc1 = converter.convert_span(span1);
        assert_eq!(loc1.line, 1);
        assert_eq!(loc1.column, 1);
        assert_eq!(loc1.byte_offset, 0);
        
        // Test second line
        let span2 = Span::new(6, 11);
        let loc2 = converter.convert_span(span2);
        assert_eq!(loc2.line, 2);
        assert_eq!(loc2.column, 1);
        assert_eq!(loc2.byte_offset, 6);
        
        // Test middle of third line
        let span3 = Span::new(14, 16);
        let loc3 = converter.convert_span(span3);
        assert_eq!(loc3.line, 3);
        assert_eq!(loc3.column, 3);
        assert_eq!(loc3.byte_offset, 14);
    }
    
    #[test]
    fn test_line_starts_computation() {
        let source = "a\nb\nc";
        let line_starts = compute_line_starts(source);
        assert_eq!(line_starts, vec![0, 2, 4]);
        
        let empty = "";
        let empty_starts = compute_line_starts(empty);
        assert_eq!(empty_starts, vec![0]);
        
        let no_newlines = "hello world";
        let no_newline_starts = compute_line_starts(no_newlines);
        assert_eq!(no_newline_starts, vec![0]);
    }
    
    #[test]
    fn test_merge_spans() {
        let source = "test source";
        let converter = SpanConverter::new(1, source.to_string());
        
        let span1 = Span::new(0, 4);
        let span2 = Span::new(5, 11);
        
        let merged_loc = converter.merge_spans(span1, span2);
        assert_eq!(merged_loc.byte_offset, 0);
        // The merged span covers from 0 to 11
    }
}