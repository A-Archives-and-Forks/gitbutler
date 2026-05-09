use std::{borrow::Cow, collections::HashSet, path::Path};

use anyhow::{Context as _, Result, bail};
use chrono::{SecondsFormat, Utc};
use git_meta_lib::{ListEntry, MetaEdit, MetaValue, Session, Target};
use serde::Serialize;
use serde_json::Value;

use crate::{
    agent::Agent,
    redaction::{redact_text, redact_value},
    transcript::{RecordKind, TranscriptBatch},
};

const MAX_TOOL_RESULT_TEXT_BYTES: usize = 4 * 1024;
const TRUNCATION_MARKER: &str = "\n[TRUNCATED]\n";

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct TranscriptSource {
    agent: &'static str,
    provider: Option<String>,
    model: Option<String>,
    tool_version: Option<String>,
}

pub(crate) fn write_transcript_batch(
    repo_path: &Path,
    agent: Agent,
    session_key: &str,
    source_key: &str,
    batch: TranscriptBatch,
) -> Result<usize> {
    let TranscriptBatch {
        provider,
        model,
        tool_version,
        mut records,
        ..
    } = batch;

    if records.is_empty() {
        return Ok(0);
    }

    let session_prefix = format!("gitbutler:agent-session:{session_key}");
    let sources_key = format!("{session_prefix}:sources");
    let source_prefix = format!("{session_prefix}:source:{source_key}");
    let transcript_key = format!("{session_prefix}:transcript");
    let record_hashes_key = format!("{session_prefix}:record-hashes");

    let gitmeta = Session::open(repo_path)?;
    let target = Target::project();
    let handle = gitmeta.target(&target);
    let mut seen_hashes = match handle.get_value(&record_hashes_key)? {
        None => HashSet::new(),
        Some(MetaValue::Set(hashes)) => hashes.into_iter().collect(),
        Some(_) => bail!("existing GitMeta key '{record_hashes_key}' is not a set"),
    };
    records.retain(|record| seen_hashes.insert(record.source_record_hash.clone()));

    if records.is_empty() {
        return Ok(0);
    }

    let records_captured = records.len();
    let mut record_hashes = Vec::with_capacity(records_captured);
    let mut transcript_entries = Vec::with_capacity(records_captured);
    let entry_timestamp = Utc::now().timestamp_millis();
    for (entry_timestamp, record) in (entry_timestamp..).zip(records) {
        let record_hash = record.source_record_hash;
        let transcript_record = TranscriptRecord {
            source_key,
            record_index: record.index,
            record_hash: &record_hash,
            timestamp: record.source_timestamp.as_deref().map(redact_text),
            kind: record.kind,
            source_event_kind: redact_text(&record.source_event_kind),
            role: record.role.as_deref().map(redact_text),
            text: stored_text(record.kind, record.text.as_deref()),
            tool_name: record.tool_name.as_deref().map(redact_text),
            tool_input: record.tool_input.map(redact_value),
            source_record: redact_value(record.source_record),
        };
        let transcript_record = serde_json::to_string(&transcript_record)?;
        record_hashes.push(record_hash);
        transcript_entries.push(ListEntry {
            value: transcript_record,
            timestamp: entry_timestamp,
        });
    }

    let source = TranscriptSource {
        agent: agent.as_str(),
        provider: provider.as_deref().map(redact_text),
        model: model.as_deref().map(redact_text),
        tool_version: tool_version.as_deref().map(redact_text),
    };

    let updated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    handle.set_add("gitbutler:agent-sessions", session_key)?;
    handle.set(
        &format!("{session_prefix}:schema"),
        "gitbutler.agent-session.v1",
    )?;
    handle.set(&format!("{session_prefix}:updated-at"), updated_at)?;
    handle.set_add(&sources_key, source_key)?;
    handle.set_record(&source_prefix, source)?;

    handle.apply_edits([
        MetaEdit::list_append(&transcript_key, &transcript_entries),
        MetaEdit::set_add(&record_hashes_key, &record_hashes),
    ])?;

    Ok(records_captured)
}

