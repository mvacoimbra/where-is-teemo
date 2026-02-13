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

    let offset = buffer.len() - trimmed.len();

    // XML processing instructions: <?xml ... ?>
    if trimmed.starts_with("<?") {
        if let Some(pos) = trimmed.find("?>") {
            return Some(offset + pos + 2);
        }
        return None;
    }

    // Closing tags like </stream:stream>
    if trimmed.starts_with("</") {
        if let Some(pos) = trimmed.find('>') {
            return Some(offset + pos + 1);
        }
        return None;
    }

    // Must start with '<' for an opening tag
    if !trimmed.starts_with('<') {
        // Non-XML data — forward up to the next '<' or end of buffer
        return Some(offset + trimmed.find('<').unwrap_or(trimmed.len()));
    }

    // Self-closing tags: <tag ... />
    if let Some(pos) = find_self_closing_end(trimmed) {
        return Some(offset + pos);
    }

    // Extract the tag name to find its closing tag dynamically
    let tag_name = extract_tag_name(trimmed)?;

    // <stream:stream> is a stream-level open — ends at '>', never closed in same stanza
    if tag_name == "stream:stream" {
        if let Some(pos) = trimmed.find('>') {
            return Some(offset + pos + 1);
        }
        return None;
    }

    // Look for the matching closing tag </tagname>
    let close_tag = format!("</{tag_name}>");
    if let Some(pos) = trimmed.find(&close_tag) {
        return Some(offset + pos + close_tag.len());
    }

    None
}

/// Extract the element name from an opening tag (e.g. "<auth " → "auth").
fn extract_tag_name(s: &str) -> Option<&str> {
    let after_lt = &s[1..]; // skip '<'
    let end = after_lt.find(|c: char| c.is_whitespace() || c == '>' || c == '/')?;
    if end == 0 {
        return None;
    }
    Some(&after_lt[..end])
}

/// Find end of a self-closing opening tag like `<presence ... />`.
/// Only matches `/>` that belongs to the root element — if we see a bare `>`
/// first (closing the opening tag), the element has body content and is NOT
/// self-closing, so we return None.
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
            '>' if !in_quotes => {
                // A bare '>' before any '/>' means the opening tag closed and
                // element has body content — not a self-closing tag.
                return None;
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

    #[test]
    fn test_find_stanza_end_auth() {
        let buf = r#"<auth xmlns="urn:ietf:params:xml:ns:xmpp-sasl" mechanism="X-Riot-RSO">dG9rZW4=</auth>"#;
        assert_eq!(find_stanza_end(buf), Some(buf.len()));
    }

    #[test]
    fn test_find_stanza_end_xml_declaration() {
        let buf = r#"<?xml version='1.0'?>"#;
        assert_eq!(find_stanza_end(buf), Some(buf.len()));
    }

    #[test]
    fn test_find_stanza_end_close_stream() {
        let buf = "</stream:stream>";
        assert_eq!(find_stanza_end(buf), Some(buf.len()));
    }

    #[test]
    fn test_find_stanza_end_stream_features() {
        let buf = r#"<stream:features><mechanisms xmlns="urn:ietf:params:xml:ns:xmpp-sasl"><mechanism>X-Riot-RSO</mechanism></mechanisms></stream:features>"#;
        assert_eq!(find_stanza_end(buf), Some(buf.len()));
    }

    #[test]
    fn test_find_stanza_end_response() {
        let buf = r#"<response xmlns="urn:ietf:params:xml:ns:xmpp-sasl">dG9rZW4=</response>"#;
        assert_eq!(find_stanza_end(buf), Some(buf.len()));
    }

    #[test]
    fn test_find_stanza_end_child_self_closing_not_confused() {
        // A presence stanza with a self-closing child element (<pty/>) should
        // NOT be split at <pty/> — it must wait for </presence>.
        let buf = r#"<presence id='5'><show>chat</show><games><keystone><pty/></keystone></games></presence>"#;
        assert_eq!(find_stanza_end(buf), Some(buf.len()));
    }
}
