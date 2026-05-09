use std::{
    fmt,
    io::Read as _,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
};

use anyhow::Context as _;
use git_meta_lib::Target;
use serde::{Deserialize, Serialize};

use crate::{
    agent::Agent,
    capture::record_transcript,
    gitmeta::{associate_session, sync_metadata},
};

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    Capture {
        #[clap(long, value_enum)]
        agent: Option<Agent>,
        #[clap(long, value_name = "PATH", value_parser = non_empty_path)]
        transcript_path: PathBuf,
        #[clap(long, value_name = "TARGET", value_parser = Target::parse)]
        associate_target: Option<Target>,
    },
    Hook {
        #[clap(long, value_enum)]
        agent: Option<Agent>,
        #[clap(long, value_name = "TARGET", value_parser = Target::parse)]
        associate_target: Option<Target>,
    },
    Sync,
}

#[derive(Debug, Serialize)]
struct CommandOutput {
    message: String,
}

impl fmt::Display for CommandOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

pub fn run_from_dir(dir: &Path, command: Command) -> anyhow::Result<impl Serialize + fmt::Display> {
    match command {
        Command::Capture {
            agent,
            transcript_path,
            associate_target,
        } => {
            let (records_written, associated_target) = record_agent_log(
                dir,
                agent.context("agent is required")?,
                &transcript_path,
                associate_target.as_ref(),
            )
            .context("failed to capture agent log")?;
            let mut message = format!("Captured {records_written} records");
            if let Some(target_label) = associated_target {
                message.push_str(&format!(" and associated session with {target_label}"));
            }
            Ok(CommandOutput { message })
        }
        Command::Hook {
            agent,
            associate_target,
        } => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .context("failed to read agent hook input")?;
            if let Some(sync_dir) = run_hook(dir, agent, associate_target.as_ref(), &input)? {
                spawn_agentlog_sync(&sync_dir);
            }
            Ok(CommandOutput {
                message: String::new(),
            })
        }
        Command::Sync => {
            sync_metadata(&resolve_workdir(dir)?).context("failed to sync GitMeta metadata")?;
            Ok(CommandOutput {
                message: "Synced GitMeta metadata".into(),
            })
        }
    }
}

fn run_hook(
    dir: &Path,
    agent: Option<Agent>,
    associate_target: Option<&Target>,
    input: &str,
) -> anyhow::Result<Option<PathBuf>> {
    let input: HookInput =
        serde_json::from_str(input).context("failed to parse agent hook input")?;
    let Some(transcript_path) = input
        .transcript_path
        .filter(|path| !path.as_os_str().is_empty())
    else {
        return Ok(None);
    };
    let dir = input
        .cwd
        .as_deref()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(dir);
    let agent = agent.context("agent is required")?;

    let (records_written, associated_target) =
        record_agent_log(dir, agent, &transcript_path, associate_target)
            .context("failed to capture agent log from hook input")?;

    if records_written == 0 && associated_target.is_none() {
        return Ok(None);
    }
    Ok(Some(dir.to_path_buf()))
}

