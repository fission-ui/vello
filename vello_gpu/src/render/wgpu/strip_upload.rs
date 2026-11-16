use crate::GpuStrip;
use crate::util::RangedSlice;
use core::mem::size_of;

// Bound queue.write_buffer payloads while avoiding mapped-at-creation staging
// buffers. The limit is a conservative transfer size under investigation, not
// a known backend limit.
const MAX_STRIP_UPLOAD_CHUNK_SIZE: u64 = 32 * 1024;
// SwiftShader's queue stalls in the fifth consecutive 32 KiB write after four successful writes
// to the frame's strip buffer. Flush every three chunks to leave headroom for preceding queue
// operations while keeping each no-draw submission well below the observed threshold.
const MAX_STRIP_WRITES_PER_SUBMIT: usize = 3;

pub(super) fn should_submit_strip_writes(completed_chunks: usize) -> bool {
    completed_chunks >= MAX_STRIP_WRITES_PER_SUBMIT
}

pub(super) fn strip_upload_ranges(
    total_len: u64,
    max_buffer_size: u64,
) -> impl Iterator<Item = core::ops::Range<u64>> {
    let strip_size = size_of::<GpuStrip>() as u64;
    let chunk_size = MAX_STRIP_UPLOAD_CHUNK_SIZE.min(max_buffer_size) / strip_size * strip_size;
    assert!(
        chunk_size >= strip_size,
        "device buffer limit cannot hold one GPU strip"
    );
    assert_eq!(
        total_len % strip_size,
        0,
        "strip upload size must be a whole number of strips"
    );

    let mut start = 0;
    core::iter::from_fn(move || {
        if start >= total_len {
            return None;
        }
        let end = start.saturating_add(chunk_size).min(total_len);
        let range = start..end;
        start = end;
        Some(range)
    })
}

pub(super) fn reserve_strip_pair(
    upload_offset: &mut u64,
    opaque_strips: &[GpuStrip],
    alpha_strips: RangedSlice<'_, GpuStrip>,
) -> core::ops::Range<u64> {
    let strip_count = opaque_strips.len() + alpha_strips.len();
    let byte_len = (strip_count * size_of::<GpuStrip>()) as u64;
    let start = *upload_offset;
    *upload_offset = upload_offset
        .checked_add(byte_len)
        .expect("scheduled strip upload offset overflowed");
    start..*upload_offset
}

#[cfg(test)]
mod tests {
    use super::{reserve_strip_pair, should_submit_strip_writes, strip_upload_ranges};
    use crate::GpuStrip;
    use crate::util::{RangedSlice, Ranges, VecExt};
    use alloc::vec::Vec;
    use core::mem::size_of;

    fn strip(x: u16) -> GpuStrip {
        GpuStrip {
            x,
            y: 0,
            width: 1,
            dense_width_or_rect_height: 1,
            col_idx_or_rect_frac: 0,
            payload: 0,
            paint_and_rect_flag: 0,
            depth_index: 0,
        }
    }

    #[test]
    fn strip_uploads_use_distinct_ranges_and_preserve_pass_order() {
        let opaque = [strip(1)];
        let mut alpha = Vec::new();
        let mut alpha_ranges = Ranges::default();
        alpha.push_ranged(&mut alpha_ranges, strip(2));
        alpha.push_ranged(&mut alpha_ranges, strip(3));

        let mut upload_offset = 0;
        let first_range =
            reserve_strip_pair(&mut upload_offset, &opaque, alpha.ranged(&alpha_ranges));
        let second_range =
            reserve_strip_pair(&mut upload_offset, &[strip(4)], RangedSlice::empty());

        let strip_size = size_of::<GpuStrip>() as u64;
        assert_eq!(first_range, 0..3 * strip_size);
        assert_eq!(second_range, 3 * strip_size..4 * strip_size);
        assert_eq!(upload_offset, 4 * strip_size);
    }

    #[test]
    fn strip_uploads_split_oversized_queue_writes_into_bounded_ranges() {
        let total_len = 72_576;
        let ranges = strip_upload_ranges(total_len, u64::MAX).collect::<Vec<_>>();
        let strip_size = size_of::<GpuStrip>() as u64;
        let chunk_size = super::MAX_STRIP_UPLOAD_CHUNK_SIZE / strip_size * strip_size;
        let second_chunk_end = chunk_size * 2;

        assert_eq!(
            ranges,
            [
                0..chunk_size,
                chunk_size..second_chunk_end,
                second_chunk_end..total_len
            ]
        );
        assert!(
            ranges
                .iter()
                .all(|range| range.end - range.start <= super::MAX_STRIP_UPLOAD_CHUNK_SIZE)
        );
        assert!(
            ranges
                .iter()
                .all(|range| (range.end - range.start) % strip_size == 0)
        );
        assert_eq!(ranges.first().map(|range| range.start), Some(0));
        assert_eq!(ranges.last().map(|range| range.end), Some(total_len));
        assert!(ranges.windows(2).all(|pair| pair[0].end == pair[1].start));
    }

    #[test]
    fn strip_uploads_split_the_65520_byte_failure_case() {
        let total_len = 65_520;
        let ranges = strip_upload_ranges(total_len, u64::MAX).collect::<Vec<_>>();
        let strip_size = size_of::<GpuStrip>() as u64;
        let chunk_size = super::MAX_STRIP_UPLOAD_CHUNK_SIZE / strip_size * strip_size;

        assert_eq!(ranges, [0..chunk_size, chunk_size..total_len]);
        assert!(
            ranges
                .iter()
                .all(|range| range.end - range.start <= super::MAX_STRIP_UPLOAD_CHUNK_SIZE)
        );
        assert!(
            ranges
                .iter()
                .all(|range| (range.end - range.start) % strip_size == 0)
        );
    }

    #[test]
    fn strip_upload_ranges_respect_the_device_buffer_limit() {
        let strip_size = size_of::<GpuStrip>() as u64;
        let total_len = 180_000;
        let max_buffer_size = 24_000;
        let ranges = strip_upload_ranges(total_len, max_buffer_size).collect::<Vec<_>>();

        assert!(
            ranges
                .iter()
                .all(|range| range.end - range.start <= max_buffer_size)
        );
        assert!(
            ranges
                .iter()
                .all(|range| (range.end - range.start) % strip_size == 0)
        );
        assert_eq!(ranges.first().map(|range| range.start), Some(0));
        assert_eq!(ranges.last().map(|range| range.end), Some(total_len));
        assert!(ranges.windows(2).all(|pair| pair[0].end == pair[1].start));
    }

    #[test]
    fn strip_upload_flushes_before_a_fourth_queued_chunk() {
        assert!(!should_submit_strip_writes(2));
        assert!(should_submit_strip_writes(3));
    }

    #[test]
    fn strip_upload_batch_boundaries_cover_exact_multiple_and_remainder() {
        fn batch_sizes(chunk_count: usize) -> Vec<usize> {
            let mut batches = Vec::new();
            let mut pending = 0;
            for _ in 0..chunk_count {
                pending += 1;
                if should_submit_strip_writes(pending) {
                    batches.push(pending);
                    pending = 0;
                }
            }
            if pending > 0 {
                batches.push(pending);
            }
            batches
        }

        assert_eq!(batch_sizes(6), [3, 3]);
        assert_eq!(batch_sizes(8), [3, 3, 2]);
    }
}
