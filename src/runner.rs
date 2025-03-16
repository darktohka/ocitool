use std::path::Path;

use tokio::{
    fs::{create_dir_all, File},
    io::AsyncWriteExt,
};

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
    ensure_dns: bool,
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
        ensure_dns: bool,
    ) -> Self {
        OciRunner {
            dir,
            config,
            volumes,
            entrypoint,
            cmd,
            workdir,
            mount_system,
            ensure_dns,
        }
    }

    pub async fn run(&self) -> Result<(), OciRunnerError> {
        if self.ensure_dns {
            let etc = self.dir.join("etc");
            create_dir_all(etc.clone()).await?;

            let resolv_conf = etc.join("resolv.conf");
            let mut resolv_conf_file = File::create(resolv_conf).await?;

            resolv_conf_file
                .write_all(b"nameserver 8.8.8.8\nnameserver 8.8.4.4\n")
                .await?;
        }

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
            for arg in cmd.split_whitespace() {
                command.arg(arg);
            }
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
