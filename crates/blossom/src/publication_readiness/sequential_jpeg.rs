use alloc::vec::Vec;

use super::{
    JpegContainerInspection, PUBLICATION_RASTER_MAX_CONTAINER_RECORDS, RadrootsBlossomError,
    RadrootsBlossomRasterDimensions, is_jpeg_start_of_frame,
};

const MAX_SEQUENTIAL_JPEG_SCANS: u8 = 4;
const MAX_SEQUENTIAL_JPEG_BLOCKS: u64 = 3_200_000;
const MAX_SEQUENTIAL_JPEG_COEFFICIENT_STEPS: u64 = MAX_SEQUENTIAL_JPEG_BLOCKS * 64;

#[derive(Clone, Copy)]
struct SequentialJpegComponent {
    id: u8,
    horizontal_sampling: u8,
    vertical_sampling: u8,
}

struct SequentialJpegFrame {
    dimensions: RadrootsBlossomRasterDimensions,
    components: Vec<SequentialJpegComponent>,
    maximum_horizontal_sampling: u8,
    maximum_vertical_sampling: u8,
}

struct SequentialJpegScanComponent {
    frame_index: usize,
    dc_table: usize,
    ac_table: usize,
}

struct SequentialJpegScan {
    components: Vec<SequentialJpegScanComponent>,
}

struct SequentialJpegHuffmanTable {
    first_codes: [u32; 16],
    value_offsets: [usize; 16],
    code_counts: [u8; 16],
    values: Vec<u8>,
}

impl SequentialJpegHuffmanTable {
    fn new(class: u8, code_counts: [u8; 16], values: &[u8]) -> Result<Self, RadrootsBlossomError> {
        if class > 1 {
            return invalid_entropy();
        }
        if values.is_empty() || values.len() > 256 {
            return invalid_entropy();
        }

        let mut first_codes = [0_u32; 16];
        let mut value_offsets = [0_usize; 16];
        let mut code = 0_u32;
        let mut value_offset = 0_usize;
        let mut unused_codes = 1_i32;
        for (index, count) in code_counts.iter().copied().enumerate() {
            unused_codes = unused_codes * 2 - i32::from(count);
            if unused_codes < 0 {
                return invalid_entropy();
            }
            first_codes[index] = code;
            value_offsets[index] = value_offset;
            code = (code + u32::from(count)) * 2;
            value_offset += usize::from(count);
        }
        if value_offset != values.len() {
            return invalid_entropy();
        }
        if unused_codes == 0 {
            return invalid_entropy();
        }

        let mut seen_values = [false; 256];
        for value in values {
            let value_index = usize::from(*value);
            let valid = if class == 0 {
                *value <= 11
            } else {
                let run = value >> 4;
                let magnitude = value & 0x0f;
                magnitude <= 10 && (magnitude != 0 || matches!(run, 0 | 15))
            };
            if !valid {
                return invalid_entropy();
            }
            if seen_values[value_index] {
                return invalid_entropy();
            }
            seen_values[value_index] = true;
        }

        let mut owned_values = Vec::new();
        owned_values
            .try_reserve_exact(values.len())
            .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeAllocationFailed)?;
        owned_values.extend_from_slice(values);
        Ok(Self {
            first_codes,
            value_offsets,
            code_counts,
            values: owned_values,
        })
    }

    fn decode_symbol(
        &self,
        reader: &mut SequentialJpegEntropyReader<'_>,
    ) -> Result<u8, RadrootsBlossomError> {
        let mut code = 0_u32;
        for index in 0..16 {
            code = (code << 1) | u32::from(reader.read_bit()?);
            let count = u32::from(self.code_counts[index]);
            let first = self.first_codes[index];
            if count != 0 && code >= first && code - first < count {
                let offset = self.value_offsets[index] + (code - first) as usize;
                return self
                    .values
                    .get(offset)
                    .copied()
                    .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed);
            }
        }
        invalid_entropy()
    }
}

struct SequentialJpegEntropyReader<'a> {
    bytes: &'a [u8],
    position: usize,
    current_byte: u8,
    bits_remaining: u8,
    bit_reads_remaining: u64,
}

impl<'a> SequentialJpegEntropyReader<'a> {
    fn new(bytes: &'a [u8], position: usize) -> Self {
        Self {
            bytes,
            position,
            current_byte: 0,
            bits_remaining: 0,
            bit_reads_remaining: u64::try_from(bytes.len())
                .ok()
                .and_then(|length| length.checked_mul(8))
                .unwrap_or(0),
        }
    }

    fn read_bit(&mut self) -> Result<u8, RadrootsBlossomError> {
        self.bit_reads_remaining = self
            .bit_reads_remaining
            .checked_sub(1)
            .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        if self.bits_remaining == 0 {
            self.current_byte = self.read_entropy_byte()?;
            self.bits_remaining = 8;
        }
        self.bits_remaining -= 1;
        Ok((self.current_byte >> self.bits_remaining) & 1)
    }

    fn discard_bits(&mut self, count: u8) -> Result<(), RadrootsBlossomError> {
        for _ in 0..count {
            self.read_bit()?;
        }
        Ok(())
    }

    fn read_entropy_byte(&mut self) -> Result<u8, RadrootsBlossomError> {
        let byte = *self
            .bytes
            .get(self.position)
            .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        self.position += 1;
        if byte != 0xff {
            return Ok(byte);
        }
        if self.bytes.get(self.position) == Some(&0x00) {
            self.position += 1;
            return Ok(0xff);
        }
        invalid_entropy()
    }

    fn finish_restart(&mut self, expected: u8) -> Result<(), RadrootsBlossomError> {
        if expected > 7 {
            return invalid_entropy();
        }
        self.finish_padding()?;
        let (marker, after_marker) = strict_marker(self.bytes, self.position)?;
        if marker != 0xd0 + expected {
            return invalid_entropy();
        }
        self.position = after_marker;
        Ok(())
    }

