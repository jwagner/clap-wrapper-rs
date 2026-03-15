use crate::{bundle::BundleOptions, util::PluginFormat};
use std::path::{Path, PathBuf};
use yansi::Paint;

mod bundle;
mod scan;
mod util;

struct Args {
    /// Install plugins to the OS-specific directories?
    install: bool,

    /// Whether to bundle VST3 plugins as a single file (instead of a folder with multiple files) on Windows
    vst3_single_file: bool,

    /// Dylibs built with `clap_wrapper` to bundle.
    dylibs: Vec<PathBuf>,
}

impl Args {
    pub fn parse() -> Option<Self> {
        let mut args = pico_args::Arguments::from_env();

        if args.contains(["-h", "--help"]) {
            return None;
        }

        let install = args.contains(["-i", "--install"]);
        let vst3_single_file = args.contains("--vst3-file");
        let dylibs = args
            .finish()
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();

        if dylibs.is_empty() {
            return None;
        }

        Some(Self {
            install,
            vst3_single_file,
            dylibs,
        })
    }
}

pub fn run() {
    let args = Args::parse().unwrap_or_else(|| {
        let exe_name = util::exe_filename();

        eprintln!("{}: {exe_name} [options] <paths...>", "Usage".bold());
        eprintln!();
        eprintln!("{}:", "Options".bold());
        eprintln!("  --vst3-file      Bundle VST3 plugins as a single file on Windows");
        eprintln!("  -i, --install    Install plugins to the OS-specific directories");
        eprintln!("  -h, --help       Print this help message");
        std::process::exit(0);
    });

    let os = util::OperatingSystem::current().unwrap_or_else(|_| {
        eprintln!("{}: unsupported os", "error".red().bold());
        std::process::exit(1);
    });

    let arch = util::Architecture::current().unwrap_or_else(|_| {
        eprintln!("{}: unsupported architecture", "error".red().bold());
        std::process::exit(1);
    });

    let mut failed = false;
    for dylib in args.dylibs {
        let dylib = util::fix_dylib_path(&dylib, os);

        eprint!("{} - ", dylib.display().bold());

        let mut formats = vec![PluginFormat::Clap];
        let library = match scan::PluginLibrary::scan(&dylib) {
            Ok(lib) => {
                if lib.has_vst3_entry {
                    formats.push(PluginFormat::Vst3);
                }

                if lib.has_auv2_entry {
                    formats.push(PluginFormat::Auv2);
                }

                eprintln!(
                    "{} - {}",
                    "OK".green().bold(),
                    formats
                        .iter()
                        .map(|f| f.bold().to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                );

                for plugin in &lib.plugins {
                    eprintln!("  - {} ({})", plugin.clap_name.bold(), plugin.clap_id);
                }

                lib
            }
            Err(e) => {
                eprintln!("{} - {}", "ERR".red().bold(), e);
                failed = true;
                continue;
            }
        };

        for format in formats {
            let options = BundleOptions {
                dylib_path: &dylib,
                output_dir: dylib.parent().unwrap_or_else(|| Path::new(".")),
                library: &library,
                format,
                os,
                arch,
                overwrite_existing: true,
                vst3_single_file: args.vst3_single_file,
            };

            match options.bundle() {
                Ok(path) => {
                    eprintln!("  - {} - {}", "OK".green().bold(), path.display());

                    if cfg!(target_os = "macos") {
                        util::sign_adhoc(&path).unwrap_or_else(|e| {
                            eprintln!(
                                "    {} - failed to sign bundle: {}",
                                "WARN".yellow().bold(),
                                e
                            );
                        });
                    }
                }
                Err(e) => {
                    eprintln!("  - {} - {}", "ERR".red().bold(), e);
                    failed = true;
                }
            }
        }
    }

    if failed {
        std::process::exit(1);
    }
}
