use std::path::Path;

use crate::{
    macros::{impl_error, impl_from_error},
    spec::config::Config,
};

impl_error!(OciRunnerError);
impl_from_error!(std::io::Error, OciRunnerError);

pub struct OciRunner<'a> {
    dir: &'a Path,
    config: &'a Option<Config>,
    volumes: Vec<String>,
    entrypoint: Option<String>,
    cmd: Option<String>,
    workdir: Option<String>,
    mount_system: bool,
}

impl<'a> OciRunner<'a> {
    pub fn new(
        dir: &'a Path,
        config: &'a Option<Config>,
        volumes: Vec<String>,
        entrypoint: Option<String>,
        cmd: Option<String>,
        workdir: Option<String>,
        mount_system: bool,
    ) -> Self {
        OciRunner {
            dir,
            config,
            volumes,
            entrypoint,
            cmd,
            workdir,
            mount_system,
        }
    }

    pub async fn run(&self) -> Result<(), OciRunnerError> {
        let proot = which::which("proot")
            .or_else(|_| Err(OciRunnerError("proot not found in PATH".to_string())))?;

        let mut command = tokio::process::Command::new(proot);

        command.arg("-r").arg(self.dir);

        if self.mount_system {
            command.arg("-b").arg("/dev:/dev");
            command.arg("-b").arg("/proc:/proc");
            command.arg("-b").arg("/sys:/sys");
        }

        for volume in &self.volumes {
            let parts: Vec<&str> = volume.split(':').collect();

            if parts.len() != 2 {
                eprintln!("Invalid volume format: {}", volume);
                std::process::exit(1);
            }

            command.arg("-b").arg(format!("{}:{}", parts[0], parts[1]));
        }

        if let Some(workdir) = &self.workdir {
            command.arg("-w").arg(workdir);
        } else if let Some(config) = &self.config {
            if let Some(workdir) = &config.working_dir {
                command.arg("-w").arg(workdir);
            }
        }

        if let Some(entrypoint) = &self.entrypoint {
            command.arg(entrypoint);
        } else if let Some(config) = &self.config {
            if let Some(entrypoints) = &config.entrypoint {
                for arg in entrypoints {
                    command.arg(arg);
                }
            }
        }

        if let Some(cmd) = &self.cmd {
            command.arg(cmd);
        } else if let Some(config) = &self.config {
            if let Some(cmd) = &config.cmd {
                for arg in cmd {
                    command.arg(arg);
                }
            }
        }

        let status = command.status().await?;

        if !status.success() {
            return Err(OciRunnerError(format!(
                "Command exited with status: {}",
                status
            )));
        }

        Ok(())
    }
}
