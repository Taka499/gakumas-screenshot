use image::{ImageBuffer, Luma, Rgba};

/// Converts image to binary by keeping only bright pixels.
///
/// Pixels where R > threshold AND G > threshold AND B > threshold become black (text).
/// All other pixels become white (background).
///
/// This isolates bright score text from the darker background elements in the game UI.
///
/// Recommended thresholds:
/// - Screenshots (clean): 190
/// - Video frames (compressed): 160
pub fn threshold_bright_pixels(
    img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    threshold: u8,
) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    let (width, height) = img.dimensions();
    let mut output = ImageBuffer::new(width, height);

    for (x, y, pixel) in img.enumerate_pixels() {
        let r = pixel[0];
        let g = pixel[1];
        let b = pixel[2];

        // If all RGB channels exceed threshold, this is bright text → black
        // Otherwise, it's background → white
        let value = if r > threshold && g > threshold && b > threshold {
            0u8 // Black (text)
        } else {
            255u8 // White (background)
        };

        output.put_pixel(x, y, Luma([value]));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_bright_pixels() {
        // Create a small test image
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(3, 1);

        // Pixel 0: Dark (should become white)
        img.put_pixel(0, 0, Rgba([100, 100, 100, 255]));

        // Pixel 1: Bright white (should become black)
        img.put_pixel(1, 0, Rgba([250, 250, 250, 255]));

        // Pixel 2: One channel dark (should become white)
        img.put_pixel(2, 0, Rgba([250, 250, 100, 255]));

        let result = threshold_bright_pixels(&img, 190);

        assert_eq!(result.get_pixel(0, 0)[0], 255, "Dark pixel should become white");
        assert_eq!(result.get_pixel(1, 0)[0], 0, "Bright pixel should become black");
        assert_eq!(result.get_pixel(2, 0)[0], 255, "Partially dark pixel should become white");
    }
}