    fn finish_scan(mut self) -> Result<usize, RadrootsBlossomError> {
        self.finish_padding()?;
        let (marker, _) = strict_marker(self.bytes, self.position)?;
        if matches!(marker, 0xd0..=0xd7) {
            return invalid_entropy();
        }
        Ok(self.position)
    }

    fn finish_padding(&mut self) -> Result<(), RadrootsBlossomError> {
        if self.bits_remaining != 0 {
            let mask = (1_u16 << self.bits_remaining) - 1;
            if u16::from(self.current_byte) & mask != mask {
                return invalid_entropy();
            }
            self.bits_remaining = 0;
        }
        Ok(())
    }
}

struct SequentialJpegWorkBudget {
    scans_remaining: u8,
    blocks_remaining: u64,
    coefficient_steps_remaining: u64,
}

impl SequentialJpegWorkBudget {
    const fn new() -> Self {
        Self {
            scans_remaining: MAX_SEQUENTIAL_JPEG_SCANS,
            blocks_remaining: MAX_SEQUENTIAL_JPEG_BLOCKS,
            coefficient_steps_remaining: MAX_SEQUENTIAL_JPEG_COEFFICIENT_STEPS,
        }
    }

    fn charge_scan(&mut self) -> Result<(), RadrootsBlossomError> {
        self.scans_remaining = self
            .scans_remaining
            .checked_sub(1)
            .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        Ok(())
    }

    fn charge_block(&mut self) -> Result<(), RadrootsBlossomError> {
        self.blocks_remaining = self
            .blocks_remaining
            .checked_sub(1)
            .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        Ok(())
    }

    fn charge_coefficient_step(&mut self) -> Result<(), RadrootsBlossomError> {
        self.coefficient_steps_remaining = self
            .coefficient_steps_remaining
            .checked_sub(1)
            .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        Ok(())
    }
}

pub(super) fn validate(
    bytes: &[u8],
    container: JpegContainerInspection,
) -> Result<(), RadrootsBlossomError> {
    if !bytes.starts_with(b"\xff\xd8") {
        return invalid_entropy();
    }
    let mut position = 2_usize;
    let mut frame: Option<SequentialJpegFrame> = None;
    let mut dc_tables: [Option<SequentialJpegHuffmanTable>; 4] = core::array::from_fn(|_| None);
    let mut ac_tables: [Option<SequentialJpegHuffmanTable>; 4] = core::array::from_fn(|_| None);
    let mut restart_interval = 0_usize;
    let mut seen_components = [false; 4];
    let mut saw_scan = false;
    let mut records = 0_usize;
    let mut work_budget = SequentialJpegWorkBudget::new();
    loop {
        records = records
            .checked_add(1)
            .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        if records > PUBLICATION_RASTER_MAX_CONTAINER_RECORDS {
            return invalid_entropy();
        }
        let (marker, after_marker) = strict_marker(bytes, position)?;
        match marker {
            0xd9 => {
                let current_frame = frame
                    .as_ref()
                    .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
                if after_marker != bytes.len() {
                    return invalid_entropy();
                }
                if !saw_scan {
                    return invalid_entropy();
                }
                if seen_components
                    .iter()
                    .take(current_frame.components.len())
                    .any(|seen| !seen)
                {
                    return invalid_entropy();
                }
                return Ok(());
            }
            0xc0 | 0xc1 => {
                if frame.is_some() {
                    return invalid_entropy();
                }
                let (payload, next) = strict_segment(bytes, after_marker)?;
                let parsed = parse_frame(payload)?;
                if parsed.dimensions != container.dimensions
                    || parsed.components.len() != usize::from(container.components)
                {
                    return Err(RadrootsBlossomError::PublicationRasterContainerDimensionMismatch);
                }
                frame = Some(parsed);
                position = next;
            }
            marker if is_jpeg_start_of_frame(marker) || marker == 0xcc => {
                return Err(RadrootsBlossomError::PublicationJpegProcessForbidden);
            }
            0xc4 => {
                let (payload, next) = strict_segment(bytes, after_marker)?;
                parse_huffman_tables(payload, &mut dc_tables, &mut ac_tables)?;
                position = next;
            }
            0xdd => {
                let (payload, next) = strict_segment(bytes, after_marker)?;
                if payload.len() != 2 {
                    return invalid_entropy();
                }
                restart_interval = usize::from(u16::from_be_bytes([payload[0], payload[1]]));
                position = next;
            }
            0xda => {
                work_budget.charge_scan()?;
                let current_frame = frame
                    .as_ref()
                    .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
                let (payload, entropy_start) = strict_segment(bytes, after_marker)?;
                let scan_result = parse_scan(
                    payload,
                    current_frame,
                    &seen_components,
                    &dc_tables,
                    &ac_tables,
                );
                let scan = scan_result?;
                position = validate_scan_entropy(
                    bytes,
                    entropy_start,
                    current_frame,
                    &scan,
                    &dc_tables,
                    &ac_tables,
                    restart_interval,
                    &mut work_budget,
                )?;
                for component in &scan.components {
                    seen_components[component.frame_index] = true;
                }
                saw_scan = true;
            }
            0xdb | 0xe0..=0xef | 0xfe => {
                let (_, next) = strict_segment(bytes, after_marker)?;
                position = next;
            }
            0x01 => position = after_marker,
            _ => return invalid_entropy(),
        }
    }
}

fn strict_marker(bytes: &[u8], position: usize) -> Result<(u8, usize), RadrootsBlossomError> {
    if bytes.get(position) != Some(&0xff) {
        return invalid_entropy();
    }
    let mut code_position = position;
    while bytes.get(code_position) == Some(&0xff) {
        code_position += 1;
    }
    let marker = *bytes
        .get(code_position)
        .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    if marker == 0x00 {
        return invalid_entropy();
    }
    let after_marker = code_position + 1;
    Ok((marker, after_marker))
}

