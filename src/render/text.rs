use smithay::{
    backend::allocator::Fourcc,
    backend::renderer::element::memory::MemoryRenderBuffer,
    utils::{Point, Rectangle, Size, Transform},
};
use std::collections::HashMap;

// Embedded 8x13 font bitmap for crisp UI text
const FONT_WIDTH: usize = 8;
const FONT_HEIGHT: usize = 13;

pub struct BadgeTextRenderer {
    cache: HashMap<String, MemoryRenderBuffer>,
}

impl BadgeTextRenderer {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn calculate_dimensions(text: &str) -> (i32, i32) {
        let padding_x = 10;
        let padding_y = 5;
        let char_count = text.chars().count();
        let width = (char_count * FONT_WIDTH + padding_x * 2) as i32;
        let height = (FONT_HEIGHT + padding_y * 2) as i32;
        (width, height)
    }

    pub fn render_badge_pixels(
        text: &str,
        bg_rgba: [u8; 4],
        fg_rgba: [u8; 4],
    ) -> (i32, i32, Vec<u8>) {
        let (width, height) = Self::calculate_dimensions(text);
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let corner_radius = 4;
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                let in_top_left = x < corner_radius && y < corner_radius && (corner_radius - x).pow(2) + (corner_radius - y).pow(2) > corner_radius.pow(2);
                let in_top_right = x >= width - corner_radius && y < corner_radius && (x - (width - corner_radius)).pow(2) + (corner_radius - y).pow(2) > corner_radius.pow(2);
                let in_bot_left = x < corner_radius && y >= height - corner_radius && (corner_radius - x).pow(2) + (y - (height - corner_radius)).pow(2) > corner_radius.pow(2);
                let in_bot_right = x >= width - corner_radius && y >= height - corner_radius && (x - (width - corner_radius)).pow(2) + (y - (height - corner_radius)).pow(2) > corner_radius.pow(2);

                if in_top_left || in_top_right || in_bot_left || in_bot_right {
                    pixels[idx] = 0;
                    pixels[idx + 1] = 0;
                    pixels[idx + 2] = 0;
                    pixels[idx + 3] = 0;
                } else {
                    pixels[idx] = bg_rgba[0];
                    pixels[idx + 1] = bg_rgba[1];
                    pixels[idx + 2] = bg_rgba[2];
                    pixels[idx + 3] = bg_rgba[3];
                }
            }
        }
        for (i, ch) in text.chars().enumerate() {
            let char_x = (10 + i * FONT_WIDTH) as i32;
            let char_y = 5 as i32;
            Self::draw_char(&mut pixels, width, char_x, char_y, ch, fg_rgba);
        }
        (width, height, pixels)
    }

    pub fn get_or_create_badge(
        &mut self,
        text: &str,
        bg_rgba: [u8; 4],
        fg_rgba: [u8; 4],
    ) -> MemoryRenderBuffer {
        let key = format!("{}_{:?}_{:?}", text, bg_rgba, fg_rgba);
        if let Some(buf) = self.cache.get(&key) {
            return buf.clone();
        }

        let (width, height, pixels) = Self::render_badge_pixels(text, bg_rgba, fg_rgba);

        let mut buffer = MemoryRenderBuffer::new(
            Fourcc::Abgr8888,
            Size::from((width, height)),
            1,
            Transform::Normal,
            None,
        );

        let _ = buffer.render().draw(|slice| {
            slice.copy_from_slice(&pixels);
            Result::<_, ()>::Ok(vec![Rectangle::new(Point::from((0, 0)), Size::from((width, height)))])
        });

        self.cache.insert(key, buffer.clone());
        buffer
    }

    fn draw_char(
        pixels: &mut [u8],
        stride: i32,
        x: i32,
        y: i32,
        ch: char,
        fg_rgba: [u8; 4],
    ) {
        let glyph = Self::get_glyph(ch);
        for row in 0..FONT_HEIGHT {
            let row_bits = glyph[row];
            for col in 0..FONT_WIDTH {
                if (row_bits & (1 << (7 - col))) != 0 {
                    let px = x + col as i32;
                    let py = y + row as i32;
                    let idx = ((py * stride + px) * 4) as usize;
                    if idx + 3 < pixels.len() {
                        pixels[idx] = fg_rgba[0];
                        pixels[idx + 1] = fg_rgba[1];
                        pixels[idx + 2] = fg_rgba[2];
                        pixels[idx + 3] = fg_rgba[3];
                    }
                }
            }
        }
    }

    fn get_glyph(ch: char) -> [u8; 13] {
        match ch {
            'A' | 'a' => [
                0b00000000,
                0b00000000,
                0b00111100,
                0b01100110,
                0b01100110,
                0b01111110,
                0b01100110,
                0b01100110,
                0b01100110,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'B' | 'b' => [
                0b00000000,
                0b00000000,
                0b01111100,
                0b01100110,
                0b01100110,
                0b01111100,
                0b01100110,
                0b01100110,
                0b01111100,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'C' | 'c' => [
                0b00000000,
                0b00000000,
                0b00111100,
                0b01100110,
                0b01100000,
                0b01100000,
                0b01100000,
                0b01100110,
                0b00111100,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'D' | 'd' => [
                0b00000000,
                0b00000000,
                0b01111000,
                0b01101100,
                0b01100110,
                0b01100110,
                0b01100110,
                0b01101100,
                0b01111000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'E' | 'e' => [
                0b00000000,
                0b00000000,
                0b01111110,
                0b01100000,
                0b01100000,
                0b01111100,
                0b01100000,
                0b01100000,
                0b01111110,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'F' | 'f' => [
                0b00000000,
                0b00000000,
                0b01111110,
                0b01100000,
                0b01100000,
                0b01111100,
                0b01100000,
                0b01100000,
                0b01100000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'G' | 'g' => [
                0b00000000,
                0b00000000,
                0b00111100,
                0b01100110,
                0b01100000,
                0b01101110,
                0b01100110,
                0b01100110,
                0b00111100,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'H' | 'h' => [
                0b00000000,
                0b00000000,
                0b01100110,
                0b01100110,
                0b01100110,
                0b01111110,
                0b01100110,
                0b01100110,
                0b01100110,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'I' | 'i' => [
                0b00000000,
                0b00000000,
                0b01111110,
                0b00011000,
                0b00011000,
                0b00011000,
                0b00011000,
                0b00011000,
                0b01111110,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'L' | 'l' => [
                0b00000000,
                0b00000000,
                0b01100000,
                0b01100000,
                0b01100000,
                0b01100000,
                0b01100000,
                0b01100000,
                0b01111110,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'M' | 'm' => [
                0b00000000,
                0b00000000,
                0b01100011,
                0b01110111,
                0b01111111,
                0b01101011,
                0b01100011,
                0b01100011,
                0b01100011,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'N' | 'n' => [
                0b00000000,
                0b00000000,
                0b01100011,
                0b01110011,
                0b01111011,
                0b01101111,
                0b01100111,
                0b01100011,
                0b01100011,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'O' | 'o' => [
                0b00000000,
                0b00000000,
                0b00111100,
                0b01100110,
                0b01100110,
                0b01100110,
                0b01100110,
                0b01100110,
                0b00111100,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'P' | 'p' => [
                0b00000000,
                0b00000000,
                0b01111100,
                0b01100110,
                0b01100110,
                0b01111100,
                0b01100000,
                0b01100000,
                0b01100000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'R' | 'r' => [
                0b00000000,
                0b00000000,
                0b01111100,
                0b01100110,
                0b01100110,
                0b01111100,
                0b01101100,
                0b01100110,
                0b01100011,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'S' | 's' => [
                0b00000000,
                0b00000000,
                0b00111110,
                0b01100000,
                0b01100000,
                0b00111100,
                0b00000110,
                0b00000110,
                0b01111100,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'T' | 't' => [
                0b00000000,
                0b00000000,
                0b01111111,
                0b00011000,
                0b00011000,
                0b00011000,
                0b00011000,
                0b00011000,
                0b00011000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'U' | 'u' => [
                0b00000000,
                0b00000000,
                0b01100110,
                0b01100110,
                0b01100110,
                0b01100110,
                0b01100110,
                0b01100110,
                0b00111100,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'V' | 'v' => [
                0b00000000,
                0b00000000,
                0b01100011,
                0b01100011,
                0b01100011,
                0b00110110,
                0b00110110,
                0b00011100,
                0b00011100,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            'W' | 'w' => [
                0b00000000,
                0b00000000,
                0b01100011,
                0b01100011,
                0b01100011,
                0b01101011,
                0b01111111,
                0b01110111,
                0b01100011,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            ':' => [
                0b00000000,
                0b00000000,
                0b00000000,
                0b00110000,
                0b00110000,
                0b00000000,
                0b00000000,
                0b00110000,
                0b00110000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            '[' => [
                0b00000000,
                0b00111100,
                0b00110000,
                0b00110000,
                0b00110000,
                0b00110000,
                0b00110000,
                0b00110000,
                0b00111100,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            ']' => [
                0b00000000,
                0b00111100,
                0b00001100,
                0b00001100,
                0b00001100,
                0b00001100,
                0b00001100,
                0b00001100,
                0b00111100,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            '&' => [
                0b00000000,
                0b00110000,
                0b01101100,
                0b01101100,
                0b00110000,
                0b01111000,
                0b01100110,
                0b01100110,
                0b00111011,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
            _ => [
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
                0b00000000,
            ],
        }
    }
}
