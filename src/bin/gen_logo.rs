// Generator for zPDF native multi-resolution .ico and .png assets
use std::fs::File;
use std::io::Write;

fn generate_icon_pixels(size: u32) -> Vec<u8> {
    let mut bgra = vec![0u8; (size * size * 4) as usize];
    let radius = size as f32 * 0.22;
    let center = size as f32 / 2.0;

    for y in 0..size {
        // ICO bitmaps are bottom-up
        let py = (size - 1 - y) as f32;
        for x in 0..size {
            let px = x as f32;
            let idx = ((y * size + x) * 4) as usize;

            // Distance to squircle bounds
            let dx = (px - center).abs() - (center - radius);
            let dy = (py - center).abs() - (center - radius);
            let dist = if dx > 0.0 && dy > 0.0 {
                (dx * dx + dy * dy).sqrt() - radius
            } else {
                dx.max(dy)
            };

            if dist > 0.5 {
                // Fully transparent outside squircle
                continue;
            }

            let alpha = if dist < -0.5 {
                255u8
            } else {
                ((0.5 - dist) * 255.0).clamp(0.0, 255.0) as u8
            };

            // Background gradient: Dark Slate to Deep Blue (#1e2430 to #11151c)
            let t = py / size as f32;
            let mut r = (30.0 * (1.0 - t) + 16.0 * t) as u8;
            let mut g = (38.0 * (1.0 - t) + 20.0 * t) as u8;
            let mut b = (52.0 * (1.0 - t) + 28.0 * t) as u8;

            // Draw Document page in center
            let doc_left = size as f32 * 0.22;
            let doc_right = size as f32 * 0.78;
            let doc_top = size as f32 * 0.16;
            let doc_bottom = size as f32 * 0.84;
            let fold = size as f32 * 0.16;

            if px >= doc_left && px <= doc_right && py >= doc_top && py <= doc_bottom {
                // Check folded corner
                if px > (doc_right - fold) && py < (doc_top + fold) {
                    if (px - (doc_right - fold)) + (doc_top + fold - py) > fold {
                        // Folded corner flap
                        r = 200; g = 208; b = 220;
                    } else {
                        // Shadow behind fold
                        r = 255; g = 255; b = 255;
                    }
                } else {
                    // Document sheet (pure white / light pearl)
                    r = 250; g = 252; b = 255;
                }

                // Draw Bold Crimson/Coral "Z" logo on the page
                // Top bar of Z: py between 0.32 and 0.42, px between 0.32 and 0.68
                let in_top_bar = py >= size as f32 * 0.30 && py <= size as f32 * 0.40 && px >= size as f32 * 0.32 && px <= size as f32 * 0.68;
                let in_bottom_bar = py >= size as f32 * 0.60 && py <= size as f32 * 0.70 && px >= size as f32 * 0.32 && px <= size as f32 * 0.68;

                // Diagonal bar of Z
                let diag_y = (py - size as f32 * 0.40) / (size as f32 * 0.20);
                let diag_target_x = size as f32 * 0.66 - diag_y * (size as f32 * 0.32);
                let in_diagonal = py >= size as f32 * 0.38 && py <= size as f32 * 0.62 && (px - diag_target_x).abs() <= size as f32 * 0.06;

                if in_top_bar || in_bottom_bar || in_diagonal {
                    // Vibrant Fiery Crimson Red (#E6283C)
                    r = 230; g = 40; b = 60;
                }

                // Small "PDF" badge at bottom
                if py >= size as f32 * 0.73 && py <= size as f32 * 0.81 && px >= size as f32 * 0.36 && px <= size as f32 * 0.64 {
                    r = 26; g = 32; b = 44; // Dark badge
                }
            }

            bgra[idx] = b;
            bgra[idx + 1] = g;
            bgra[idx + 2] = r;
            bgra[idx + 3] = alpha;
        }
    }

    bgra
}

fn create_ico_entry(size: u32, offset: u32) -> (Vec<u8>, Vec<u8>) {
    let pixels = generate_icon_pixels(size);
    let and_mask_stride = ((size + 31) / 32) * 4;
    let and_mask = vec![0u8; (and_mask_stride * size) as usize];

    let header_size = 40u32;
    let image_size = (pixels.len() + and_mask.len()) as u32;
    let total_res_size = header_size + image_size;

    // ICONDIRENTRY (16 bytes)
    let mut entry = Vec::with_capacity(16);
    entry.push(if size == 256 { 0 } else { size as u8 }); // bWidth
    entry.push(if size == 256 { 0 } else { size as u8 }); // bHeight
    entry.push(0); // bColorCount
    entry.push(0); // bReserved
    entry.extend_from_slice(&1u16.to_le_bytes()); // wPlanes
    entry.extend_from_slice(&32u16.to_le_bytes()); // wBitCount
    entry.extend_from_slice(&total_res_size.to_le_bytes()); // dwBytesInRes
    entry.extend_from_slice(&offset.to_le_bytes()); // dwImageOffset

    // BITMAPINFOHEADER (40 bytes)
    let mut res_data = Vec::with_capacity(total_res_size as usize);
    res_data.extend_from_slice(&header_size.to_le_bytes()); // biSize
    res_data.extend_from_slice(&(size as i32).to_le_bytes()); // biWidth
    res_data.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // biHeight (double for icon!)
    res_data.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    res_data.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    res_data.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    res_data.extend_from_slice(&(pixels.len() as u32).to_le_bytes()); // biSizeImage
    res_data.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    res_data.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    res_data.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    res_data.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    res_data.extend_from_slice(&pixels);
    res_data.extend_from_slice(&and_mask);

    (entry, res_data)
}

fn main() -> std::io::Result<()> {
    let sizes = [48u32, 32, 16];
    let count = sizes.len() as u16;

    let mut dir = Vec::new();
    dir.extend_from_slice(&0u16.to_le_bytes()); // idReserved
    dir.extend_from_slice(&1u16.to_le_bytes()); // idType (1 = icon)
    dir.extend_from_slice(&count.to_le_bytes()); // idCount

    let mut current_offset = 6 + (count as u32 * 16);
    let mut entries = Vec::new();
    let mut res_blocks = Vec::new();

    for &s in &sizes {
        let (entry, res_data) = create_ico_entry(s, current_offset);
        current_offset += res_data.len() as u32;
        entries.extend_from_slice(&entry);
        res_blocks.extend_from_slice(&res_data);
    }

    let mut out = File::create("assets/logo.ico")?;
    out.write_all(&dir)?;
    out.write_all(&entries)?;
    out.write_all(&res_blocks)?;

    println!("Generated assets/logo.ico ({} bytes)", 6 + entries.len() + res_blocks.len());
    Ok(())
}
