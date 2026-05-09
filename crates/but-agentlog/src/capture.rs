use std::path::Path;

use anyhow::{Context as _, Result};
use sha2::{Digest, Sha256};

use crate::{agent::Agent, gitmeta::write_transcript_batch, transcript::TranscriptBatch};

pub(crate) fn record_transcript(
    repo_path: &Path,
    agent: Agent,
    transcript_path: &Path,
) -> Result<(usize, Option<String>)> {
    let source_path =
        std::fs::canonicalize(transcript_path).context("transcript path is not readable")?;
    let snapshot = std::fs::read(&source_path).context("transcript path is not readable")?;
    let batch = TranscriptBatch::parse(agent, &snapshot)?;
    let has_capturable_records = !batch.records.is_empty();

    let session_id = batch
        .session_id
        .as_deref()
        .filter(|session_id| !session_id.is_empty());
    let source_identity = session_id
        .map(str::as_bytes)
        .unwrap_or_else(|| source_path.as_os_str().as_encoded_bytes());
    let session_key = agent_identity_key(agent, source_identity);
    let source_key = agent_identity_key(agent, source_path.as_os_str().as_encoded_bytes());

    let records_written =
        write_transcript_batch(repo_path, agent, &session_key, &source_key, batch)?;
    Ok((
        records_written,
        has_capturable_records.then_some(session_key),
    ))
}

fn agent_identity_key(agent: Agent, identity: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(agent.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(identity);
    let digest = hasher.finalize();
    format!("sha256-{}", hex::encode(&digest[..16]))
}