fn strict_segment(
    bytes: &[u8],
    after_marker: usize,
) -> Result<(&[u8], usize), RadrootsBlossomError> {
    let payload_start = after_marker
        .checked_add(2)
        .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    let length_bytes = bytes
        .get(after_marker..payload_start)
        .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    let length = usize::from(u16::from_be_bytes([length_bytes[0], length_bytes[1]]));
    if length < 2 {
        return invalid_entropy();
    }
    let end = after_marker
        .checked_add(length)
        .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    let payload = bytes
        .get(payload_start..end)
        .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    Ok((payload, end))
}

fn parse_frame(payload: &[u8]) -> Result<SequentialJpegFrame, RadrootsBlossomError> {
    if payload.len() < 6 || payload[0] != 8 {
        return invalid_entropy();
    }
    let component_count = usize::from(payload[5]);
    let expected_length = component_count * 3 + 6;
    if !matches!(component_count, 1 | 3 | 4) || payload.len() != expected_length {
        return invalid_entropy();
    }
    let dimensions = RadrootsBlossomRasterDimensions::new(
        u32::from(u16::from_be_bytes([payload[3], payload[4]])),
        u32::from(u16::from_be_bytes([payload[1], payload[2]])),
    )?;
    let mut components = Vec::new();
    components
        .try_reserve_exact(component_count)
        .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeAllocationFailed)?;
    let mut maximum_horizontal_sampling = 0_u8;
    let mut maximum_vertical_sampling = 0_u8;
    let mut sampling_product_sum = 0_u8;
    for data in payload[6..].chunks_exact(3) {
        let horizontal_sampling = data[1] >> 4;
        let vertical_sampling = data[1] & 0x0f;
        if components
            .iter()
            .any(|component: &SequentialJpegComponent| component.id == data[0])
            || !(1..=4).contains(&horizontal_sampling)
            || !(1..=4).contains(&vertical_sampling)
            || data[2] > 3
        {
            return invalid_entropy();
        }
        sampling_product_sum += horizontal_sampling * vertical_sampling;
        if sampling_product_sum > 10 {
            return invalid_entropy();
        }
        maximum_horizontal_sampling = maximum_horizontal_sampling.max(horizontal_sampling);
        maximum_vertical_sampling = maximum_vertical_sampling.max(vertical_sampling);
        components.push(SequentialJpegComponent {
            id: data[0],
            horizontal_sampling,
            vertical_sampling,
        });
    }
    Ok(SequentialJpegFrame {
        dimensions,
        components,
        maximum_horizontal_sampling,
        maximum_vertical_sampling,
    })
}

fn parse_huffman_tables(
    payload: &[u8],
    dc_tables: &mut [Option<SequentialJpegHuffmanTable>; 4],
    ac_tables: &mut [Option<SequentialJpegHuffmanTable>; 4],
) -> Result<(), RadrootsBlossomError> {
    let mut position = 0_usize;
    while position < payload.len() {
        let selector = *payload
            .get(position)
            .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        position += 1;
        let class = selector >> 4;
        let destination = usize::from(selector & 0x0f);
        if class > 1 || destination >= 4 {
            return invalid_entropy();
        }
        let counts_end = position + 16;
        let counts_slice = payload
            .get(position..counts_end)
            .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        let code_counts: [u8; 16] = counts_slice
            .try_into()
            .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        position = counts_end;
        let value_count: usize = code_counts.iter().map(|count| usize::from(*count)).sum();
        if value_count > 256 {
            return invalid_entropy();
        }
        let values_end = position + value_count;
        let values = payload
            .get(position..values_end)
            .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        position = values_end;
        let table = SequentialJpegHuffmanTable::new(class, code_counts, values)?;
        if class == 0 {
            dc_tables[destination] = Some(table);
        } else {
            ac_tables[destination] = Some(table);
        }
    }
    Ok(())
}

fn parse_scan(
    payload: &[u8],
    frame: &SequentialJpegFrame,
    seen_components: &[bool; 4],
    dc_tables: &[Option<SequentialJpegHuffmanTable>; 4],
    ac_tables: &[Option<SequentialJpegHuffmanTable>; 4],
) -> Result<SequentialJpegScan, RadrootsBlossomError> {
    let component_count = payload.first().copied().map_or(0, usize::from);
    let expected_length = component_count * 2 + 4;
    if component_count == 0
        || component_count > frame.components.len()
        || payload.len() != expected_length
        || payload[payload.len() - 3..] != [0, 63, 0]
    {
        return invalid_entropy();
    }
    let selectors_end = component_count * 2 + 1;
    let mut components = Vec::new();
    components
        .try_reserve_exact(component_count)
        .map_err(|_| RadrootsBlossomError::PublicationRasterDecodeAllocationFailed)?;
    for data in payload[1..selectors_end].chunks_exact(2) {
        let frame_index = frame
            .components
            .iter()
            .position(|component| component.id == data[0])
            .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
        let dc_table = usize::from(data[1] >> 4);
        let ac_table = usize::from(data[1] & 0x0f);
        if components
            .iter()
            .any(|component: &SequentialJpegScanComponent| component.frame_index == frame_index)
            || seen_components[frame_index]
            || dc_table >= 4
            || ac_table >= 4
            || dc_tables[dc_table].is_none()
            || ac_tables[ac_table].is_none()
        {
            return invalid_entropy();
        }
        components.push(SequentialJpegScanComponent {
            frame_index,
            dc_table,
            ac_table,
        });
    }
    Ok(SequentialJpegScan { components })
}

