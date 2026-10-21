use anyhow::{Result, ensure};
use eframe::egui::ColorImage;
use std::{io::Read, path::Path};

const MAX_LOGO_BYTES: u64 = 2 * 1024 * 1024;

pub fn logo_from_file(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(5 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    normalize_logo(&bytes)
}

pub fn normalize_logo(bytes: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= 5 * 1024 * 1024,
        "Choose a PNG or JPEG logo up to 5 MiB."
    );
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    ensure!(
        matches!(
            reader.format(),
            Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
        ),
        "Choose a PNG or JPEG logo."
    );
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode()?;
    let logo = decoded.resize(512, 512, image::imageops::FilterType::Lanczos3);
    let mut output = std::io::Cursor::new(Vec::new());
    logo.write_to(&mut output, image::ImageFormat::Png)?;
    let png = output.into_inner();
    ensure!(
        png.len() as u64 <= MAX_LOGO_BYTES,
        "The normalized company logo is too large."
    );
    Ok(png)
}

/// Fit the entire logo inside a transparent circle instead of cropping its edges.
pub fn circular_logo(bytes: &[u8]) -> Result<ColorImage> {
    let png = normalize_logo(bytes)?;
    let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)?;
    let fit = image
        .resize(180, 180, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let mut canvas = image::RgbaImage::new(256, 256);
    image::imageops::overlay(
        &mut canvas,
        &fit,
        ((256 - fit.width()) / 2) as i64,
        ((256 - fit.height()) / 2) as i64,
    );
    Ok(ColorImage::from_rgba_unmultiplied(
        [256, 256],
        canvas.as_raw(),
    ))
}

/// Company branding is installed separately from HR behavior and never stores account data.
pub fn load_logo(path: &Path) -> Result<ColorImage> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_LOGO_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_LOGO_BYTES,
        "Company logo is too large."
    );
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let rgba = reader.decode()?.to_rgba8();
    Ok(ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}

pub fn initials(name: &str) -> String {
    let letters: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .flat_map(char::to_uppercase)
        .take(2)
        .collect();
    if letters.is_empty() {
        "HR".into()
    } else {
        letters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uploaded_logo_preserves_transparency_and_fits_a_circle_without_cropping() {
        let mut image = image::RgbaImage::from_pixel(400, 80, image::Rgba([20, 100, 150, 255]));
        image.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let normalized = normalize_logo(bytes.get_ref()).unwrap();
        assert_eq!(
            image::load_from_memory(&normalized)
                .unwrap()
                .to_rgba8()
                .get_pixel(0, 0)
                .0[3],
            0
        );
        let circle = circular_logo(&normalized).unwrap();
        assert_eq!(circle.size, [256, 256]);
        assert_eq!(circle.pixels[0].a(), 0);
        assert_eq!(circle.pixels[128 * 256].a(), 0);
        assert!(circle.pixels[128 * 256 + 39].a() > 0);
        assert!(circle.pixels[128 * 256 + 215].a() > 0);
        assert!(normalize_logo(b"not an image").is_err());
    }

    #[test]
    fn logo_preserves_alpha_and_rejects_invalid_or_oversized_images() {
        let folder = std::env::temp_dir().join(format!("hris-logo-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let logo = folder.join("company-logo.png");
        let pixels =
            image::RgbaImage::from_raw(2, 1, vec![20, 40, 60, 0, 20, 40, 60, 128]).unwrap();
        pixels.save(&logo).unwrap();
        let loaded = load_logo(&logo).unwrap();
        assert_eq!(loaded.size, [2, 1]);
        assert_eq!(loaded.pixels[0].a(), 0);
        assert_eq!(loaded.pixels[1].a(), 128);
        std::fs::write(&logo, b"not an image").unwrap();
        assert!(load_logo(&logo).is_err());
        image::RgbaImage::new(2049, 1).save(&logo).unwrap();
        assert!(load_logo(&logo).is_err());
        std::fs::write(&logo, vec![0; MAX_LOGO_BYTES as usize + 1]).unwrap();
        assert!(load_logo(&logo).is_err());
        std::fs::remove_file(logo).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
}
