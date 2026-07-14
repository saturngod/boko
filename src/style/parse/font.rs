//! Font-related CSS parsing.

use cssparser::{ParseError, Parser, Token};

use crate::model::FontFace;
use crate::style::properties::{FontStyle, FontWeight, Length};

use super::keywords::parse_font_style;

/// Parse font-size value (handles lengths, percentages, and keywords).
///
/// Supports absolute keywords: xx-small, x-small, small, medium, large, x-large, xx-large
/// Supports relative keywords: smaller, larger
pub(crate) fn parse_font_size(input: &mut Parser<'_, '_>) -> Option<Length> {
    match input.next().ok()? {
        Token::Dimension { value, unit, .. } => {
            let length = match unit.as_ref() {
                "px" => Length::Px(*value),
                "em" => Length::Em(*value),
                "rem" => Length::Rem(*value),
                "%" => Length::Percent(*value),
                "pt" => Length::Px(*value * 96.0 / 72.0), // Convert pt to px
                _ => return None,
            };
            Some(length)
        }
        Token::Percentage { unit_value, .. } => Some(Length::Percent(*unit_value * 100.0)),
        Token::Number { value, .. } if *value == 0.0 => Some(Length::Px(0.0)),
        Token::Ident(ident) => match ident.as_ref() {
            // Absolute size keywords (based on 16px default)
            // Values from CSS spec: https://www.w3.org/TR/css-fonts-3/#absolute-size-value
            "xx-small" => Some(Length::Rem(0.5625)), // 9px / 16px
            "x-small" => Some(Length::Rem(0.625)),   // 10px / 16px
            "small" => Some(Length::Rem(0.8125)),    // 13px / 16px
            "medium" => Some(Length::Rem(1.0)),      // 16px / 16px
            "large" => Some(Length::Rem(1.125)),     // 18px / 16px
            "x-large" => Some(Length::Rem(1.5)),     // 24px / 16px
            "xx-large" => Some(Length::Rem(2.0)),    // 32px / 16px
            "xxx-large" => Some(Length::Rem(3.0)),   // 48px / 16px (CSS4)
            // Relative size keywords (relative to parent, use em)
            "smaller" => Some(Length::Em(0.833)), // ~1/1.2
            "larger" => Some(Length::Em(1.2)),
            _ => None,
        },
        _ => None,
    }
}