#[allow(clippy::too_many_arguments)]
fn validate_scan_entropy(
    bytes: &[u8],
    entropy_start: usize,
    frame: &SequentialJpegFrame,
    scan: &SequentialJpegScan,
    dc_tables: &[Option<SequentialJpegHuffmanTable>; 4],
    ac_tables: &[Option<SequentialJpegHuffmanTable>; 4],
    restart_interval: usize,
    work_budget: &mut SequentialJpegWorkBudget,
) -> Result<usize, RadrootsBlossomError> {
    let interleaved = scan.components.len() > 1;
    let mcu_count = scan_mcu_count(frame, scan, interleaved)?;
    let mut reader = SequentialJpegEntropyReader::new(bytes, entropy_start);
    let mut expected_restart = 0_u8;
    for mcu in 0..mcu_count {
        if restart_interval != 0 && mcu != 0 && mcu % restart_interval == 0 {
            reader.finish_restart(expected_restart)?;
            expected_restart = (expected_restart + 1) & 7;
        }
        for scan_component in &scan.components {
            let frame_component = frame
                .components
                .get(scan_component.frame_index)
                .copied()
                .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
            let blocks = if interleaved {
                usize::from(frame_component.horizontal_sampling)
                    * usize::from(frame_component.vertical_sampling)
            } else {
                1
            };
            let dc_table = dc_tables
                .get(scan_component.dc_table)
                .and_then(Option::as_ref)
                .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
            let ac_table = ac_tables
                .get(scan_component.ac_table)
                .and_then(Option::as_ref)
                .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
            for _ in 0..blocks {
                work_budget.charge_block()?;
                validate_block(&mut reader, dc_table, ac_table, work_budget)?;
            }
        }
    }
    reader.finish_scan()
}

fn scan_mcu_count(
    frame: &SequentialJpegFrame,
    scan: &SequentialJpegScan,
    interleaved: bool,
) -> Result<usize, RadrootsBlossomError> {
    let width = frame.dimensions.width() as usize;
    let height = frame.dimensions.height() as usize;
    if interleaved {
        let mcu_width = 8 * usize::from(frame.maximum_horizontal_sampling);
        let mcu_height = 8 * usize::from(frame.maximum_vertical_sampling);
        return checked_mcu_grid_count(width, height, mcu_width, mcu_height);
    }
    let scan_component = scan
        .components
        .first()
        .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    let component = frame
        .components
        .get(scan_component.frame_index)
        .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)?;
    if frame.maximum_horizontal_sampling == 0 || frame.maximum_vertical_sampling == 0 {
        return invalid_entropy();
    }
    let component_width = (width * usize::from(component.horizontal_sampling))
        .div_ceil(usize::from(frame.maximum_horizontal_sampling));
    let component_height = (height * usize::from(component.vertical_sampling))
        .div_ceil(usize::from(frame.maximum_vertical_sampling));
    checked_mcu_grid_count(component_width, component_height, 8, 8)
}

fn checked_mcu_grid_count(
    width: usize,
    height: usize,
    mcu_width: usize,
    mcu_height: usize,
) -> Result<usize, RadrootsBlossomError> {
    if width == 0 {
        return invalid_entropy();
    }
    if height == 0 {
        return invalid_entropy();
    }
    if mcu_width == 0 {
        return invalid_entropy();
    }
    if mcu_height == 0 {
        return invalid_entropy();
    }
    width
        .div_ceil(mcu_width)
        .checked_mul(height.div_ceil(mcu_height))
        .ok_or(RadrootsBlossomError::PublicationRasterDecodeFailed)
}

fn validate_block(
    reader: &mut SequentialJpegEntropyReader<'_>,
    dc_table: &SequentialJpegHuffmanTable,
    ac_table: &SequentialJpegHuffmanTable,
    work_budget: &mut SequentialJpegWorkBudget,
) -> Result<(), RadrootsBlossomError> {
    let dc_magnitude = dc_table.decode_symbol(reader)?;
    reader.discard_bits(dc_magnitude)?;
    let mut coefficient = 1_usize;
    while coefficient < 64 {
        work_budget.charge_coefficient_step()?;
        let symbol = ac_table.decode_symbol(reader)?;
        let run = usize::from(symbol >> 4);
        let magnitude = symbol & 0x0f;
        if magnitude == 0 {
            if run == 0 {
                break;
            }
            coefficient += 16;
            if coefficient > 64 {
                return invalid_entropy();
            }
            continue;
        }
        coefficient += run;
        if coefficient >= 64 {
            return invalid_entropy();
        }
        reader.discard_bits(magnitude)?;
        coefficient += 1;
    }
    Ok(())
}

