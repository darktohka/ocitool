use crate::cleanup::cleanup_command;
use crate::client::{ImagePermission, ImagePermissions, OciClient};
use crate::compose::pull::pull_command;
use crate::compose::up::up_command;
use crate::downloader::IndexResponse;
use crate::parser::FullImageWithTag;
use crate::spec::manifest::ImageManifest;
use downloader::OciDownloaderError;
use platform::PlatformMatcher;
use runner::OciRunner;
use spec::plan::ImagePlan;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::Arc;
use walkdir::WalkDir;

mod access;
mod cleanup;
mod client;
mod compose;
mod digest;
mod downloader;
mod execution;
mod macros;
mod parser;
mod platform;
mod runner;
mod spec;
mod system_login;
mod uploader;
mod walk;
mod whiteout;

xflags::xflags! {
    /// Uploads an OCI image to a registry
    cmd ocitool {
        /// Sets the username to authenticate to the registry with
        /// If not set, the DOCKER_USERNAME environment variable will be used
        optional -u, --username username: String

        /// Sets the password to authenticate to the registry with
        /// If not set, the DOCKER_PASSWORD environment variable will be used
        optional -p, --password password: String

        /// Disables the on-disk cache
        optional --no-cache

        cmd compose {
            /// Sets the path to the compose directory
            /// If not set, the current directory will be used
            optional -d,--dir dir: PathBuf

            /// Sets the maximum depth to search for docker-compose files
            /// If not set, the default is 1
            optional -m,--max-depth max_depth: usize

            /// Pulls all images from the respective registries
            cmd pull {

            }

            /// Creates the necessary networks
            cmd up {

            }
        }

        cmd upload {
            /// Sets a custom plan filename to use
            optional --plan plan: String

            /// Sets the compression level to use when compressing layers
            /// If not set, the COMPRESSION_LEVEL environment variable will be used
            /// If that is not set, the default compression level will be used
            /// The compression level must be between 1 and 22
            optional -c, --compression-level compression_level: i32
        }

        cmd run {
            /// Sets the image name to run
            required -i,--image image: String

            /// Volumes to mount in the container
            repeated -v,--volume volumes: String

            /// Optional entrypoint to use
            optional -e,--entrypoint entrypoint: String

            /// Optional command to run
            optional -c,--cmd cmd: String

            /// Optional working directory
            optional -w,--workdir workdir: String

            /// Disables mounting the system directories (/proc, /sys, /dev)
            optional --no-mount-system

            /// Disables ensuring the DNS configuration
            optional --no-ensure-dns
        }

        /// Cleans up dangling data in a Docker registry server
        /// Removes dangling commit hashes, indexes, layers, and blobs
        cmd cleanup {
            /// The directory that contains the Docker registry data
            /// that is to be cleaned up
            required -d,--dir dir: PathBuf

            /// Remove dangling commit hashes
            optional --commits

            /// Remove dangling indexes
            optional --indexes

            /// Remove dangling layers
            optional --layers

            /// Remove dangling blobs
            optional --blobs

            /// Cleanup everything
            optional -a,--all

            /// Agree to the cleanup without prompting
            optional -y,--yes
        }
}
}

async fn upload_command(
    args: &Upload,
    no_cache: bool,
    username: Option<String>,
    password: Option<String>,
) {
    let compression_level = args.compression_level.unwrap_or_else(|| {
        env::var("COMPRESSION_LEVEL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(19)
    });

    let plan = args.plan.clone().unwrap_or_else(|| "oci.json".to_string());
    let plan_path = Path::new(&plan);
    let plan = if plan_path.exists() {
        plan
    } else {
        let plan_basename = plan_path.file_name().expect("Invalid plan filename");
        WalkDir::new(env::current_dir().unwrap())
            .into_iter()
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.file_name() == plan_basename)
            .expect("Plan file not found")
            .into_path()
            .to_str()
            .unwrap()
            .to_string()
    };

    println!("Executing plan: {}", plan);

    // Set the current directory to the plan file's directory
    if let Some(parent) = Path::new(&plan).parent() {
        if parent.exists() {
            env::set_current_dir(parent).expect("Failed to set current directory");
        }
    }

    let file = File::open(plan).expect("Failed to open plan file");
    let plan: ImagePlan = serde_json::from_reader(file).unwrap();
    let default_credentials = match (username, password) {
        (Some(user), Some(pass)) => Some(client::LoginCredentials {
            username: user,
            password: pass,
        }),
        _ => None,
    };

    let client = Arc::new(OciClient::new(HashMap::new(), default_credentials));
    let mut execution = execution::PlanExecution::new(plan, client, no_cache, compression_level);

    if let Err(e) = execution.execute().await {
        eprintln!("Error: {}", e);
        exit(1);
    }
}

