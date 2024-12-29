use std::io::{Read, Write};
use std::process::Stdio;

use anyhow::{Context as _, Result};
use bitflags::bitflags;
use fift::core::*;

pub struct ShellUtils;

#[fift_module]
impl ShellUtils {
    // runshell (cmd:string args:tuple(string...) -- exit_code:int)
    // runshellx (cmd:string args:tuple(string...) [stdin:string] mode:int -- [stdout:string/bytes] [stderr:string] exit_code:int)
    #[cmd(name = "runshell", stack, args(mode = Some(ShellMode::DEFAULT)))]
    #[cmd(name = "runshellx", stack, args(mode = None))]
    fn interpret_run_shell(stack: &mut Stack, mode: Option<ShellMode>) -> Result<()> {
        let mode = match mode {
            Some(m) => m,
            None => ShellMode::from_bits_retain(stack.pop_smallint_range(0, 7)? as u8),
        };

        let mut stdin = None;
        let (stdin_descr, stdin) = if mode.contains(ShellMode::WRITE_STDIN) {
            let value = stdin.insert(stack.pop()?);
            let value = if mode.contains(ShellMode::STDIN_AS_BYTES) {
                value.as_bytes()?
            } else {
                value.as_string()?.as_bytes()
            };

            (Stdio::piped(), value)
        } else {
            (Stdio::null(), [].as_slice())
        };

        let args = stack.pop_tuple()?;
        let args = args
            .iter()
            .map(|arg| arg.as_string())
            .collect::<Result<Vec<_>>>()?;

        let cmd = stack.pop_string()?;

        let mut child = std::process::Command::new(cmd.as_ref())
            .args(args)
            .stdin(stdin_descr)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn a child process")?;

        if let Some(mut child_stdin) = child.stdin.take() {
            child_stdin
                .write_all(stdin)
                .context("Failed to write to stdin")?;
        }

        let exit_code = child
            .wait()?
            .code()
            .context("The child process was terminated by signal")?;

        if mode.contains(ShellMode::READ_STDOUT) {
            let mut bytes = Vec::new();
            if let Some(mut stdout) = child.stdout.take() {
                stdout.read_to_end(&mut bytes)?;
            }
            if mode.contains(ShellMode::STDOUT_AS_BYTES) {
                stack.push(bytes)?;
            } else {
                stack.push(String::from_utf8_lossy(&bytes).to_string())?;
            }
        }

        if mode.contains(ShellMode::READ_STDERR) {
            let mut bytes = Vec::new();
            if let Some(mut stderr) = child.stderr.take() {
                stderr.read_to_end(&mut bytes)?;
            }
            stack.push(String::from_utf8_lossy(&bytes).to_string())?;
        }

        stack.push_int(exit_code)
    }
}

bitflags! {
    struct ShellMode: u8 {
        /// +1 = use stdin as string from stack (empty otherwise)
        const WRITE_STDIN = 1;
        /// +2 = push stdout as string on stack after execution
        const READ_STDOUT = 2;
        /// +4 = push stderr as string on stack after execution
        const READ_STDERR = 4;
        /// +8 = if stdin is present it is required to be bytes
        const STDIN_AS_BYTES = 8;
        /// +16 = if stdout is present it is required to be bytes
        const STDOUT_AS_BYTES = 16;

        const DEFAULT = 0;
    }
}
