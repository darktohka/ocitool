use clap::{command, Parser};
use spec::plan::ImagePlan;
use std::env;
use std::fs::File;
mod digest;
mod execution;
mod spec;
mod uploader;
mod walk;

/// A simple CLI tool to build and push OCI images
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Sets a custom plan filename to use
    #[arg(long, default_value = "oci.json")]
    plan: String,

    /// Sets a service to authenticate to the registry with
    /// If not set, the DOCKER_SERVICE environment variable will be used
    /// If that is not set, the registry URL will be used
    #[arg(short, long)]
    service: Option<String>,

    /// Sets the username to authenticate to the registry with
    /// If not set, the DOCKER_USERNAME environment variable will be used
    #[arg(short, long)]
    username: Option<String>,

    /// Sets the password to authenticate to the registry with
    /// If not set, the DOCKER_PASSWORD environment variable will be used
    #[arg(short, long)]
    password: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

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

    let file = File::open(args.plan).expect("Failed to open plan file");
    let plan: ImagePlan = serde_json::from_reader(file).unwrap();

    let mut execution = execution::PlanExecution::new(plan, service, username, password);

    execution.execute().await;
}
