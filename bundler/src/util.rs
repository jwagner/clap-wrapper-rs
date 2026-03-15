use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginFormat {
    Clap,
    Vst3,
    Auv2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatingSystem {
    Windows,
    MacOS,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    X86,
    X86_64,
    Arm,
    Aarch64,
}

impl Architecture {
    /// Get the current architecture, or an error if the architecture is not supported.
    pub fn current() -> Result<Self> {
        if cfg!(target_arch = "x86") {
            Ok(Self::X86)
        } else if cfg!(target_arch = "x86_64") {
            Ok(Self::X86_64)
        } else if cfg!(target_arch = "arm") {
            Ok(Self::Arm)
        } else if cfg!(target_arch = "aarch64") {
            Ok(Self::Aarch64)
        } else {
            Err(anyhow::anyhow!("Unsupported architecture"))
        }
    }
}

impl OperatingSystem {
    /// Get the current operating system, or an error if the OS is not supported.
    pub fn current() -> Result<Self> {
        if cfg!(target_os = "windows") {
            Ok(Self::Windows)
        } else if cfg!(target_os = "macos") {
            Ok(Self::MacOS)
        } else if cfg!(target_os = "linux") {
            Ok(Self::Linux)
        } else {
            Err(anyhow::anyhow!("Unsupported operating system"))
        }
    }
}

/// If the given path does not exist, try to fix it by adding the OS-specific dynamic library prefix and suffix.
pub fn fix_dylib_path(path: &Path, os: OperatingSystem) -> PathBuf {
    pub fn os_dylib_filename(name: &str, os: OperatingSystem) -> String {
        match os {
            OperatingSystem::Windows => format!("{}.dll", name),
            OperatingSystem::MacOS => format!("lib{}.dylib", name),
            OperatingSystem::Linux => format!("lib{}.so", name),
        }
    }

    if let Some(filename) = path.with_extension("").file_name().and_then(|f| f.to_str()) {
        let maybe_lib = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(os_dylib_filename(filename, os));

        // toctou be dambed
        if path.exists() || !maybe_lib.exists() {
            path.to_path_buf()
        } else {
            maybe_lib
        }
    } else {
        path.to_path_buf()
    }
}

/// Get the OS-specific global plugin directory for the given plugin format
pub fn os_plugin_dir(format: PluginFormat) -> Option<PathBuf> {
    match format {
        PluginFormat::Clap => {
            if cfg!(target_os = "windows") {
                std::env::var_os("LOCALAPPDATA")
                    .map(|x| PathBuf::from(x).join("Programs/Common/CLAP/"))
            } else if cfg!(target_os = "macos") {
                std::env::var_os("HOME")
                    .map(|x| PathBuf::from(x).join("Library/Audio/Plug-Ins/VST3/"))
            } else {
                std::env::var_os("HOME").map(|x| PathBuf::from(x).join(".clap/"))
            }
        }

        PluginFormat::Vst3 => {
            if cfg!(target_os = "windows") {
                std::env::var_os("LOCALAPPDATA")
                    .map(|x| PathBuf::from(x).join("Programs/Common/VST3/"))
            } else if cfg!(target_os = "macos") {
                std::env::var_os("HOME")
                    .map(|x| PathBuf::from(x).join("Library/Audio/Plug-Ins/VST3/"))
            } else {
                std::env::var_os("HOME").map(|x| PathBuf::from(x).join(".vst3/"))
            }
        }

        PluginFormat::Auv2 => {
            if cfg!(target_os = "macos") {
                std::env::var_os("HOME")
                    .map(|x| PathBuf::from(x).join("Library/Audio/Plug-Ins/Components/"))
            } else {
                None
            }
        }
    }
}

/// Sign the given bundle with an ad-hoc signature using the `codesign` tool on macOS.
pub fn sign_adhoc(bundle: &Path) -> Result<()> {
    if !cfg!(target_os = "macos") {
        anyhow::bail!("Code signing is only supported on macOS");
    }

    let status = std::process::Command::new("codesign")
        .arg("--force")
        .arg("--timestamp")
        .arg("--deep")
        .arg("-s")
        .arg("-")
        .arg(bundle)
        .spawn()?
        .wait()?;

    anyhow::ensure!(status.success(), "codesign failed with status: {}", status);
    Ok(())
}

/// Get the filename of the currently running executable, without the path.
/// It is used for printing the usage message.
pub fn exe_filename() -> String {
    std::env::args()
        .next()
        .and_then(|p| {
            PathBuf::from(p)
                .file_name()
                .and_then(|f| f.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "bundle".to_string())
}

impl std::fmt::Display for PluginFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginFormat::Clap => write!(f, "CLAP"),
            PluginFormat::Vst3 => write!(f, "VST3"),
            PluginFormat::Auv2 => write!(f, "AUv2"),
        }
    }
}
