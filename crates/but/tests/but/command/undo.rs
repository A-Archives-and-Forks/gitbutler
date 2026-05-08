use crate::utils::Sandbox;

mod undo_rub;

/// Run an undo test tests a roundtrip `mutate` -> `but undo`, and asserts that the status output is
/// the same before and after the roundtrip.
fn run_mutate_undo_roundtrip_test<F>(env: &Sandbox, mutate: F) -> anyhow::Result<()>
where
    F: FnOnce(&Sandbox) -> anyhow::Result<()>,
{
    // Arrange
    let status_output_before = env.but("status").output()?;

    {
        eprintln!("Status before mutation:");
        let output = String::from_utf8(status_output_before.stdout.clone()).unwrap();
        for line in output.lines() {
            eprintln!("    {line}");
        }
    }

    mutate(env)?;
    let status_output_after_mutate = env.but("status").output()?;

    {
        eprintln!();
        eprintln!("Status after mutation:");
        let output = String::from_utf8(status_output_after_mutate.stdout.clone()).unwrap();
        for line in output.lines() {
            eprintln!("    {line}");
        }
    }

    assert_ne!(
        status_output_before, status_output_after_mutate,
        "mutate must visibly change state"
    );

    // Act
    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: [..]
✓ Undo completed successfully! Restored to snapshot:[..]
"#,
    );

    // Assert
    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_output_before.stdout)
        .stderr_eq(status_output_before.stderr);

    Ok(())
}

#[track_caller]
fn reword(
    env: &Sandbox,
    commit_before: &str,
    commit_after: &str,
    new_message: &str,
) -> anyhow::Result<std::process::Output> {
    env.but("reword")
        .args([commit_before, "-m", new_message])
        .assert()
        .success()
        .stdout_eq(format!(
            "Updated commit message for {commit_before} (now {commit_after})\n"
        ))
        .stderr_eq("");
    Ok(env.but("status").output()?)
}

#[test]
fn can_undo_discard() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("discard")
            .arg(path)
            .assert()
            .success()
            .stdout_eq("Successfully discarded changes to 1 item\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata_at_target(&["A"], "origin/main")?;
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.file("new-file.txt", "content");

        env.but("commit -m 'Add file'")
            .assert()
            .success()
            .stdout_eq("✓ Created commit [..] on branch A\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
#[ignore = "Test harness runs with cv3 feature flag, and but_core::worktree::safe_checkout_from_head does not restore the worktree file A for some reason"]
fn can_undo_unapply() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("unapply A")
            .assert()
            .success()
            .stdout_eq("Unapplied stack with branches 'A' from workspace\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
#[ignore = "Test harness runs with cv3 feature flag, and but_core::worktree::safe_checkout_from_head does not remove the worktree file A for some reason"]
fn can_undo_clean_apply() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.but("unapply A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("apply A")
            .assert()
            .success()
            .stdout_eq("Applied branch 'A' to workspace\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_rewording_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("reword")
            .args(["9ac4652", "-m", "reworded"])
            .assert()
            .success()
            .stdout_eq("Updated commit message for [..] (now [..])\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_rewording_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("reword")
            .args(["A", "-m", "reworded-branch"])
            .assert()
            .success()
            .stdout_eq("Renamed branch 'A' to 'reworded-branch'\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_repeatedly() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    let status_one = reword(&env, "9ac4652", "9f9095e", "one")?;
    let status_two = reword(&env, "9f9095e", "baa6a31", "two")?;
    let status_three = reword(&env, "baa6a31", "a1fa8e0", "three")?;
    reword(&env, "a1fa8e0", "c5d642c", "four")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: f4e985f
"#,
    );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_three.stdout)
        .stderr_eq(status_three.stderr);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: e637109
"#,
    );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_two.stdout)
        .stderr_eq(status_two.stderr);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
7214e58 2000-01-02 00:00:00 [UNDO] Restored from snapshot
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: 90c8e9b
"#,
    );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_one.stdout)
        .stderr_eq(status_one.stderr);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
00c7dd1 2000-01-02 00:00:00 [UNDO] Restored from snapshot
7214e58 2000-01-02 00:00:00 [UNDO] Restored from snapshot
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    Ok(())
}

