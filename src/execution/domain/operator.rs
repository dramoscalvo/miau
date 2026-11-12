//! Commands an agent asks the human operator to run on their own terminal.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// `miau ask` exit code when the operator did not answer in time.
pub const EXIT_TIMEOUT: i32 = 124;
/// `miau ask` exit code when the request could not be delivered or completed.
pub const EXIT_UNAVAILABLE: i32 = 125;
/// `miau ask` exit code when the operator refused the command.
pub const EXIT_DENIED: i32 = 126;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorRequest {
    pub id: u32,
    pub token: String,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    #[serde(default)]
    pub cancelled: bool,
}

/// What the asking process sends to the miau that started its agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Message {
    Ask {
        token: String,
        argv: Vec<String>,
        cwd: PathBuf,
    },
    Cancel {
        token: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Exited { code: Option<i32>, log: PathBuf },
    Denied { reason: String },
    Failed { message: String },
}

/// Tokens name files, so they are restricted to a path-safe alphabet.
pub fn is_valid_token(token: &str) -> bool {
    (1..=64).contains(&token.len()) && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// Renders arguments as one shell command line that is safe to show verbatim.
///
/// The operator decides from this text whether to run a command as root, so
/// control and direction-changing characters are escaped rather than printed.
pub fn command_line(argv: &[String]) -> String {
    argv.iter()
        .map(|argument| quote(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The request and the exact command lines that answer it, for the operator.
pub fn summary(request: &OperatorRequest) -> String {
    let OperatorRequest { id, argv, cwd, .. } = request;
    format!(
        "[{id}] {}\n    in {}\n    run it:    miau op {id}\n    refuse it: miau op {id} --deny \"reason\"",
        command_line(argv),
        cwd.display()
    )
}

fn quote(argument: &str) -> String {
    let is_plain =
        |character: char| character.is_ascii_alphanumeric() || "_@%+=:,./-".contains(character);
    if !argument.is_empty() && argument.chars().all(is_plain) {
        return argument.to_owned();
    }
    if !argument.chars().any(is_deceptive) {
        return format!("'{}'", argument.replace('\'', "'\\''"));
    }
    let mut quoted = String::from("$'");
    for character in argument.chars() {
        match character {
            '\'' => quoted.push_str("\\'"),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            other if is_deceptive(other) => {
                quoted.push_str(&format!("\\u{:04x}", u32::from(other)));
            }
            other => quoted.push(other),
        }
    }
    quoted.push('\'');
    quoted
}

fn is_deceptive(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(argv: &[&str]) -> String {
        command_line(
            &argv
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn plain_arguments_are_shown_unquoted() {
        assert_eq!(
            line(&[
                "sudo",
                "docker",
                "compose",
                "-f",
                "/srv/app.yml",
                "up",
                "-d"
            ]),
            "sudo docker compose -f /srv/app.yml up -d"
        );
    }

    #[test]
    fn arguments_with_shell_syntax_are_quoted_as_one_word() {
        assert_eq!(
            line(&["sh", "-c", "echo it's; rm -rf /", ""]),
            "sh -c 'echo it'\\''s; rm -rf /' ''"
        );
    }

    #[test]
    fn terminal_escapes_and_direction_overrides_are_never_printed_raw() {
        let shown = line(&["echo", "safe\u{1b}[2K\rhidden\u{202e}"]);
        assert_eq!(shown, "echo $'safe\\u001b[2K\\u000dhidden\\u202e'");
    }

    #[test]
    fn the_summary_gives_the_operator_the_command_lines_to_type() {
        let request = OperatorRequest {
            id: 2,
            token: "t".into(),
            argv: vec!["sudo".into(), "docker".into(), "ps".into()],
            cwd: "/work/app".into(),
            cancelled: false,
        };
        assert_eq!(
            summary(&request),
            "[2] sudo docker ps\n    in /work/app\n    run it:    miau op 2\n    refuse it: miau op 2 --deny \"reason\""
        );
    }

    #[test]
    fn tokens_cannot_name_paths() {
        assert!(is_valid_token("a1B2"));
        assert!(!is_valid_token(""));
        assert!(!is_valid_token("../state"));
        assert!(!is_valid_token(&"a".repeat(65)));
    }
}
