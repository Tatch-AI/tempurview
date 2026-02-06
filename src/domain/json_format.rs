use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use serde_json::Value;

/// Convert a JSON value to syntax-highlighted ratatui Lines.
///
/// Colors: Keys=Cyan bold, Strings=Green, Numbers=Yellow,
/// Booleans=Magenta, Null=DarkGray dim, Punctuation=White.
/// Uses 2-space indentation; empty `{}` / `[]` on one line.
pub fn json_to_lines(value: &Value) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    render_value(value, 0, &mut lines, false);
    lines
}

fn render_value(value: &Value, indent: usize, lines: &mut Vec<Line<'static>>, inline: bool) {
    let prefix = if inline {
        String::new()
    } else {
        " ".repeat(indent)
    };

    match value {
        Value::Null => {
            lines.push(Line::from(vec![
                Span::raw(prefix),
                Span::styled(
                    "null",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM),
                ),
            ]));
        }
        Value::Bool(b) => {
            lines.push(Line::from(vec![
                Span::raw(prefix),
                Span::styled(b.to_string(), Style::default().fg(Color::Magenta)),
            ]));
        }
        Value::Number(n) => {
            lines.push(Line::from(vec![
                Span::raw(prefix),
                Span::styled(n.to_string(), Style::default().fg(Color::Yellow)),
            ]));
        }
        Value::String(s) => {
            // Show escape sequences visually
            let escaped = s.replace('\n', "\\n").replace('\r', "\\r");
            lines.push(Line::from(vec![
                Span::raw(prefix),
                Span::styled(
                    format!("\"{}\"", escaped),
                    Style::default().fg(Color::Green),
                ),
            ]));
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw(prefix),
                    Span::styled("[]", Style::default().fg(Color::White)),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw(prefix),
                    Span::styled("[", Style::default().fg(Color::White)),
                ]));
                for (i, item) in arr.iter().enumerate() {
                    render_value(item, indent + 2, lines, false);
                    // Add comma to the last line if not the last element
                    if i < arr.len() - 1 {
                        if let Some(last_line) = lines.last_mut() {
                            last_line.spans.push(Span::styled(
                                ",",
                                Style::default().fg(Color::White),
                            ));
                        }
                    }
                }
                lines.push(Line::from(vec![
                    Span::raw(" ".repeat(indent)),
                    Span::styled("]", Style::default().fg(Color::White)),
                ]));
            }
        }
        Value::Object(obj) => {
            if obj.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw(prefix),
                    Span::styled("{}", Style::default().fg(Color::White)),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw(prefix),
                    Span::styled("{", Style::default().fg(Color::White)),
                ]));
                let entries: Vec<_> = obj.iter().collect();
                for (i, (key, val)) in entries.iter().enumerate() {
                    let key_indent = " ".repeat(indent + 2);
                    // Check if value is simple (not object or array)
                    let is_simple = matches!(
                        val,
                        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
                    );
                    let is_empty_container = match val {
                        Value::Array(a) => a.is_empty(),
                        Value::Object(o) => o.is_empty(),
                        _ => false,
                    };

                    if is_simple || is_empty_container {
                        // Render key: value on one line
                        let mut spans = vec![
                            Span::raw(key_indent),
                            Span::styled(
                                format!("\"{}\"", key),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(": ", Style::default().fg(Color::White)),
                        ];
                        spans.extend(value_spans(val));
                        if i < entries.len() - 1 {
                            spans.push(Span::styled(",", Style::default().fg(Color::White)));
                        }
                        lines.push(Line::from(spans));
                    } else {
                        // Key on its own, then value block
                        lines.push(Line::from(vec![
                            Span::raw(key_indent),
                            Span::styled(
                                format!("\"{}\"", key),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(": ", Style::default().fg(Color::White)),
                        ]));
                        // Render the container value inline (opening brace on same line)
                        // We pop the last line (key line) and append the opening brace
                        let key_line = lines.pop().unwrap();
                        let mut key_spans = key_line.spans;

                        match val {
                            Value::Array(arr) => {
                                key_spans.push(Span::styled(
                                    "[",
                                    Style::default().fg(Color::White),
                                ));
                                lines.push(Line::from(key_spans));
                                for (j, item) in arr.iter().enumerate() {
                                    render_value(item, indent + 4, lines, false);
                                    if j < arr.len() - 1 {
                                        if let Some(last_line) = lines.last_mut() {
                                            last_line.spans.push(Span::styled(
                                                ",",
                                                Style::default().fg(Color::White),
                                            ));
                                        }
                                    }
                                }
                                let mut close_spans = vec![
                                    Span::raw(" ".repeat(indent + 2)),
                                    Span::styled("]", Style::default().fg(Color::White)),
                                ];
                                if i < entries.len() - 1 {
                                    close_spans.push(Span::styled(
                                        ",",
                                        Style::default().fg(Color::White),
                                    ));
                                }
                                lines.push(Line::from(close_spans));
                            }
                            Value::Object(inner_obj) => {
                                key_spans.push(Span::styled(
                                    "{",
                                    Style::default().fg(Color::White),
                                ));
                                lines.push(Line::from(key_spans));
                                let inner_entries: Vec<_> = inner_obj.iter().collect();
                                for (j, (ik, iv)) in inner_entries.iter().enumerate() {
                                    // Recursively handle inner objects
                                    let ik_indent = " ".repeat(indent + 4);
                                    let is_inner_simple = matches!(
                                        iv,
                                        Value::Null
                                            | Value::Bool(_)
                                            | Value::Number(_)
                                            | Value::String(_)
                                    );
                                    let is_inner_empty = match iv {
                                        Value::Array(a) => a.is_empty(),
                                        Value::Object(o) => o.is_empty(),
                                        _ => false,
                                    };

                                    if is_inner_simple || is_inner_empty {
                                        let mut s = vec![
                                            Span::raw(ik_indent),
                                            Span::styled(
                                                format!("\"{}\"", ik),
                                                Style::default()
                                                    .fg(Color::Cyan)
                                                    .add_modifier(Modifier::BOLD),
                                                ),
                                            Span::styled(
                                                ": ",
                                                Style::default().fg(Color::White),
                                            ),
                                        ];
                                        s.extend(value_spans(iv));
                                        if j < inner_entries.len() - 1 {
                                            s.push(Span::styled(
                                                ",",
                                                Style::default().fg(Color::White),
                                            ));
                                        }
                                        lines.push(Line::from(s));
                                    } else {
                                        // Deep nesting: fall back to recursive render
                                        lines.push(Line::from(vec![
                                            Span::raw(ik_indent),
                                            Span::styled(
                                                format!("\"{}\"", ik),
                                                Style::default()
                                                    .fg(Color::Cyan)
                                                    .add_modifier(Modifier::BOLD),
                                            ),
                                            Span::styled(
                                                ": ",
                                                Style::default().fg(Color::White),
                                            ),
                                        ]));
                                        render_value(iv, indent + 4, lines, true);
                                        if j < inner_entries.len() - 1 {
                                            if let Some(last_line) = lines.last_mut() {
                                                last_line.spans.push(Span::styled(
                                                    ",",
                                                    Style::default().fg(Color::White),
                                                ));
                                            }
                                        }
                                    }
                                }
                                let mut close_spans = vec![
                                    Span::raw(" ".repeat(indent + 2)),
                                    Span::styled("}", Style::default().fg(Color::White)),
                                ];
                                if i < entries.len() - 1 {
                                    close_spans.push(Span::styled(
                                        ",",
                                        Style::default().fg(Color::White),
                                    ));
                                }
                                lines.push(Line::from(close_spans));
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                lines.push(Line::from(vec![
                    Span::raw(" ".repeat(indent)),
                    Span::styled("}", Style::default().fg(Color::White)),
                ]));
            }
        }
    }
}

/// Produce inline spans for a simple value (null, bool, number, string, or empty container).
fn value_spans(value: &Value) -> Vec<Span<'static>> {
    match value {
        Value::Null => vec![Span::styled(
            "null",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        )],
        Value::Bool(b) => vec![Span::styled(
            b.to_string(),
            Style::default().fg(Color::Magenta),
        )],
        Value::Number(n) => vec![Span::styled(
            n.to_string(),
            Style::default().fg(Color::Yellow),
        )],
        Value::String(s) => {
            let escaped = s.replace('\n', "\\n").replace('\r', "\\r");
            vec![Span::styled(
                format!("\"{}\"", escaped),
                Style::default().fg(Color::Green),
            )]
        }
        Value::Array(a) if a.is_empty() => {
            vec![Span::styled("[]", Style::default().fg(Color::White))]
        }
        Value::Object(o) if o.is_empty() => {
            vec![Span::styled("{}", Style::default().fg(Color::White))]
        }
        _ => vec![Span::raw(value.to_string())],
    }
}

/// Apply search highlighting to lines, returning highlighted lines and indices of matching lines.
///
/// Case-insensitive search. Matching substrings get `bg(Yellow) + fg(Black)`.
pub fn highlight_search_matches(
    lines: &[Line<'static>],
    query: &str,
) -> (Vec<Line<'static>>, Vec<usize>) {
    if query.is_empty() {
        return (lines.to_vec(), vec![]);
    }

    let query_lower = query.to_lowercase();
    let mut result_lines = Vec::with_capacity(lines.len());
    let mut match_indices = Vec::new();

    let highlight_style = Style::default().fg(Color::Black).bg(Color::Yellow);

    for (line_idx, line) in lines.iter().enumerate() {
        // Collect all text from spans
        let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let full_lower = full_text.to_lowercase();

        if !full_lower.contains(&query_lower) {
            result_lines.push(line.clone());
            continue;
        }

        match_indices.push(line_idx);

        // Build a flat list of (char_offset, style) for each character
        let mut char_styles: Vec<(char, Style)> = Vec::with_capacity(full_text.len());
        for span in &line.spans {
            for ch in span.content.chars() {
                char_styles.push((ch, span.style));
            }
        }

        // Find all match positions in the lowercased text
        let mut match_positions = Vec::new();
        let mut search_from = 0;
        while let Some(pos) = full_lower[search_from..].find(&query_lower) {
            let abs_pos = search_from + pos;
            match_positions.push((abs_pos, abs_pos + query_lower.len()));
            search_from = abs_pos + 1;
        }

        // Mark characters that are part of a match
        let mut is_highlighted = vec![false; char_styles.len()];
        for (start, end) in &match_positions {
            for flag in is_highlighted.iter_mut().take(*end).skip(*start) {
                *flag = true;
            }
        }

        // Build new spans by grouping consecutive characters with the same effective style
        let mut new_spans: Vec<Span<'static>> = Vec::new();
        let mut current_text = String::new();
        let mut current_hl = false;
        let mut current_base_style = Style::default();

        for (i, (ch, base_style)) in char_styles.iter().enumerate() {
            let hl = is_highlighted[i];

            if i > 0 && (hl != current_hl || (!hl && *base_style != current_base_style)) {
                // Flush current span
                let style = if current_hl {
                    highlight_style
                } else {
                    current_base_style
                };
                new_spans.push(Span::styled(std::mem::take(&mut current_text), style));
            }
            current_text.push(*ch);
            current_hl = hl;
            current_base_style = *base_style;
        }
        // Flush remaining
        if !current_text.is_empty() {
            let style = if current_hl {
                highlight_style
            } else {
                current_base_style
            };
            new_spans.push(Span::styled(current_text, style));
        }

        result_lines.push(Line::from(new_spans));
    }

    (result_lines, match_indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: collect all visible text from lines
    fn lines_text(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_null() {
        let lines = json_to_lines(&json!(null));
        assert_eq!(lines_text(&lines), "null");
    }

    #[test]
    fn test_boolean_true() {
        let lines = json_to_lines(&json!(true));
        assert_eq!(lines_text(&lines), "true");
    }

    #[test]
    fn test_boolean_false() {
        let lines = json_to_lines(&json!(false));
        assert_eq!(lines_text(&lines), "false");
    }

    #[test]
    fn test_number() {
        let lines = json_to_lines(&json!(42));
        assert_eq!(lines_text(&lines), "42");
    }

    #[test]
    fn test_string() {
        let lines = json_to_lines(&json!("hello world"));
        assert_eq!(lines_text(&lines), "\"hello world\"");
    }

    #[test]
    fn test_string_with_newlines() {
        let lines = json_to_lines(&json!("line1\nline2\rline3"));
        assert_eq!(lines_text(&lines), "\"line1\\nline2\\rline3\"");
    }

    #[test]
    fn test_empty_array() {
        let lines = json_to_lines(&json!([]));
        assert_eq!(lines_text(&lines), "[]");
    }

    #[test]
    fn test_empty_object() {
        let lines = json_to_lines(&json!({}));
        assert_eq!(lines_text(&lines), "{}");
    }

    #[test]
    fn test_simple_object() {
        let lines = json_to_lines(&json!({"name": "test", "count": 5}));
        let text = lines_text(&lines);
        assert!(text.contains("{"));
        assert!(text.contains("}"));
        assert!(text.contains("\"name\""));
        assert!(text.contains("\"test\""));
        assert!(text.contains("\"count\""));
        assert!(text.contains("5"));
    }

    #[test]
    fn test_nested_object() {
        let val = json!({
            "outer": {
                "inner": "value"
            }
        });
        let lines = json_to_lines(&val);
        let text = lines_text(&lines);
        assert!(text.contains("\"outer\""));
        assert!(text.contains("\"inner\""));
        assert!(text.contains("\"value\""));
    }

    #[test]
    fn test_array_of_mixed_types() {
        let val = json!([1, "two", true, null]);
        let lines = json_to_lines(&val);
        let text = lines_text(&lines);
        assert!(text.contains("1"));
        assert!(text.contains("\"two\""));
        assert!(text.contains("true"));
        assert!(text.contains("null"));
    }

    #[test]
    fn test_search_single_match() {
        let lines = vec![
            Line::from("hello world"),
            Line::from("foo bar"),
            Line::from("hello again"),
        ];
        let (_, indices) = highlight_search_matches(&lines, "hello");
        assert_eq!(indices, vec![0, 2]);
    }

    #[test]
    fn test_search_case_insensitive() {
        let lines = vec![Line::from("Hello World"), Line::from("no match")];
        let (_, indices) = highlight_search_matches(&lines, "hello");
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn test_search_no_match() {
        let lines = vec![Line::from("hello world")];
        let (_, indices) = highlight_search_matches(&lines, "xyz");
        assert!(indices.is_empty());
    }

    #[test]
    fn test_search_empty_query() {
        let lines = vec![Line::from("hello")];
        let (result, indices) = highlight_search_matches(&lines, "");
        assert!(indices.is_empty());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_search_highlight_applied() {
        let lines = vec![Line::from(Span::styled(
            "find me here",
            Style::default().fg(Color::Green),
        ))];
        let (result, indices) = highlight_search_matches(&lines, "me");
        assert_eq!(indices, vec![0]);
        // The result line should have more spans due to splitting
        assert!(result[0].spans.len() > 1);
        // Check the highlighted span has yellow background
        let highlight_span = result[0]
            .spans
            .iter()
            .find(|s| s.content.as_ref() == "me")
            .expect("should find 'me' span");
        assert_eq!(highlight_span.style.bg, Some(Color::Yellow));
        assert_eq!(highlight_span.style.fg, Some(Color::Black));
    }

    #[test]
    fn test_search_multiple_matches_in_one_line() {
        let lines = vec![Line::from("aba aba")];
        let (result, indices) = highlight_search_matches(&lines, "aba");
        assert_eq!(indices, vec![0]);
        // Both "aba" substrings should be highlighted
        let highlighted_chars: usize = result[0]
            .spans
            .iter()
            .filter(|s| s.style.bg == Some(Color::Yellow))
            .map(|s| s.content.len())
            .sum();
        assert_eq!(highlighted_chars, 6); // "aba" + "aba"
    }
}
