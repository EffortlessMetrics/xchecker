use crate::ring_buffer::RingBuffer;
use std::io;
use std::process::ExitStatus;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, ChildStderr, ChildStdout};
use tokio::time::timeout;

#[derive(Debug)]
pub(crate) enum PipeReadError {
    Stdout(io::Error),
    Stderr(io::Error),
    Wait(io::Error),
}

pub(crate) async fn read_pipes_until_exit(
    child: &mut Child,
    stdout_pipe: &mut ChildStdout,
    stderr_pipe: &mut ChildStderr,
    stdout_buffer: &mut RingBuffer,
    stderr_buffer: &mut RingBuffer,
) -> Result<ExitStatus, PipeReadError> {
    let mut stdout_buf = vec![0u8; 8192];
    let mut stderr_buf = vec![0u8; 8192];

    loop {
        tokio::select! {
            stdout_result = stdout_pipe.read(&mut stdout_buf) => {
                match stdout_result {
                    Ok(0) => break, // EOF
                    Ok(n) => stdout_buffer.write(&stdout_buf[..n]),
                    Err(err) => return Err(PipeReadError::Stdout(err)),
                }
            }
            stderr_result = stderr_pipe.read(&mut stderr_buf) => {
                match stderr_result {
                    Ok(0) => {}, // EOF on stderr, continue reading stdout
                    Ok(n) => stderr_buffer.write(&stderr_buf[..n]),
                    Err(err) => return Err(PipeReadError::Stderr(err)),
                }
            }
        }
    }

    let status = child.wait().await.map_err(PipeReadError::Wait)?;
    Ok(status)
}

pub(crate) async fn drain_pipes(
    stdout_pipe: &mut ChildStdout,
    stderr_pipe: &mut ChildStderr,
    stdout_buffer: &mut RingBuffer,
    stderr_buffer: &mut RingBuffer,
) -> io::Result<()> {
    let mut stdout_buf = vec![0u8; 8192];
    let mut stderr_buf = vec![0u8; 8192];

    // Try to drain for a short time
    let drain_timeout = Duration::from_millis(100);
    let _ = timeout(drain_timeout, async {
        loop {
            tokio::select! {
                stdout_result = stdout_pipe.read(&mut stdout_buf) => {
                    match stdout_result {
                        Ok(0) => break,
                        Ok(n) => stdout_buffer.write(&stdout_buf[..n]),
                        Err(_) => break,
                    }
                }
                stderr_result = stderr_pipe.read(&mut stderr_buf) => {
                    match stderr_result {
                        Ok(0) => {},
                        Ok(n) => stderr_buffer.write(&stderr_buf[..n]),
                        Err(_) => {},
                    }
                }
            }
        }
    })
    .await;

    Ok(())
}