fn spawn_agentlog_sync(dir: &Path) {
    #[cfg(target_os = "linux")]
    let but_path = Path::new("/proc/self/exe");
    #[cfg(not(target_os = "linux"))]
    let but_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => return,
    };

    let _ = ProcessCommand::new(but_path)
        .arg("-C")
        .arg(dir)
        .args(["agentlog", "sync"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn record_agent_log(
    dir: &Path,
    agent: Agent,
    transcript_path: &Path,
    associate_target: Option<&Target>,
) -> anyhow::Result<(usize, Option<String>)> {
    let workdir = resolve_workdir(dir)?;
    let transcript_path = if transcript_path.is_absolute() {
        transcript_path.to_path_buf()
    } else {
        dir.join(transcript_path)
    };
    let (records_written, session_key) = record_transcript(&workdir, agent, &transcript_path)?;

    let associated_target =
        if let (Some(target), Some(session_key)) = (associate_target, session_key.as_deref()) {
            let target_label = target.to_string();
            associate_session(&workdir, target, session_key)
                .context("failed to associate agent log")?;
            Some(target_label)
        } else {
            None
        };

    Ok((records_written, associated_target))
}

fn resolve_workdir(dir: &Path) -> anyhow::Result<PathBuf> {
    let repo =
        gix::discover(dir).context("No git repository found. Use -C to choose a repository.")?;
    let workdir = repo
        .workdir()
        .context("Bare repositories are not supported.")?;
    std::fs::canonicalize(workdir).context("failed to resolve repository worktree")
}

fn non_empty_path(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() {
        Err("transcript path is required".into())
    } else {
        Ok(value.into())
    }
}

#[derive(Deserialize)]
struct HookInput {
    transcript_path: Option<PathBuf>,
    cwd: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        path::Path,
        process::{Command as ProcessCommand, Stdio},
    };

    use git_meta_lib::{MetaValue, Session, Target};
    use tempfile::TempDir;

    use super::{Command, run_from_dir, run_hook};
    use crate::Agent;

    #[derive(Debug, clap::Parser)]
    struct Args {
        #[clap(subcommand)]
        command: Command,
    }

    #[test]
    fn rejects_unsupported_agent_while_parsing_args() {
        use clap::Parser as _;

        Args::try_parse_from([
            "but-agentlog",
            "capture",
            "--agent",
            "cursor",
            "--transcript-path",
            "missing.jsonl",
        ])
        .expect_err("unsupported agent should fail");
    }

    #[test]
    fn rejects_empty_transcript_path_while_parsing_args() {
        use clap::Parser as _;

        Args::try_parse_from([
            "but-agentlog",
            "capture",
            "--agent",
            "codex",
            "--transcript-path",
            "",
        ])
        .expect_err("empty transcript path should fail");
    }

    #[test]
    fn parses_association_target_flag_without_agent() {
        use clap::Parser as _;

        let args = Args::try_parse_from([
            "but-agentlog",
            "capture",
            "--transcript-path",
            "session.jsonl",
            "--associate-target",
            "branch:main",
        ])
        .expect("parse args");

        let Command::Capture {
            agent,
            associate_target,
            ..
        } = args.command
        else {
            panic!("expected capture command");
        };
        assert_eq!(agent, None);
        assert_eq!(
            associate_target.expect("association target").to_string(),
            "branch:main"
        );
    }

    #[test]
    fn parses_sync_command() {
        use clap::Parser as _;

        let args = Args::try_parse_from(["but-agentlog", "sync"]).expect("parse args");

        let Command::Sync = args.command else {
            panic!("expected sync command");
        };
    }

    #[test]
    fn capture_can_associate_existing_session_without_new_records() {
        let repo = setup_repo();
        write_transcript_with_message(repo.path());
        run_from_dir(
            repo.path(),
            Command::Capture {
                agent: Some(Agent::Codex),
                transcript_path: "session.jsonl".into(),
                associate_target: None,
            },
        )
        .expect("initial capture");

        let output = run_from_dir(
            repo.path(),
            Command::Capture {
                agent: Some(Agent::Codex),
                transcript_path: "session.jsonl".into(),
                associate_target: Some(Target::branch("main")),
            },
        )
        .expect("associate existing session")
        .to_string();

        assert_eq!(
            output,
            "Captured 0 records and associated session with branch:main"
        );
        assert_eq!(
            session_keys(repo.path(), &Target::branch("main")),
            session_keys(repo.path(), &Target::project())
        );
    }

    #[test]
    fn hook_reads_transcript_path_from_payload() {
        let repo = setup_repo();
        write_transcript_with_message(repo.path());
        let payload = serde_json::json!({
            "transcript_path": repo.path().join("session.jsonl").display().to_string(),
            "cwd": repo.path().display().to_string(),
        })
        .to_string();

        run_hook(repo.path(), Some(Agent::Codex), None, &payload).expect("run hook");

        assert_eq!(session_keys(repo.path(), &Target::project()).len(), 1);
    }

    #[test]
    fn hook_without_transcript_path_noops() {
        let repo = setup_repo();
        let payload = serde_json::json!({
            "cwd": repo.path().display().to_string(),
        })
        .to_string();

        run_hook(repo.path(), None, None, &payload).expect("run hook");

        assert!(
            target_value(repo.path(), &Target::project(), "gitbutler:agent-sessions").is_none()
        );
    }

    #[test]
    fn sync_with_empty_metadata_remote_outputs_message() {
        let repo = setup_repo();
        let remote = TempDir::new().expect("temp remote");
        let status = ProcessCommand::new("git")
            .args(["init", "--bare"])
            .current_dir(remote.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git init bare");
        assert!(status.success());

        let status = ProcessCommand::new("git")
            .args(["remote", "add", "origin", &remote.path().to_string_lossy()])
            .current_dir(repo.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git remote add");
        assert!(status.success());
        let status = ProcessCommand::new("git")
            .args(["config", "remote.origin.meta", "true"])
            .current_dir(repo.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git config remote origin meta");
        assert!(status.success());

        let session = Session::open(repo.path()).expect("open session");
        session
            .target(&Target::project())
            .set("gitbutler:test", "value")
            .expect("write metadata");

        let output = run_from_dir(repo.path(), Command::Sync)
            .expect("sync")
            .to_string();

        assert_eq!(output, "Synced GitMeta metadata");
    }

    fn write_transcript_with_message(repo: &Path) {
        fs::write(
            repo.join("session.jsonl"),
            concat!(
                r#"{"timestamp":"2026-05-07T09:00:00Z","type":"session_meta","payload":{"id":"session-1"}}"#,
                "\n",
                r#"{"timestamp":"2026-05-07T09:00:01Z","type":"response_item","payload":{"type":"message","content":"hello"}}"#,
                "\n",
            ),
        )
        .expect("write transcript");
    }

    fn setup_repo() -> TempDir {
        let dir = TempDir::new().expect("temp repo");
        let status = ProcessCommand::new("git")
            .args(["init", "--initial-branch", "main"])
            .current_dir(dir.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git init");
        assert!(status.success());
        dir
    }

    fn session_keys(repo: &Path, target: &Target) -> BTreeSet<String> {
        let Some(MetaValue::Set(keys)) = target_value(repo, target, "gitbutler:agent-sessions")
        else {
            panic!("expected session index set");
        };
        keys
    }

    fn target_value(repo: &Path, target: &Target, key: &str) -> Option<MetaValue> {
        Session::open(repo)
            .expect("open session")
            .target(target)
            .get_value(key)
            .expect("read GitMeta value")
    }
}