/// Parse line-height value (handles unitless numbers and "normal" keyword).
pub(crate) fn parse_line_height(input: &mut Parser<'_, '_>) -> Option<Length> {
    match input.next().ok()? {
        Token::Dimension { value, unit, .. } => {
            let length = match unit.as_ref() {
                "px" => Length::Px(*value),
                "em" => Length::Em(*value),
                "rem" => Length::Rem(*value),
                "%" => Length::Percent(*value),
                _ => return None,
            };
            Some(length)
        }
        Token::Percentage { unit_value, .. } => Some(Length::Percent(*unit_value * 100.0)),
        // Unitless number becomes em multiplier
        Token::Number { value, .. } => Some(Length::Em(*value)),
        Token::Ident(ident) => match ident.as_ref() {
            "normal" => Some(Length::Auto),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn parse_font_weight(input: &mut Parser<'_, '_>) -> Option<FontWeight> {
    if let Ok(token) = input.try_parse(|i| i.expect_ident_cloned()) {
        let weight = match token.as_ref() {
            "normal" => FontWeight::NORMAL,
            "bold" => FontWeight::BOLD,
            "lighter" => FontWeight(300),
            "bolder" => FontWeight(700),
            _ => return None,
        };
        return Some(weight);
    }

    if let Ok(Token::Number {
        int_value: Some(v), ..
    }) = input.next()
    {
        let v = *v;
        if (100..=900).contains(&v) && v % 100 == 0 {
            return Some(FontWeight(v as u16));
        }
    }

    None
}

pub(crate) fn parse_font_family(input: &mut Parser<'_, '_>) -> Option<String> {
    let mut families = Vec::new();

    loop {
        // A family name is either a quoted string, or a sequence of one or more
        // unquoted identifiers joined by spaces (e.g. `Palatino Linotype`).
        if let Ok(token) = input.try_parse(|i| i.expect_string_cloned()) {
            families.push(token.to_string());
        } else {
            let mut idents = Vec::new();
            while let Ok(token) = input.try_parse(|i| i.expect_ident_cloned()) {
                idents.push(token.to_string());
            }
            if idents.is_empty() {
                break;
            }
            families.push(idents.join(" "));
        }

        if input.try_parse(|i| i.expect_comma()).is_err() {
            break;
        }
    }

    if families.is_empty() {
        None
    } else {
        Some(families.join(", "))
    }
}

// ============================================================================
// @font-face Parsing
// ============================================================================

/// Parse a @font-face block and return a FontFace if successful.
///
/// @font-face rules have the form:
/// ```css
/// @font-face {
///     font-family: "Ubuntu";
///     font-weight: bold;
///     font-style: normal;
///     src: url(../fonts/Ubuntu-B.ttf);
/// }
/// ```
pub(crate) fn parse_font_face_block(input: &mut Parser<'_, '_>) -> Option<FontFace> {
    let mut font_family: Option<String> = None;
    let mut font_weight = FontWeight::NORMAL;
    let mut font_style = FontStyle::Normal;
    let mut src: Option<String> = None;

    // Parse declarations within the @font-face block. Each declaration's value
    // is parsed inside its own `;`-delimited scope so that tokens a specific
    // parser leaves behind (e.g. the `format("woff")` after a `src` url) can't
    // leak into and derail the next declaration.
    while let Ok(name) = input.expect_ident_cloned() {
        let name_str = name.as_ref().to_string();
        if input.expect_colon().is_err() {
            continue;
        }
        let _ = input.parse_until_after(
            cssparser::Delimiter::Semicolon,
            |value_input| -> Result<(), ParseError<'_, ()>> {
                match name_str.as_str() {
                    "font-family" => font_family = parse_font_face_family(value_input),
                    "font-weight" => {
                        if let Some(w) = parse_font_weight(value_input) {
                            font_weight = w;
                        }
                    }
                    "font-style" => {
                        if let Some(s) = parse_font_style(value_input) {
                            font_style = s;
                        }
                    }
                    "src" => src = parse_font_face_src(value_input),
                    _ => {}
                }
                // Drain anything the value parser didn't consume so the scope
                // advances cleanly to (and past) the delimiter.
                while value_input.next().is_ok() {}
                Ok(())
            },
        );
    }

    // Require both font-family and src
    match (font_family, src) {
        (Some(family), Some(source)) => {
            Some(FontFace::new(family, font_weight, font_style, source))
        }
        _ => None,
    }
}

/// Parse font-family value in @font-face (quoted or unquoted name).
fn parse_font_face_family(input: &mut Parser<'_, '_>) -> Option<String> {
    // Try quoted string first
    if let Ok(s) = input.try_parse(|i| i.expect_string_cloned()) {
        return Some(s.to_string());
    }
    // Try unquoted identifier
    if let Ok(s) = input.try_parse(|i| i.expect_ident_cloned()) {
        return Some(s.to_string());
    }
    None
}

/// Parse src value in @font-face: url(...) or local(...).
fn parse_font_face_src(input: &mut Parser<'_, '_>) -> Option<String> {
    // We support url() format only
    if let Ok(url) = input.try_parse(|i| i.expect_url_or_string()) {
        return Some(url.as_ref().to_string());
    }
    // Try parsing url() function with string argument
    if let Ok(url) = input.try_parse(|i| -> Result<String, ParseError<'_, ()>> {
        i.expect_function_matching("url")?;
        let url_str = i.parse_nested_block(|nested| {
            nested
                .expect_string_cloned()
                .map(|s| s.to_string())
                .map_err(|e| e.into())
        })?;
        Ok(url_str)
    }) {
        return Some(url);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssparser::{Parser, ParserInput};

    fn family(css: &str) -> Option<String> {
        let mut input = ParserInput::new(css);
        parse_font_family(&mut Parser::new(&mut input))
    }

    #[test]
    fn multi_word_unquoted_family_is_not_truncated() {
        // "Palatino Linotype" is one family; the fallback must survive too.
        assert_eq!(
            family("Palatino Linotype, serif").as_deref(),
            Some("Palatino Linotype, serif")
        );
    }

    #[test]
    fn quoted_and_unquoted_families_mix() {
        assert_eq!(
            family("\"Times New Roman\", Georgia, serif").as_deref(),
            Some("Times New Roman, Georgia, serif")
        );
    }
}