#[test]
fn can_undo_explicit_restore() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    reword(&env, "9ac4652", "9f9095e", "one")?;
    let status_two = reword(&env, "9f9095e", "baa6a31", "two")?;
    reword(&env, "baa6a31", "a1fa8e0", "three")?;
    let status_four = reword(&env, "a1fa8e0", "c5d642c", "four")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("oplog")
        .args(["restore", "e637109"])
        .assert()
        .success()
        .stdout_eq(
            r#"
✓ Restore completed successfully!

Workspace has been restored to the selected snapshot.
"#,
        );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_two.stdout.clone())
        .stderr_eq(status_two.stderr.clone());

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
2859d85 2000-01-02 00:00:00 [RESTORE] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: Restored from snapshot (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: 2859d85
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_four.stdout)
        .stderr_eq(status_four.stderr);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
63bad11 2000-01-02 00:00:00 [UNDO] Restored from snapshot
2859d85 2000-01-02 00:00:00 [RESTORE] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    Ok(())
}

#[test]
fn can_undo_perform_operation_then_undo_again() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    reword(&env, "9ac4652", "9f9095e", "one")?;
    let status_two = reword(&env, "9f9095e", "baa6a31", "two")?;
    let status_three = reword(&env, "baa6a31", "a1fa8e0", "three")?;
    reword(&env, "a1fa8e0", "c5d642c", "four")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: f4e985f
"#,
    );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_three.stdout.clone())
        .stderr_eq(status_three.stderr.clone());

    reword(&env, "a1fa8e0", "022806e", "three-new")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
f48a8d3 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: f48a8d3
"#,
    );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_three.stdout.clone())
        .stderr_eq(status_three.stderr);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
428645c 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f48a8d3 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: e637109
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_two.stdout.clone())
        .stderr_eq(status_two.stderr.clone());

    Ok(())
}

#[test]
fn undoing_past_end_of_oplog() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    let status_zero = env.but("status").output()?;
    let status_one = reword(&env, "9ac4652", "9f9095e", "one")?;
    reword(&env, "9f9095e", "baa6a31", "two")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: 90c8e9b
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_one.stdout.clone())
        .stderr_eq(status_one.stderr.clone());

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
56ad039 2000-01-02 00:00:00 [UNDO] Restored from snapshot
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: 7665ea7
"#,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
c6dfa5f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
56ad039 2000-01-02 00:00:00 [UNDO] Restored from snapshot
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_zero.stdout.clone())
        .stderr_eq(status_zero.stderr.clone());

    env.but("undo").assert().success().stdout_eq(
        r#"No previous operations to undo.
"#,
    );

    Ok(())
}

#[test]
fn can_redo() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    reword(&env, "9ac4652", "9f9095e", "one")?;
    reword(&env, "9f9095e", "baa6a31", "two")?;
    let status_three = reword(&env, "baa6a31", "a1fa8e0", "three")?;
    let status_four = reword(&env, "a1fa8e0", "c5d642c", "four")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: f4e985f
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_three.stdout)
        .stderr_eq(status_three.stderr);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("redo").assert().success().stdout_eq(
        r#"Redoing operation...
  Reverting to: Restored from snapshot (2000-01-02 00:00:00)
✓ Redo completed successfully! Restored to snapshot: d9fd48f
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_four.stdout)
        .stderr_eq(status_four.stderr);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
06a99ab 2000-01-02 00:00:00 [REDO] Restored from snapshot
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("redo").assert().success().stdout_eq(
        r#"No previous undo to redo.
"#,
    );

    Ok(())
}

