use crate::memory_map::Region;
use crate::sections::SectionInfo;

pub struct RegionLayout<'a> {
    pub region: &'a Region,
    pub sections: Vec<&'a SectionInfo>,
    pub used_bytes: u64,
}

pub fn map_sections_to_regions<'a>(
    sections: &'a [SectionInfo],
    regions: &'a [Region],
) -> Vec<RegionLayout<'a>> {
    let mut layouts = regions
        .iter()
        .map(|region| RegionLayout {
            region,
            sections: Vec::new(),
            used_bytes: 0,
        })
        .collect::<Vec<_>>();

    for section in sections {
        if let Some(layout) = layouts
            .iter_mut()
            .rfind(|layout| contains(layout.region, section))
        {
            layout.used_bytes = layout.used_bytes.saturating_add(section.size);
            layout.sections.push(section);
        }
    }

    layouts
}

fn contains(region: &Region, section: &SectionInfo) -> bool {
    region.start <= section.starts_at() && region.ends_at() >= section.ends_at()
}

#[cfg(test)]
mod tests {
    use super::map_sections_to_regions;
    use crate::memory_map::Region;
    use crate::sections::SectionInfo;

    #[test]
    fn keeps_empty_regions() {
        let regions = vec![
            Region {
                name: "FLASH".to_string(),
                start: 0x0000,
                length: 0x1000,
            },
            Region {
                name: "RAM".to_string(),
                start: 0x2000,
                length: 0x1000,
            },
        ];
        let sections = vec![SectionInfo {
            name: ".text".to_string(),
            address: 0x0100,
            size: 0x20,
        }];

        let layouts = map_sections_to_regions(&sections, &regions);

        assert_eq!(layouts.len(), 2);
        assert_eq!(layouts[0].sections.len(), 1);
        assert!(layouts[1].sections.is_empty());
    }

    #[test]
    fn ignores_unmapped_sections() {
        let regions = vec![Region {
            name: "FLASH".to_string(),
            start: 0x0000,
            length: 0x1000,
        }];
        let sections = vec![SectionInfo {
            name: ".text".to_string(),
            address: 0x3000,
            size: 0x20,
        }];

        let layouts = map_sections_to_regions(&sections, &regions);

        assert!(layouts[0].sections.is_empty());
        assert_eq!(layouts[0].used_bytes, 0);
    }
}
