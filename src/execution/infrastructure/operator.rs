//! File and named-pipe transport for operator requests.
//!
//! An agent may run in a sandbox that forbids file writes (Codex read-only),
//! so the asking process only writes to a named pipe and only reads files.
//! The miau that started the agent owns the pipe and persists each request.

use crate::execution::{
    application::operator::{OperatorError, OperatorRequests, find_pending, receive},
    domain::operator::{
        EXIT_DENIED, EXIT_TIMEOUT, EXIT_UNAVAILABLE, Message, OperatorRequest, Outcome,
        command_line,
    },
};
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    net::unix::pipe,
    signal::unix::{SignalKind, signal},
    task::JoinHandle,
};

/// Environment variable that tells `miau ask` and `miau op` where requests live.
pub const DIR_ENV: &str = "MIAU_OPERATOR_DIR";
/// Writes up to this size reach a pipe atomically on every supported platform.
const MAX_MESSAGE_BYTES: usize = 4000;
const MAX_REPLAYED_OUTPUT_BYTES: usize = 64 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Debug, Clone)]
pub struct FileOperatorRequests {
    directory: PathBuf,
}

impl FileOperatorRequests {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    fn inbox(&self) -> PathBuf {
        self.directory.join("inbox")
    }

    fn request_path(&self, id: u32) -> PathBuf {
        self.directory.join(format!("{id}.json"))
    }

    fn outcome_path(&self, token: &str) -> PathBuf {
        self.directory.join(format!("{token}.outcome.json"))
    }

    fn log_path(&self, id: u32) -> PathBuf {
        self.directory.join(format!("{id}.log"))
    }

    fn write_atomically(&self, path: &Path, text: &str) -> Result<(), OperatorError> {
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, path)?;
        Ok(())
    }

    fn requests(&self) -> Result<Vec<OperatorRequest>, OperatorError> {
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
            Err(error) => return Err(error.into()),
        };
        let mut requests = Vec::new();
        for entry in entries {
            let path = entry?.path();
            let is_request = path
                .extension()
                .is_some_and(|extension| extension == "json")
                && path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem.bytes().all(|byte| byte.is_ascii_digit()));
            if is_request {
                requests.push(serde_json::from_str::<OperatorRequest>(
                    &fs::read_to_string(&path)?,
                )?);
            }
        }
        requests.sort_by_key(|request| request.id);
        Ok(requests)
    }

    fn save(&self, request: &OperatorRequest) -> Result<(), OperatorError> {
        self.write_atomically(
            &self.request_path(request.id),
            &serde_json::to_string_pretty(request)?,
        )
    }

    fn cancel_where(
        &self,
        matches: impl Fn(&OperatorRequest) -> bool,
    ) -> Result<(), OperatorError> {
        for mut request in self.requests()? {
            let is_waiting = !request.cancelled && !self.outcome_path(&request.token).exists();
            if is_waiting && matches(&request) {
                request.cancelled = true;
                self.save(&request)?;
            }
        }
        Ok(())
    }
}

impl OperatorRequests for FileOperatorRequests {
    fn record(
        &self,
        token: String,
        argv: Vec<String>,
        cwd: PathBuf,
    ) -> Result<OperatorRequest, OperatorError> {
        let id = self.requests()?.last().map_or(1, |last| last.id + 1);
        let request = OperatorRequest {
            id,
            token,
            argv,
            cwd,
            cancelled: false,
        };
        self.save(&request)?;
        Ok(request)
    }

    fn cancel(&self, token: &str) -> Result<(), OperatorError> {
        self.cancel_where(|request| request.token == token)
    }

    fn cancel_all(&self) -> Result<(), OperatorError> {
        self.cancel_where(|_| true)
    }

    fn pending(&self) -> Result<Vec<OperatorRequest>, OperatorError> {
        Ok(self
            .requests()?
            .into_iter()
            .filter(|request| !request.cancelled && !self.outcome_path(&request.token).exists())
            .collect())
    }

    fn answer(&self, request: &OperatorRequest, outcome: &Outcome) -> Result<(), OperatorError> {
        self.write_atomically(
            &self.outcome_path(&request.token),
            &serde_json::to_string_pretty(outcome)?,
        )
    }