fn invalid_entropy<T>() -> Result<T, RadrootsBlossomError> {
    Err(RadrootsBlossomError::PublicationRasterDecodeFailed)
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use super::*;

    fn assert_decode_failed<T>(result: Result<T, RadrootsBlossomError>) {
        let error = result
            .err()
            .expect("expected publication raster decode failure");
        assert_eq!(error.code(), "publication_raster_decode_failed");
    }

    fn one_symbol_table(class: u8, symbol: u8) -> SequentialJpegHuffmanTable {
        let mut counts = [0_u8; 16];
        counts[0] = 1;
        SequentialJpegHuffmanTable::new(class, counts, &[symbol]).unwrap()
    }

    fn single_component_frame() -> SequentialJpegFrame {
        parse_frame(&[8, 0, 1, 0, 1, 1, 1, 0x11, 0]).unwrap()
    }

    fn one_symbol_tables() -> (
        [Option<SequentialJpegHuffmanTable>; 4],
        [Option<SequentialJpegHuffmanTable>; 4],
    ) {
        let mut dc_tables = core::array::from_fn(|_| None);
        dc_tables[0] = Some(one_symbol_table(0, 0));
        let mut ac_tables = core::array::from_fn(|_| None);
        ac_tables[0] = Some(one_symbol_table(1, 0));
        (dc_tables, ac_tables)
    }

    fn append_segment(bytes: &mut Vec<u8>, marker: u8, payload: &[u8]) {
        bytes.extend_from_slice(&[0xff, marker]);
        bytes.extend_from_slice(&u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
        bytes.extend_from_slice(payload);
    }

    fn one_component_frame_payload(width: u16) -> [u8; 9] {
        let [width_high, width_low] = width.to_be_bytes();
        [8, 0, 1, width_high, width_low, 1, 1, 0x11, 0]
    }

    fn single_scan_jpeg(frame_payload: &[u8], before_scan: &[u8], entropy: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xd8];
        append_segment(&mut bytes, 0xc0, frame_payload);

        let mut huffman = Vec::new();
        huffman.push(0x00);
        huffman.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        huffman.push(0);
        huffman.push(0x10);
        huffman.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        huffman.push(0);
        append_segment(&mut bytes, 0xc4, &huffman);
        bytes.extend_from_slice(before_scan);
        append_segment(&mut bytes, 0xda, &[1, 1, 0, 0, 63, 0]);
        bytes.extend_from_slice(entropy);
        bytes.extend_from_slice(&[0xff, 0xd9]);
        bytes
    }

    fn container(width: u32, components: u8) -> JpegContainerInspection {
        JpegContainerInspection {
            dimensions: RadrootsBlossomRasterDimensions::new(width, 1).unwrap(),
            components,
        }
    }

    #[test]
    fn terminal_padding_requires_one_bits_and_no_extra_entropy() {
        let mut accepted = SequentialJpegEntropyReader::new(&[0x7f], 0);
        assert_eq!(accepted.read_bit().unwrap(), 0);
        accepted.finish_padding().unwrap();

        let mut rejected = SequentialJpegEntropyReader::new(&[0x7e], 0);
        assert_eq!(rejected.read_bit().unwrap(), 0);
        assert_decode_failed(rejected.finish_padding());

        let mut exact = SequentialJpegEntropyReader::new(&[0x7f, 0xff, 0xd9], 0);
        exact.read_bit().unwrap();
        assert_eq!(exact.finish_scan().unwrap(), 1);

        let mut extra = SequentialJpegEntropyReader::new(&[0x7f, 0x00, 0xff, 0xd9], 0);
        extra.read_bit().unwrap();
        assert_decode_failed(extra.finish_scan());
    }

    #[test]
    fn restart_markers_require_exact_sequence_and_padding() {
        let mut accepted = SequentialJpegEntropyReader::new(&[0x7f, 0xff, 0xd0], 0);
        accepted.read_bit().unwrap();
        accepted.finish_restart(0).unwrap();
        assert_eq!(accepted.position, 3);

        let mut wrong = SequentialJpegEntropyReader::new(&[0x7f, 0xff, 0xd1], 0);
        wrong.read_bit().unwrap();
        assert_decode_failed(wrong.finish_restart(0));

        let mut out_of_range = SequentialJpegEntropyReader::new(&[0xff, 0xd0], 0);
        assert_decode_failed(out_of_range.finish_restart(8));

        assert_decode_failed(SequentialJpegEntropyReader::new(&[0xff, 0xd0], 0).finish_scan());
    }

    #[test]
    fn entropy_reader_and_huffman_symbol_decoding_cover_canonical_boundaries() {
        let mut stuffed = SequentialJpegEntropyReader::new(&[0xff, 0x00], 0);
        assert_eq!(stuffed.read_entropy_byte().unwrap(), 0xff);
        assert_eq!(stuffed.position, 2);
        assert_decode_failed(SequentialJpegEntropyReader::new(&[], 0).read_entropy_byte());
        assert_decode_failed(
            SequentialJpegEntropyReader::new(&[0xff, 0xd9], 0).read_entropy_byte(),
        );

        let mut counts = [0_u8; 16];
        counts[1] = 2;
        let table = SequentialJpegHuffmanTable::new(0, counts, &[7, 8]).unwrap();
        let mut first = SequentialJpegEntropyReader::new(&[0x3f], 0);
        assert_eq!(table.decode_symbol(&mut first).unwrap(), 7);
        let mut second = SequentialJpegEntropyReader::new(&[0x7f], 0);
        assert_eq!(table.decode_symbol(&mut second).unwrap(), 8);
        let mut absent = SequentialJpegEntropyReader::new(&[0x80, 0x00], 0);
        assert_decode_failed(table.decode_symbol(&mut absent));

        let malformed_table = SequentialJpegHuffmanTable {
            first_codes: [0; 16],
            value_offsets: [0; 16],
            code_counts: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            values: Vec::new(),
        };
        let mut missing_value = SequentialJpegEntropyReader::new(&[0x7f], 0);
        assert_decode_failed(malformed_table.decode_symbol(&mut missing_value));

        let noncanonical_table = SequentialJpegHuffmanTable {
            first_codes: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            value_offsets: [0; 16],
            code_counts: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            values: vec![0],
        };
        let mut code_below_first = SequentialJpegEntropyReader::new(&[0x7f, 0x00], 0);
        assert_decode_failed(noncanonical_table.decode_symbol(&mut code_below_first));
    }

    #[test]
    fn huffman_tables_reject_full_overfull_duplicate_and_oversized_inventories() {
        assert_decode_failed(SequentialJpegHuffmanTable::new(2, [0; 16], &[0]));
        assert_decode_failed(SequentialJpegHuffmanTable::new(0, [0; 16], &[]));
        assert_decode_failed(SequentialJpegHuffmanTable::new(0, [0; 16], &[0; 257]));

        let mut incomplete = [0_u8; 16];
        incomplete[0] = 1;
        SequentialJpegHuffmanTable::new(0, incomplete, &[0]).unwrap();
        assert_decode_failed(SequentialJpegHuffmanTable::new(0, incomplete, &[0, 1]));
        assert_decode_failed(SequentialJpegHuffmanTable::new(0, incomplete, &[12]));

        let mut valid_ac = [0_u8; 16];
        valid_ac[1] = 3;
        SequentialJpegHuffmanTable::new(1, valid_ac, &[0x00, 0xf0, 0x01]).unwrap();
        assert_decode_failed(SequentialJpegHuffmanTable::new(1, incomplete, &[0x0b]));
        assert_decode_failed(SequentialJpegHuffmanTable::new(1, incomplete, &[0x10]));

        let mut full = [0_u8; 16];
        full[0] = 2;
        assert_decode_failed(SequentialJpegHuffmanTable::new(0, full, &[0, 1]));

        let mut overfull = [0_u8; 16];
        overfull[0] = 3;
        assert_decode_failed(SequentialJpegHuffmanTable::new(0, overfull, &[0, 1, 2]));

        let mut duplicate = [0_u8; 16];
        duplicate[0] = 1;
        duplicate[1] = 1;
        assert_decode_failed(SequentialJpegHuffmanTable::new(0, duplicate, &[0, 0]));

        let mut oversized_payload = vec![0_u8; 1 + 16 + 257];
        oversized_payload[1] = 255;
        oversized_payload[2] = 2;
        let mut dc_tables = core::array::from_fn(|_| None);
        let mut ac_tables = core::array::from_fn(|_| None);
        assert_decode_failed(parse_huffman_tables(
            &oversized_payload,
            &mut dc_tables,
            &mut ac_tables,
        ));
    }

    #[test]
    fn marker_segment_frame_and_huffman_parsers_reject_every_invalid_shape() {
        assert_decode_failed(strict_marker(&[0x00], 0));
        assert_decode_failed(strict_marker(&[0xff], 0));
        assert_decode_failed(strict_marker(&[0xff, 0x00], 0));
        assert_eq!(strict_marker(&[0xff, 0xff, 0x01], 0).unwrap(), (0x01, 3));

        assert_decode_failed(strict_segment(&[], 0));
        assert_decode_failed(strict_segment(&[0, 1], 0));
        assert_decode_failed(strict_segment(&[0, 4, 0], 0));
        assert_decode_failed(strict_segment(&[], usize::MAX));
        assert_eq!(strict_segment(&[0, 3, 9], 0).unwrap(), (&[9][..], 3));

        assert_decode_failed(parse_frame(&[]));
        assert_decode_failed(parse_frame(&[7, 0, 1, 0, 1, 0]));
        assert_decode_failed(parse_frame(&[8, 0, 1, 0, 1, 2, 1, 0x11, 0, 2, 0x11, 0]));
        assert_decode_failed(parse_frame(&[8, 0, 1, 0, 1, 1]));
        assert_eq!(
            parse_frame(&[8, 0, 1, 0, 0, 1, 1, 0x11, 0])
                .err()
                .unwrap()
                .code(),
            "publication_raster_dimensions_out_of_range"
        );
        assert_eq!(
            parse_frame(&[8, 0, 0, 0, 1, 1, 1, 0x11, 0])
                .err()
                .unwrap()
                .code(),
            "publication_raster_dimensions_out_of_range"
        );

        let valid = one_component_frame_payload(1);
        let mut duplicate = [8, 0, 1, 0, 1, 3, 1, 0x11, 0, 1, 0x11, 0, 3, 0x11, 0];
        assert_decode_failed(parse_frame(&duplicate));
        duplicate[9] = 2;
        duplicate[7] = 0x01;
        assert_decode_failed(parse_frame(&duplicate));
        duplicate[7] = 0x51;
        assert_decode_failed(parse_frame(&duplicate));
        duplicate[7] = 0x10;
        assert_decode_failed(parse_frame(&duplicate));
        duplicate[7] = 0x15;
        assert_decode_failed(parse_frame(&duplicate));
        duplicate[7] = 0x11;
        duplicate[8] = 4;
        assert_decode_failed(parse_frame(&duplicate));
        parse_frame(&valid).unwrap();

        let mut dc_tables = core::array::from_fn(|_| None);
        let mut ac_tables = core::array::from_fn(|_| None);
        assert_decode_failed(parse_huffman_tables(
            &[0x20],
            &mut dc_tables,
            &mut ac_tables,
        ));
        assert_decode_failed(parse_huffman_tables(
            &[0x04],
            &mut dc_tables,
            &mut ac_tables,
        ));
        assert_decode_failed(parse_huffman_tables(
            &[0x00],
            &mut dc_tables,
            &mut ac_tables,
        ));
        let mut missing_value = vec![0x00, 1];
        missing_value.extend_from_slice(&[0; 15]);
        assert_decode_failed(parse_huffman_tables(
            &missing_value,
            &mut dc_tables,
            &mut ac_tables,
        ));
        parse_huffman_tables(&[], &mut dc_tables, &mut ac_tables).unwrap();
    }

    #[test]
    fn scan_parser_requires_exact_components_tables_and_sequential_parameters() {
        let frame = single_component_frame();
        let (dc_tables, ac_tables) = one_symbol_tables();
        let unseen = [false; 4];
        assert_eq!(
            parse_scan(
                &[1, 1, 0, 0, 63, 0],
                &frame,
                &unseen,
                &dc_tables,
                &ac_tables,
            )
            .unwrap()
            .components
            .len(),
            1
        );
        assert_decode_failed(parse_scan(&[], &frame, &unseen, &dc_tables, &ac_tables));
        assert_decode_failed(parse_scan(
            &[2, 1, 0, 1, 0, 0, 63, 0],
            &frame,
            &unseen,
            &dc_tables,
            &ac_tables,
        ));
        assert_decode_failed(parse_scan(&[1], &frame, &unseen, &dc_tables, &ac_tables));
        assert_decode_failed(parse_scan(
            &[1, 1, 0, 1, 63, 0],
            &frame,
            &unseen,
            &dc_tables,
            &ac_tables,
        ));
        assert_decode_failed(parse_scan(
            &[1, 2, 0, 0, 63, 0],
            &frame,
            &unseen,
            &dc_tables,
            &ac_tables,
        ));

        let three_component_frame =
            parse_frame(&[8, 0, 1, 0, 1, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0]).unwrap();
        assert_decode_failed(parse_scan(
            &[2, 1, 0, 1, 0, 0, 63, 0],
            &three_component_frame,
            &unseen,
            &dc_tables,
            &ac_tables,
        ));

        let seen = [true, false, false, false];
        assert_decode_failed(parse_scan(
            &[1, 1, 0, 0, 63, 0],
            &frame,
            &seen,
            &dc_tables,
            &ac_tables,
        ));
        assert_decode_failed(parse_scan(
            &[1, 1, 0x40, 0, 63, 0],
            &frame,
            &unseen,
            &dc_tables,
            &ac_tables,
        ));
        assert_decode_failed(parse_scan(
            &[1, 1, 0x04, 0, 63, 0],
            &frame,
            &unseen,
            &dc_tables,
            &ac_tables,
        ));

        let missing_dc = core::array::from_fn(|_| None);
        assert_decode_failed(parse_scan(
            &[1, 1, 0, 0, 63, 0],
            &frame,
            &unseen,
            &missing_dc,
            &ac_tables,
        ));
        let missing_ac = core::array::from_fn(|_| None);
        assert_decode_failed(parse_scan(
            &[1, 1, 0, 0, 63, 0],
            &frame,
            &unseen,
            &dc_tables,
            &missing_ac,
        ));
    }

    #[test]
    fn validator_covers_completion_marker_restart_and_container_agreement_rules() {
        let frame_payload = one_component_frame_payload(1);
        let valid = single_scan_jpeg(&frame_payload, &[], &[0x3f]);
        validate(&valid, container(1, 1)).unwrap();
        assert_decode_failed(validate(&[0], container(1, 1)));
        assert_decode_failed(validate(&[0xff, 0xd8, 0xff, 0xd9], container(1, 1)));

        let mut no_scan = vec![0xff, 0xd8];
        append_segment(&mut no_scan, 0xc0, &frame_payload);
        no_scan.extend_from_slice(&[0xff, 0xd9]);
        assert_decode_failed(validate(&no_scan, container(1, 1)));

        let mut trailing = valid.clone();
        trailing.push(0);
        assert_decode_failed(validate(&trailing, container(1, 1)));

        let three_component_payload = [8, 0, 1, 0, 1, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0];
        let missing_components = single_scan_jpeg(&three_component_payload, &[], &[0x3f]);
        assert_decode_failed(validate(&missing_components, container(1, 3)));

        let mut duplicate_frame = vec![0xff, 0xd8];
        append_segment(&mut duplicate_frame, 0xc0, &frame_payload);
        duplicate_frame.extend_from_slice(&valid[2..]);
        assert_decode_failed(validate(&duplicate_frame, container(1, 1)));
        assert_eq!(
            validate(&[0xff, 0xd8, 0xff, 0xcc], container(1, 1))
                .unwrap_err()
                .code(),
            "publication_jpeg_process_forbidden"
        );
        assert_eq!(
            validate(&[0xff, 0xd8, 0xff, 0xc2], container(1, 1))
                .unwrap_err()
                .code(),
            "publication_jpeg_process_forbidden"
        );
        assert_decode_failed(validate(&[0xff, 0xd8, 0xff, 0xda], container(1, 1)));
        assert_decode_failed(validate(
            &single_scan_jpeg(&frame_payload, &[0xff, 0x02], &[0x3f]),
            container(1, 1),
        ));
        validate(
            &single_scan_jpeg(&frame_payload, &[0xff, 0x01], &[0x3f]),
            container(1, 1),
        )
        .unwrap();

        assert_eq!(
            validate(&valid, container(2, 1)).unwrap_err().code(),
            "publication_raster_container_dimension_mismatch"
        );
        assert_eq!(
            validate(&valid, container(1, 3)).unwrap_err().code(),
            "publication_raster_container_dimension_mismatch"
        );

        assert_decode_failed(validate(
            &single_scan_jpeg(&frame_payload, &[0xff, 0xdd, 0, 3, 0], &[0x3f]),
            container(1, 1),
        ));
        let restart_frame = one_component_frame_payload(9);
        validate(
            &single_scan_jpeg(
                &restart_frame,
                &[0xff, 0xdd, 0, 4, 0, 1],
                &[0x3f, 0xff, 0xd0, 0x3f],
            ),
            container(9, 1),
        )
        .unwrap();
        validate(
            &single_scan_jpeg(&restart_frame, &[0xff, 0xdd, 0, 4, 0, 2], &[0x0f]),
            container(9, 1),
        )
        .unwrap();
    }

    #[test]
    fn block_validation_covers_eob_zero_runs_nonzero_runs_and_coefficient_limits() {
        let dc = one_symbol_table(0, 0);
        let eob = one_symbol_table(1, 0);
        let mut eob_reader = SequentialJpegEntropyReader::new(&[0x3f], 0);
        validate_block(
            &mut eob_reader,
            &dc,
            &eob,
            &mut SequentialJpegWorkBudget::new(),
        )
        .unwrap();

        let zrl = one_symbol_table(1, 0xf0);
        let mut zrl_reader = SequentialJpegEntropyReader::new(&[0x07], 0);
        assert_decode_failed(validate_block(
            &mut zrl_reader,
            &dc,
            &zrl,
            &mut SequentialJpegWorkBudget::new(),
        ));

        let overflowing_run = one_symbol_table(1, 0xf1);
        let mut overflowing_reader = SequentialJpegEntropyReader::new(&[0x00], 0);
        assert_decode_failed(validate_block(
            &mut overflowing_reader,
            &dc,
            &overflowing_run,
            &mut SequentialJpegWorkBudget::new(),
        ));

        let exact_run = one_symbol_table(1, 0x81);
        let mut exact_reader = SequentialJpegEntropyReader::new(&[0x00, 0x00], 0);
        validate_block(
            &mut exact_reader,
            &dc,
            &exact_run,
            &mut SequentialJpegWorkBudget::new(),
        )
        .unwrap();

        let mut mixed_counts = [0_u8; 16];
        mixed_counts[1] = 2;
        let mixed = SequentialJpegHuffmanTable::new(1, mixed_counts, &[0x11, 0]).unwrap();
        let mut mixed_reader = SequentialJpegEntropyReader::new(&[0x07], 0);
        validate_block(
            &mut mixed_reader,
            &dc,
            &mixed,
            &mut SequentialJpegWorkBudget::new(),
        )
        .unwrap();
    }

    #[test]
    fn frame_sampling_products_are_limited_to_ten() {
        let valid = [8, 0, 1, 0, 1, 3, 1, 0x22, 0, 2, 0x11, 0, 3, 0x11, 0];
        assert_eq!(
            parse_frame(&valid)
                .unwrap()
                .components
                .iter()
                .map(|component| {
                    u16::from(component.horizontal_sampling)
                        * u16::from(component.vertical_sampling)
                })
                .sum::<u16>(),
            6
        );

        let excessive = [8, 0, 1, 0, 1, 3, 1, 0x22, 0, 2, 0x22, 0, 3, 0x22, 0];
        assert_decode_failed(parse_frame(&excessive));
    }

    #[test]
    fn mcu_grid_count_checks_bounds_and_overflow() {
        assert_eq!(checked_mcu_grid_count(16, 16, 16, 16).unwrap(), 1);
        assert_eq!(checked_mcu_grid_count(17, 17, 16, 16).unwrap(), 4);
        assert_decode_failed(checked_mcu_grid_count(0, 1, 8, 8));
        assert_decode_failed(checked_mcu_grid_count(1, 0, 8, 8));
        assert_decode_failed(checked_mcu_grid_count(1, 1, 0, 8));
        assert_decode_failed(checked_mcu_grid_count(1, 1, 8, 0));
        assert_decode_failed(checked_mcu_grid_count(usize::MAX, usize::MAX, 1, 1));

        let frame = SequentialJpegFrame {
            dimensions: RadrootsBlossomRasterDimensions::new(17, 17).unwrap(),
            components: vec![SequentialJpegComponent {
                id: 1,
                horizontal_sampling: 1,
                vertical_sampling: 1,
            }],
            maximum_horizontal_sampling: 2,
            maximum_vertical_sampling: 2,
        };
        let scan = SequentialJpegScan {
            components: vec![SequentialJpegScanComponent {
                frame_index: 0,
                dc_table: 0,
                ac_table: 0,
            }],
        };
        assert_eq!(scan_mcu_count(&frame, &scan, false).unwrap(), 4);
        assert_eq!(scan_mcu_count(&frame, &scan, true).unwrap(), 4);
        assert_decode_failed(scan_mcu_count(
            &frame,
            &SequentialJpegScan {
                components: Vec::new(),
            },
            false,
        ));

        let invalid_index = SequentialJpegScan {
            components: vec![SequentialJpegScanComponent {
                frame_index: 1,
                dc_table: 0,
                ac_table: 0,
            }],
        };
        assert_decode_failed(scan_mcu_count(&frame, &invalid_index, false));

        let zero_horizontal_maximum = SequentialJpegFrame {
            dimensions: RadrootsBlossomRasterDimensions::new(1, 1).unwrap(),
            components: vec![SequentialJpegComponent {
                id: 1,
                horizontal_sampling: 1,
                vertical_sampling: 1,
            }],
            maximum_horizontal_sampling: 0,
            maximum_vertical_sampling: 1,
        };
        assert_decode_failed(scan_mcu_count(&zero_horizontal_maximum, &scan, false));
        let zero_vertical_maximum = SequentialJpegFrame {
            maximum_horizontal_sampling: 1,
            maximum_vertical_sampling: 0,
            ..zero_horizontal_maximum
        };
        assert_decode_failed(scan_mcu_count(&zero_vertical_maximum, &scan, false));
    }

    #[test]
    fn entropy_validation_rejects_unresolved_component_and_table_references() {
        let frame = single_component_frame();
        let (dc_tables, ac_tables) = one_symbol_tables();
        let invalid_component_scan = SequentialJpegScan {
            components: vec![
                SequentialJpegScanComponent {
                    frame_index: 1,
                    dc_table: 0,
                    ac_table: 0,
                },
                SequentialJpegScanComponent {
                    frame_index: 0,
                    dc_table: 0,
                    ac_table: 0,
                },
            ],
        };
        assert_decode_failed(validate_scan_entropy(
            &[0xff, 0xd9],
            0,
            &frame,
            &invalid_component_scan,
            &dc_tables,
            &ac_tables,
            0,
            &mut SequentialJpegWorkBudget::new(),
        ));

        let missing_dc_scan = SequentialJpegScan {
            components: vec![SequentialJpegScanComponent {
                frame_index: 0,
                dc_table: 1,
                ac_table: 0,
            }],
        };
        assert_decode_failed(validate_scan_entropy(
            &[0xff, 0xd9],
            0,
            &frame,
            &missing_dc_scan,
            &dc_tables,
            &ac_tables,
            0,
            &mut SequentialJpegWorkBudget::new(),
        ));

        let missing_ac_scan = SequentialJpegScan {
            components: vec![SequentialJpegScanComponent {
                frame_index: 0,
                dc_table: 0,
                ac_table: 1,
            }],
        };
        assert_decode_failed(validate_scan_entropy(
            &[0xff, 0xd9],
            0,
            &frame,
            &missing_ac_scan,
            &dc_tables,
            &ac_tables,
            0,
            &mut SequentialJpegWorkBudget::new(),
        ));
    }

    #[test]
    fn entropy_and_structural_work_budgets_fail_closed_at_exhaustion() {
        assert_eq!(MAX_SEQUENTIAL_JPEG_SCANS, 4);
        assert_eq!(MAX_SEQUENTIAL_JPEG_BLOCKS, 3_200_000);
        assert_eq!(MAX_SEQUENTIAL_JPEG_COEFFICIENT_STEPS, 204_800_000);

        let mut reader = SequentialJpegEntropyReader::new(&[0; 1], 0);
        for _ in 0..8 {
            reader.read_bit().unwrap();
        }
        assert_decode_failed(reader.read_bit());

        let mut scan_budget = SequentialJpegWorkBudget {
            scans_remaining: 1,
            ..SequentialJpegWorkBudget::new()
        };
        scan_budget.charge_scan().unwrap();
        assert_decode_failed(scan_budget.charge_scan());

        let mut block_budget = SequentialJpegWorkBudget {
            blocks_remaining: 1,
            ..SequentialJpegWorkBudget::new()
        };
        block_budget.charge_block().unwrap();
        assert_decode_failed(block_budget.charge_block());

        let mut coefficient_budget = SequentialJpegWorkBudget {
            coefficient_steps_remaining: 1,
            ..SequentialJpegWorkBudget::new()
        };
        coefficient_budget.charge_coefficient_step().unwrap();
        assert_decode_failed(coefficient_budget.charge_coefficient_step());
    }
}
