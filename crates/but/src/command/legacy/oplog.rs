use but_api::legacy::oplog::RestoreKind;
use but_core::RepositoryExt;
use gitbutler_oplog::entry::{OperationKind, Snapshot};
use gix::{date::time::CustomFormat, prelude::ObjectIdExt};

use crate::{
    theme::{self, Paint},
    utils::{Confirm, ConfirmDefault, OutputChannel, shorten_object_id},
};

pub const ISO8601_NO_TZ: CustomFormat = CustomFormat::new("%Y-%m-%d %H:%M:%S");

/// Filter for oplog entries by operation kind
#[derive(Debug, Clone, Copy)]
pub enum OplogFilter {
    /// Show only on-demand snapshot entries
    Snapshot,
}

impl OplogFilter {
    /// Convert the filter to a list of OperationKind to include
    fn to_include_kinds(self) -> Vec<OperationKind> {
        match self {
            OplogFilter::Snapshot => vec![OperationKind::OnDemandSnapshot],
        }
    }
}

pub(crate) fn show_oplog(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    since: Option<&str>,
    filter: Option<OplogFilter>,
) -> anyhow::Result<()> {
    // Convert filter to include_kind parameter for the API
    let include_kind = filter.map(|f| f.to_include_kinds());

    // Resolve partial SHA to full SHA using rev_parse if provided
    let since_sha = if let Some(sha_prefix) = since {
        let repo = ctx.repo.get()?;
        let resolved = repo
            .rev_parse_single(sha_prefix)
            .map_err(|_| anyhow::anyhow!("No oplog entry found matching SHA: {sha_prefix}"))?;
        Some(resolved.detach())
    } else {
        None
    };

    let snapshots = but_api::legacy::oplog::snapshots_iter(ctx, since_sha, None, include_kind)?
        .take(20)
        .collect::<anyhow::Result<Vec<_>>>()?;

    if snapshots.is_empty() {
        if let Some(out) = out.for_json() {
            out.write_value(&snapshots)?;
        } else if let Some(out) = out.for_human() {
            writeln!(out, "No operations found in history.")?;
        }
        return Ok(());
    }

    if let Some(out) = out.for_json() {
        out.write_value(&snapshots)?;
    } else if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?.clone().for_commit_shortening();
        let t = theme::get();
        writeln!(out, "{}", t.important.paint("Operations History"))?;
        writeln!(out, "{}", t.hint.paint("─".repeat(50)))?;
        // Find the longest short ID length to keep all lines aligned.
        let longest_short_id_len = snapshots
            .iter()
            .filter_map(|s| {
                let prefix = s.commit_id.attach(&repo).shorten().ok()?;
                Some(prefix.hex_len())
            })
            .max()
            .unwrap_or(7);

        for snapshot in snapshots {
            let time_string = snapshot_time_string(&snapshot);
            let short = snapshot.commit_id;
            let short = short.to_hex_with_len(longest_short_id_len);
            let commit_id = t.cli_id.paint(short.to_string());

            let (operation_type, title) = if let Some(details) = &snapshot.details {
                let op_type = details.operation.as_str();
                // For OnDemandSnapshot, show the message (body) if available
                // For Discard, show file names from trailers if available
                let display_title = if details.operation == OperationKind::OnDemandSnapshot {
                    details
                        .body
                        .as_ref()
                        .filter(|b| !b.is_empty())
                        .cloned()
                        .unwrap_or_else(|| details.title.clone())
                } else if details.operation == OperationKind::Discard {
                    // Extract file names from trailers
                    let file_names: Vec<String> = details
                        .trailers
                        .iter()
                        .filter(|t| t.key == "file")
                        .map(|t| t.value.clone())
                        .collect();

                    if !file_names.is_empty() {
                        format!("{} ({})", details.title, file_names.join(", "))
                    } else {
                        details.title.clone()
                    }
                } else {
                    details.title.clone()
                };

                let display_title = out.truncate_if_unpaged(&display_title, 80);
                (op_type, display_title)
            } else {
                ("UNKNOWN", "Unknown operation".to_string())
            };

            // TODO(david): don't use strings here, use the `OperationKind`
            let operation_colored = match operation_type {
                "COMMIT" => t.success.paint(operation_type),
                "AMEND" | "REWORD" => t.attention.paint(operation_type),
                "UNDO_COMMIT" | "RESTORE" | "UNDO" => t.error.paint(operation_type),
                "DISCARD" => t.error.paint(operation_type),
                "BRANCH" | "CHECKOUT" => t.local_branch.paint(operation_type),
                "MOVE" | "REORDER" | "MOVE_HUNK" => t.info.paint(operation_type),
                "SNAPSHOT" => t.hint.paint(operation_type),
                _ => t.default.paint(operation_type),
            };

            writeln!(
                out,
                "{} {} [{}] {}",
                commit_id,
                t.time.paint(&time_string),
                operation_colored,
                title
            )?;
        }
    }

    Ok(())
}

