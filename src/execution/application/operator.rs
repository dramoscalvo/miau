//! Operator-request port and the use case that accepts requests from an agent.

use crate::execution::domain::operator::{Message, OperatorRequest, Outcome, is_valid_token};
use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OperatorError {
    #[error("operator request I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("operator request data is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

/// Storage shared by the miau that runs the agent, `miau ask`, and `miau op`.
pub trait OperatorRequests {
    /// Stores a new request under the next identifier.
    fn record(
        &self,
        token: String,
        argv: Vec<String>,
        cwd: PathBuf,
    ) -> Result<OperatorRequest, OperatorError>;
    fn cancel(&self, token: &str) -> Result<(), OperatorError>;
    fn cancel_all(&self) -> Result<(), OperatorError>;
    /// Requests that are neither cancelled nor answered, oldest first.
    fn pending(&self) -> Result<Vec<OperatorRequest>, OperatorError>;
    fn answer(&self, request: &OperatorRequest, outcome: &Outcome) -> Result<(), OperatorError>;
    fn outcome(&self, token: &str) -> Result<Option<Outcome>, OperatorError>;
}

/// Applies one line received from an asking process.
pub fn receive(store: &impl OperatorRequests, line: &str) -> Result<(), OperatorError> {
    match serde_json::from_str(line)? {
        Message::Ask { token, argv, cwd } => {
            if !is_valid_token(&token) {
                return Err(OperatorError::Invalid("invalid request token".into()));
            }
            if argv.first().is_none_or(String::is_empty) {
                return Err(OperatorError::Invalid("request has no command".into()));
            }
            store.record(token, argv, cwd).map(|_| ())
        }
        Message::Cancel { token } => store.cancel(&token),
    }
}

pub fn find_pending(
    store: &impl OperatorRequests,
    id: u32,
) -> Result<OperatorRequest, OperatorError> {
    store
        .pending()?
        .into_iter()
        .find(|request| request.id == id)
        .ok_or_else(|| {
            OperatorError::Invalid(format!(
                "request {id} is not pending; it was answered, withdrawn, or never made"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Memory {
        requests: RefCell<Vec<OperatorRequest>>,
    }

    impl OperatorRequests for Memory {
        fn record(
            &self,
            token: String,
            argv: Vec<String>,
            cwd: PathBuf,
        ) -> Result<OperatorRequest, OperatorError> {
            let mut requests = self.requests.borrow_mut();
            let request = OperatorRequest {
                id: requests.len() as u32 + 1,
                token,
                argv,
                cwd,
                cancelled: false,
            };
            requests.push(request.clone());
            Ok(request)
        }
        fn cancel(&self, token: &str) -> Result<(), OperatorError> {
            for request in self.requests.borrow_mut().iter_mut() {
                request.cancelled |= request.token == token;
            }
            Ok(())
        }
        fn cancel_all(&self) -> Result<(), OperatorError> {
            Ok(())
        }
        fn pending(&self) -> Result<Vec<OperatorRequest>, OperatorError> {
            Ok(self
                .requests
                .borrow()
                .iter()
                .filter(|request| !request.cancelled)
                .cloned()
                .collect())
        }
        fn answer(&self, _: &OperatorRequest, _: &Outcome) -> Result<(), OperatorError> {
            Ok(())
        }
        fn outcome(&self, _: &str) -> Result<Option<Outcome>, OperatorError> {
            Ok(None)
        }
    }

    #[test]
    fn an_ask_becomes_a_pending_request_and_a_cancel_withdraws_it() {
        let store = Memory::default();
        receive(
            &store,
            r#"{"kind":"ask","token":"t1","argv":["sudo","true"],"cwd":"/work"}"#,
        )
        .unwrap();
        assert_eq!(find_pending(&store, 1).unwrap().argv, ["sudo", "true"]);

        receive(&store, r#"{"kind":"cancel","token":"t1"}"#).unwrap();
        assert!(find_pending(&store, 1).is_err());
    }

    #[test]
    fn malformed_unsafe_or_empty_requests_are_rejected() {
        let store = Memory::default();
        for line in [
            "not json",
            r#"{"kind":"ask","token":"../x","argv":["true"],"cwd":"/"}"#,
            r#"{"kind":"ask","token":"t","argv":[],"cwd":"/"}"#,
            r#"{"kind":"ask","token":"t","argv":[""],"cwd":"/"}"#,
        ] {
            assert!(receive(&store, line).is_err(), "{line}");
        }
        assert!(store.pending().unwrap().is_empty());
    }
}
