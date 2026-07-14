//! Pure markdown escaping utilities.
//!
//! These functions handle escaping special Markdown characters and calculating
//! appropriate fence/tick lengths for code blocks and inline code.

/// Escape special Markdown characters in text.
///
/// `at_line_start` states whether the first character of `text` will land at
/// the start of an output line; line-start-only markers are escaped only at
/// true line starts (tracked across any newlines inside the chunk).
///
/// Escapes characters that have special meaning in Markdown:
/// - Backslash: `\\`
/// - Emphasis: `*`, `_`
/// - Links: `[`, `]`
/// - Code: `` ` ``
/// - Headings: `#` (only at line start)
/// - Bullet list markers: `-`, `+` followed by whitespace (only at line start)
/// - Ordered list markers: digits then `.` or `)` then whitespace (only at
///   line start; the delimiter is escaped)
/// - Tables: `|`
/// - HTML: `<`, `>`
/// - Images: `!` (when followed by `[`)
///
/// # Examples
///
/// ```ignore (crate-internal; exercised by unit tests)
/// use crate::markdown::escape_markdown_at;
///
/// assert_eq!(escape_markdown_at("*bold*", true), "\\*bold\\*");
/// assert_eq!(escape_markdown_at("- item", true), "\\- item");
/// assert_eq!(escape_markdown_at("- item", false), "- item");
/// ```
pub fn escape_markdown_at(text: &str, mut at_line_start: bool) -> String {
    let mut result = String::with_capacity(text.len() + text.len() / 10);
    let bytes = text.as_bytes();
    let mut chars = text.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => result.push_str("\\\\"),
            '*' | '_' => {
                result.push('\\');
                result.push(c);
            }
            '[' | ']' => {
                result.push('\\');
                result.push(c);
            }
            '`' => {
                result.push('\\');
                result.push(c);
            }
            '#' if at_line_start => {
                result.push('\\');
                result.push(c);
            }
            '-' | '+' if at_line_start && is_marker_terminator(bytes.get(i + 1)) => {
                // A bullet list marker at a line start would turn the text
                // into a list item.
                result.push('\\');
                result.push(c);
            }
            '0'..='9' if at_line_start => {
                // Digits at a line start followed by `.` or `)` and
                // whitespace form an ordered-list marker; escape the
                // delimiter so `1. item` stays literal text.
                let mut j = i + 1;
                while bytes.get(j).is_some_and(u8::is_ascii_digit) {
                    j += 1;
                }
                let is_marker = matches!(bytes.get(j), Some(b'.' | b')'))
                    && is_marker_terminator(bytes.get(j + 1));
                result.push(c);
                if is_marker {
                    // Emit the remaining digits, then the escaped delimiter.
                    while let Some(&(k, d)) = chars.peek() {
                        if k >= j {
                            break;
                        }
                        result.push(d);
                        chars.next();
                    }
                    if let Some((_, delim)) = chars.next() {
                        result.push('\\');
                        result.push(delim);
                    }
                }
            }
            '|' => {
                result.push('\\');
                result.push(c);
            }
            '<' | '>' => {
                result.push('\\');
                result.push(c);
            }
            '!' if chars.peek().map(|&(_, n)| n) == Some('[') => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
        at_line_start = c == '\n';
    }

    result
}

/// Whether the byte after a candidate list marker makes it an actual marker:
/// whitespace or end of text.
fn is_marker_terminator(byte: Option<&u8>) -> bool {
    matches!(byte, None | Some(b' ' | b'\t' | b'\n'))
}

/// Calculate the minimum fence length needed for a code block.
///
/// Returns the smallest number of fence characters (at least 3) that
/// doesn't appear as a run in the content.
///
/// # Examples
///
/// ```ignore (crate-internal; exercised by unit tests)
/// use crate::markdown::calculate_fence_length;
///
/// // Normal content needs 3 backticks
/// assert_eq!(calculate_fence_length("let x = 1;", '`'), 3);
///
/// // Content with 3 backticks needs 4
/// assert_eq!(calculate_fence_length("```rust\ncode\n```", '`'), 4);
/// ```
pub fn calculate_fence_length(content: &str, fence_char: char) -> usize {
    let mut max_run = 0;
    let mut current_run = 0;

    for c in content.chars() {
        if c == fence_char {
            current_run += 1;
            max_run = max_run.max(current_run);
        } else {
            current_run = 0;
        }
    }

    max_run.max(2) + 1
}

