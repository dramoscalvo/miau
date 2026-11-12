//! End-to-end behavior of `miau ask` (agent side) and `miau op` (operator side).

use miau::execution::{
    application::operator::OperatorRequests,
    domain::operator::{EXIT_DENIED, EXIT_TIMEOUT, EXIT_UNAVAILABLE, OperatorRequest},
    infrastructure::operator::{DIR_ENV, FileOperatorRequests, OperatorInbox},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::process::{Child, Command};

fn temporary_directory(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("miau-operator-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    directory
}

fn ask(directory: &Path, timeout: &str, command: &[&str]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_miau"))
        .args(["ask", "--timeout", timeout, "--"])
        .args(command)
        .env(DIR_ENV, directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

async fn op(directory: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_miau"))
        .arg("op")
        .args(arguments)
        .env(DIR_ENV, directory)
        .stdin(Stdio::null())
        .output()
        .await
        .unwrap()
}

async fn until<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(value) = probe() {
                return value;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("condition was not met in time")
}

async fn first_pending(store: &FileOperatorRequests) -> OperatorRequest {
    until(|| store.pending().unwrap().into_iter().next()).await
}

#[tokio::test]
async fn the_agent_receives_the_output_and_exit_code_of_the_command_the_operator_ran() {
    let directory = temporary_directory("run");
    let store = FileOperatorRequests::new(&directory);
    let _inbox = OperatorInbox::open(&directory).unwrap();

    let asking = ask(
        &directory,
        "20",
        &["sh", "-c", "echo out; echo err >&2; exit 3"],
    );
    let request = first_pending(&store).await;
    assert_eq!(request.id, 1);
    assert_eq!(request.argv, ["sh", "-c", "echo out; echo err >&2; exit 3"]);

    let listed = op(&directory, &[]).await;
    assert!(String::from_utf8_lossy(&listed.stdout).contains("miau op 1"));

    let ran = op(&directory, &["1"]).await;
    assert!(ran.status.success(), "{ran:?}");

    let answered = asking.wait_with_output().await.unwrap();
    let stdout = String::from_utf8_lossy(&answered.stdout);
    assert_eq!(answered.status.code(), Some(3), "{answered:?}");
    assert!(stdout.contains("out") && stdout.contains("err"), "{stdout}");
    assert!(store.pending().unwrap().is_empty());

    let repeated = op(&directory, &["1"]).await;
    assert!(
        !repeated.status.success(),
        "an answered request must not run twice"
    );
    let _ = fs::remove_dir_all(directory);
}

#[tokio::test]
async fn a_refusal_reaches_the_agent_with_the_reason_and_runs_nothing() {
    let directory = temporary_directory("deny");
    let store = FileOperatorRequests::new(&directory);
    let _inbox = OperatorInbox::open(&directory).unwrap();
    let marker = directory.join("ran");

    let asking = ask(&directory, "20", &["touch", marker.to_str().unwrap()]);
    first_pending(&store).await;
    let denied = op(&directory, &["1", "--deny", "use the test database"]).await;
    assert!(denied.status.success(), "{denied:?}");

    let answered = asking.wait_with_output().await.unwrap();
    assert_eq!(answered.status.code(), Some(EXIT_DENIED));
    assert!(String::from_utf8_lossy(&answered.stdout).contains("use the test database"));
    assert!(!marker.exists());
    let _ = fs::remove_dir_all(directory);
}

#[tokio::test]
async fn an_unanswered_request_times_out_and_is_withdrawn() {
    let directory = temporary_directory("timeout");
    let store = FileOperatorRequests::new(&directory);
    let _inbox = OperatorInbox::open(&directory).unwrap();

    let answered = ask(&directory, "1", &["true"])
        .wait_with_output()
        .await
        .unwrap();

    assert_eq!(answered.status.code(), Some(EXIT_TIMEOUT));
    assert!(String::from_utf8_lossy(&answered.stdout).contains("did not run"));
    until(|| store.pending().unwrap().is_empty().then_some(())).await;
    let late = op(&directory, &["1"]).await;
    assert!(!late.status.success(), "a withdrawn request must not run");
    let _ = fs::remove_dir_all(directory);
}

#[tokio::test]
async fn asking_without_a_listening_miau_fails_immediately() {
    let directory = temporary_directory("unavailable");
    fs::create_dir_all(&directory).unwrap();

    let answered = ask(&directory, "20", &["true"])
        .wait_with_output()
        .await
        .unwrap();
    assert_eq!(answered.status.code(), Some(EXIT_UNAVAILABLE));

    let outside = Command::new(env!("CARGO_BIN_EXE_miau"))
        .args(["ask", "--", "true"])
        .env_remove(DIR_ENV)
        .output()
        .await
        .unwrap();
    assert_eq!(outside.status.code(), Some(EXIT_UNAVAILABLE));
    let _ = fs::remove_dir_all(directory);
}

#[tokio::test]
async fn closing_the_inbox_withdraws_requests_the_agent_left_behind() {
    let directory = temporary_directory("closed");
    let store = FileOperatorRequests::new(&directory);
    let inbox = OperatorInbox::open(&directory).unwrap();

    let mut asking = ask(&directory, "20", &["true"]);
    first_pending(&store).await;
    asking.start_kill().unwrap();
    let _ = asking.wait().await;
    drop(inbox);

    assert!(store.pending().unwrap().is_empty());
    let _ = fs::remove_dir_all(directory);
}
