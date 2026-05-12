use std::{
    fmt,
    fmt::{Debug, Display, Formatter},
    str::FromStr,
};

use anyhow::{Result, anyhow};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

const OPERATION_TRAILER_KEY: &str = "Operation";
const VERSION_TRAILER_KEY: &str = "Version";

/// A snapshot of the repository and virtual branches state that GitButler can restore to.
/// It captures the state of the working directory, virtual branches and commits.
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The id of the commit that represents the snapshot
    #[serde(rename = "id", with = "but_serde::object_id")]
    pub commit_id: gix::ObjectId,
    /// Snapshot creation time in milliseconds from Unix epoch, based on a commit as `commit_id`.
    #[serde(serialize_with = "but_serde::as_time_milliseconds_from_unix_epoch")]
    pub created_at: gix::date::Time,
    /// Snapshot details as persisted in the commit message, or `None` if the details couldn't be parsed.
    pub details: Option<SnapshotDetails>,
}

/// The payload of a snapshot commit
///
/// This is persisted as a commit message in the title, body and trailers format (<https://git-scm.com/docs/git-interpret-trailers>)
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDetails {
    /// The version of the snapshot format
    pub version: Version,
    /// The type of operation that was performed just before the snapshot was created
    pub operation: OperationKind,
    /// The title / label of the snapshot
    pub title: String,
    /// Additional text describing the snapshot
    pub body: Option<String>,
    /// Additional key value pairs that describe the snapshot
    pub trailers: Vec<Trailer>,
}

impl SnapshotDetails {
    pub fn new(operation: OperationKind) -> Self {
        SnapshotDetails {
            version: Default::default(),
            operation,
            title: operation.as_persisted_str().to_string(),
            body: None,
            trailers: vec![],
        }
    }

    pub fn with_count(mut self, count: usize) -> Self {
        if count > 1 {
            self.title = format!("{} ({})", self.title, count);
        }
        self
    }

    pub fn with_trailers(mut self, trailers: Vec<Trailer>) -> Self {
        self.trailers = trailers;
        self
    }
}

impl FromStr for SnapshotDetails {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let message_lines: Vec<&str> = s.lines().collect();
        let mut split: Vec<&[&str]> = message_lines.split(|line| line.is_empty()).collect();
        let title = split.remove(0).join("\n");
        let mut trailers: Vec<Trailer> = split
            .pop()
            .ok_or(anyhow!("No trailers found on snapshot commit message"))?
            .iter()
            .filter_map(|s| Trailer::from_str(s).ok())
            .collect();
        let body = split.iter().map(|v| v.join("\n")).join("\n\n");
        let body = if body.is_empty() { None } else { Some(body) };

        let version = trailers
            .iter()
            .find(|t| t.key == VERSION_TRAILER_KEY)
            .ok_or(anyhow!("No version found on snapshot commit message"))?
            .value
            .parse()?;

        let operation_trailer = &trailers
            .iter()
            .find(|t| t.key == OPERATION_TRAILER_KEY)
            .ok_or(anyhow!("No operation found on snapshot commit message"))?;
        let operation = OperationKind::parse_persisted_str(&operation_trailer.value)
            .unwrap_or(OperationKind::Unknown);

        // remove the version and operation attributes from the trailers since they have dedicated fields
        trailers.retain(|t| t.key != VERSION_TRAILER_KEY && t.key != OPERATION_TRAILER_KEY);

        Ok(SnapshotDetails {
            version,
            operation,
            title,
            body,
            trailers,
        })
    }
}

impl Display for SnapshotDetails {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "{}\n", self.title)?;
        if let Some(body) = &self.body {
            writeln!(f, "{body}\n")?;
        }
        writeln!(f, "{VERSION_TRAILER_KEY}: {}", self.version)?;
        writeln!(
            f,
            "{OPERATION_TRAILER_KEY}: {}",
            self.operation.as_persisted_str()
        )?;
        for line in &self.trailers {
            writeln!(f, "{line}")?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize, strum::EnumIter)]
