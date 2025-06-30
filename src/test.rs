#[cfg(test)]
pub mod tests {
    use std::{
        error::Error,
        io::{self},
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        sync::Arc,
        time::Duration,
    };

    use tempfile::tempdir;
    use tokio::time::timeout;

    use crate::compose::lease::LeasedClient;
    use std::fs::File;
    use std::io::Write;

    pub struct ContainerdTestEnv {
        _root: tempfile::TempDir,
        _state: tempfile::TempDir,
        pub socket_path: PathBuf,
        containerd_process: Child,
    }

    impl ContainerdTestEnv {
        pub async fn new() -> Result<Self, Box<dyn Error>> {
            // Create temporary directories for containerd root and state
            let root = tempdir()?;
            let state = tempdir()?;
            let socket_path = root.path().join("containerd.sock");
            let ttrpc_socket_path = root.path().join("ttrpc.sock");

            // Create a configuration file for containerd
            let config_path = root.path().join("config.toml");
            let mut config_file = File::create(&config_path)?;
            writeln!(
                config_file,
                r#"disabled_plugins = ["io.containerd.ttrpc.v1.otelttrpc", "io.containerd.grpc.v1.healthcheck", "io.containerd.grpc.v1.cri", "io.containerd.nri.v1.nri"]

                [ttrpc]
                address = "{}"
                uid = {}
                gid = {}
            "#,
                ttrpc_socket_path.to_str().unwrap(),
                nix::unistd::getuid().as_raw(),
                nix::unistd::getgid().as_raw()
            )?;

            // Pass the configuration file to containerd
            let process = Command::new("sudo")
                .arg("containerd")
                .arg("--config")
                .arg(config_path)
                .arg("--root")
                .arg(root.path())
                .arg("--state")
                .arg(state.path())
                .arg("--address")
                .arg(socket_path.to_str().unwrap())
                .stderr(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            // Wait for containerd to create the socket
            wait_for_file(socket_path.to_str().unwrap()).await.unwrap();

            Ok(Self {
                _root: root,
                _state: state,
                socket_path,
                containerd_process: process,
            })
        }
    }

    impl Drop for ContainerdTestEnv {
        fn drop(&mut self) {
            self.containerd_process.kill().unwrap();
        }
    }

    pub async fn create_test_client(
        socket_path: &PathBuf,
    ) -> Result<Arc<LeasedClient>, Box<dyn Error>> {
        let client =
            LeasedClient::with_path("test".to_string(), socket_path.to_str().unwrap()).await?;
        Ok(Arc::new(client))
    }

    pub async fn wait_for_file(path: &str) -> Result<(), io::Error> {
        let path = path.to_string();

        let check_file = async {
            loop {
                if Path::new(&path).exists() {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };

        match timeout(Duration::from_secs(2), check_file).await {
            Ok(result) => result,
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "File not created within 2 seconds",
            )),
        }
    }

    #[tokio::test]
    async fn test_tempdir_file_creation_and_reading() -> Result<(), Box<dyn Error>> {
        // Create a temporary directory
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("test_file.txt");

        // Write some content to a file in the temporary directory
        let mut file = File::create(&file_path)?;
        writeln!(file, "Hello, world!")?;

        // Read the content back from the file
        let content = tokio::fs::read_to_string(&file_path).await?;
        assert_eq!(content.trim(), "Hello, world!");

        Ok(())
    }

    #[tokio::test]
    async fn test_containerd_version_fetch() -> Result<(), Box<dyn Error>> {
        // Initialize the ContainerdTestEnv
        let env = ContainerdTestEnv::new().await?;

        // Create a test client using the socket path
        let client = create_test_client(&env.socket_path).await?;

        // Fetch the containerd version using the client
        client.client().version().version({}).await?;
        Ok(())
    }
}
