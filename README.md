# ocitool - The Zebrafish CLI

[![Zebrafish Project](https://img.shields.io/badge/project-Zebrafish-blue.svg)](https://github.com/darktohka/zebrafish)

`ocitool` is a powerful command-line interface for managing OCI (Open Container Initiative) compatible container images and workloads. It is the primary CLI for Zebrafish's multi-compose project support.

It provides a set of utilities for pulling, running, and managing container images, with a special focus on performance and compatibility with the OCI ecosystem, including direct integration with `containerd`.

## Features

- **OCI images**: Pull, inspect, and manage OCI container images from various registries.
- **Container execution in CI/CD**: Run containers based on OCI images.
- **Compose functionality**: Integrated support for Compose files for defining and running multi-container applications.
- **Direct `containerd` integration**: Communicates directly with the `containerd` daemon.
- **Compression**: Efficiently handles image layers, including support for `zstd` compression.
- **System integration**: Manages registry authentication credentials from the Zebrafish system.

## Using the Docker image

We provide a prebuilt Docker image for `ocitool`, available for both `x86_64` and `aarch64` architectures. You can use the image directly without needing to build from source.

### Pulling the Docker Image

To pull the Docker image, run:

```bash
docker pull darktohka/ocitool
```

### Running `ocitool` with Docker

You can run `ocitool` using the Docker image:

```bash
docker run -it darktohka/ocitool
```

This will start an interactive session with `ocitool` inside the container.

## Building from Source

To build `ocitool` from source, you will need to have the Rust toolchain installed.

1.  **Clone the repository:**

    ```bash
    git clone https://github.com/darktohka/ocitool.git
    cd ocitool
    ```

2.  **Build the project:**
    ```bash
    cargo build --release
    ```
    The optimized binary will be available at `target/release/ocitool`.

## Testing

To run tests for `ocitool`, ensure you have the Rust toolchain installed and navigate to the `ocitool` directory.

1. **Run Tests:**

   ```bash
   cargo test
   ```

   This will execute all tests (unit tests, integration tests and E2E tests).

## Usage

`ocitool` provides several subcommands to interact with containers and images.

### Authentication

Credentials can be scoped to specific registries using `--host`, `--username`, and `--password` flags:

```bash
ocitool --host registry.example.com --username user --password pass upload
```

Multiple registry credentials can be specified:

```bash
ocitool \
  --host registry.example.com --username user1 --password pass1 \
  --host ghcr.io --username user2 --password pass2 \
  upload
```

If `--host`, `--username`, and `--password` are not provided, the `DOCKER_USERNAME` and `DOCKER_PASSWORD` environment variables are used as a fallback for all registries.

System-level authentication from the kernel command line (`dockerlogin=hostname,username,password;...`) is also supported for compose workflows.

### Subcommands

- **Pull all images from a multi-compose project:**

  ```bash
  ocitool compose --dir /compose pull
  ```

- **Prepare a multi-compose project:**

  ```bash
  ocitool compose --dir /compose up
  ```

- **Run a container image:**

  ```bash
  ocitool run --image ubuntu:latest -- /bin/bash
  ```

- **Upload an OCI image plan:**

  ```bash
  ocitool upload
  ```

  For more details on specific commands, you can use the `--help` flag:

```bash
ocitool --help
ocitool compose --help
```
