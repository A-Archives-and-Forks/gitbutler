use serde_json::Value;

const REDACTION: &str = "[REDACTED:entropy]";
const MIN_CANDIDATE_LEN: usize = 20;
const ENTROPY_THRESHOLD: f64 = 4.0;
const HEX_ENTROPY_THRESHOLD: f64 = 3.0;

pub(crate) fn redact_value(mut value: Value) -> Value {
    redact_value_in_place(&mut value, false);
    value
}

pub(crate) fn redact_text(value: &str) -> String {
    redact_string(value).unwrap_or_else(|| value.to_owned())
}

fn redact_value_in_place(value: &mut Value, sensitive_key: bool) {
    match value {
        Value::String(text) => {
            if sensitive_key && !text.is_empty() {
                *text = REDACTION.to_string();
            } else if let Some(redacted_text) = redact_string(text) {
                *text = redacted_text;
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_value_in_place(value, sensitive_key);
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                redact_value_in_place(value, sensitive_key || is_sensitive_key(key));
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    let normalized = lower.replace(['-', '_'], "");
    matches!(
        normalized.as_str(),
        "branch" | "cwd" | "path" | "uuid" | "worktree"
    ) || normalized.ends_with("apikey")
        || normalized.ends_with("authorization")
        || normalized.ends_with("password")
        || normalized.ends_with("passphrase")
        || normalized.ends_with("privatekey")
        || normalized == "id"
        || normalized.ends_with("token")
        || normalized.ends_with("secret")
        || normalized.ends_with("path")
        || lower.ends_with("_id")
        || lower.ends_with("-id")
        || key.ends_with("Id")
        || lower.ends_with("_uuid")
        || lower.ends_with("-uuid")
        || key.ends_with("Uuid")
        || lower.ends_with("_branch")
        || lower.ends_with("-branch")
        || key.ends_with("Branch")
}

fn redact_string(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut redacted = String::new();
    let mut changed = false;
    let mut last_written = 0;
    let mut index = 0;

    while index < bytes.len() {
        if !is_candidate_byte(bytes[index]) {
            index += 1;
            continue;
        }

        let start = index;
        while index < bytes.len() && is_candidate_byte(bytes[index]) {
            index += 1;
        }

        if index - start < MIN_CANDIDATE_LEN {
            continue;
        }

        let candidate = &text[start..index];
        if should_redact(candidate) {
            redacted.push_str(&text[last_written..start]);
            redacted.push_str(REDACTION);
            last_written = index;
            changed = true;
        }
    }

    if !changed {
        return None;
    }

    redacted.push_str(&text[last_written..]);
    Some(redacted)
}

fn should_redact(candidate: &str) -> bool {
    if contains_uuid(candidate) {
        return true;
    }

    let entropy = shannon_entropy(candidate);
    is_random_hex(candidate, entropy) || entropy > ENTROPY_THRESHOLD
}

fn is_candidate_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'+' | b'/' | b'='
    )
}

fn is_random_hex(candidate: &str, entropy: f64) -> bool {
    candidate.len() >= 32
        && candidate.bytes().all(|byte| byte.is_ascii_hexdigit())
        && entropy > HEX_ENTROPY_THRESHOLD
}

fn contains_uuid(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    if bytes.len() < 36 || !bytes.contains(&b'-') {
        return false;
    }

    bytes.windows(36).any(is_uuid_bytes)
}

fn is_uuid_bytes(bytes: &[u8]) -> bool {
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(index, byte)| {
        matches!(index, 8 | 13 | 18 | 23) && *byte == b'-'
            || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
    })
}

fn shannon_entropy(candidate: &str) -> f64 {
    let bytes = candidate.as_bytes();
    let mut counts = [0usize; 256];
    for byte in bytes {
        counts[*byte as usize] += 1;
    }

    let len = bytes.len() as f64;
    counts
        .into_iter()
        .filter(|count| *count > 0)
        .map(|count| {
            let probability = count as f64 / len;
            -probability * probability.log2()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn redacts_token_shaped_text() {
        for (input, expected) in [
            (
                "token: Nf9K2pLm8QwEr7TyUi4OzXa3Bv6Cn0Md done",
                "token: [REDACTED:entropy] done",
            ),
            ("550e8400-e29b-41d4-a716-446655440000", "[REDACTED:entropy]"),
            (
                "session_id=550e8400-e29b-41d4-a716-446655440000",
                "[REDACTED:entropy]",
            ),
            (
                "TOKEN=0123456789abcdef0123456789abcdef01234567; git commit -m fix",
                "[REDACTED:entropy]; git commit -m fix",
            ),
            (
                "token AbCdEfGhIjKl/MnOpQrSt/UvWxYz12+34= done",
                "token [REDACTED:entropy] done",
            ),
        ] {
            assert_eq!(redact_text(input), expected);
        }
    }

    #[test]
    fn keeps_obvious_non_secrets() {
        assert_eq!(
            redact_text("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(redact_text("2026-05-07T09-30-00Z"), "2026-05-07T09-30-00Z");
    }

    #[test]
    fn redacts_sensitive_key_values_recursively() {
        let value = json!({
            "api_key": "short",
            "openai_api_key": "short",
            "db_password": "short",
            "token": {
                "parts": ["alpha", "beta"],
            },
            "nested": {
                "secret": "Zx8Cv7Bn6Mm5Ll4Kk3Jj2Hh1Gg0Ff9Dd8",
            },
            "checksum": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "note": {
                "value": "short",
            },
        });

        let redacted = redact_value(value);

        assert_eq!(redacted["api_key"], "[REDACTED:entropy]");
        assert_eq!(redacted["openai_api_key"], "[REDACTED:entropy]");
        assert_eq!(redacted["db_password"], "[REDACTED:entropy]");
        assert_eq!(redacted["token"]["parts"][0], "[REDACTED:entropy]");
        assert_eq!(redacted["token"]["parts"][1], "[REDACTED:entropy]");
        assert_eq!(redacted["nested"]["secret"], "[REDACTED:entropy]");
        assert_eq!(redacted["checksum"], "[REDACTED:entropy]");
        assert_eq!(redacted["note"]["value"], "short");
    }
}