/// Calculate the minimum backtick count needed for inline code.
///
/// Returns the smallest number of backticks (at least 1) that doesn't
/// appear as a run in the content.
///
/// # Examples
///
/// ```ignore (crate-internal; exercised by unit tests)
/// use crate::markdown::calculate_inline_code_ticks;
///
/// // Normal content needs 1 backtick
/// assert_eq!(calculate_inline_code_ticks("code"), 1);
///
/// // Content with backticks needs more
/// assert_eq!(calculate_inline_code_ticks("code with ` backtick"), 2);
/// ```
pub fn calculate_inline_code_ticks(content: &str) -> usize {
    let mut max_run = 0;
    let mut current_run = 0;

    for c in content.chars() {
        if c == '`' {
            current_run += 1;
            max_run = max_run.max(current_run);
        } else {
            current_run = 0;
        }
    }

    max_run + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Escape assuming the chunk begins at the start of an output line.
    fn escape_markdown(text: &str) -> String {
        escape_markdown_at(text, true)
    }

    #[test]
    fn test_escape_backslash() {
        assert_eq!(escape_markdown("a\\b"), "a\\\\b");
    }

    #[test]
    fn test_escape_emphasis() {
        assert_eq!(escape_markdown("*bold*"), "\\*bold\\*");
        assert_eq!(escape_markdown("_italic_"), "\\_italic\\_");
    }

    #[test]
    fn test_escape_links() {
        assert_eq!(escape_markdown("[link]"), "\\[link\\]");
    }

    #[test]
    fn test_escape_code() {
        assert_eq!(escape_markdown("`code`"), "\\`code\\`");
    }

    #[test]
    fn test_escape_heading_at_line_start() {
        assert_eq!(escape_markdown("# heading"), "\\# heading");
        // # in the middle of a line is not escaped
        assert_eq!(escape_markdown("not # heading"), "not # heading");
        assert_eq!(escape_markdown("line\n# heading"), "line\n\\# heading");
    }

    #[test]
    fn test_no_heading_escape_mid_line_chunk() {
        // A chunk that begins mid-line must not escape line-start markers.
        assert_eq!(escape_markdown_at("# heading", false), "# heading");
        // ...but a newline inside the chunk restores line-start tracking.
        assert_eq!(
            escape_markdown_at("tail\n# heading", false),
            "tail\n\\# heading"
        );
    }

    #[test]
    fn test_escape_bullet_marker_at_line_start() {
        assert_eq!(escape_markdown("- item"), "\\- item");
        assert_eq!(escape_markdown("+ item"), "\\+ item");
        assert_eq!(escape_markdown("line\n- item"), "line\n\\- item");
        // Mid-line dashes are not list markers.
        assert_eq!(escape_markdown("foo - bar"), "foo - bar");
        assert_eq!(escape_markdown_at("- item", false), "- item");
        // Hyphenated words are untouched even at line start.
        assert_eq!(escape_markdown("well-known"), "well-known");
    }

    #[test]
    fn test_escape_ordered_list_marker_at_line_start() {
        assert_eq!(escape_markdown("1. item"), "1\\. item");
        assert_eq!(escape_markdown("12) item"), "12\\) item");
        assert_eq!(escape_markdown("line\n2. item"), "line\n2\\. item");
        // Mid-line or mid-chunk numbers are untouched.
        assert_eq!(escape_markdown("see 1. item"), "see 1. item");
        assert_eq!(escape_markdown_at("1. item", false), "1. item");
        // Digits not followed by a list delimiter are untouched.
        assert_eq!(escape_markdown("1990 was"), "1990 was");
        assert_eq!(escape_markdown("3.14 is pi"), "3.14 is pi");
    }

    #[test]
    fn test_escape_table_pipe() {
        assert_eq!(escape_markdown("a | b"), "a \\| b");
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_markdown("<tag>"), "\\<tag\\>");
    }

    #[test]
    fn test_escape_image_start() {
        // ! followed by [ starts an image, so ! is escaped
        assert_eq!(escape_markdown("![alt]"), "\\!\\[alt\\]");
        // ! not followed by [ is not escaped
        assert_eq!(escape_markdown("! not image"), "! not image");
    }

    #[test]
    fn test_fence_length_no_backticks() {
        assert_eq!(calculate_fence_length("let x = 1;", '`'), 3);
    }

    #[test]
    fn test_fence_length_with_backticks() {
        assert_eq!(calculate_fence_length("``", '`'), 3);
        assert_eq!(calculate_fence_length("```", '`'), 4);
        assert_eq!(calculate_fence_length("````", '`'), 5);
    }

    #[test]
    fn test_fence_length_multiple_runs() {
        assert_eq!(calculate_fence_length("`` and ```", '`'), 4);
    }

    #[test]
    fn test_inline_code_ticks_no_backticks() {
        assert_eq!(calculate_inline_code_ticks("code"), 1);
    }

    #[test]
    fn test_inline_code_ticks_with_backticks() {
        assert_eq!(calculate_inline_code_ticks("`"), 2);
        assert_eq!(calculate_inline_code_ticks("``"), 3);
    }
}
