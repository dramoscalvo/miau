//! Git-backed working-tree inspection.

use crate::workflow::{
    application::WorkingTreeRepository,
    domain::{WorkingTreeChange, WorkingTreeChangeKind},
};
use std::{io, path::Path, process::Command};

pub struct GitWorkingTree;

impl WorkingTreeRepository for GitWorkingTree {
    fn changes(&self, project: &Path) -> io::Result<Vec<WorkingTreeChange>> {
        let output = Command::new("git")
            .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
            .current_dir(project)
            .output()?;
        successful_output(output).map(|stdout| parse_porcelain(&stdout))
    }

    fn diff(&self, project: &Path, change: &WorkingTreeChange) -> io::Result<String> {
        let mut command = Command::new("git");
        command
            .args(["diff", "--no-ext-diff", "--color=never", "HEAD", "--"])
            .current_dir(project);
        if let Some(previous) = &change.previous_path {
            command.arg(previous);
        }
        command.arg(&change.path);
        let output = command.output()?;
        let mut diff = String::from_utf8_lossy(&successful_output(output)?).into_owned();

        if change.kind == WorkingTreeChangeKind::Added && diff.is_empty() {
            let output = Command::new("git")
                .args(["diff", "--no-index", "--no-ext-diff", "--color=never", "--"])
                .arg(null_path())
                .arg(&change.path)
                .current_dir(project)
                .output()?;
            if !matches!(output.status.code(), Some(0 | 1)) {
                return Err(command_error(&output.stderr));
            }
            diff = String::from_utf8_lossy(&output.stdout).into_owned();
        }

        Ok(if diff.is_empty() {
            "No textual diff available.".into()
        } else {
            diff
        })
    }
}

fn successful_output(output: std::process::Output) -> io::Result<Vec<u8>> {
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(command_error(&output.stderr))
    }
}

fn command_error(stderr: &[u8]) -> io::Error {
    io::Error::other(String::from_utf8_lossy(stderr).trim().to_owned())
}

#[cfg(unix)]
fn null_path() -> &'static str {
    "/dev/null"
}

#[cfg(windows)]
fn null_path() -> &'static str {
    "NUL"
}

fn parse_porcelain(output: &[u8]) -> Vec<WorkingTreeChange> {
    let fields = output
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    let mut fields = fields.peekable();
    let mut changes = Vec::new();

    while let Some(field) = fields.next() {
        if field.len() < 4 || field[2] != b' ' {
            continue;
        }
        let status = &field[..2];
        let path = path_from_bytes(&field[3..]);
        let renamed = status.iter().any(|code| matches!(code, b'R' | b'C'));
        let previous_path = renamed
            .then(|| fields.next().map(path_from_bytes))
            .flatten();
        let kind = change_kind(status);
        changes.push(WorkingTreeChange {
            path,
            previous_path,
            kind,
        });
    }

    changes.sort_by(|left, right| left.path.cmp(&right.path));
    changes
}

fn path_from_bytes(path: &[u8]) -> std::path::PathBuf {
    String::from_utf8_lossy(path).into_owned().into()
}

fn change_kind(status: &[u8]) -> WorkingTreeChangeKind {
    if status.contains(&b'D') {
        WorkingTreeChangeKind::Deleted
    } else if status.iter().any(|code| matches!(code, b'R' | b'C')) {
        WorkingTreeChangeKind::Renamed
    } else if status.iter().any(|code| matches!(code, b'A' | b'?')) {
        WorkingTreeChangeKind::Added
    } else {
        WorkingTreeChangeKind::Modified
    }
}

#[cfg(test)]
mod tests {
    use super::{GitWorkingTree, parse_porcelain};
    use crate::workflow::application::WorkingTreeRepository;
    use crate::workflow::domain::{WorkingTreeChange, WorkingTreeChangeKind};
    use std::{fs, path::PathBuf, process::Command, time::SystemTime};

    #[test]
    fn porcelain_parser_normalizes_statuses_and_rename_paths() {
        let changes = parse_porcelain(
            b" M src/changed.rs\0?? notes/new file.md\0D  old.txt\0R  src/new.rs\0src/old.rs\0",
        );

        assert_eq!(
            changes,
            vec![
                WorkingTreeChange::new("notes/new file.md", WorkingTreeChangeKind::Added),
                WorkingTreeChange::new("old.txt", WorkingTreeChangeKind::Deleted),
                WorkingTreeChange::new("src/changed.rs", WorkingTreeChangeKind::Modified),
                WorkingTreeChange::renamed("src/old.rs", "src/new.rs"),
            ]
        );
    }

    #[test]
    fn porcelain_parser_keeps_non_utf8_paths_visible() {
        let changes = parse_porcelain(b"?? bad\xffname\0");

        assert_eq!(changes[0].path, PathBuf::from("bad\u{fffd}name"));
    }

    #[test]
    fn git_reader_lists_changes_and_loads_selected_diff() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project =
            std::env::temp_dir().join(format!("miau-working-tree-{}-{unique}", std::process::id()));
        fs::create_dir_all(&project).unwrap();
        git(&project, &["init", "-q"]);
        fs::write(project.join("tracked.txt"), "before\n").unwrap();
        git(&project, &["add", "tracked.txt"]);
        git(
            &project,
            &[
                "-c",
                "user.name=miau test",
                "-c",
                "user.email=miau@example.invalid",
                "commit",
                "-qm",
                "initial",
            ],
        );
        fs::write(project.join("tracked.txt"), "after\n").unwrap();
        fs::write(project.join("new.txt"), "new line\n").unwrap();

        let reader = GitWorkingTree;
        let changes = reader.changes(&project).unwrap();
        let added = changes
            .iter()
            .find(|change| change.path == std::path::Path::new("new.txt"))
            .unwrap();
        let diff = reader.diff(&project, added).unwrap();
        fs::remove_dir_all(&project).unwrap();

        assert!(
            changes.len() == 2
                && added.kind == WorkingTreeChangeKind::Added
                && diff.contains("+new line")
        );
    }

    fn git(project: &std::path::Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(project)
            .status()
            .unwrap();
        assert!(status.success());
    }
}
