//!
//! Object file sections related code
//!

use std::error::Error;
use std::path::Path;

use object::{Object, ObjectSection, Section, SectionKind};

/// SectionInfo re-wraps items from ObjectSection.
/// This is mostly done for ownership sake.
#[derive(Debug)]
pub struct SectionInfo {
    pub name: String,
    pub address: u64,
    pub size: u64,
}

impl SectionInfo {
    fn from_section(section: &Section) -> Self {
        Self {
            name: section.name().unwrap_or("UNKNOWN").to_string(),
            address: section.address(),
            size: section.size(),
        }
    }

    pub fn starts_at(&self) -> u64 {
        self.address
    }

    pub fn ends_at(&self) -> u64 {
        self.address.saturating_add(self.size)
    }
}

/// Parse the given ELF/object file and return a list of linker sections in
/// the file.
///
/// Empty sections or sections not deemed to be included in the final binary
/// are excluded. Results are sorted by (starting) address.
pub fn from_object_file(filename: &Path) -> Result<Vec<SectionInfo>, Box<dyn Error>> {
    let binary_file = std::fs::read(filename)?;
    let object_file = object::File::parse(&*binary_file)?;

    let mut sections: Vec<SectionInfo> = object_file
        .sections()
        .filter(|section| section.size() != 0)
        .filter(|section| is_included_in_binary(section))
        .map(|section| SectionInfo::from_section(&section))
        .collect();
    sections.sort_by_key(|section| section.address);

    Ok(sections)
}

/// Determine whether a given section in the object file should be included
/// in the memory map.
///
/// The assumption here is that if it's included in the binary, it should
/// appear in the map.
fn is_included_in_binary(section: &Section) -> bool {
    matches!(
        section.kind(),
        SectionKind::Text
            | SectionKind::Data
            | SectionKind::ReadOnlyData
            | SectionKind::ReadOnlyString
            | SectionKind::UninitializedData
            | SectionKind::Common
            | SectionKind::Tls
            | SectionKind::UninitializedTls
    )
}