fn snapshot_time_string(snapshot: &Snapshot) -> String {
    let time = snapshot.created_at;
    // TODO: use `format_or_unix`.
    time.format(ISO8601_NO_TZ)
        .unwrap_or_else(|_| time.seconds.to_string())
}

pub(crate) fn restore_to_oplog(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    oplog_sha: &str,
    force: bool,
) -> anyhow::Result<()> {
    let commit_id = ctx.repo.get()?.rev_parse_single(oplog_sha)?.detach();
    let target_snapshot = &but_api::legacy::oplog::get_snapshot(ctx, commit_id)?;
    let commit_short = {
        let repo = ctx.repo.get()?;
        shorten_object_id(&repo, commit_id)
    };

    let target_operation = target_snapshot
        .details
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Unknown operation");

    let target_time = snapshot_time_string(target_snapshot);

    if let Some(mut out) = out.prepare_for_terminal_input() {
        use std::fmt::Write;
        let t = theme::get();
        writeln!(
            out,
            "{}",
            t.progress.paint("Restoring to oplog snapshot...")
        )?;
        writeln!(
            out,
            "  Target: {} ({})",
            t.important.paint(target_operation),
            t.time.paint(&target_time)
        )?;
        writeln!(out, "  Snapshot: {}", t.commit_id.paint(&commit_short))?;

        // Confirm the restoration (safety check)
        if !force {
            writeln!(
                out,
                "\n{}",
                t.attention
                    .paint("⚠️  This will overwrite your current workspace state.")
            )?;
            if out.confirm("Continue with restore?", ConfirmDefault::No)? == Confirm::No {
                return Ok(());
            }
        }
    }

    // Restore to the target snapshot using the but-api crate
    but_api::legacy::oplog::restore_snapshot_with_kind(
        ctx,
        RestoreKind::ExplicitRestoreFromSnapshot,
        commit_id,
    )?;

    if let Some(out) = out.for_human() {
        let t = theme::get();
        writeln!(out, "\n{} Restore completed successfully!", t.sym().success,)?;

        writeln!(
            out,
            "{}",
            t.success
                .paint("\nWorkspace has been restored to the selected snapshot.")
        )?;
    }

    Ok(())
}

pub(crate) fn undo_last_operation(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let Some(target_snapshot) = but_api::legacy::oplog::get_undo_target_snapshot(ctx)? else {
        print_no_snapshot_to_restore_to(out)?;
        return Ok(());
    };

    let target_operation = target_snapshot
        .details
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Unknown operation");

    let target_time = snapshot_time_string(&target_snapshot);

    if let Some(out) = out.for_human() {
        let t = theme::get();
        writeln!(out, "{}", t.progress.paint("Undoing operation..."))?;
        writeln!(
            out,
            "  Reverting to: {} ({})",
            t.important.paint(target_operation),
            t.time.paint(&target_time)
        )?;
    }

    // Restore to the previous snapshot using the but_api
    // TODO: Why does this not require force? It will also overwrite user changes (I think).
    but_api::legacy::oplog::restore_snapshot_with_kind(
        ctx,
        RestoreKind::RestoreFromSnapshotViaUndo,
        target_snapshot.commit_id,
    )?;

    if let Some(out) = out.for_human() {
        let t = theme::get();
        let repo = ctx.repo.get()?;
        let short = shorten_object_id(&repo, target_snapshot.commit_id);

        writeln!(
            out,
            "{} Undo completed successfully! Restored to snapshot: {}",
            t.sym().success,
            t.cli_id.paint(&short)
        )?;
    }

    Ok(())
}

fn print_no_snapshot_to_restore_to(out: &mut OutputChannel) -> anyhow::Result<()> {
    if let Some(out) = out.for_human() {
        let t = theme::get();
        writeln!(
            out,
            "{}",
            t.attention.paint("No previous operations to undo.")
        )?;
    }
    Ok(())
}

pub(crate) fn create_snapshot(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    message: Option<&str>,
) -> anyhow::Result<()> {
    let snapshot_id = but_api::legacy::oplog::create_snapshot(ctx, message.map(String::from))?;

    if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "snapshot_id": snapshot_id.to_string(),
            "message": message.unwrap_or(""),
            "operation": "create_snapshot"
        }))?;
    } else if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        let short = shorten_object_id(&repo, snapshot_id);
        let t = theme::get();
        writeln!(out, "{}", t.success.paint("Snapshot created successfully!"))?;

        if let Some(msg) = message {
            writeln!(out, "  Message: {}", t.info.paint(msg))?;
        }

        writeln!(out, "  Snapshot ID: {}", t.cli_id.paint(&short))?;
        writeln!(
            out,
            "\n{} Use 'but oplog restore {}' to restore to this snapshot later.",
            t.info.paint("💡"),
            short
        )?;
    }

    Ok(())
}
