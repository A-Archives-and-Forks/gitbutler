use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt,
    fs::File,
    path::{Path, PathBuf},
};

use but_testsupport::gix_testtools::{
    scripted_fixture_read_only, scripted_fixture_writable, tempfile::TempDir,
};
use but_testsupport::{CommandExt, git_at_dir, visualize_disk_tree_skip_dot_git};

#[test]
fn normal_repo_includes_git_state_and_unignored_worktree() -> anyhow::Result<()> {
    let repo = read_only_repo("dump-normal-repo-with-worktree-changes.sh", "你好 repo")?;
    let output_dir = TempDir::new()?;
    let initial = visualize_disk_tree_skip_dot_git(&repo)?;

    insta::assert_snapshot!(initial, @r"
.
├── .git:40755
├── .gitignore:100644
├── executable.sh:100755
├── ignored-dir:40755
│   └── file.txt:100644
├── ignored.ignored:100644
├── tracked.ignored:100644
└── visible.txt:100644
");

    let output = output_dir.path().join("out.zip");
    let dump_output = run_dump(&repo, &output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&output)?, @r"
你好-repo-dump/
├── .git/
│   ├── HEAD:100644
│   ├── config:100644
│   ├── gitbutler:40755/
│   │   └── vb.toml:100644
│   └── ... 25 files and 15 directories not shown
├── .gitignore:100644
├── executable.sh:100755
├── tracked.ignored:100644
└── visible.txt:100644
");

    Ok(())
}

#[test]
fn git_only_skips_worktree_files() -> anyhow::Result<()> {
    let repo = read_only_repo("dump-normal-repo-with-worktree-changes.sh", "你好 repo")?;
    let output_dir = TempDir::new()?;
    let initial = visualize_disk_tree_skip_dot_git(&repo)?;

    insta::assert_snapshot!(initial, @r"
.
├── .git:40755
├── .gitignore:100644
├── executable.sh:100755
├── ignored-dir:40755
│   └── file.txt:100644
├── ignored.ignored:100644
├── tracked.ignored:100644
└── visible.txt:100644
");

    let output = output_dir.path().join("out.zip");
    let dump_output = run_dump(&repo, &output, true)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&output)?, @r"
你好-repo-dump/
└── .git/
    ├── HEAD:100644
    ├── config:100644
    ├── gitbutler:40755/
    │   └── vb.toml:100644
    └── ... 25 files and 15 directories not shown
");

    Ok(())
}

#[test]
fn bare_repo_extracts_as_dump_git_directory() -> anyhow::Result<()> {
    let repo = read_only_repo("dump-bare-repo.sh", "sample.git")?;
    let output_dir = TempDir::new()?;
    let initial = visualize_disk_tree_skip_dot_git(&repo)?;

    insta::assert_snapshot!(initial, @r"
.
├── HEAD:100644
├── config:100644
├── description:100644
├── hooks:40755
│   ├── applypatch-msg.sample:100755
│   ├── commit-msg.sample:100755
│   ├── fsmonitor-watchman.sample:100755
│   ├── post-update.sample:100755
│   ├── pre-applypatch.sample:100755
│   ├── pre-commit.sample:100755
│   ├── pre-merge-commit.sample:100755
│   ├── pre-push.sample:100755
│   ├── pre-rebase.sample:100755
│   ├── pre-receive.sample:100755
│   ├── prepare-commit-msg.sample:100755
│   ├── push-to-checkout.sample:100755
│   ├── sendemail-validate.sample:100755
│   └── update.sample:100755
├── info:40755
│   └── exclude:100644
├── objects:40755
│   ├── info:40755
│   └── pack:40755
└── refs:40755
    ├── heads:40755
    └── tags:40755
");

    let output = output_dir.path().join("out.zip");
    let dump_output = run_dump(&repo, &output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&output)?, @r"
sample-dump.git/
├── HEAD:100644
├── config:100644
└── ... 16 files and 8 directories not shown
");

    Ok(())
}

#[test]
fn linked_worktree_is_unlinked_into_real_git_directory() -> anyhow::Result<()> {
    let linked = read_only_repo("dump-linked-worktree.sh", "linked")?;
    let output_dir = TempDir::new()?;
    let initial = visualize_disk_tree_skip_dot_git(&linked)?;

    insta::assert_snapshot!(initial, @r"
.
├── .git:100644
├── .gitignore:100644
├── linked-worktree-added-to-index.txt:100644
├── linked-worktree-untracked.txt:100644
└── tracked.ignored:100644
");

    let output = output_dir.path().join("out.zip");
    let dump_output = run_dump(&linked, &output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&output), @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&output)?, ".git is now a directory", @r"
linked-dump/
├── .git/
│   ├── HEAD:100644
│   ├── config:100644
│   └── ... 30 files and 17 directories not shown
├── .gitignore:100644
├── linked-worktree-added-to-index.txt:100644
├── linked-worktree-untracked.txt:100644
└── tracked.ignored:100644
");

    let extraction = TempDir::new()?;
    unzip(&output, extraction.path())?;
    let unpacked = extraction.path().join("linked-dump");
    git_at_dir(&unpacked).arg("status").run();
    insta::assert_snapshot!(git_status(&unpacked)?, "the unpacked repository keeps the linked worktree HEAD and index", @r"
## linked
A  linked-worktree-added-to-index.txt
 M tracked.ignored
?? linked-worktree-untracked.txt
");

    Ok(())
}

#[test]
fn current_output_inside_worktree_is_not_archived() -> anyhow::Result<()> {
    let fixture = writable_fixture("dump-normal-repo-with-worktree-changes.sh")?;
    let repo = fixture.path().join("你好 repo");
    let initial = visualize_disk_tree_skip_dot_git(&repo)?;

    insta::assert_snapshot!(initial, "before any dump output exists in the worktree", @r"
.
├── .git:40755
├── .gitignore:100644
├── executable.sh:100755
├── ignored-dir:40755
│   └── file.txt:100644
├── ignored.ignored:100644
├── tracked.ignored:100644
└── visible.txt:100644
");

    let first_output = repo.join("nested/output/sample-dump.zip");
    let dump_output = run_dump(&repo, &first_output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&first_output), "first dump output", @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&first_output)?, "the archive being written is not included in itself", @r"
你好-repo-dump/
├── .git/
│   ├── HEAD:100644
│   ├── config:100644
│   ├── gitbutler:40755/
│   │   └── vb.toml:100644
│   └── ... 25 files and 15 directories not shown
├── .gitignore:100644
├── executable.sh:100755
├── nested:40755/
│   └── output:40755/
├── tracked.ignored:100644
└── visible.txt:100644
");

    let second_output = repo.join("sample-second-dump.zip");
    let dump_output = run_dump(&repo, &second_output, false)?;
    insta::assert_snapshot!(dump_output.display_for_snapshot(&second_output), "second dump output", @"
    stdout:
    Archive at: [output path]
    stderr:
    ");

    insta::assert_snapshot!(archive_tree(&second_output)?, "a previous dump archive in the worktree is included like any other visible file", @r"
你好-repo-dump/
├── .git/
│   ├── HEAD:100644
│   ├── config:100644
│   ├── gitbutler:40755/
│   │   └── vb.toml:100644
│   └── ... 25 files and 15 directories not shown
├── .gitignore:100644
├── executable.sh:100755
├── nested:40755/
│   └── output:40755/
│       └── sample-dump.zip:100644
├── tracked.ignored:100644
└── visible.txt:100644
");

    Ok(())
}

fn read_only_repo(fixture: &str, repo_name: &str) -> anyhow::Result<PathBuf> {
    Ok(scripted_fixture_read_only(fixture)
        .map_err(anyhow::Error::from_boxed)?
        .join(repo_name))
}

fn writable_fixture(fixture: &str) -> anyhow::Result<TempDir> {
    scripted_fixture_writable(fixture).map_err(|err| anyhow::anyhow!("{err}"))
}

struct DumpOutput {
    stdout: String,
    stderr: String,
}

impl fmt::Display for DumpOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stdout:\n{}stderr:\n{}", self.stdout, self.stderr)
    }
}

impl DumpOutput {
    fn display_for_snapshot(&self, output: &Path) -> String {
        self.to_string()
            .replace(&output.display().to_string(), "[output path]")
    }
}

fn run_dump(repo: &Path, output: &Path, git_only: bool) -> anyhow::Result<DumpOutput> {
    let mut args = vec![
        OsString::from("but-debug"),
        OsString::from("dump"),
        OsString::from("-C"),
        repo.as_os_str().to_owned(),
        OsString::from("--output"),
        output.as_os_str().to_owned(),
    ];
    if git_only {
        args.push(OsString::from("--git-only"));
    }
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    but_debug::handle_args(args.into_iter(), &mut stdout, &mut stderr)?;
    Ok(DumpOutput {
        stdout: String::from_utf8(stdout)?,
        stderr: String::from_utf8(stderr)?,
    })
}

fn git_status(repo: &Path) -> anyhow::Result<String> {
    let output = git_at_dir(repo)
        .args(["status", "--short", "--branch"])
        .output()?;
    assert!(
        output.status.success(),
        "git status failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

#[derive(Default)]
struct Node {
    children: BTreeMap<String, Node>,
    explicit_dir: bool,
    explicit_file: bool,
    mode: Option<u32>,
    hidden_files: usize,
    hidden_dirs: usize,
}

fn archive_tree(path: &Path) -> anyhow::Result<termtree::Tree<String>> {
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut root = Node::default();
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        let name = file.name();
        let mode = file.unix_mode();
        match classify_entry(name, file.is_dir()) {
            EntryDisplay::Shown => root.insert(name, mode),
            EntryDisplay::Hidden { parent, entry_kind } => root.insert_hidden(&parent, entry_kind),
        }
    }
    Ok(root.into_single_tree())
}

fn unzip(archive: &Path, destination: &Path) -> anyhow::Result<()> {
    let file = File::open(archive)?;
    zip::ZipArchive::new(file)?.extract(destination)?;
    Ok(())
}

impl Node {
    fn insert(&mut self, name: &str, mode: Option<u32>) {
        let is_dir = name.ends_with('/');
        let mut node = self;
        for component in name.trim_end_matches('/').split('/') {
            if component.is_empty() {
                continue;
            }
            node = node.children.entry(component.to_owned()).or_default();
        }
        if is_dir {
            node.explicit_dir = true;
        } else {
            node.explicit_file = true;
        }
        node.mode = mode;
    }

    fn insert_hidden(&mut self, parent: &[String], entry_kind: EntryKind) {
        let mut node = self;
        for component in parent {
            node = node.children.entry(component.to_owned()).or_default();
        }
        match entry_kind {
            EntryKind::File => node.hidden_files += 1,
            EntryKind::Directory => node.hidden_dirs += 1,
        }
    }

    fn into_tree(self, name: String) -> termtree::Tree<String> {
        let mut tree = termtree::Tree::new(self.label(name));
        for (name, child) in self.children {
            tree.push(child.into_tree(name));
        }
        if self.hidden_files != 0 || self.hidden_dirs != 0 {
            tree.push(hidden_summary(self.hidden_files, self.hidden_dirs));
        }
        tree
    }

    fn into_single_tree(self) -> termtree::Tree<String> {
        let mut children = self.children;
        assert_eq!(children.len(), 1, "archive should have exactly one root");
        let (name, child) = children.pop_first().expect("archive root is present");
        child.into_tree(name)
    }

    fn label(&self, name: String) -> String {
        let mode = self
            .mode
            .map(|mode| format!(":{mode:o}"))
            .unwrap_or_default();
        match (
            self.children.is_empty(),
            self.explicit_dir,
            self.explicit_file,
        ) {
            (false, _, true) => format!("{name}{mode} [file+dir]"),
            (false, _, false) | (true, true, false) => format!("{name}{mode}/"),
            _ => format!("{name}{mode}"),
        }
    }
}

#[derive(Clone, Copy)]
enum EntryKind {
    File,
    Directory,
}

enum EntryDisplay {
    Shown,
    Hidden {
        parent: Vec<String>,
        entry_kind: EntryKind,
    },
}

fn classify_entry(name: &str, is_dir: bool) -> EntryDisplay {
    let components: Vec<_> = name
        .trim_end_matches('/')
        .split('/')
        .filter(|component| !component.is_empty())
        .collect();

    if components.contains(&"..") {
        return EntryDisplay::Shown;
    }

    if let Some(git_position) = components.iter().position(|component| *component == ".git") {
        let git_path = &components[git_position + 1..];
        return match git_path {
            []
            | ["HEAD" | "config" | "commondir" | "gitdir"]
            | ["gitbutler"]
            | ["gitbutler", "vb.toml"] => EntryDisplay::Shown,
            ["worktrees", ..] => EntryDisplay::Shown,
            _ => EntryDisplay::Hidden {
                parent: components[..=git_position]
                    .iter()
                    .map(|component| component.to_string())
                    .collect(),
                entry_kind: entry_kind(is_dir),
            },
        };
    }

    if let Some((root, git_path)) = components.split_first()
        && root.ends_with(".git")
    {
        return match git_path {
            [] | ["HEAD" | "config"] | ["gitbutler"] | ["gitbutler", "vb.toml"] => {
                EntryDisplay::Shown
            }
            _ => EntryDisplay::Hidden {
                parent: vec![root.to_string()],
                entry_kind: entry_kind(is_dir),
            },
        };
    }

    EntryDisplay::Shown
}

fn entry_kind(is_dir: bool) -> EntryKind {
    if is_dir {
        EntryKind::Directory
    } else {
        EntryKind::File
    }
}

fn hidden_summary(files: usize, directories: usize) -> termtree::Tree<String> {
    termtree::Tree::new(format!(
        "... {files} {} and {directories} {} not shown",
        if files == 1 { "file" } else { "files" },
        if directories == 1 {
            "directory"
        } else {
            "directories"
        },
    ))
}
