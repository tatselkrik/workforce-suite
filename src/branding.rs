use anyhow::{Result, ensure};
use eframe::egui::ColorImage;
use std::{io::Read, path::Path};

const MAX_LOGO_BYTES: u64 = 2 * 1024 * 1024;

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
