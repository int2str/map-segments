use crate::layout::RegionLayout;
use crate::sections::SectionInfo;

const BYTES_PER_KB: u64 = 1_024;

const TARGET_VERTICAL_ROWS: u64 = 16;

struct VerticalRow {
    start: u64,
    bar: String,
    label: String,
}

pub fn print_horizontal_map(region_layouts: &[RegionLayout<'_>], width: usize) {
    let (section_name_width, region_name_width) = longest_names(region_layouts);
    let bar_width = width
        .saturating_sub(section_name_width + region_name_width + 23)
        .max(1);

    for (index, layout) in region_layouts
        .iter()
        .filter(|layout| !layout.sections.is_empty())
        .enumerate()
    {
        if index > 0 {
            println!();
        }

        println!(
            "{} ({}% used; {}kb / {}kb)",
            layout.region.name,
            usage_percent(layout.used_bytes, layout.region.length),
            bytes_to_kb(layout.used_bytes),
            bytes_to_kb(layout.region.length),
        );

        for section in &layout.sections {
            let bar = build_horizontal_bar(
                layout.region.start,
                layout.region.start.saturating_add(layout.region.length),
                section.address,
                section.size,
                bar_width,
            );

            println!(
                "  {:section_width$} {:08x} {:7} {:region_width$} {}",
                section.name,
                section.address,
                section.size,
                layout.region.name,
                bar,
                section_width = section_name_width,
                region_width = region_name_width
            );
        }
    }
}

pub fn print_vertical_map(region_layouts: &[RegionLayout<'_>], width: usize) {
    let (_, region_name_width) = longest_names(region_layouts);
    let bar_width = if width >= 120 { 16 } else { 8 };

    for (index, layout) in region_layouts
        .iter()
        .filter(|l| !l.sections.is_empty())
        .enumerate()
    {
        if index > 0 {
            println!();
        }

        println!(
            "{:name_width$} {:08x}..{:08x} ({}% used; {}kb / {}kb)",
            layout.region.name,
            layout.region.start,
            layout.region.ends_at(),
            usage_percent(layout.used_bytes, layout.region.length),
            bytes_to_kb(layout.used_bytes),
            bytes_to_kb(layout.region.length),
            name_width = region_name_width,
        );

        if layout.region.length == 0 {
            println!(
                "{:name_width$} {:08x} [{}] (empty region)",
                "",
                layout.region.start,
                " ".repeat(bar_width),
                name_width = region_name_width,
            );
            continue;
        }

        let bytes_per_cell = choose_bytes_per_cell(layout.region.length, bar_width);
        let bytes_per_row = bytes_per_cell.saturating_mul(bar_width as u64);

        println!(
            "{:name_width$} Scale: 1 character = {} bytes, row = {} bytes",
            "",
            bytes_per_cell,
            bytes_per_row,
            name_width = region_name_width,
        );

        let rows = build_vertical_rows(
            layout,
            layout.region.start,
            layout.region.ends_at(),
            bytes_per_row,
            bytes_per_cell,
            bar_width,
        );
        for row in rows {
            println!(
                "{:name_width$} {:08x} [{}] {}",
                "",
                row.start,
                row.bar,
                row.label,
                name_width = region_name_width,
            );
        }
    }
}

fn build_horizontal_bar(
    region_start: u64,
    region_end: u64,
    block_start: u64,
    block_size: u64,
    total_width: usize,
) -> String {
    let region_size = region_end.saturating_sub(region_start);
    let offset = map_range(
        block_start.saturating_sub(region_start),
        region_size,
        total_width,
    );
    let width = map_range(block_size, region_size, total_width);

    let block = if width == 0 { "\u{258f}" } else { "\u{2588}" };
    let fill_width = width.max(1);
    let trailing = total_width
        .saturating_sub(offset)
        .saturating_sub(fill_width);

    format!(
        "[{}{}{}]",
        " ".repeat(offset),
        block.repeat(fill_width),
        " ".repeat(trailing),
    )
}

fn build_vertical_rows(
    layout: &RegionLayout<'_>,
    region_start: u64,
    region_end: u64,
    bytes_per_row: u64,
    bytes_per_cell: u64,
    bar_columns: usize,
) -> Vec<VerticalRow> {
    let rows = div_ceil(layout.region.length, bytes_per_row).max(1) as usize;
    (0..rows)
        .map(|row| {
            let row_start = region_start.saturating_add((row as u64).saturating_mul(bytes_per_row));
            let row_end = row_start.saturating_add(bytes_per_row).min(region_end);
            let bar = build_vertical_bar(
                layout.sections.as_slice(),
                row_start,
                row_end,
                region_end,
                bytes_per_cell,
                bar_columns,
            );

            let label = layout
                .sections
                .as_slice()
                .iter()
                .filter(|section| section_in_range(section, row_start, row_end))
                .map(|section| section.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            VerticalRow {
                start: row_start,
                bar,
                label,
            }
        })
        .collect()
}

fn build_vertical_bar(
    sections: &[&SectionInfo],
    row_start: u64,
    row_end: u64,
    region_end: u64,
    bytes_per_cell: u64,
    bar_columns: usize,
) -> String {
    let mut bar = String::with_capacity(bar_columns);
    for column in 0..bar_columns {
        let cell_start = row_start.saturating_add((column as u64).saturating_mul(bytes_per_cell));
        let cell_end = cell_start.saturating_add(bytes_per_cell).min(region_end);
        if cell_start >= row_end || cell_end <= cell_start {
            bar.push(' ');
            continue;
        }
        let covered_bytes = covered_bytes_in_range(sections, cell_start, cell_end);
        bar.push(coverage_char(
            covered_bytes,
            cell_end.saturating_sub(cell_start),
        ));
    }
    bar
}

fn usage_percent(used_bytes: u64, region_length: u64) -> u64 {
    if region_length == 0 {
        0
    } else {
        used_bytes
            .saturating_mul(100)
            .saturating_add(region_length - 1)
            .saturating_div(region_length)
            .min(100)
    }
}

fn bytes_to_kb(bytes: u64) -> u64 {
    div_ceil(bytes, BYTES_PER_KB)
}

fn choose_bytes_per_cell(region_length: u64, columns: usize) -> u64 {
    let target_cells = (columns as u64).saturating_mul(TARGET_VERTICAL_ROWS).max(1);
    let bytes_per_cell = div_ceil(region_length, target_cells).max(1);

    if bytes_per_cell <= 1 {
        bytes_per_cell
    } else {
        div_ceil(bytes_per_cell, BYTES_PER_KB).saturating_mul(BYTES_PER_KB)
    }
}

fn div_ceil(value: u64, divisor: u64) -> u64 {
    if divisor == 0 {
        0
    } else {
        value.saturating_add(divisor - 1).saturating_div(divisor)
    }
}

fn coverage_char(covered_bytes: u64, total_bytes: u64) -> char {
    const BLOCKS: [char; 9] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
    if total_bytes == 0 || covered_bytes == 0 {
        return ' ';
    }
    let block_index = covered_bytes
        .saturating_mul((BLOCKS.len() - 1) as u64)
        .saturating_add(total_bytes - 1)
        .saturating_div(total_bytes)
        .min((BLOCKS.len() - 1) as u64) as usize;
    BLOCKS[block_index]
}

fn covered_bytes_in_range(sections: &[&SectionInfo], start: u64, end: u64) -> u64 {
    sections.iter().fold(0u64, |bytes, section| {
        let overlap_start = section.starts_at().max(start);
        let overlap_end = section.ends_at().min(end);
        let overlap = overlap_end.saturating_sub(overlap_start);
        bytes.saturating_add(overlap)
    })
}

fn section_in_range(section: &SectionInfo, start: u64, end: u64) -> bool {
    section.starts_at() < end && section.ends_at() > start
}

fn longest_names(region_layouts: &[RegionLayout<'_>]) -> (usize, usize) {
    let section_width = region_layouts
        .iter()
        .flat_map(|layout| layout.sections.iter().map(|section| section.name.len()))
        .max()
        .unwrap_or(0);

    let region_width = region_layouts
        .iter()
        .map(|layout| layout.region.name.len())
        .max()
        .unwrap_or(0);

    (section_width, region_width)
}

fn map_range(value: u64, input_range: u64, output_range: usize) -> usize {
    if input_range == 0 {
        0
    } else {
        value
            .saturating_mul(output_range as u64)
            .saturating_div(input_range) as usize
    }
}
