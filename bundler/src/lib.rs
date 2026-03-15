use crate::{
    bundle::BundleOptions,
    util::{AUv2Id, PluginFormat},
};
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

    /// Override the AUv2 ID (only works if the library exports only a single plugin)
    auv2_override_id: Option<AUv2Id>,

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
        let auv2_id = args.opt_value_from_str::<_, AUv2Id>("--auv2-id").ok()?;
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
            auv2_override_id: auv2_id,
            vst3_single_file,
            dylibs,
        })
    }
}

pub fn run() {
    let args = Args::parse().unwrap_or_else(|| {
        let exe_name = util::exe_filename();

        eprintln!(
            r#"
{}: {exe_name} [options] <paths...>

{}: 
  --vst3-file               Bundle VST3 plugins as a single file on Windows
  --auv2-id manu:type:subt  Set the AUv2 ID (only if the library exports a single plugin)
  --[i]nstall               Install plugins to the OS-specific directories
  --[h]elp                  Print this help message 
"#,
            "Usage".bold(),
            "Options".bold()
        );

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

    if args.auv2_override_id.is_some() && args.dylibs.len() != 1 {
        eprintln!(
            "{}: the --auv2-id option can only be used when bundling a single plugin",
            "error".red().bold()
        );
        std::process::exit(1);
    }

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
                auv2_override_id: args.auv2_override_id,
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

                        if format == PluginFormat::Auv2 {
                            util::kill_audio_component_registrar().ok();
                        }
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
