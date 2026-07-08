pub mod discord;

use std::time::{SystemTime, UNIX_EPOCH};

/// Render `data` as a scannable QR code PNG (black on white, with a quiet zone).
///
/// Used to turn a Spotify Jam link into a code that can be scanned with a phone
/// camera, similar to the Spotify app's own share screen.
pub fn qr_png(data: &str) -> anyhow::Result<Vec<u8>> {
    use image::{ImageBuffer, ImageFormat, Luma};
    use qrcode::{Color, QrCode};

    let code = QrCode::new(data.as_bytes())?;
    let width = code.width();
    let colors = code.to_colors();

    // 4-module quiet zone (per the QR spec) and 8px per module.
    const QUIET: usize = 4;
    const SCALE: u32 = 8;

    let side = (width + QUIET * 2) as u32 * SCALE;
    let mut image = ImageBuffer::from_pixel(side, side, Luma([255u8]));

    for y in 0..width {
        for x in 0..width {
            if colors[y * width + x] != Color::Dark {
                continue;
            }

            let origin_x = (x + QUIET) as u32 * SCALE;
            let origin_y = (y + QUIET) as u32 * SCALE;

            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    image.put_pixel(origin_x + dx, origin_y + dy, Luma([0u8]));
                }
            }
        }
    }

    let mut buffer = std::io::Cursor::new(Vec::new());
    image.write_to(&mut buffer, ImageFormat::Png)?;

    Ok(buffer.into_inner())
}

pub fn get_time() -> u128 {
    let now = SystemTime::now();
    let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    since_the_epoch.as_millis()
}

pub fn time_to_string(time: u32) -> String {
    let hour = 3600;
    let min = 60;

    if time / hour >= 1 {
        format!(
            "{}h{}m{}s",
            time / hour,
            (time % hour) / min,
            (time % hour) % min
        )
    } else if time / min >= 1 {
        format!("{}m{}s", time / min, time % min)
    } else {
        format!("{}s", time)
    }
}
