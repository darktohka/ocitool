use downloader::OciDownloaderError;
use parser::ParsedImage;
use platform::PlatformMatcher;
use runner::OciRunner;
use spec::plan::ImagePlan;
use std::env;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use walkdir::WalkDir;
mod client;
mod digest;
mod downloader;
mod execution;
mod macros;
mod parser;
mod platform;
mod runner;
mod spec;
mod uploader;
mod walk;
mod whiteout;

xflags::xflags! {
    /// Uploads an OCI image to a registry
    cmd ocitool {
        /// Sets a service to authenticate to the registry with
        /// If not set, the DOCKER_SERVICE environment variable will be used
        /// If that is not set, the registry URL will be used
        optional -s, --service service: String

        /// Sets the username to authenticate to the registry with
        /// If not set, the DOCKER_USERNAME environment variable will be used
        optional -u, --username username: String

        /// Sets the password to authenticate to the registry with
        /// If not set, the DOCKER_PASSWORD environment variable will be used
        optional -p, --password password: String

        /// Disables the on-disk cache
        optional --no-cache

        default cmd upload {
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
        }
}
}

async fn upload_command(
    args: &Upload,
    no_cache: bool,
    service: Option<String>,
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

    let client = Arc::new(Mutex::new(client::OciClient::new(
        plan.get_registry_url(),
        username,
        password,
        service.unwrap_or_else(|| plan.get_service_url()),
    )));
    let mut execution = execution::PlanExecution::new(plan, client, no_cache, compression_level);

    if let Err(e) = execution.execute().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run_command(
    args: &Run,
    no_cache: bool,
    service: Option<String>,
    username: Option<String>,
    password: Option<String>,
) {
    let image_name = args.image.clone();
    let volumes = args.volume.clone();
    let entrypoint = args.entrypoint.clone();
    let cmd = args.cmd.clone();
    let workdir = args.workdir.clone();

    let image = ParsedImage::from_image_name(&image_name);
    let service = if let Some(arg_service) = service {
        arg_service
    } else {
        image.service.clone()
    };

    let client = Arc::new(Mutex::new(client::OciClient::new(
        image.registry,
        username,
        password,
        service,
    )));
    let downloader = downloader::OciDownloader::new(client, no_cache);

    let index = downloader
        .download_index(&image.library_name, &image.tag)
        .await
        .unwrap();

    let platform_matcher = PlatformMatcher::new();
    let manifest = platform_matcher
        .find_manifest(&index.manifests)
        .ok_or(OciDownloaderError("No matching platform found".to_string()))
        .unwrap();

    let downloaded_manifest: spec::manifest::ImageManifest = downloader
        .download_manifest(&image.library_name, &manifest.digest)
        .await
        .unwrap();

    let downloaded_config = downloader
        .download_config(&image.library_name, &downloaded_manifest.config.digest)
        .await
        .unwrap();

    let tmpdir = tempfile::tempdir().unwrap();
    let tmpdir_path = tmpdir.path();

    for layer in downloaded_manifest.layers {
        downloader
            .extract_layer(
                &image.library_name,
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
    );

    runner.run().await.expect("Failed to run command");
}

#[tokio::main]
async fn main() {
    let args = Ocitool::from_env_or_exit();

    let service = args
        .service
        .map(|s| s.to_string())
        .or_else(|| env::var("DOCKER_SERVICE").ok());
    let username = args
        .username
        .map(|s| s.to_string())
        .or_else(|| env::var("DOCKER_USERNAME").ok());
    let password = args
        .password
        .map(|s| s.to_string())
        .or_else(|| env::var("DOCKER_PASSWORD").ok());

    match args.subcommand {
        OcitoolCmd::Upload(upload) => {
            upload_command(&upload, args.no_cache, service, username, password).await
        }
        OcitoolCmd::Run(run) => run_command(&run, args.no_cache, service, username, password).await,
    }
}
