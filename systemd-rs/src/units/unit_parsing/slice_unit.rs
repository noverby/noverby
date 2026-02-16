use log::{trace, warn};

use crate::units::{
    ParsedCommonConfig, ParsedFile, ParsedSliceConfig, ParsingErrorReason, parse_install_section,
    parse_unit_section,
};
use std::path::PathBuf;

pub fn parse_slice(
    parsed_file: ParsedFile,
    path: &PathBuf,
) -> Result<ParsedSliceConfig, ParsingErrorReason> {
    let mut install_config = None;
    let mut unit_config = None;

    for (name, section) in parsed_file {
        match name.as_str() {
            "[Unit]" => {
                unit_config = Some(parse_unit_section(section)?);
            }
            "[Install]" => {
                install_config = Some(parse_install_section(section)?);
            }
            "[Slice]" => {
                // Slice-specific settings (e.g. resource control directives like
                // MemoryMax=, CPUQuota=, etc.) are recognised but currently ignored.
                // This allows slice unit files to be loaded without error.
                trace!(
                    "Ignoring [Slice] section settings in slice unit {path:?} (resource control not yet implemented)"
                );
            }
            _ if name.starts_with("[X-") || name.starts_with("[x-") => {
                trace!("Silently ignoring vendor extension section in slice unit {path:?}: {name}");
            }
            _ => {
                warn!("Ignoring unknown section in slice unit {path:?}: {name}");
            }
        }
    }

    Ok(ParsedSliceConfig {
        common: ParsedCommonConfig {
            name: path.file_name().unwrap().to_str().unwrap().to_owned(),
            unit: unit_config.unwrap_or_else(Default::default),
            install: install_config.unwrap_or_else(Default::default),
        },
    })
}