#[test]
fn can_mix_undo_and_redo() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    let status_one = reword(&env, "9ac4652", "9f9095e", "one")?;
    let status_two = reword(&env, "9f9095e", "baa6a31", "two")?;
    let status_three = reword(&env, "baa6a31", "a1fa8e0", "three")?;
    let status_four = reword(&env, "a1fa8e0", "c5d642c", "four")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: f4e985f
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_three.stdout.clone())
        .stderr_eq(status_three.stderr.clone());

    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: e637109
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_two.stdout.clone())
        .stderr_eq(status_two.stderr.clone());

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
7214e58 2000-01-02 00:00:00 [UNDO] Restored from snapshot
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    // redo
    env.but("redo").assert().success().stdout_eq(
        r#"Redoing operation...
  Reverting to: Restored from snapshot (2000-01-02 00:00:00)
✓ Redo completed successfully! Restored to snapshot: 7214e58
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_three.stdout.clone())
        .stderr_eq(status_three.stderr.clone());

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
95f6f16 2000-01-02 00:00:00 [REDO] Restored from snapshot
7214e58 2000-01-02 00:00:00 [UNDO] Restored from snapshot
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    // undo
    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: Restored from snapshot (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: 95f6f16
"#,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
2cf5b5e 2000-01-02 00:00:00 [UNDO] Restored from snapshot
95f6f16 2000-01-02 00:00:00 [REDO] Restored from snapshot
7214e58 2000-01-02 00:00:00 [UNDO] Restored from snapshot
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_two.stdout.clone())
        .stderr_eq(status_two.stderr.clone());

    // undo
    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: UpdateCommitMessage (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: 90c8e9b
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_one.stdout)
        .stderr_eq(status_one.stderr);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
6d38f68 2000-01-02 00:00:00 [UNDO] Restored from snapshot
2cf5b5e 2000-01-02 00:00:00 [UNDO] Restored from snapshot
95f6f16 2000-01-02 00:00:00 [REDO] Restored from snapshot
7214e58 2000-01-02 00:00:00 [UNDO] Restored from snapshot
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    // redo
    env.but("redo").assert().success().stdout_eq(
        r#"Redoing operation...
  Reverting to: Restored from snapshot (2000-01-02 00:00:00)
✓ Redo completed successfully! Restored to snapshot: 6d38f68
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_two.stdout.clone())
        .stderr_eq(status_two.stderr.clone());

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
7eb783e 2000-01-02 00:00:00 [REDO] Restored from snapshot
6d38f68 2000-01-02 00:00:00 [UNDO] Restored from snapshot
2cf5b5e 2000-01-02 00:00:00 [UNDO] Restored from snapshot
95f6f16 2000-01-02 00:00:00 [REDO] Restored from snapshot
7214e58 2000-01-02 00:00:00 [UNDO] Restored from snapshot
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    // redo
    env.but("redo").assert().success().stdout_eq(
        r#"Redoing operation...
  Reverting to: Restored from snapshot (2000-01-02 00:00:00)
✓ Redo completed successfully! Restored to snapshot: 2cf5b5e
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_three.stdout.clone())
        .stderr_eq(status_three.stderr.clone());

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
505fa4a 2000-01-02 00:00:00 [REDO] Restored from snapshot
7eb783e 2000-01-02 00:00:00 [REDO] Restored from snapshot
6d38f68 2000-01-02 00:00:00 [UNDO] Restored from snapshot
2cf5b5e 2000-01-02 00:00:00 [UNDO] Restored from snapshot
95f6f16 2000-01-02 00:00:00 [REDO] Restored from snapshot
7214e58 2000-01-02 00:00:00 [UNDO] Restored from snapshot
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    // redo
    env.but("redo").assert().success().stdout_eq(
        r#"Redoing operation...
  Reverting to: Restored from snapshot (2000-01-02 00:00:00)
✓ Redo completed successfully! Restored to snapshot: d9fd48f
"#,
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(status_four.stdout.clone())
        .stderr_eq(status_four.stderr.clone());

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
376d3cb 2000-01-02 00:00:00 [REDO] Restored from snapshot
505fa4a 2000-01-02 00:00:00 [REDO] Restored from snapshot
7eb783e 2000-01-02 00:00:00 [REDO] Restored from snapshot
6d38f68 2000-01-02 00:00:00 [UNDO] Restored from snapshot
2cf5b5e 2000-01-02 00:00:00 [UNDO] Restored from snapshot
95f6f16 2000-01-02 00:00:00 [REDO] Restored from snapshot
7214e58 2000-01-02 00:00:00 [UNDO] Restored from snapshot
d9fd48f 2000-01-02 00:00:00 [UNDO] Restored from snapshot
f4e985f 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
e637109 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
90c8e9b 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
7665ea7 2000-01-02 00:00:00 [REWORD] UpdateCommitMessage
"#,
        );

    Ok(())
}

#[test]
fn cannot_redo_without_undoing_first() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    reword(&env, "9ac4652", "9f9095e", "one")?;

    env.but("redo").assert().success().stdout_eq(
        r#"No previous undo to redo.
"#,
    );

    Ok(())
}
