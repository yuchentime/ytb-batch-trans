use std::io::{self, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::watch;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[cfg(windows)]
use core::ffi::c_void;

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};

#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
  AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
  SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
  JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

#[cfg(windows)]
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

#[derive(Debug)]
pub struct PlatformProcess {
  #[cfg(unix)]
  pub pgid: Option<i32>,
  #[cfg(windows)]
  pub job: Option<JobHandle>,
}

#[cfg(windows)]
#[derive(Debug)]
pub struct JobHandle {
  handle: isize,
}

#[cfg(windows)]
impl JobHandle {
  #[inline]
  fn as_handle(&self) -> HANDLE {
    self.handle as HANDLE
  }
}

#[cfg(windows)]
impl Drop for JobHandle {
  fn drop(&mut self) {
    if self.handle != 0 {
      unsafe { CloseHandle(self.as_handle()) };
    }
  }
}

pub fn configure_command(command: &mut Command) -> io::Result<()> {
  #[cfg(unix)]
  {
    unsafe {
      command.pre_exec(|| {
        let result = libc::setpgid(0, 0);
        if result == 0 {
          Ok(())
        } else {
          Err(io::Error::last_os_error())
        }
      });
    }
  }

  #[cfg(windows)]
  {
    command.creation_flags(CREATE_NO_WINDOW);
  }

  Ok(())
}

pub fn platform_process_from_child(child: &std::process::Child) -> Result<PlatformProcess, String> {
  #[cfg(unix)]
  {
    Ok(PlatformProcess {
      pgid: Some(child.id() as i32),
    })
  }

  #[cfg(windows)]
  {
    let job = create_job_for_child(child)
      .map(Some)
      .map_err(|e| format!("yt-dlp job object error: {e}"))?;
    Ok(PlatformProcess { job })
  }

  #[cfg(not(any(unix, windows)))]
  {
    let _ = child;
    Ok(PlatformProcess {})
  }
}

pub fn kill_platform_process(platform: &PlatformProcess) {
  #[cfg(unix)]
  {
    if let Some(pgid) = platform.pgid {
      unsafe {
        libc::killpg(pgid, libc::SIGTERM);
      }
    }
  }

  #[cfg(windows)]
  {
    if let Some(job) = &platform.job {
      unsafe { TerminateJobObject(job.as_handle(), 1) };
    }
  }
}

#[cfg(windows)]
fn create_job_for_child(child: &std::process::Child) -> io::Result<JobHandle> {
  let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null::<u16>()) };
  if handle.is_null() {
    return Err(io::Error::last_os_error());
  }

  let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
  info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

  let info_ptr = (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION) as *const c_void;
  let info_size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;

  let set_ok = unsafe {
    SetInformationJobObject(
      handle,
      JobObjectExtendedLimitInformation,
      info_ptr,
      info_size,
    )
  };
  if set_ok == 0 {
    unsafe { CloseHandle(handle) };
    return Err(io::Error::last_os_error());
  }

  let process_handle = child.as_raw_handle() as HANDLE;
  let assign_ok = unsafe { AssignProcessToJobObject(handle, process_handle) };
  if assign_ok == 0 {
    unsafe { CloseHandle(handle) };
    return Err(io::Error::last_os_error());
  }

  Ok(JobHandle {
    handle: handle as isize,
  })
}

/// Prepends the app's `bin_dir` to `PATH` so child processes resolve the managed tools first.
pub fn prepend_bin_dir_to_path(command: &mut Command, bin_dir: &Path) {
  let separator = if cfg!(windows) { ';' } else { ':' };
  let path_env = std::env::var("PATH").unwrap_or_default();
  let new_path = format!("{}{}{}", bin_dir.display(), separator, path_env);
  command.env("PATH", new_path);
}

#[derive(Debug, Clone, Copy)]
pub struct TerminatedPayload {
  pub code: Option<i32>,
}

#[derive(Debug, Clone)]
pub enum ProcessEvent {
  Stdout(Vec<u8>),
  Stderr(Vec<u8>),
  Terminated(TerminatedPayload),
  Error(String),
}

/// A running child process. `kill_tree` uses the platform process group / Job Object, the
/// same cancellation semantics as the yt-dlp runner (W001).
pub struct PipedProcess {
  platform: PlatformProcess,
}

impl PipedProcess {
  pub fn kill_tree(&self) -> Result<(), String> {
    kill_platform_process(&self.platform);
    Ok(())
  }
}

