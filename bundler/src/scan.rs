use crate::util::AUv2Id;
use anyhow::Result;
use clap_sys::{
    entry::clap_plugin_entry,
    factory::plugin_factory::{CLAP_PLUGIN_FACTORY_ID, clap_plugin_factory},
};
use std::{
    ffi::{CStr, CString, OsString, c_void},
    path::Path,
};

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PluginLibrary {
    pub plugins: Vec<PluginInfo>,
    pub has_vst3_entry: bool,
    pub has_auv2_entry: bool,
}

/// Information about a CLAP plugin.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PluginInfo {
    pub clap_id: String,
    pub clap_name: String,
    pub clap_vendor: Option<String>,
    pub clap_version: Option<String>,
    pub clap_description: Option<String>,
    pub auv2_id: Option<AUv2Id>,
}

impl PluginLibrary {
    /// Scan the given path for CLAP plugins and return their information.
    pub fn scan(path: &Path) -> Result<Self> {
        unsafe {
            let library = libloading::Library::new(path)?;

            let clap_entry = &**library
                .get::<*const clap_plugin_entry>(c"clap_entry")
                .map_err(|_| anyhow::anyhow!("Failed to find clap_entry symbol"))?;

            let has_vst3_entry = library.get::<*const c_void>(c"GetPluginFactory").is_ok();
            let has_auv2_entry = library
                .get::<*const c_void>(c"GetPluginFactoryAUV2")
                .is_ok();

            let clap_init = clap_entry
                .init
                .ok_or_else(|| anyhow::anyhow!("clap_entry missing init function"))?;

            let clap_deinit = clap_entry
                .deinit
                .ok_or_else(|| anyhow::anyhow!("clap_entry missing deinit function"))?;

            let clap_get_factory = clap_entry
                .get_factory
                .ok_or_else(|| anyhow::anyhow!("clap_entry missing get_factory function"))?;

            clap_init(CString::new(OsString::from(path).as_encoded_bytes())?.as_ptr());

            let plugin_factory =
                clap_get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr()) as *const clap_plugin_factory;

            let plugin_factory = if plugin_factory.is_null() {
                return Err(anyhow::anyhow!(
                    "clap_entry::get_factory returned null for CLAP_PLUGIN_FACTORY_ID"
                ));
            } else {
                &*plugin_factory
            };

            let plugin_factory_as_auv2 =
                clap_get_factory(sys::CLAP_PLUGIN_FACTORY_INFO_AUV2.as_ptr())
                    as *const sys::clap_plugin_factory_as_auv2;

            let plugin_factory_as_auv2 = if plugin_factory_as_auv2.is_null() {
                None
            } else {
                Some(&*plugin_factory_as_auv2)
            };

            let clap_get_plugin_count = plugin_factory.get_plugin_count.ok_or_else(|| {
                anyhow::anyhow!("clap_plugin_factory missing get_plugin_count function")
            })?;

            let clap_get_plugin_descriptor =
                plugin_factory.get_plugin_descriptor.ok_or_else(|| {
                    anyhow::anyhow!("clap_plugin_factory missing get_plugin_descriptor function")
                })?;

            let plugin_count = clap_get_plugin_count(plugin_factory as *const _);
            if plugin_count == 0 {
                return Err(anyhow::anyhow!(
                    "clap_plugin_factory::get_plugin_count returned 0"
                ));
            }

            let plugins = (0..plugin_count)
                .map(|i| {
                    let descriptor = clap_get_plugin_descriptor(plugin_factory as *const _, i);
                    let descriptor = if descriptor.is_null() {
                        return Err(anyhow::anyhow!(
                            "clap_plugin_factory::get_plugin_descriptor returned null for index {i}"
                        ));
                    } else {
                        &*descriptor
                    };

                    let auv2_info = match plugin_factory_as_auv2 {
                        Some(factory) => {
                            let get_auv2_info = factory.get_auv2_info.ok_or_else(|| {
                                anyhow::anyhow!(
                                    "clap_plugin_factory_as_auv2 missing get_auv2_info function"
                                )
                            })?;

                            let mut info = std::mem::zeroed();
                            if get_auv2_info(factory as *const _ as *mut _, i, &mut info) {
                                Some(info)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };

                    let auv2_code_subtype = auv2_info
                        .and_then(|info| parse_auv2_code(info.au_subt.as_ptr()).transpose())
                        .transpose()?;
                    let auv2_code_type = auv2_info
                        .and_then(|info| parse_auv2_code(info.au_type.as_ptr()).transpose())
                        .transpose()?;
                    let auv2_code_manu = plugin_factory_as_auv2
                        .and_then(|factory| {
                            parse_auv2_code(factory.manufacturer_code as *const _).transpose()
                        })
                        .transpose()?;

                    Ok(PluginInfo {
                        clap_id: cstr_to_string(descriptor.id)?.unwrap_or_default(),
                        clap_name: cstr_to_string(descriptor.name)?.unwrap_or_default(),
                        clap_vendor: cstr_to_string(descriptor.vendor)?,
                        clap_version: cstr_to_string(descriptor.version)?,
                        clap_description: cstr_to_string(descriptor.description)?,

                        auv2_id: match (auv2_code_manu, auv2_code_type, auv2_code_subtype) {
                            (Some(manu), Some(type_), Some(subtype)) => Some(AUv2Id {
                                manufacturer: manu,
                                type_,
                                subtype,
                            }),
                            _ => None,
                        },
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            clap_deinit();

            Ok(PluginLibrary {
                plugins,
                has_auv2_entry,
                has_vst3_entry,
            })
        }
    }
}

unsafe fn cstr_to_string(cstr: *const i8) -> Result<Option<String>> {
    if cstr.is_null() {
        return Ok(None);
    }

    unsafe { Ok(Some(CStr::from_ptr(cstr).to_str()?.to_string())) }
}

unsafe fn parse_auv2_code(code: *const u8) -> Result<Option<[u8; 4]>> {
    if code.is_null() {
        Ok(None)
    } else {
        let len = (0..4)
            .position(|i| unsafe { *code.add(i) } == 0)
            .unwrap_or(4);
        if len == 4 {
            unsafe { Ok(Some((code as *const [u8; 4]).read())) }
        } else if len == 0 {
            Ok(None)
        } else {
            anyhow::bail!("AUv2 code contains a null byte")
        }
    }
}

#[allow(non_camel_case_types)]
mod sys {
    use std::ffi::{CStr, c_char};

    pub const CLAP_PLUGIN_FACTORY_INFO_AUV2: &CStr = c"clap.plugin-factory-info-as-auv2.draft0";

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct clap_plugin_info_as_auv2 {
        pub au_type: [u8; 5],
        pub au_subt: [u8; 5],
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct clap_plugin_factory_as_auv2 {
        pub manufacturer_code: *const c_char,
        pub manufacturer_name: *const c_char,
        pub get_auv2_info: Option<
            unsafe extern "C" fn(
                factory: *mut clap_plugin_factory_as_auv2,
                index: u32,
                info: *mut clap_plugin_info_as_auv2,
            ) -> bool,
        >,
    }
}