pub enum OperationKind {
    CreateCommit,
    CreateBranch,
    StashIntoBranch,
    SetBaseBranch,
    MergeUpstream,
    UpdateWorkspaceBase,
    MoveHunk,
    UpdateBranchName,
    UpdateBranchNotes,
    ReorderBranches,
    UpdateBranchRemoteName,
    GenericBranchUpdate,
    DeleteBranch,
    ApplyBranch,
    DiscardLines,
    DiscardHunk,
    DiscardFile,
    DiscardChanges,
    Discard,
    AmendCommit,
    Absorb,
    AutoCommit,
    UndoCommit,
    DiscardCommit,
    UnapplyBranch,
    CherryPick,
    SquashCommit,
    UpdateCommitMessage,
    MoveCommit,
    MoveBranch,
    TearOffBranch,
    /// Restore via `but undo`
    RestoreFromSnapshotViaUndo,
    /// Restore via `but redo`
    RestoreFromSnapshotViaRedo,
    /// Restore via `but oplog restore`
    ///
    /// Or old oplog entries that existed before `RestoreFromSnapshotViaUndo` and
    /// `RestoreFromSnapshotViaRedo` were introduced.
    RestoreFromSnapshot,
    ReorderCommit,
    InsertBlankCommit,
    MoveCommitFile,
    FileChanges,
    EnterEditMode,
    SyncWorkspace,
    CreateDependentBranch,
    RemoveDependentBranch,
    UpdateDependentBranchName,
    UpdateDependentBranchDescription,
    UpdateDependentBranchPrNumber,
    AutoHandleChangesBefore,
    AutoHandleChangesAfter,
    SplitBranch,
    CleanWorkspace,
    OnDemandSnapshot,
    Unknown,
}