/// Spawns `command` with piped stdio, a hidden window and a kill-on-close process group.
pub fn spawn_piped(
  mut command: Command,
) -> Result<(UnboundedReceiver<ProcessEvent>, PipedProcess), String> {
  command
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  configure_command(&mut command)
    .map_err(|error| format!("process spawn setup failed: {error}"))?;

  let mut raw_child = command
    .spawn()
    .map_err(|error| format!("process failed to spawn: {error}"))?;
  let stdout = raw_child.stdout.take();
  let stderr = raw_child.stderr.take();
  let platform = match platform_process_from_child(&raw_child) {
    Ok(platform) => platform,
    Err(error) => {
      let _ = raw_child.kill();
      return Err(error);
    }
  };

  let (tx, rx) = unbounded_channel();

  if let Some(stdout) = stdout {
    spawn_reader(stdout, tx.clone(), true);
  }
  if let Some(stderr) = stderr {
    spawn_reader(stderr, tx.clone(), false);
  }

  let wait_tx = tx.clone();
  thread::spawn(move || match raw_child.wait() {
    Ok(status) => {
      let payload = TerminatedPayload {
        code: status.code(),
      };
      let _ = wait_tx.send(ProcessEvent::Terminated(payload));
    }
    Err(error) => {
      let _ = wait_tx.send(ProcessEvent::Error(error.to_string()));
    }
  });

  Ok((rx, PipedProcess { platform }))
}

fn spawn_reader<R: Read + Send + 'static>(
  reader: R,
  tx: UnboundedSender<ProcessEvent>,
  is_stdout: bool,
) {
  thread::spawn(move || {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    let mut byte = [0_u8; 1];

    loop {
      match reader.read(&mut byte) {
        Ok(0) => {
          if !buf.is_empty() {
            send_line(&tx, is_stdout, &mut buf);
          }
          break;
        }
        Ok(_) if matches!(byte[0], b'\n' | b'\r') => {
          if !buf.is_empty() {
            send_line(&tx, is_stdout, &mut buf);
          }
        }
        Ok(_) => buf.push(byte[0]),
        Err(error) => {
          let _ = tx.send(ProcessEvent::Error(error.to_string()));
          break;
        }
      }
    }
  });
}

fn send_line(tx: &UnboundedSender<ProcessEvent>, is_stdout: bool, buf: &mut Vec<u8>) {
  let out = std::mem::take(buf);
  let event = if is_stdout {
    ProcessEvent::Stdout(out)
  } else {
    ProcessEvent::Stderr(out)
  };
  let _ = tx.send(event);
}

#[derive(Debug, Default)]
pub struct ProcessResult {
  pub code: Option<i32>,
  pub stdout: String,
  pub stderr_tail: Vec<String>,
  pub cancelled: bool,
  pub reader_error: Option<String>,
}

/// Runs `command` to completion while streaming lines to the callbacks (stdout is also
/// collected in full; stderr keeps only the last 64 lines).
///
/// `cancel` is the group-state watch channel: a `true` value kills the process tree and
/// returns a cancelled result (AC-16). Callers must also check the initial value before
/// calling, because a value that was already `true` never produces a change.
pub async fn run_streaming<F, G>(
  command: Command,
  mut cancel: watch::Receiver<bool>,
  mut on_stdout: F,
  mut on_stderr: G,
) -> Result<ProcessResult, String>
where
  F: FnMut(&str),
  G: FnMut(&str),
{
  const STDERR_TAIL_LINES: usize = 64;

  if *cancel.borrow() {
    return Ok(ProcessResult {
      cancelled: true,
      ..ProcessResult::default()
    });
  }

  let (mut rx, process) = spawn_piped(command)?;
  let mut result = ProcessResult::default();
  let mut watch_cancel = true;

  loop {
    let event = if watch_cancel {
      tokio::select! {
        event = rx.recv() => event,
        changed = cancel.changed() => {
          if changed.is_ok() {
            let _ = process.kill_tree();
            result.cancelled = true;
            break;
          }
          watch_cancel = false;
          continue;
        }
      }
    } else {
      rx.recv().await
    };

    let Some(event) = event else {
      break;
    };

    match event {
      ProcessEvent::Stdout(bytes) => {
        let line = String::from_utf8_lossy(&bytes).into_owned();
        on_stdout(&line);
        result.stdout.push_str(&line);
        result.stdout.push('\n');
      }
      ProcessEvent::Stderr(bytes) => {
        let line = String::from_utf8_lossy(&bytes).into_owned();
        on_stderr(&line);
        if result.stderr_tail.len() >= STDERR_TAIL_LINES {
          result.stderr_tail.remove(0);
        }
        result.stderr_tail.push(line);
      }
      ProcessEvent::Terminated(payload) => result.code = payload.code,
      ProcessEvent::Error(error) => result.reader_error = Some(error),
    }
  }

  Ok(result)
}

/// Joins the last `count` lines of a captured stderr into a single message-safe excerpt.
pub fn tail_excerpt(lines: &[String], count: usize) -> String {
  let start = lines.len().saturating_sub(count);
  lines[start..].join(" | ")
}
