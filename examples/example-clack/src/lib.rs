#![deny(unsafe_code)]

mod synth;

clack_plugin::clack_export_entry!(clack_plugin::entry::SinglePluginEntry<synth::PolySynthPlugin>);
clap_wrapper::export_vst3!();
clap_wrapper::export_auv2!();
