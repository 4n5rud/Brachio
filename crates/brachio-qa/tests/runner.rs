//! Integration tests with a direct child process fixture.

use std::env;
use std::fs;
use std::io;
use std::process;
use std::time::Duration;

use brachio_git::{local_head, Git};
use brachio_qa::{run_approved, FeedbackDecision, FeedbackGate, QaCommand, QaExit, QaNotRun};

fn fixture() -> Result<(tempfile::TempDir, QaCommand, String), Box<dyn std::error::Error>> {
    let worktree = tempfile::tempdir()?;
    let git = Git::new(worktree.path());
    assert!(git.output(&["init"])?.status.success());
    assert!(git
        .output(&[
            "-c",
            "user.name=QA Fixture",
            "-c",
            "user.email=qa@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "fixture",
        ])?
        .status
        .success());
    let sha = local_head(worktree.path())?;
    let command = QaCommand::new(
        env::current_exe()?,
        ["--exact", "child_fixture", "--ignored", "--nocapture"]
            .map(Into::into)
            .into(),
        worktree.path(),
        Duration::from_secs(20),
    )?;
    Ok((worktree, command, sha))
}

#[test]
#[ignore = "spawned only by runner tests"]
fn child_fixture() -> io::Result<()> {
    match fs::read_to_string("qa-mode")?.as_str() {
        "fail" => {
            eprintln!("fixture secret-value");
            process::exit(23);
        }
        "timeout" => std::thread::sleep(Duration::from_secs(600)),
        "large" => println!("{}", "a".repeat(131_072)),
        "change" => {
            let git = Git::new(&env::current_dir()?);
            let output = git
                .output(&[
                    "-c",
                    "user.name=QA Fixture",
                    "-c",
                    "user.email=qa@example.invalid",
                    "commit",
                    "--allow-empty",
                    "-m",
                    "new head",
                ])
                .map_err(io::Error::other)?;
            assert!(output.status.success());
        }
        _ => println!("fixture passed"),
    }
    Ok(())
}

#[test]
fn approved_commands_report_pass_failure_and_stop_repeated_feedback(
) -> Result<(), Box<dyn std::error::Error>> {
    let (worktree, command, sha) = fixture()?;
    fs::write(worktree.path().join("qa-mode"), "pass")?;

    let pass = run_approved(Some(&command), Some(command.approval(sha.clone())), &[])?;
    assert_eq!(pass.exit, QaExit::Passed);
    assert!(pass.head_unchanged);
    assert!(pass.stdout.text.contains("fixture passed"));

    fs::write(worktree.path().join("qa-mode"), "fail")?;
    let mut result = run_approved(
        Some(&command),
        Some(command.approval(sha.clone())),
        &["secret-value"],
    )?;
    assert!(matches!(result.exit, QaExit::Failed(status) if status.code() == Some(23)));
    assert!(result.stderr.text.contains("[redacted]"));
    assert!(!result.stderr.text.contains("secret-value"));
    let mut gate = FeedbackGate::new(3);
    assert_eq!(result.feedback(&mut gate), Some(FeedbackDecision::Send));
    assert_eq!(result.feedback(&mut gate), None);
    let mut repeated = run_approved(
        Some(&command),
        Some(command.approval(sha)),
        &["secret-value"],
    )?;
    assert_eq!(
        repeated.feedback(&mut gate),
        Some(FeedbackDecision::StopNoProgress)
    );
    Ok(())
}

#[test]
fn opt_out_denial_and_stale_head_do_not_execute() -> Result<(), Box<dyn std::error::Error>> {
    let (_worktree, command, _sha) = fixture()?;

    assert!(matches!(
        run_approved(None, Some(command.approval("obsolete".to_owned())), &[]),
        Err(QaNotRun::Disabled)
    ));
    assert!(matches!(
        run_approved(Some(&command), None, &[]),
        Err(QaNotRun::NotApproved)
    ));
    assert!(matches!(
        run_approved(
            Some(&command),
            Some(command.approval("obsolete".to_owned())),
            &[]
        ),
        Err(QaNotRun::StaleHead)
    ));
    Ok(())
}

#[test]
fn output_overflow_and_changed_head_cannot_offer_feedback() -> Result<(), Box<dyn std::error::Error>>
{
    let (worktree, command, sha) = fixture()?;

    fs::write(worktree.path().join("qa-mode"), "large")?;
    let mut large = run_approved(Some(&command), Some(command.approval(sha.clone())), &[])?;
    assert_eq!(large.exit, QaExit::OutputLimit);
    assert!(large.stdout.truncated);
    let mut gate = FeedbackGate::new(3);
    assert_eq!(large.feedback(&mut gate), None);

    fs::write(worktree.path().join("qa-mode"), "change")?;
    let mut changed = run_approved(Some(&command), Some(command.approval(sha)), &[])?;
    assert_eq!(changed.exit, QaExit::Passed);
    assert!(!changed.head_unchanged);
    assert_eq!(changed.feedback(&mut gate), None);
    Ok(())
}

#[test]
fn timeout_and_missing_program_are_explicit() -> Result<(), Box<dyn std::error::Error>> {
    let (worktree, command, sha) = fixture()?;
    fs::write(worktree.path().join("qa-mode"), "timeout")?;
    let timeout = QaCommand::new(
        command.argv()[0].clone().into(),
        command.argv()[1..].to_vec(),
        command.cwd(),
        Duration::from_millis(100),
    )?;
    assert!(matches!(
        run_approved(Some(&timeout), Some(command.approval(sha.clone())), &[]),
        Err(QaNotRun::NotApproved)
    ));
    let result = run_approved(Some(&timeout), Some(timeout.approval(sha.clone())), &[])?;
    assert_eq!(result.exit, QaExit::TimedOut);
    let missing = QaCommand::new(
        worktree.path().join("missing.exe"),
        Vec::new(),
        worktree.path(),
        Duration::from_secs(5),
    )?;
    let result = run_approved(Some(&missing), Some(missing.approval(sha)), &[])?;
    assert!(matches!(
        result.exit,
        QaExit::ProcessError(io::ErrorKind::NotFound)
    ));
    Ok(())
}
