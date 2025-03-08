use spec::plan::ImagePlan;
use std::env;
use std::fs::File;
use std::path::Path;
use walkdir::WalkDir;
mod client;
mod digest;
mod execution;
mod spec;
mod uploader;
mod walk;

xflags::xflags! {
    /// Uploads an OCI image to a registry
    cmd app {
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

        default cmd upload {
            /// Sets a custom plan filename to use
            optional --plan plan: String

            /// Sets the compression level to use when compressing layers
            /// If not set, the COMPRESSION_LEVEL environment variable will be used
            /// If that is not set, the default compression level will be used
            /// The compression level must be between 1 and 22
            optional -c, --compression-level compression_level: i32
        }
}
}

async fn upload_command(
    args: &Upload,
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

    let mut execution =
        execution::PlanExecution::new(plan, service, username, password, compression_level);

    if let Err(e) = execution.execute().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[tokio::main]
async fn main() {
    let args = App::from_env_or_exit();

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
        AppCmd::Upload(upload) => upload_command(&upload, service, username, password).await,
        _ => {
            eprintln!("No subcommand specified");
            std::process::exit(1);
        }
    }
}