pub(crate) fn associate_session(
    repo_path: &Path,
    target: &Target,
    session_key: &str,
) -> Result<()> {
    let gitmeta = Session::open(repo_path)?;
    gitmeta
        .target(target)
        .set_add("gitbutler:agent-sessions", session_key)?;
    Ok(())
}

pub(crate) fn sync_metadata(repo_path: &Path) -> Result<()> {
    const MAX_RETRIES: usize = 5;

    let gitmeta = Session::open(repo_path).context("failed to open GitMeta session")?;
    match gitmeta.pull(None) {
        Ok(_) => {}
        // An empty metadata remote has no ref yet; the push below initializes it.
        Err(git_meta_lib::Error::GitCommand(message))
            if message.contains("couldn't find remote ref") => {}
        Err(err) => return Err(err).context("failed to pull GitMeta metadata"),
    }

    let mut attempts = 0;
    loop {
        attempts += 1;
        let output = gitmeta
            .push_once(None)
            .context("failed to push GitMeta metadata")?;
        if output.success {
            return Ok(());
        }
        if !output.non_fast_forward {
            bail!("push failed");
        }
        if attempts >= MAX_RETRIES {
            bail!("push failed after {MAX_RETRIES} attempts");
        }
        gitmeta
            .resolve_push_conflict(None)
            .context("failed to resolve GitMeta push conflict")?;
    }
}

fn stored_text(kind: RecordKind, text: Option<&str>) -> Option<String> {
    let text = text?;
    Some(match kind {
        RecordKind::ToolResult => redact_text(cap_tool_result_text(text).as_ref()),
        _ => redact_text(text),
    })
}

fn cap_tool_result_text(text: &str) -> Cow<'_, str> {
    if text.len() <= MAX_TOOL_RESULT_TEXT_BYTES {
        return Cow::Borrowed(text);
    }

    let body_bytes = MAX_TOOL_RESULT_TEXT_BYTES - TRUNCATION_MARKER.len();
    let head_end = floor_char_boundary(text, body_bytes / 2);
    let tail_start = ceil_char_boundary(text, text.len() - (body_bytes - head_end));
    Cow::Owned(format!(
        "{}{}{}",
        &text[..head_end],
        TRUNCATION_MARKER,
        &text[tail_start..]
    ))
}

fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

