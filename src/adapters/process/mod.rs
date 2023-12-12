use std::{
    process::Stdio,
    time::{Duration, SystemTime},
};

use tokio::io::{AsyncBufReadExt, BufReader, BufWriter};
use tokio::{
    io::AsyncWriteExt,
    process::{Child, Command},
};

#[derive(Debug)]
pub struct Process {
    pub process_id: String,
    pub child: Child,
    pub start_time: SystemTime,
}

impl Process {
    pub fn new(command_str: &str, args: &Vec<&str>) -> anyhow::Result<Self> {
        let child = Command::new(command_str)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let process_id = rand::random::<usize>().to_string();
        let start_time = SystemTime::now();
        Ok(Self {
            process_id,
            child,
            start_time,
        })
    }

    pub async fn read(&mut self) -> anyhow::Result<(String, String)> {
        let mut stdout_string = String::default();
        let mut stderr_string = String::default();

        if let Some(child_stdout) = self.child.stdout.take() {
            let mut reader = BufReader::new(child_stdout).lines();
            let mut lines = vec![];
            while let Some(line) = reader.next_line().await? {
                lines.push(line);
            }
            stdout_string = lines.join("\n");
        }

        if let Some(child_stderr) = self.child.stderr.take() {
            let mut reader = BufReader::new(child_stderr).lines();
            let mut lines = vec![];
            while let Some(line) = reader.next_line().await? {
                lines.push(line);
            }
            stderr_string = lines.join("\n");
        }

        Ok((stdout_string, stderr_string))
    }

    pub async fn send(&mut self, input_string: String) -> anyhow::Result<(String, String)> {
        if let Some(child_stdin) = self.child.stdin.take() {
            let mut writer = BufWriter::new(child_stdin);
            match writer.write(input_string.as_bytes()).await {
                Ok(_) => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.read().await
    }

    pub fn elapsed_time(&self) -> anyhow::Result<Duration> {
        let now = SystemTime::now();
        Ok(now.duration_since(self.start_time)?)
    }

    pub fn is_running(&self) -> bool {
        self.child.id().is_some()
    }

    pub async fn kill(&mut self) -> anyhow::Result<()> {
        Ok(self.child.kill().await?)
    }

    pub async fn wait_with_output(&mut self) -> anyhow::Result<(i32, String, String)> {
        let status = self.child.wait().await?.code().unwrap_or(99);
        let (stdout, stderr) = self.read().await?;
        Ok((status, stdout, stderr))
    }

    pub async fn wait(&mut self) -> anyhow::Result<i32> {
        Ok(self.child.wait().await?.code().unwrap_or(99))
    }
}