async fn run_command(
    args: &Run,
    no_cache: bool,
    username: Option<String>,
    password: Option<String>,
) {
    let image_name = args.image.clone();
    let volumes = args.volume.clone();
    let entrypoint = args.entrypoint.clone();
    let cmd = args.cmd.clone();
    let workdir = args.workdir.clone();

    let image = FullImageWithTag::from_image_name(&image_name);

    let default_credentials = match (username, password) {
        (Some(user), Some(pass)) => Some(client::LoginCredentials {
            username: user,
            password: pass,
        }),
        _ => None,
    };
    let client = Arc::new(OciClient::new(HashMap::new(), default_credentials));

    client
        .login(&[ImagePermission {
            full_image: image.image.clone(),
            permissions: ImagePermissions::Pull,
        }])
        .await
        .map_err(|e| OciDownloaderError(format!("Failed to login to registry: {}", e)))
        .unwrap();

    let downloader = downloader::OciDownloader::new(client, no_cache);

    let index = downloader.download_index(image.clone()).await.unwrap().0;

    let platform_matcher = PlatformMatcher::new();

    let downloaded_manifest = match index {
        IndexResponse::ImageIndex(index) => {
            let manifest = platform_matcher
                .find_manifest(&index.manifests)
                .ok_or(OciDownloaderError("No matching platform found".to_string()))
                .unwrap();

            let downloaded_manifest = downloader
                .download_manifest(image.image.clone(), &manifest.digest)
                .await
                .unwrap()
                .0;

            Ok::<ImageManifest, OciDownloaderError>(downloaded_manifest)
        }
        IndexResponse::ImageManifest(index) => Ok(index),
    }
    .unwrap();

    let downloaded_config = downloader
        .download_config(image.image.clone(), &downloaded_manifest.config.digest)
        .await
        .unwrap()
        .0;

    let tmpdir = tempfile::tempdir().unwrap();
    let tmpdir_path = tmpdir.path();

    for layer in downloaded_manifest.layers {
        downloader
            .extract_layer(
                image.image.clone(),
                &layer.digest,
                &layer.media_type,
                &tmpdir_path.to_path_buf(),
            )
            .await
            .expect("Failed to extract layer");
    }

    let runner = OciRunner::new(
        tmpdir_path,
        &downloaded_config.config,
        volumes,
        entrypoint,
        cmd,
        workdir,
        !args.no_mount_system,
        !args.no_ensure_dns,
    );

    runner.run().await.expect("Failed to run command");
}

#[tokio::main]
async fn main() {
    let args = Ocitool::from_env_or_exit();

    let username = args
        .username
        .map(|s| s.to_string())
        .or_else(|| env::var("DOCKER_USERNAME").ok());
    let password = args.password.map(|s| s.to_string());

    match args.subcommand {
        OcitoolCmd::Upload(upload) => {
            upload_command(&upload, args.no_cache, username, password).await
        }
        OcitoolCmd::Run(run) => run_command(&run, args.no_cache, username, password).await,
        OcitoolCmd::Cleanup(cleanup) => {
            if let Err(e) = cleanup_command(cleanup) {
                eprintln!("Cleanup error: {}", e);
                exit(1);
            }
        }
        OcitoolCmd::Compose(ref compose) => match compose.subcommand {
            ComposeCmd::Pull(ref _pull) => {
                if let Err(e) = pull_command(&compose).await {
                    eprintln!("Pull error: {}", e);
                    exit(1);
                }
            }
            ComposeCmd::Up(ref _up) => {
                if let Err(e) = up_command(&compose).await {
                    eprintln!("Up error: {}", e);
                    exit(1);
                }
            }
        },
    }
}