    fn outcome(&self, token: &str) -> Result<Option<Outcome>, OperatorError> {
        match fs::read_to_string(self.outcome_path(token)) {
            Ok(text) => Ok(Some(serde_json::from_str(&text)?)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

/// Receives requests for as long as one agent runs.
///
/// Dropping it stops listening and withdraws every unanswered request, because
/// no process is left to receive the answer.
#[derive(Debug)]
pub struct OperatorInbox {
    store: FileOperatorRequests,
    task: JoinHandle<()>,
}

impl OperatorInbox {
    /// Must be called inside a Tokio runtime.
    pub fn open(directory: &Path) -> Result<Self, OperatorError> {
        fs::create_dir_all(directory)?;
        let store = FileOperatorRequests::new(directory);
        store.cancel_all()?;
        let inbox = store.inbox();
        create_pipe(&inbox)?;
        let receiver = open_receiver(&inbox)?;
        let listener = store.clone();
        let task = tokio::spawn(async move {
            let mut receiver = Some(receiver);
            loop {
                let Some(pipe) = receiver.take().or_else(|| open_receiver(&inbox).ok()) else {
                    return;
                };
                let mut lines = BufReader::new(pipe).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // A rejected line has no recipient to report to; the asker times out.
                    let _ = receive(&listener, &line);
                }
                // Only reached where the pipe cannot be held open for writing as well.
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        });
        Ok(Self { store, task })
    }

    pub fn directory(&self) -> &Path {
        &self.store.directory
    }

    pub fn pending(&self) -> Result<Vec<OperatorRequest>, OperatorError> {
        self.store.pending()
    }
}

impl Drop for OperatorInbox {
    fn drop(&mut self) {
        self.task.abort();
        let _ = self.store.cancel_all();
    }
}

fn create_pipe(path: &Path) -> Result<(), OperatorError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.file_type().is_fifo() => return Ok(()),
        Ok(_) => {
            return Err(OperatorError::Invalid(format!(
                "{} exists and is not a named pipe",
                path.display()
            )));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let status = std::process::Command::new("mkfifo")
        .args(["-m", "600"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(OperatorError::Invalid(format!(
            "mkfifo could not create {}",
            path.display()
        )))
    }
}

fn open_receiver(path: &Path) -> io::Result<pipe::Receiver> {
    let mut options = pipe::OpenOptions::new();
    // Holding a write end keeps the pipe open between asking processes.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    options.read_write(true);
    options.open_receiver(path)
}

async fn send(store: &FileOperatorRequests, message: &Message) -> Result<(), OperatorError> {
    let mut line = serde_json::to_string(message)?;
    line.push('\n');
    if line.len() > MAX_MESSAGE_BYTES {
        return Err(OperatorError::Invalid(
            "the command is too long; put it in a script file and ask to run that file".into(),
        ));
    }
    let mut sender = pipe::OpenOptions::new()
        .open_sender(store.inbox())
        .map_err(|_| {
            OperatorError::Invalid(
                "no miau is listening for operator requests in this session".into(),
            )
        })?;
    sender.write_all(line.as_bytes()).await?;
    Ok(())
}

/// Resolves the request directory from an explicit path or the environment.
pub fn directory(explicit: Option<PathBuf>) -> Result<PathBuf, OperatorError> {
    explicit
        .or_else(|| std::env::var_os(DIR_ENV).map(PathBuf::from))
        .ok_or_else(|| {
            OperatorError::Invalid(format!(
                "{DIR_ENV} is not set; this process was not started by miau"
            ))
        })
}

/// Agent side: asks the operator to run `argv` and waits for the answer.
///
/// Returns the exit code for the asking process and writes the command output,
/// or the reason nothing ran, to `output`.
pub async fn ask(
    directory: Option<PathBuf>,
    argv: Vec<String>,
    timeout: Duration,
    output: &mut impl Write,
) -> io::Result<i32> {
    let store = match directory_store(directory) {
        Ok(store) => store,
        Err(error) => return unavailable(output, &error),
    };
    let token = format!(
        "{:x}x{:x}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos())
    );
    let message = Message::Ask {
        token: token.clone(),
        argv,
        cwd: std::env::current_dir()?,
    };
    if let Err(error) = send(&store, &message).await {
        return unavailable(output, &error);
    }
    let answer = tokio::select! {
        answer = wait_for_outcome(&store, &token, timeout) => answer,
        () = interrupted() => Ok(None),
    };
    match answer {
        Ok(Some(Outcome::Exited { code, log })) => {
            replay(&log, output)?;
            match code {
                Some(code) => Ok(code),
                None => {
                    writeln!(output, "miau: the command was terminated by a signal.")?;
                    Ok(EXIT_UNAVAILABLE)
                }
            }
        }
        Ok(Some(Outcome::Denied { reason })) => {
            writeln!(
                output,
                "miau: the operator refused this command; it did not run. Do not retry it or work around it.\nReason: {reason}"
            )?;
            Ok(EXIT_DENIED)
        }
        Ok(Some(Outcome::Failed { message })) => {
            writeln!(output, "miau: the command could not be started: {message}")?;
            Ok(EXIT_UNAVAILABLE)
        }
        Ok(None) => {
            let _ = send(&store, &Message::Cancel { token }).await;
            writeln!(
                output,
                "miau: the operator did not answer within {} seconds; the command did not run. \
                 Record the need as an open decision in the artifact instead of retrying.",
                timeout.as_secs()
            )?;
            Ok(EXIT_TIMEOUT)
        }
        Err(error) => unavailable(output, &error),
    }
}

fn directory_store(explicit: Option<PathBuf>) -> Result<FileOperatorRequests, OperatorError> {
    directory(explicit).map(FileOperatorRequests::new)
}

fn unavailable(output: &mut impl Write, error: &OperatorError) -> io::Result<i32> {
    writeln!(
        output,
        "miau: the operator could not be asked ({error}); the command did not run. \
         Record the need as an open decision in the artifact."
    )?;
    Ok(EXIT_UNAVAILABLE)
}

async fn wait_for_outcome(
    store: &FileOperatorRequests,
    token: &str,
    timeout: Duration,
) -> Result<Option<Outcome>, OperatorError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(outcome) = store.outcome(token)? {
            return Ok(Some(outcome));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Resolves when the agent's tool runner stops waiting for this process.
async fn interrupted() {
    let (Ok(mut terminate), Ok(mut interrupt), Ok(mut hangup)) = (
        signal(SignalKind::terminate()),
        signal(SignalKind::interrupt()),
        signal(SignalKind::hangup()),
    ) else {
        return std::future::pending().await;
    };
    tokio::select! {
        _ = terminate.recv() => {}
        _ = interrupt.recv() => {}
        _ = hangup.recv() => {}
    }
}

fn replay(log: &Path, output: &mut impl Write) -> io::Result<()> {
    let bytes = fs::read(log)?;
    let skipped = bytes.len().saturating_sub(MAX_REPLAYED_OUTPUT_BYTES);
    if skipped > 0 {
        writeln!(
            output,
            "miau: output truncated to its last {MAX_REPLAYED_OUTPUT_BYTES} bytes; the full output is in {}",
            log.display()
        )?;
    }
    output.write_all(String::from_utf8_lossy(&bytes[skipped..]).as_bytes())
}

/// Operator side: the requests that wait for an answer.
pub fn pending(directory: Option<PathBuf>) -> Result<Vec<OperatorRequest>, OperatorError> {
    directory_store(directory)?.pending()
}

/// Operator side: refuses a pending request.
pub fn deny(directory: Option<PathBuf>, id: u32, reason: String) -> Result<(), OperatorError> {
    let store = directory_store(directory)?;
    let request = find_pending(&store, id)?;
    store.answer(&request, &Outcome::Denied { reason })
}

/// Operator side: runs a pending request on the current terminal and sends
/// its output and exit code to the waiting agent.
///
/// The child inherits stdin so `sudo` can prompt; stdout and stderr are shown
/// and saved. Returns the command's exit code when it exited normally.
pub async fn run(directory: Option<PathBuf>, id: u32) -> Result<Option<i32>, OperatorError> {
    let store = directory_store(directory)?;
    let request = find_pending(&store, id)?;
    let log_path = store.log_path(request.id);
    let log = Arc::new(Mutex::new(fs::File::create(&log_path)?));
    writeln!(
        io::stderr(),
        "miau: running in {}: {}",
        request.cwd.display(),
        command_line(&request.argv)
    )?;
    let mut command = tokio::process::Command::new(&request.argv[0]);
    command
        .args(&request.argv[1..])
        .current_dir(&request.cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let message = error.to_string();
            store.answer(&request, &Outcome::Failed { message })?;
            return Err(error.into());
        }
    };
    // Ctrl+C reaches the child through the terminal; this process must survive
    // it to report the result, or the agent would wait for nothing.
    let mut interrupt = signal(SignalKind::interrupt())?;
    let stdout = child
        .stdout
        .take()
        .map(|pipe| tee(pipe, io::stdout(), log.clone()));
    let stderr = child
        .stderr
        .take()
        .map(|pipe| tee(pipe, io::stderr(), log.clone()));
    let status = loop {
        tokio::select! {
            status = child.wait() => break status?,
            _ = interrupt.recv() => {}
        }
    };
    for copier in [stdout, stderr].into_iter().flatten() {
        let _ = copier.await;
    }
    store.answer(
        &request,
        &Outcome::Exited {
            code: status.code(),
            log: log_path,
        },
    )?;
    Ok(status.code())
}

fn tee(
    mut source: impl AsyncRead + Unpin + Send + 'static,
    mut terminal: impl Write + Send + 'static,
    log: Arc<Mutex<fs::File>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buffer = [0_u8; 8192];
        while let Ok(count @ 1..) = source.read(&mut buffer).await {
            let _ = terminal.write_all(&buffer[..count]);
            let _ = terminal.flush();
            if let Ok(mut log) = log.lock() {
                let _ = log.write_all(&buffer[..count]);
            }
        }
    })
}
