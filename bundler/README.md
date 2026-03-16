# clap-wrapper-rs bundler tool

This is a simple tool that can be used to automate the bundling process of VST3 and AUv2 plugins created with `clap-wrapper-rs`. 

## Usage

The bundler is a command line tool that can be run after building your plugin library. It will copy the necessary files to the correct directories and optionally install the plugin to the system folder. 


The bundler is intended to be used as an [xtask](https://github.com/matklad/cargo-xtask) in your workspace. You can add a separate binary to your workspace with the following dependency in `Cargo.toml`:
```toml
[dependencies]
clap-wrapper-bundler = { git = "https://github.com/blepfx/clap-wrapper-rs.git" }
```

and the following `main.rs`:
```rust
fn main() {
    clap_wrapper_bundler::run();
}
```

Additionally, you can add a cargo alias to make it easier to run the bundler:
```toml
[alias]
bundle = "run --package <your-bundler-package-name-here> --"
```

Then, you can run the bundler with the following command:
```bash
cargo bundle --help
```

### Arguments

The bundler accepts the following arguments:
```
clap-wrapper-bundler [-i] [--vst3-folder] [--auv2-id <auv2-id>] [--] [<libraries...>]
```

- `libraries`: The paths to the plugin libraries to bundle. Must be used on the output dynamic library emitted by the build step (`cargo build`). If the filename is not in the format of `my_plugin.dll` (Windows), `libmy_plugin.so` (Linux) or `libmy_plugin.dylib` (MacOS), you can provide it as `my_plugin` and the bundler will try to find the correct file by adding the appropriate prefix and suffix for the current platform.

- `-i, --install`: Whether to install the plugin to the system folder after bundling. This is optional and can be used if you want to test the plugin without having to manually copy it to the system folder.

- `--vst3-folder`: Whether to bundle VST3 plugins as a folder on Windows. By default, VST3 plugins are emitted as a single .vst3 file on Windows. Single file VST3 plugins are technically deprecated by Steinberg but many plugin developers use them and they work just fine.

- `--auv2-id <type:subt:manu>`: Set the AUv2 plugin ID. By default, the bundler will use the information provided by the `plugin-factory-info-as-auv2` extension if present.
If the plugin does not implement said extension, you **have** to provide the ID yourself.
This only works for single-plugin AUv2 plugin bundles, as multi-plugin bundles have to use the `plugin-factory-info-as-auv2` extension to provide the ID for each plugin.