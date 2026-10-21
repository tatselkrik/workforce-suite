use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use eframe::egui::ColorImage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

const MAX_LOGO_BYTES: u64 = 2 * 1024 * 1024;

/// Only company branding may survive sign-out. No employee/account/session fields.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct LoginBranding {
    pub company_name: String,
    pub logo_png: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CachedBranding {
    company_name: String,
    logo_png_base64: Option<String>,
}

fn cache_root() -> Result<std::path::PathBuf> {
    Ok(std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("Local company-branding storage is unavailable."))?
        .join("HRIS Desktop")
        .join("branding"))
}

fn cache_file(root: &Path, config: &crate::backend::ConnectionConfig) -> std::path::PathBuf {
    let mut hash = Sha256::new();
    hash.update(config.url.trim().trim_end_matches('/'));
    hash.update([0]);
    hash.update(config.publishable_key.trim());
    root.join(format!("{:x}.json", hash.finalize()))
}

pub fn load_login_branding(config: &crate::backend::ConnectionConfig) -> Option<LoginBranding> {
    load_login_branding_at(&cache_root().ok()?, config).ok()
}

fn load_login_branding_at(
    root: &Path,
    config: &crate::backend::ConnectionConfig,
) -> Result<LoginBranding> {
    config.validate()?;
    let mut bytes = Vec::new();
    std::fs::File::open(cache_file(root, config))?
        .take(3 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 3 * 1024 * 1024,
        "Invalid company-branding cache."
    );
    let cached: CachedBranding = serde_json::from_slice(&bytes)?;
    ensure!(
        !cached.company_name.trim().is_empty() && cached.company_name.chars().count() <= 100,
        "Invalid company name."
    );
    let logo_png = cached
        .logo_png_base64
        .map(|encoded| {
            let png = STANDARD.decode(encoded)?;
            ensure!(png.len() as u64 <= MAX_LOGO_BYTES, "Invalid cached logo.");
            normalize_logo(&png)
        })
        .transpose()?;
    Ok(LoginBranding {
        company_name: cached.company_name,
        logo_png,
    })
}

pub fn save_login_branding(
    config: &crate::backend::ConnectionConfig,
    branding: &LoginBranding,
) -> Result<()> {
    save_login_branding_at(&cache_root()?, config, branding)
}

fn save_login_branding_at(
    root: &Path,
    config: &crate::backend::ConnectionConfig,
    branding: &LoginBranding,
) -> Result<()> {
    config.validate()?;
    ensure!(
        !branding.company_name.trim().is_empty() && branding.company_name.chars().count() <= 100,
        "Invalid company name."
    );
    let cached = CachedBranding {
        company_name: branding.company_name.trim().into(),
        logo_png_base64: branding
            .logo_png
            .as_ref()
            .map(|png| normalize_logo(png).map(|normalized| STANDARD.encode(normalized)))
            .transpose()?,
    };
    std::fs::create_dir_all(root)?;
    let temporary = root.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temporary, serde_json::to_vec(&cached)?)?;
    let result = std::fs::rename(&temporary, cache_file(root, config));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result?;
    Ok(())
}

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
    fn login_branding_survives_restart_clears_removed_logo_and_is_scoped_to_deployment() {
        let root =
            std::env::temp_dir().join(format!("hris-branding-test-{}", uuid::Uuid::new_v4()));
        let config = crate::backend::ConnectionConfig {
            url: "https://company.example.invalid".into(),
            publishable_key: "sb_publishable_synthetic_fixture".into(),
        };
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(80, 80, image::Rgba([0, 0, 0, 0]))
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let mut branding = LoginBranding {
            company_name: "Synthetic company".into(),
            logo_png: Some(normalize_logo(&encoded.into_inner()).unwrap()),
        };
        save_login_branding_at(&root, &config, &branding).unwrap();
        assert!(load_login_branding_at(&root, &config).unwrap() == branding);
        let raw = std::fs::read_to_string(cache_file(&root, &config)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 2);
        assert!(!raw.contains(&config.url) && !raw.contains(&config.publishable_key));
        let other = crate::backend::ConnectionConfig {
            url: "https://another.example.invalid".into(),
            ..config.clone()
        };
        assert!(load_login_branding_at(&root, &other).is_err());
        branding.logo_png = None;
        save_login_branding_at(&root, &config, &branding).unwrap();
        assert!(load_login_branding_at(&root, &config).unwrap() == branding);
        std::fs::remove_file(cache_file(&root, &config)).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn invalid_or_oversized_login_branding_is_ignored_without_opening_company_records() {
        let root =
            std::env::temp_dir().join(format!("hris-branding-test-{}", uuid::Uuid::new_v4()));
        let config = crate::backend::ConnectionConfig {
            url: "https://company.example.invalid".into(),
            publishable_key: "sb_publishable_synthetic_fixture".into(),
        };
        std::fs::create_dir(&root).unwrap();
        let file = cache_file(&root, &config);
        for bytes in [
            b"malformed JSON".to_vec(),
            vec![b' '; 3 * 1024 * 1024 + 1],
            br#"{"company_name":"Synthetic","logo_png_base64":null,"session":"forbidden"}"#
                .to_vec(),
        ] {
            std::fs::write(&file, bytes).unwrap();
            assert!(load_login_branding_at(&root, &config).is_err());
        }
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

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