impl OperationKind {
    pub fn kind_str(self) -> &'static str {
        match self {
            OperationKind::CreateCommit => "COMMIT",
            OperationKind::CreateBranch => "BRANCH",
            OperationKind::AmendCommit => "AMEND",
            OperationKind::Absorb => "ABSORB",
            OperationKind::AutoCommit => "AUTO_COMMIT",
            OperationKind::UndoCommit => "UNDO_COMMIT",
            OperationKind::DiscardCommit => "DISCARD_COMMIT",
            OperationKind::SquashCommit => "SQUASH",
            OperationKind::UpdateCommitMessage => "REWORD",
            OperationKind::MoveCommit => "MOVE",
            OperationKind::RestoreFromSnapshotViaUndo => "UNDO",
            OperationKind::RestoreFromSnapshotViaRedo => "REDO",
            OperationKind::RestoreFromSnapshot => "RESTORE",
            OperationKind::ReorderCommit => "REORDER",
            OperationKind::InsertBlankCommit => "INSERT_COMMIT",
            OperationKind::MoveHunk => "MOVE_HUNK",
            OperationKind::ReorderBranches => "REORDER_BRANCH",
            OperationKind::UpdateWorkspaceBase => "UPDATE_BASE",
            OperationKind::UpdateBranchName => "RENAME",
            OperationKind::GenericBranchUpdate => "BRANCH_UPDATE",
            OperationKind::ApplyBranch => "APPLY",
            OperationKind::UnapplyBranch => "UNAPPLY",
            OperationKind::DeleteBranch => "DELETE",
            OperationKind::DiscardChanges => "DISCARD",
            OperationKind::Discard => "DISCARD",
            OperationKind::CleanWorkspace => "CLEAN",
            OperationKind::OnDemandSnapshot => "SNAPSHOT",
            OperationKind::DiscardLines => "DISCARD_LINES",
            OperationKind::DiscardHunk => "DISCARD_HUNK",
            OperationKind::DiscardFile => "DISCARD_FILE",
            OperationKind::CherryPick => "CHERRY_PICK",
            OperationKind::MoveBranch => "MOVE_BRANCH",
            OperationKind::TearOffBranch => "UNSTACK_BRANCH",
            OperationKind::MoveCommitFile => "MOVE_FILE",
            OperationKind::CreateDependentBranch => "CREATE_BRANCH",
            OperationKind::RemoveDependentBranch => "REMOVE_BRANCH",
            OperationKind::UpdateDependentBranchName
            | OperationKind::UpdateDependentBranchDescription
            | OperationKind::UpdateDependentBranchPrNumber => "UPDATE_BRANCH",
            OperationKind::SplitBranch => "SPLIT_BRANCH",
            OperationKind::StashIntoBranch
            | OperationKind::SetBaseBranch
            | OperationKind::MergeUpstream
            | OperationKind::UpdateBranchNotes
            | OperationKind::UpdateBranchRemoteName
            | OperationKind::FileChanges
            | OperationKind::EnterEditMode
            | OperationKind::SyncWorkspace
            | OperationKind::AutoHandleChangesBefore
            | OperationKind::AutoHandleChangesAfter => "OTHER",
            OperationKind::Unknown => "UNKNOWN",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            OperationKind::CreateCommit => "Created commit",
            OperationKind::CreateBranch => "Created branch",
            OperationKind::StashIntoBranch => "Stashed into branch",
            OperationKind::SetBaseBranch => "Set base branch",
            OperationKind::MergeUpstream => "Merged upstream",
            OperationKind::UpdateWorkspaceBase => "Updated workspace base",
            OperationKind::MoveHunk => "Moved hunk",
            OperationKind::UpdateBranchName => "Renamed branch",
            OperationKind::UpdateBranchNotes => "Updated branch notes",
            OperationKind::ReorderBranches => "Reordered branches",
            OperationKind::UpdateBranchRemoteName => "Updated branch remote",
            OperationKind::GenericBranchUpdate => "Updated branch",
            OperationKind::DeleteBranch => "Deleted branch",
            OperationKind::ApplyBranch => "Applied branch",
            OperationKind::DiscardLines => "Discarded lines",
            OperationKind::DiscardHunk => "Discarded hunk",
            OperationKind::DiscardFile => "Discarded file",
            OperationKind::DiscardChanges | OperationKind::Discard => "Discarded changes",
            OperationKind::AmendCommit => "Amended commit",
            OperationKind::Absorb => "Absorbed changes",
            OperationKind::AutoCommit => "Auto-committed changes",
            OperationKind::UndoCommit => "Undid commit",
            OperationKind::DiscardCommit => "Discarded commit",
            OperationKind::UnapplyBranch => "Unapplied branch",
            OperationKind::CherryPick => "Cherry-picked commit",
            OperationKind::SquashCommit => "Squashed commit",
            OperationKind::UpdateCommitMessage => "Updated commit message",
            OperationKind::MoveCommit => "Moved commit",
            OperationKind::MoveBranch => "Moved branch",
            OperationKind::TearOffBranch => "Unstacked branch",
            OperationKind::RestoreFromSnapshotViaUndo
            | OperationKind::RestoreFromSnapshotViaRedo
            | OperationKind::RestoreFromSnapshot => "Restored from snapshot",
            OperationKind::ReorderCommit => "Reordered commit",
            OperationKind::InsertBlankCommit => "Inserted blank commit",
            OperationKind::MoveCommitFile => "Moved file",
            OperationKind::FileChanges => "Updated file changes",
            OperationKind::EnterEditMode => "Entered edit mode",
            OperationKind::SyncWorkspace => "Synced workspace",
            OperationKind::CreateDependentBranch => "Created branch",
            OperationKind::RemoveDependentBranch => "Removed branch",
            OperationKind::UpdateDependentBranchName => "Updated branch name",
            OperationKind::UpdateDependentBranchDescription => "Updated branch description",
            OperationKind::UpdateDependentBranchPrNumber => "Updated branch pull request number",
            OperationKind::AutoHandleChangesBefore => "Handled changes before action",
            OperationKind::AutoHandleChangesAfter => "Handled changes after action",
            OperationKind::SplitBranch => "Split branch",
            OperationKind::CleanWorkspace => "Cleaned workspace",
            OperationKind::OnDemandSnapshot => "Created snapshot",
            OperationKind::Unknown => "Unknown operation",
        }
    }

    pub fn as_persisted_str(self) -> &'static str {
        match self {
            OperationKind::CreateCommit => "CreateCommit",
            OperationKind::CreateBranch => "CreateBranch",
            OperationKind::StashIntoBranch => "StashIntoBranch",
            OperationKind::SetBaseBranch => "SetBaseBranch",
            OperationKind::MergeUpstream => "MergeUpstream",
            OperationKind::UpdateWorkspaceBase => "UpdateWorkspaceBase",
            OperationKind::MoveHunk => "MoveHunk",
            OperationKind::UpdateBranchName => "UpdateBranchName",
            OperationKind::UpdateBranchNotes => "UpdateBranchNotes",
            OperationKind::ReorderBranches => "ReorderBranches",
            OperationKind::UpdateBranchRemoteName => "UpdateBranchRemoteName",
            OperationKind::GenericBranchUpdate => "GenericBranchUpdate",
            OperationKind::DeleteBranch => "DeleteBranch",
            OperationKind::ApplyBranch => "ApplyBranch",
            OperationKind::DiscardLines => "DiscardLines",
            OperationKind::DiscardHunk => "DiscardHunk",
            OperationKind::DiscardFile => "DiscardFile",
            OperationKind::DiscardChanges => "DiscardChanges",
            OperationKind::Discard => "Discard",
            OperationKind::AmendCommit => "AmendCommit",
            OperationKind::Absorb => "Absorb",
            OperationKind::AutoCommit => "AutoCommit",
            OperationKind::UndoCommit => "UndoCommit",
            OperationKind::DiscardCommit => "DiscardCommit",
            OperationKind::UnapplyBranch => "UnapplyBranch",
            OperationKind::CherryPick => "CherryPick",
            OperationKind::SquashCommit => "SquashCommit",
            OperationKind::UpdateCommitMessage => "UpdateCommitMessage",
            OperationKind::MoveCommit => "MoveCommit",
            OperationKind::MoveBranch => "MoveBranch",
            OperationKind::TearOffBranch => "TearOffBranch",
            OperationKind::RestoreFromSnapshotViaUndo => "RestoreFromSnapshotViaUndo",
            OperationKind::RestoreFromSnapshotViaRedo => "RestoreFromSnapshotViaRedo",
            OperationKind::RestoreFromSnapshot => "RestoreFromSnapshot",
            OperationKind::ReorderCommit => "ReorderCommit",
            OperationKind::InsertBlankCommit => "InsertBlankCommit",
            OperationKind::MoveCommitFile => "MoveCommitFile",
            OperationKind::FileChanges => "FileChanges",
            OperationKind::EnterEditMode => "EnterEditMode",
            OperationKind::SyncWorkspace => "SyncWorkspace",
            OperationKind::CreateDependentBranch => "CreateDependentBranch",
            OperationKind::RemoveDependentBranch => "RemoveDependentBranch",
            OperationKind::UpdateDependentBranchName => "UpdateDependentBranchName",
            OperationKind::UpdateDependentBranchDescription => "UpdateDependentBranchDescription",
            OperationKind::UpdateDependentBranchPrNumber => "UpdateDependentBranchPrNumber",
            OperationKind::AutoHandleChangesBefore => "AutoHandleChangesBefore",
            OperationKind::AutoHandleChangesAfter => "AutoHandleChangesAfter",
            OperationKind::SplitBranch => "SplitBranch",
            OperationKind::CleanWorkspace => "CleanWorkspace",
            OperationKind::OnDemandSnapshot => "OnDemandSnapshot",
            OperationKind::Unknown => "Unknown",
        }
    }

    pub fn parse_persisted_str(s: &str) -> Option<Self> {
        Some(match s {
            "CreateCommit" => Self::CreateCommit,
            "CreateBranch" => Self::CreateBranch,
            "StashIntoBranch" => Self::StashIntoBranch,
            "SetBaseBranch" => Self::SetBaseBranch,
            "MergeUpstream" => Self::MergeUpstream,
            "UpdateWorkspaceBase" => Self::UpdateWorkspaceBase,
            "MoveHunk" => Self::MoveHunk,
            "UpdateBranchName" => Self::UpdateBranchName,
            "UpdateBranchNotes" => Self::UpdateBranchNotes,
            "ReorderBranches" => Self::ReorderBranches,
            "UpdateBranchRemoteName" => Self::UpdateBranchRemoteName,
            "GenericBranchUpdate" => Self::GenericBranchUpdate,
            "DeleteBranch" => Self::DeleteBranch,
            "ApplyBranch" => Self::ApplyBranch,
            "DiscardLines" => Self::DiscardLines,
            "DiscardHunk" => Self::DiscardHunk,
            "DiscardFile" => Self::DiscardFile,
            "DiscardChanges" => Self::DiscardChanges,
            "Discard" => Self::Discard,
            "AmendCommit" => Self::AmendCommit,
            "Absorb" => Self::Absorb,
            "AutoCommit" => Self::AutoCommit,
            "UndoCommit" => Self::UndoCommit,
            "DiscardCommit" => Self::DiscardCommit,
            "UnapplyBranch" => Self::UnapplyBranch,
            "CherryPick" => Self::CherryPick,
            "SquashCommit" => Self::SquashCommit,
            "UpdateCommitMessage" => Self::UpdateCommitMessage,
            "MoveCommit" => Self::MoveCommit,
            "MoveBranch" => Self::MoveBranch,
            "TearOffBranch" => Self::TearOffBranch,
            "RestoreFromSnapshotViaUndo" => Self::RestoreFromSnapshotViaUndo,
            "RestoreFromSnapshotViaRedo" => Self::RestoreFromSnapshotViaRedo,
            "RestoreFromSnapshot" => Self::RestoreFromSnapshot,
            "ReorderCommit" => Self::ReorderCommit,
            "InsertBlankCommit" => Self::InsertBlankCommit,
            "MoveCommitFile" => Self::MoveCommitFile,
            "FileChanges" => Self::FileChanges,
            "EnterEditMode" => Self::EnterEditMode,
            "SyncWorkspace" => Self::SyncWorkspace,
            "CreateDependentBranch" => Self::CreateDependentBranch,
            "RemoveDependentBranch" => Self::RemoveDependentBranch,
            "UpdateDependentBranchName" => Self::UpdateDependentBranchName,
            "UpdateDependentBranchDescription" => Self::UpdateDependentBranchDescription,
            "UpdateDependentBranchPrNumber" => Self::UpdateDependentBranchPrNumber,
            "AutoHandleChangesBefore" => Self::AutoHandleChangesBefore,
            "AutoHandleChangesAfter" => Self::AutoHandleChangesAfter,
            "SplitBranch" => Self::SplitBranch,
            "CleanWorkspace" => Self::CleanWorkspace,
            "OnDemandSnapshot" => Self::OnDemandSnapshot,
            "Unknown" => Self::Unknown,
            _ => return None,
        })
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize)]
pub struct Version(pub u32);

impl Default for Version {
    fn default() -> Self {
        Version(3)
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl FromStr for Version {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Version(u32::from_str(s)?))
    }
}

/// Represents a key value pair stored in a snapshot, like `key: value\n`
/// Using the git trailer format (<https://git-scm.com/docs/git-interpret-trailers>)
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trailer {
    /// Trailer key
    pub key: String,
    /// Trailer value
    pub value: String,
}

impl Display for Trailer {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let escaped_value = self.value.replace('\n', "\\n");
        write!(f, "{}: {}", self.key, escaped_value)
    }
}

impl FromStr for Trailer {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid trailer format, expected `key: value`"));
        }
        let unescaped_value = parts[1].trim().replace("\\n", "\n");
        Ok(Self {
            key: parts[0].trim().to_string(),
            value: unescaped_value,
        })
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::OperationKind;

    #[test]
    fn parsing_operation_kinds() {
        for kind in OperationKind::iter() {
            let s = kind.as_persisted_str();
            assert_eq!(kind, OperationKind::parse_persisted_str(s).unwrap());
        }
    }
}
