use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    #[serde(rename = "application/vnd.oci.image.index.v1+json")]
    OciImageIndexV1Json,
    #[serde(rename = "application/vnd.oci.image.manifest.v1+json")]
    OciImageManifestV1Json,
    #[serde(rename = "application/vnd.oci.image.config.v1+json")]
    OciImageConfigV1ConfigJson,
    #[serde(rename = "application/vnd.oci.image.layer.v1.tar+zstd")]
    OciImageLayerV1TarZstd,
    #[serde(rename = "application/vnd.oci.image.layer.v1.tar+gzip")]
    OciImageLayerV1TarGzip,
    #[serde(rename = "application/vnd.oci.image.layer.v1.tar")]
    OciImageLayerV1Tar,
    #[serde(rename = "application/vnd.docker.distribution.manifest.list.v2+json")]
    DockerManifestListV2Json,
    #[serde(rename = "application/vnd.docker.distribution.manifest.v2+json")]
    DockerManifestV2Json,
    #[serde(rename = "application/vnd.docker.container.image.v1+json")]
    DockerConfigV1Json,
    #[serde(rename = "application/vnd.docker.image.rootfs.diff.tar.gzip")]
    DockerImageRootfsDiffTarGzip,
    #[serde(rename = "application/vnd.docker.image.rootfs.diff.tar.zstd")]
    DockerImageRootfsDiffTarZstd,
    #[serde(rename = "application/vnd.docker.image.rootfs.diff.tar")]
    DockerImageRootfsDiffTar,
}

impl MediaType {
    pub fn to_string(&self) -> &'static str {
        match self {
            MediaType::OciImageIndexV1Json => "application/vnd.oci.image.index.v1+json",
            MediaType::OciImageManifestV1Json => "application/vnd.oci.image.manifest.v1+json",
            MediaType::OciImageConfigV1ConfigJson => "application/vnd.oci.image.config.v1+json",
            MediaType::OciImageLayerV1TarZstd => "application/vnd.oci.image.layer.v1.tar+zstd",
            MediaType::OciImageLayerV1TarGzip => "application/vnd.oci.image.layer.v1.tar+gzip",
            MediaType::OciImageLayerV1Tar => "application/vnd.oci.image.layer.v1.tar",
            MediaType::DockerManifestListV2Json => {
                "application/vnd.docker.distribution.manifest.list.v2+json"
            }
            MediaType::DockerManifestV2Json => {
                "application/vnd.docker.distribution.manifest.v2+json"
            }
            MediaType::DockerConfigV1Json => "application/vnd.docker.container.image.v1+json",
            MediaType::DockerImageRootfsDiffTarGzip => {
                "application/vnd.docker.image.rootfs.diff.tar.gzip"
            }
            MediaType::DockerImageRootfsDiffTarZstd => {
                "application/vnd.docker.image.rootfs.diff.tar.zstd"
            }
            MediaType::DockerImageRootfsDiffTar => "application/vnd.docker.image.rootfs.diff.tar",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum PlatformArchitecture {
    #[serde(rename = "amd64")]
    Amd64,
    #[serde(rename = "386")]
    X86,
    #[serde(rename = "arm64")]
    Arm64,
    #[serde(rename = "arm")]
    Arm,
    #[serde(rename = "wasm")]
    Wasm,
    #[serde(rename = "ppc64")]
    Ppc64,
    #[serde(rename = "ppc64le")]
    Ppc64Le,
    #[serde(rename = "loong64")]
    Loong64,
    #[serde(rename = "mips")]
    Mips,
    #[serde(rename = "mipsle")]
    Mipsle,
    #[serde(rename = "mips64")]
    Mips64,
    #[serde(rename = "mips64le")]
    Mips64le,
    #[serde(rename = "riscv64")]
    Riscv64,
    #[serde(rename = "s390x")]
    S390x,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PlatformOS {
    #[serde(rename = "aix")]
    Aix,
    #[serde(rename = "android")]
    Android,
    #[serde(rename = "darwin")]
    Darwin,
    #[serde(rename = "dragonfly")]
    Dragonfly,
    #[serde(rename = "freebsd")]
    Freebsd,
    #[serde(rename = "illumos")]
    Illumos,
    #[serde(rename = "ios")]
    Ios,
    #[serde(rename = "js")]
    Js,
    #[serde(rename = "linux")]
    Linux,
    #[serde(rename = "netbsd")]
    Netbsd,
    #[serde(rename = "openbsd")]
    Openbsd,
    #[serde(rename = "plan9")]
    Plan9,
    #[serde(rename = "solaris")]
    Solaris,
    #[serde(rename = "wasip1")]
    Wasip1,
    #[serde(rename = "windows")]
    Windows,
    #[serde(rename = "unknown")]
    Unknown,
}
