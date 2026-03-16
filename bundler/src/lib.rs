use crate::{
    bundle::BundleOptions,
    util::{AUv2Id, PluginFormat},
};
use anyhow::Result;
use argh::FromArgs;
use std::path::{Path, PathBuf};
use yansi::Paint;

mod bundle;
mod scan;
mod util;

/// A CLI tool to bundle CLAP plugins built with `clap_wrapper` into OS-specific and format-specific bundles (e.g. VST3 on Windows, AUv2 on macOS)
#[derive(FromArgs)]
#[argh(help_triggers("-h", "--help", "help"))]
struct Args {
    /// install plugins to the OS-specific directories?
    #[argh(switch, short = 'i')]
    install: bool,

    /// whether to bundle VST3 plugins as a folder (instead of a single .vst3 file) on Windows
    #[argh(switch)]
    vst3_folder: bool,

    /// override the AUv2 ID in the `type:subt:manu` format, only works if the library exports only a single plugin)
    #[argh(option)]
    auv2_id: Option<AUv2Id>,

    /// dylibs built with `clap_wrapper` to bundle.
    #[argh(positional)]
    libraries: Vec<PathBuf>,
}

pub fn run() -> ! {
    match run_result() {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("{}: {}", "error".red().bold(), e);
            std::process::exit(1)
        }
    }
}

fn run_result() -> Result<()> {
    let args: Args = argh::from_env();
    let os = util::OperatingSystem::current()?;
    let arch = util::Architecture::current()?;

    if args.libraries.is_empty() {
        anyhow::bail!("no input files provided");
    }

    if args.auv2_id.is_some() && args.libraries.len() != 1 {
        anyhow::bail!("--auv2-id option can only be used when bundling a single plugin");
    }

    let mut failed = false;
    for dylib in args.libraries {
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
                vst3_single_file: args.vst3_folder,
                auv2_override_id: args.auv2_id,
            };

            match options.bundle() {
                Ok(path) => {
                    eprintln!("  - {} {}", "OK".green().bold(), path.display());

                    if cfg!(target_os = "macos") {
                        util::sign_adhoc(&path).ok();
                    }

                    if args.install
                        && let Some(install_dir) = util::os_plugin_dir(format)
                    {
                        match util::copy_all(&path, &install_dir.join(path.file_name().unwrap())) {
                            Ok(()) => {
                                eprintln!(
                                    "    {} installed to the system folder",
                                    "OK".green().bold()
                                );
                            }
                            Err(e) => {
                                eprintln!("    {} {}", "ERR".red().bold(), e);
                                failed = true;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  - {} {}", "ERR".red().bold(), e);
                    failed = true;
                }
            }
        }
    }

    if cfg!(target_os = "macos") {
        util::kill_audio_component_registrar().ok();
    }

    std::process::exit(if failed { 100 } else { 0 });
}
