// Copyright 2025 the Vello Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Direct queue uploads for texture data used while encoding a render.

use wgpu::{Extent3d, Origin3d, Queue, TexelCopyBufferLayout, Texture, TextureAspect};

/// Upload CPU texture data directly to its destination texture.
///
/// `Queue::write_texture` accepts the source row pitch directly and avoids creating staging
/// buffers or issuing many small `write_buffer` calls before the frame is submitted.
pub(super) fn write_texture_data(
    queue: &Queue,
    texture: &Texture,
    origin: Origin3d,
    data: &[u8],
    bytes_per_row: u32,
    rows_per_image: u32,
    extent: Extent3d,
    diagnostic: Option<fn(u32, u32)>,
) {
    let required_len = bytes_per_row as usize
        * rows_per_image.max(extent.height) as usize
        * extent.depth_or_array_layers as usize;
    debug_assert!(data.len() >= required_len);

    if let Some(diagnostic) = diagnostic {
        diagnostic(536, data.len().try_into().unwrap_or(u32::MAX));
        diagnostic(545, 0);
        diagnostic(547, data.len().try_into().unwrap_or(u32::MAX));
    }

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin,
            aspect: TextureAspect::All,
        },
        data,
        texture_upload_layout(bytes_per_row, rows_per_image),
        extent,
    );

    if let Some(diagnostic) = diagnostic {
        diagnostic(546, 0);
        diagnostic(537, data.len().try_into().unwrap_or(u32::MAX));
        diagnostic(538, extent.height);
    }
}

fn texture_upload_layout(bytes_per_row: u32, rows_per_image: u32) -> TexelCopyBufferLayout {
    TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(bytes_per_row),
        rows_per_image: Some(rows_per_image),
    }
}

#[cfg(test)]
mod tests {
    use super::texture_upload_layout;

    #[test]
    fn texture_upload_preserves_source_row_pitch_without_staging_padding() {
        let layout = texture_upload_layout(65_520, 2);

        assert_eq!(layout.offset, 0);
        assert_eq!(layout.bytes_per_row, Some(65_520));
        assert_eq!(layout.rows_per_image, Some(2));
    }
}
