use crate::spec::{enums::PlatformArchitecture, index::Manifest};
use std::env;

pub struct PlatformMatcher {
    pub platform: PlatformArchitecture,
}

impl PlatformMatcher {
    pub fn new() -> Self {
        let platform = match env::consts::ARCH {
            "x86" => PlatformArchitecture::X86,
            "x86_64" => PlatformArchitecture::Amd64,
            "arm" => PlatformArchitecture::Arm,
            "aarch64" => PlatformArchitecture::Arm64,
            "mips" => PlatformArchitecture::Mips,
            "mips64" => PlatformArchitecture::Mips64,
            "powerpc64" => PlatformArchitecture::Ppc64,
            "riscv64" => PlatformArchitecture::Riscv64,
            "s390x" => PlatformArchitecture::S390x,
            "loongarch64" => PlatformArchitecture::Loong64,
            _ => PlatformArchitecture::Unknown,
        };

        PlatformMatcher { platform }
    }

    pub fn match_architecture(platform: PlatformArchitecture) -> Self {
        PlatformMatcher { platform }
    }

    pub fn matches(&self, image_platform: &PlatformArchitecture) -> bool {
        self.platform == *image_platform
    }

    pub fn find_manifest<'a, I>(&'a self, manifests: I) -> Option<&'a Manifest>
    where
        I: IntoIterator<Item = &'a Manifest>,
    {
        for manifest in manifests {
            if let Some(platform) = &manifest.platform {
                if self.matches(&platform.architecture) {
                    return Some(manifest);
                }
            }
        }

        None
    }
}
