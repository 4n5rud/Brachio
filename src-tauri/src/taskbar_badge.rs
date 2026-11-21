//! The Windows taskbar badge image: a red disc with the Needs you count.

const BADGE_SIZE: usize = 32;
const BADGE_RED: [u8; 4] = [0xE5, 0x48, 0x4D, 0xFF];
const BADGE_WHITE: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

/// 3x5 digit shapes, one row per entry, high bit on the left.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];
/// Shape for the "+" of "9+".
const PLUS: [u8; 5] = [0b000, 0b010, 0b111, 0b010, 0b000];

/// The text drawn in the badge: the count, or `9+` from ten up.
pub fn badge_text(count: usize) -> String {
    if count > 9 {
        "9+".to_owned()
    } else {
        count.to_string()
    }
}

/// RGBA pixels of a red round badge with the count in white, `badge_size()` square.
pub fn badge_rgba(count: usize) -> Vec<u8> {
    let mut pixels = vec![0_u8; BADGE_SIZE * BADGE_SIZE * 4];
    let centre = (BADGE_SIZE as f32 - 1.0) / 2.0;
    let radius = BADGE_SIZE as f32 / 2.0;
    for y in 0..BADGE_SIZE {
        for x in 0..BADGE_SIZE {
            let (dx, dy) = (x as f32 - centre, y as f32 - centre);
            if dx * dx + dy * dy <= radius * radius {
                put(&mut pixels, x, y, BADGE_RED);
            }
        }
    }
    let glyphs: Vec<[u8; 5]> = badge_text(count)
        .chars()
        .filter_map(|c| match c {
            '+' => Some(PLUS),
            digit => digit.to_digit(10).map(|d| DIGITS[d as usize]),
        })
        .collect();
    let scale = if glyphs.len() > 1 { 4 } else { 6 };
    let gap = scale;
    let width = glyphs.len() * 3 * scale + glyphs.len().saturating_sub(1) * gap;
    let left = (BADGE_SIZE - width) / 2;
    let top = (BADGE_SIZE - 5 * scale) / 2;
    for (index, glyph) in glyphs.iter().enumerate() {
        let origin = left + index * (3 * scale + gap);
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..3 {
                if (bits >> (2 - column)) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            put(
                                &mut pixels,
                                origin + column * scale + sx,
                                top + row * scale + sy,
                                BADGE_WHITE,
                            );
                        }
                    }
                }
            }
        }
    }
    pixels
}

fn put(pixels: &mut [u8], x: usize, y: usize, colour: [u8; 4]) {
    let start = (y * BADGE_SIZE + x) * 4;
    pixels[start..start + 4].copy_from_slice(&colour);
}

pub const fn badge_size() -> u32 {
    BADGE_SIZE as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_text_caps_at_nine_plus() {
        assert_eq!(badge_text(1), "1");
        assert_eq!(badge_text(9), "9");
        assert_eq!(badge_text(10), "9+");
    }

    #[test]
    fn badge_image_is_a_red_disc_with_white_digits() {
        let pixels = badge_rgba(8);
        assert_eq!(pixels.len(), (badge_size() * badge_size() * 4) as usize);
        assert_eq!(&pixels[0..4], &[0, 0, 0, 0]);
        let start = 16 * 4;
        assert_eq!(&pixels[start..start + 4], &BADGE_RED);
        assert!(pixels.chunks(4).any(|p| p == BADGE_WHITE));
        assert_ne!(badge_rgba(1), badge_rgba(2));
    }
}
