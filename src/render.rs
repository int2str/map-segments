use crate::memory_map::Region;
use crate::sections::SectionInfo;

pub fn print_map(region_map: Vec<(&SectionInfo, &Region)>, width: usize) {
    let (section_name_width, region_name_width) = longest_names(&region_map);
    let bar_width = width
        .saturating_sub(section_name_width + region_name_width + 21)
        .max(1);
    let mut last_region: Option<u64> = None;

    for (section, region) in region_map {
        if last_region != Some(region.id) {
            if last_region.is_some() {
                println!();
            }
            last_region = Some(region.id);
        }

        print!(
            "{:width$} {:08x} {:7}",
            section.name,
            section.address,
            section.size,
            width = section_name_width,
        );

        print!(" {:width$} ", region.name, width = region_name_width);
        print_memory(
            region.start,
            region.start + region.length,
            section.address,
            section.size,
            bar_width,
        );

        println!();
    }
}

fn longest_names(region_map: &[(&SectionInfo, &Region)]) -> (usize, usize) {
    region_map.iter().fold((0, 0), |(section, region), (s, r)| {
        (section.max(s.name.len()), region.max(r.name.len()))
    })
}

fn map_range(value: u64, input_range: u64, output_range: usize) -> usize {
    ((output_range as f64 / input_range as f64) * value as f64) as usize
}

fn print_memory(
    region_start: u64,
    region_end: u64,
    block_start: u64,
    block_size: u64,
    total_width: usize,
) {
    let region_size = region_end - region_start;
    let offset = map_range(block_start - region_start, region_size, total_width);
    let width = map_range(block_size, region_size, total_width);

    let block = if width == 0 { "\u{258f}" } else { "\u{2588}" };

    print!(
        "[{}{}{}]",
        " ".repeat(offset),
        block.repeat(width.max(1)),
        " ".repeat(total_width - offset - width.max(1)),
    );
}