// GitMeta list order is storage order, not the public transcript ordering
// contract. Readers should sort by transcript fields, primarily source_key and
// record_index, when rendering a session timeline.
#[derive(Serialize)]
struct TranscriptRecord<'a> {
    source_key: &'a str,
    record_index: usize,
    record_hash: &'a str,
    timestamp: Option<String>,
    kind: RecordKind,
    source_event_kind: String,
    role: Option<String>,
    text: Option<String>,
    tool_name: Option<String>,
    tool_input: Option<Value>,
    source_record: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::record_transcript;
    use git_meta_lib::MetaValue;
    use std::{
        fs,
        process::{Command, Stdio},
    };
    use tempfile::TempDir;

    fn setup_repo() -> TempDir {
        let dir = TempDir::new().expect("temp repo");
        let status = Command::new("git")
            .args(["init", "--initial-branch", "main"])
            .current_dir(dir.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git init");
        assert!(status.success());
        dir
    }

    fn write_transcript(repo: &Path, body: &str) -> std::path::PathBuf {
        let path = repo.join("session.jsonl");
        fs::write(&path, body).expect("write transcript");
        path
    }

    fn project_target() -> Target {
        Target::parse("project").expect("project target")
    }

    fn capture_project(repo: &Path, agent: Agent, transcript: &Path) -> usize {
        record_transcript(repo, agent, transcript)
            .expect("capture")
            .0
    }

    fn target_value(repo: &Path, target: &Target, key: &str) -> Option<MetaValue> {
        let session = Session::open(repo).expect("open session");
        session
            .target(target)
            .get_value(key)
            .expect("read GitMeta value")
    }

    fn project_value(repo: &Path, key: &str) -> Option<MetaValue> {
        target_value(repo, &project_target(), key)
    }

    fn jsonl(records: impl IntoIterator<Item = serde_json::Value>) -> String {
        let mut output = String::new();
        for record in records {
            output.push_str(&record.to_string());
            output.push('\n');
        }
        output
    }

    fn codex_fixture() -> String {
        jsonl([
            serde_json::json!({
                "timestamp": "2026-05-07T09:00:00Z",
                "type": "session_meta",
                "payload": {
                    "id": "session-1",
                    "model_provider": "openai",
                    "cli_version": "0.1.0",
                },
            }),
            serde_json::json!({
                "timestamp": "2026-05-07T09:00:01Z",
                "type": "turn_context",
                "payload": {
                    "model": "gpt-5.5",
                },
            }),
            serde_json::json!({
                "timestamp": "2026-05-07T09:00:02Z",
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "turn_id": "turn-1",
                    "role": "assistant",
                    "content": "Implemented change",
                },
            }),
        ])
    }

    fn only_session_key(repo: &Path) -> String {
        let index = project_value(repo, "gitbutler:agent-sessions").expect("session index value");
        let MetaValue::Set(values) = index else {
            panic!("expected session index set");
        };
        assert_eq!(values.len(), 1);
        values.into_iter().next().expect("session key")
    }

    fn only_source_key(repo: &Path, session_key: &str) -> String {
        let session_prefix = format!("gitbutler:agent-session:{session_key}");
        let sources =
            project_value(repo, &format!("{session_prefix}:sources")).expect("source index value");
        let MetaValue::Set(values) = sources else {
            panic!("expected source index set");
        };
        assert_eq!(values.len(), 1);
        values.into_iter().next().expect("source key")
    }

    fn transcript_entries(repo: &Path, session_key: &str) -> Vec<String> {
        let transcript_value = project_value(
            repo,
            &format!("gitbutler:agent-session:{session_key}:transcript"),
        )
        .expect("transcript list");
        let MetaValue::List(entries) = transcript_value else {
            panic!("expected transcript list");
        };
        entries.into_iter().map(|entry| entry.value).collect()
    }

    fn record_hashes(repo: &Path, session_key: &str) -> Vec<String> {
        let hashes = project_value(
            repo,
            &format!("gitbutler:agent-session:{session_key}:record-hashes"),
        )
        .expect("record hash set");
        let MetaValue::Set(hashes) = hashes else {
            panic!("expected record hash set");
        };
        hashes.into_iter().collect()
    }

    fn session_index(repo: &Path, target: &Target) -> Vec<String> {
        let Some(MetaValue::Set(values)) = target_value(repo, target, "gitbutler:agent-sessions")
        else {
            panic!("expected session index set");
        };
        values.into_iter().collect()
    }

    #[test]
    fn codex_capture_can_be_read_back_and_is_idempotent() {
        let repo = setup_repo();
        let transcript = write_transcript(repo.path(), &codex_fixture());

        let report = capture_project(repo.path(), Agent::Codex, &transcript);
        let report_again = capture_project(repo.path(), Agent::Codex, &transcript);

        assert_eq!(report, 1);
        assert_eq!(report_again, 0);

        let session_key = only_session_key(repo.path());
        let source_key = only_source_key(repo.path(), &session_key);
        let session = Session::open(repo.path()).expect("open session");
        let target = project_target();
        let source_prefix = format!("gitbutler:agent-session:{session_key}:source:{source_key}");
        let source: serde_json::Value = session
            .target(&target)
            .get_record(&source_prefix)
            .expect("read source record")
            .expect("source record");
        assert_eq!(source["agent"], "codex");
        assert_eq!(source["provider"], "openai");
        assert_eq!(source["model"], "gpt-5.5");
        assert_eq!(source["tool-version"], "0.1.0");

        let records = transcript_entries(repo.path(), &session_key)
            .into_iter()
            .map(|entry| {
                serde_json::from_str::<serde_json::Value>(&entry).expect("transcript record json")
            })
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        assert_eq!(record_hashes(repo.path(), &session_key).len(), 1);
        assert_eq!(records[0]["source_key"], source_key);
        assert_eq!(records[0]["kind"], "message");
        assert_eq!(
            records[0]["source_event_kind"],
            "codex:response_item:message"
        );
        assert_eq!(records[0]["role"], "assistant");
        assert_eq!(records[0]["text"], "Implemented change");
        assert_eq!(records[0]["source_record"]["type"], "response_item");
        assert_eq!(records[0]["source_record"]["payload"]["type"], "message");
        assert!(
            !records[0]["source_record"]
                .to_string()
                .contains("Implemented change")
        );
    }

    #[test]
    fn transcript_records_redact_secrets_and_copied_scalar_fields() {
        let repo = setup_repo();
        let session_id = "550e8400-e29b-41d4-a716-446655440000";
        let secret = "Nf9K2pLm8QwEr7TyUi4OzXa3Bv6Cn0Md";
        let record = serde_json::json!({
            "timestamp": "2026-05-07T09:00:00Z",
            "type": "response_item",
            "payload": {
                "type": "message",
                "id": session_id,
                "message_id": session_id,
                "api_key": secret,
                "content": format!("token: {secret}"),
            },
        });
        let transcript = write_transcript(repo.path(), &jsonl([record]));

        let report = capture_project(repo.path(), Agent::Codex, &transcript);

        assert_eq!(report, 1);
        let session_key = only_session_key(repo.path());
        let entries = transcript_entries(repo.path(), &session_key);
        let record: serde_json::Value =
            serde_json::from_str(&entries[0]).expect("transcript record json");
        assert_eq!(record["text"], "token: [REDACTED:entropy]");
        let source_record = record["source_record"].to_string();
        assert!(!source_record.contains(secret));
        assert!(!source_record.contains(session_id));
    }

    #[test]
    fn tool_payloads_are_slimmed_before_storage() {
        let repo = setup_repo();
        let long_output = format!(
            "start:{}:end",
            "x".repeat(MAX_TOOL_RESULT_TEXT_BYTES + 1024)
        );
        let transcript = write_transcript(
            repo.path(),
            &jsonl([
                serde_json::json!({
                    "timestamp": "2026-05-07T09:00:00Z",
                    "type": "session_meta",
                    "payload": { "id": "session-1" },
                }),
                serde_json::json!({
                    "timestamp": "2026-05-07T09:00:01Z",
                    "type": "response_item",
                    "payload": {
                        "type": "function_call",
                        "name": "exec_command",
                        "arguments": "{\"cmd\":\"cargo test\"}",
                    },
                }),
                serde_json::json!({
                    "timestamp": "2026-05-07T09:00:02Z",
                    "type": "response_item",
                    "payload": {
                        "type": "function_call_output",
                        "output": long_output,
                    },
                }),
                serde_json::json!({
                    "timestamp": "2026-05-07T09:00:03Z",
                    "type": "response_item",
                    "payload": {
                        "type": "function_call_output",
                        "output": {
                            "raw": "x".repeat(MAX_TOOL_RESULT_TEXT_BYTES + 1024),
                        },
                    },
                }),
            ]),
        );

        assert_eq!(capture_project(repo.path(), Agent::Codex, &transcript), 3);
        let session_key = only_session_key(repo.path());
        let records = transcript_entries(repo.path(), &session_key)
            .into_iter()
            .map(|entry| {
                serde_json::from_str::<serde_json::Value>(&entry).expect("transcript record json")
            })
            .collect::<Vec<_>>();

        assert_eq!(records[0]["kind"], "tool_call");
        assert_eq!(records[0]["tool_input"]["cmd"], "cargo test");
        assert_eq!(records[1]["kind"], "tool_result");
        let text = records[1]["text"].as_str().expect("tool result text");
        assert!(text.len() <= MAX_TOOL_RESULT_TEXT_BYTES);
        assert!(text.starts_with("start:"));
        assert!(text.contains(TRUNCATION_MARKER.trim()));
        assert!(text.ends_with(":end"));
        assert!(records[1]["source_record"]["payload"]["output"].is_null());
        assert_eq!(records[2]["kind"], "tool_result");
        assert!(records[2]["text"].is_null());
        assert!(records[2]["source_record"]["payload"]["output"].is_null());
    }

    #[test]
    fn transcript_without_capturable_records_does_not_create_session_metadata() {
        let repo = setup_repo();
        let transcript = write_transcript(
            repo.path(),
            &jsonl([
                serde_json::json!({
                    "timestamp": "2026-05-07T09:00:00Z",
                    "type": "session_meta",
                    "payload": { "id": "session-1" },
                }),
                serde_json::json!({
                    "timestamp": "2026-05-07T09:00:01Z",
                    "type": "event_msg",
                    "payload": { "type": "info" },
                }),
            ]),
        );

        let report = capture_project(repo.path(), Agent::Codex, &transcript);

        assert_eq!(report, 0);
        assert!(project_value(repo.path(), "gitbutler:agent-sessions").is_none());
    }

    #[test]
    fn duplicate_records_in_one_capture_are_deduplicated() {
        let repo = setup_repo();
        let record = serde_json::json!({
            "timestamp": "2026-05-07T09:00:00Z",
            "type": "response_item",
            "payload": {
                "type": "message",
                "content": "Implemented change",
            },
        });
        let transcript = write_transcript(repo.path(), &jsonl([record.clone(), record]));

        let report = capture_project(repo.path(), Agent::Codex, &transcript);

        assert_eq!(report, 1);
        let session_key = only_session_key(repo.path());
        assert_eq!(transcript_entries(repo.path(), &session_key).len(), 1);
    }

    #[test]
    fn growing_fixture_captures_only_new_records() {
        let repo = setup_repo();
        let mut fixture = codex_fixture();
        let transcript = write_transcript(repo.path(), &fixture);
        capture_project(repo.path(), Agent::Codex, &transcript);

        fixture.push_str("{\"timestamp\":\"2026-05-07T09:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"content\":\"Follow-up\"}}\n");
        fs::write(&transcript, fixture).expect("grow transcript");
        let report = capture_project(repo.path(), Agent::Codex, &transcript);

        assert_eq!(report, 1);
        let session_key = only_session_key(repo.path());
        assert_eq!(transcript_entries(repo.path(), &session_key).len(), 2);
    }

    #[test]
    fn association_is_idempotent_and_can_link_multiple_targets() {
        let repo = setup_repo();
        let transcript = write_transcript(repo.path(), &codex_fixture());
        capture_project(repo.path(), Agent::Codex, &transcript);
        let session_key = only_session_key(repo.path());
        let main_target = Target::branch("main");
        let change_target = Target::change_id("change-123");

        associate_session(repo.path(), &main_target, &session_key).expect("associate main");
        associate_session(repo.path(), &main_target, &session_key).expect("associate main again");
        associate_session(repo.path(), &change_target, &session_key).expect("associate change");

        assert_eq!(
            session_index(repo.path(), &main_target),
            vec![session_key.clone()]
        );
        assert!(
            target_value(
                repo.path(),
                &main_target,
                &format!("gitbutler:agent-session:{session_key}:transcript"),
            )
            .is_none()
        );
        assert_eq!(
            session_index(repo.path(), &change_target),
            vec![session_key]
        );
    }
}
