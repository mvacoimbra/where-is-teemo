use crate::state::StealthMode;

/// Filter outgoing XMPP stanzas. When stealth mode is Offline,
/// replace <presence> stanzas with an "unavailable" type.
/// All other stanzas pass through unmodified.
pub fn filter_outgoing(stanza: &str, mode: &StealthMode) -> String {
    if *mode == StealthMode::Online {
        return stanza.to_string();
    }

    let trimmed = stanza.trim();

    // Only intercept <presence stanzas
    if !trimmed.starts_with("<presence") {
        return stanza.to_string();
    }

    // Self-closing presence: <presence ... />
    if trimmed.ends_with("/>") {
        return make_unavailable_self_closing(trimmed);
    }

    // Full presence stanza: <presence ...> ... </presence>
    if trimmed.contains("</presence>") {
        return make_unavailable(trimmed);
    }

    // If it doesn't match expected patterns, pass through
    stanza.to_string()
}

/// Replace a self-closing <presence .../> with type="unavailable".
fn make_unavailable_self_closing(stanza: &str) -> String {
    // Remove existing type attribute if present
    let without_type = remove_attribute(stanza, "type");
    // Insert type="unavailable" after <presence
    without_type.replacen("<presence", r#"<presence type="unavailable""#, 1)
}

/// Replace a full <presence>...</presence> with a minimal unavailable stanza.
fn make_unavailable(stanza: &str) -> String {
    // Extract the opening tag to preserve 'to', 'from', 'id' attributes
    let tag_end = stanza.find('>').unwrap_or(stanza.len());
    let opening = &stanza[..tag_end];

    // Remove existing type attribute, add unavailable
    let without_type = remove_attribute(opening, "type");
    format!(r#"{} type="unavailable"/>"#, without_type.trim_end_matches('/'))
}

/// Remove an XML attribute from a tag string.
fn remove_attribute(tag: &str, attr: &str) -> String {
    // Match: attr="value" or attr='value'
    let patterns = [
        format!(r#" {}=""#, attr),
        format!(r#" {}='"#, attr),
    ];

    for pat in &patterns {
        if let Some(start) = tag.find(pat.as_str()) {
            let quote = tag.as_bytes()[start + pat.len() - 1] as char;
            let value_start = start + pat.len();
            if let Some(end) = tag[value_start..].find(quote) {
                let mut result = String::with_capacity(tag.len());
                result.push_str(&tag[..start]);
                result.push_str(&tag[value_start + end + 1..]);
                return result;
            }
        }
    }

    tag.to_string()
}

/// Find the end of a complete XMPP stanza in a buffer.
/// Returns the byte index just past the closing tag, or None if incomplete.
pub fn find_stanza_end(buffer: &str) -> Option<usize> {
    let trimmed = buffer.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    // Pass through XML declarations and stream opening/features
    // These are special and should not be buffered as stanzas
    for prefix in &["<?xml", "<stream:stream", "<stream:features"] {
        if trimmed.starts_with(prefix) {
            // Find the end of this element
            if let Some(pos) = trimmed.find('>') {
                let offset = buffer.len() - trimmed.len();
                return Some(offset + pos + 1);
            }
            return None;
        }
    }

    // Handle </stream:stream> close
    if trimmed.starts_with("</stream:stream>") {
        let offset = buffer.len() - trimmed.len();
        return Some(offset + "</stream:stream>".len());
    }

    // Self-closing tags: <tag ... />
    if let Some(pos) = find_self_closing_end(buffer) {
        return Some(pos);
    }

    // Look for known stanza closing tags
    for close_tag in &["</presence>", "</message>", "</iq>", "</stream:features>"] {
        if let Some(pos) = buffer.find(close_tag) {
            return Some(pos + close_tag.len());
        }
    }

    None
}

/// Find end of self-closing tag like <presence ... />
fn find_self_closing_end(buffer: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut quote_char = '"';

    for (i, ch) in buffer.char_indices() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            c if c == quote_char && in_quotes => {
                in_quotes = false;
            }
            '/' if !in_quotes => {
                if buffer[i + 1..].starts_with('>') {
                    return Some(i + 2);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::StealthMode;

    #[test]
    fn test_filter_online_passthrough() {
        let stanza = r#"<presence><show>chat</show></presence>"#;
        assert_eq!(filter_outgoing(stanza, &StealthMode::Online), stanza);
    }

    #[test]
    fn test_filter_offline_full_presence() {
        let stanza = r#"<presence from="user@server" to="friend@server"><show>chat</show><status>Playing</status></presence>"#;
        let result = filter_outgoing(stanza, &StealthMode::Offline);
        assert!(result.contains(r#"type="unavailable""#));
        assert!(result.contains(r#"from="user@server""#));
        assert!(!result.contains("<show>"));
    }

    #[test]
    fn test_filter_offline_self_closing() {
        let stanza = r#"<presence from="user@server"/>"#;
        let result = filter_outgoing(stanza, &StealthMode::Offline);
        assert!(result.contains(r#"type="unavailable""#));
        assert!(result.contains(r#"from="user@server""#));
    }

    #[test]
    fn test_filter_non_presence_passthrough() {
        let stanza = r#"<message to="friend@server"><body>hello</body></message>"#;
        assert_eq!(filter_outgoing(stanza, &StealthMode::Offline), stanza);
    }

    #[test]
    fn test_find_stanza_end_complete() {
        let buf = r#"<presence><show>chat</show></presence>"#;
        assert_eq!(find_stanza_end(buf), Some(buf.len()));
    }

    #[test]
    fn test_find_stanza_end_incomplete() {
        let buf = r#"<presence><show>chat</show>"#;
        assert_eq!(find_stanza_end(buf), None);
    }

    #[test]
    fn test_find_stanza_end_self_closing() {
        let buf = r#"<presence from="user@server"/>"#;
        assert_eq!(find_stanza_end(buf), Some(buf.len()));
    }

    #[test]
    fn test_find_stanza_end_stream_open() {
        let buf = r#"<stream:stream xmlns="jabber:client" to="server">"#;
        assert_eq!(find_stanza_end(buf), Some(buf.len()));
    }

    #[test]
    fn test_replace_existing_type() {
        let stanza = r#"<presence type="available" from="user@server"><show>chat</show></presence>"#;
        let result = filter_outgoing(stanza, &StealthMode::Offline);
        assert!(result.contains(r#"type="unavailable""#));
        assert!(!result.contains(r#"type="available""#));
    }
}
