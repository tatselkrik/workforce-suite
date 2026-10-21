use anyhow::{Context, Result, ensure};
use eframe::egui::ColorImage;
use image::ImageDecoder;
use serde::{Deserialize, Serialize};
use std::{
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const MAX_PHOTO_BYTES: usize = 2 * 1024 * 1024;
const MAX_INPUT_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PersonalDetails {
    pub full_name: String,
    pub preferred_name: String,
    pub contact_phone: String,
}

impl PersonalDetails {
    pub fn validate(&self) -> Result<()> {
        for (name, value, max) in [
            ("Full name", &self.full_name, 120),
            ("Preferred name", &self.preferred_name, 60),
            ("Contact phone", &self.contact_phone, 40),
        ] {
            ensure!(
                value.trim().chars().count() <= max && !value.chars().any(char::is_control),
                "{name} must be at most {max} characters with no line breaks."
            );
        }
        ensure!(!self.full_name.trim().is_empty(), "Enter your full name.");
        Ok(())
    }

    pub fn trimmed(&self) -> Self {
        Self {
            full_name: self.full_name.trim().into(),
            preferred_name: self.preferred_name.trim().into(),
            contact_phone: self.contact_phone.trim().into(),
        }
    }

    pub fn display_name(&self) -> &str {
        if !self.preferred_name.trim().is_empty() {
            self.preferred_name.trim()
        } else if !self.full_name.trim().is_empty() {
            self.full_name.trim()
        } else {
            "Your account"
        }
    }
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayPreferences {
    pub scale_percent: u16,
    pub clock_24h: bool,
    pub theme: ThemeChoice,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    #[default]
    Light,
    Dark,
    System,
}

impl ThemeChoice {
    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::System => "Use system setting",
        }
    }
    pub fn preference(self) -> eframe::egui::ThemePreference {
        match self {
            Self::Light => eframe::egui::ThemePreference::Light,
            Self::Dark => eframe::egui::ThemePreference::Dark,
            Self::System => eframe::egui::ThemePreference::System,
        }
    }
}

impl Default for DisplayPreferences {
    fn default() -> Self {
        Self {
            scale_percent: 100,
            clock_24h: true,
            theme: ThemeChoice::Light,
        }
    }
}

impl DisplayPreferences {
    pub fn validate(self) -> Result<()> {
        ensure!(
            (100..=125).contains(&self.scale_percent),
            "Choose a display scale between 100% and 125%."
        );
        Ok(())
    }

    pub fn time_format(self) -> &'static str {
        if self.clock_24h { "%H:%M" } else { "%I:%M %p" }
    }
}

#[derive(Clone, Default, PartialEq)]
pub struct AccountProfile {
    pub id: Uuid,
    pub email: String,
    pub pending_email: String,
    pub email_confirmed_at: Option<String>,
    pub created_at: Option<String>,
    pub last_sign_in_at: Option<String>,
    pub details: PersonalDetails,
    pub display: DisplayPreferences,
    pub photo: Option<Vec<u8>>,
    pub photo_warning: Option<String>,
}

impl AccountProfile {
    pub fn demo() -> Self {
        Self {
            email: "preview@example.invalid".into(),
            details: PersonalDetails {
                full_name: "Alex Morgan".into(),
                preferred_name: "Alex".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

// These commands contain secrets in memory. Do not derive Debug, serialize or log them.
pub enum AccountAction {
    SaveDetails(PersonalDetails),
    SaveDisplay(DisplayPreferences),
    UploadPhoto(PathBuf),
    RemovePhoto,
    ChangePassword {
        current: String,
        new: String,
        confirmation: String,
    },
    ChangeEmail {
        current: String,
        new_email: String,
    },
    SignOutOthers,
}

pub fn validate_password_change(current: &str, new: &str, confirmation: &str) -> Result<()> {
    ensure!(
        !current.is_empty() && current.len() <= 1024,
        "Enter your current password."
    );
    ensure!(
        (12..=128).contains(&new.chars().count()),
        "Use a new password between 12 and 128 characters."
    );
    ensure!(new == confirmation, "The new passwords do not match.");
    ensure!(
        new != current,
        "Choose a password different from your current password."
    );
    Ok(())
}

pub fn validate_email(email: &str) -> Result<()> {
    let email = email.trim();
    let parts: Vec<_> = email.split('@').collect();
    ensure!(
        email.len() <= 254
            && parts.len() == 2
            && !parts[0].is_empty()
            && parts[1].contains('.')
            && !parts[1].starts_with('.')
            && !parts[1].ends_with('.')
            && !email.chars().any(char::is_whitespace)
            && !email.chars().any(char::is_control),
        "Enter a valid account email address."
    );
    Ok(())
}

pub fn photo_from_file(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .context("Could not open that photo.")?
        .take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    normalize_photo(&bytes)
}

pub fn normalize_photo(bytes: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        bytes.len() <= MAX_INPUT_BYTES,
        "Choose a PNG or JPEG photo smaller than 5 MiB."
    );
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    ensure!(
        matches!(
            reader.format(),
            Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
        ),
        "Choose a PNG or JPEG photo."
    );
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(80 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .context("Could not read that photo. Use a PNG or JPEG up to 4096 by 4096 pixels.")?;
    let orientation = decoder.orientation()?;
    let mut decoded = image::DynamicImage::from_decoder(decoder)?;
    decoded.apply_orientation(orientation);
    let decoded = decoded.to_rgba8();
    let side = decoded.width().min(decoded.height());
    ensure!(side > 0, "Choose a valid photo.");
    let cropped = image::imageops::crop_imm(
        &decoded,
        (decoded.width() - side) / 2,
        (decoded.height() - side) / 2,
        side,
        side,
    );
    let resized = image::imageops::resize(
        &cropped.to_image(),
        256,
        256,
        image::imageops::FilterType::Lanczos3,
    );
    let mut output = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(resized).write_to(&mut output, image::ImageFormat::Png)?;
    let png = output.into_inner();
    ensure!(
        png.len() <= MAX_PHOTO_BYTES,
        "The prepared photo is too large."
    );
    Ok(png)
}

pub fn circular_photo(bytes: &[u8]) -> Result<ColorImage> {
    let png = normalize_photo(bytes)?;
    let mut rgba = image::load_from_memory_with_format(&png, image::ImageFormat::Png)?.to_rgba8();
    for (x, y, pixel) in rgba.enumerate_pixels_mut() {
        let distance = ((x as f32 + 0.5 - 128.0).powi(2) + (y as f32 + 0.5 - 128.0).powi(2)).sqrt();
        pixel.0[3] = (pixel.0[3] as f32 * (128.0 - distance).clamp(0.0, 1.0)).round() as u8;
    }
    Ok(ColorImage::from_rgba_unmultiplied(
        [256, 256],
        rgba.as_raw(),
    ))
}

#[cfg(windows)]
pub fn choose_photo() -> Result<Option<PathBuf>> {
    choose_image("Choose a profile photo")
}

#[cfg(windows)]
pub fn choose_image(title: &str) -> Result<Option<PathBuf>> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::UI::{Controls::Dialogs::*, Input::KeyboardAndMouse::GetActiveWindow};
    let mut filename = vec![0u16; 32768];
    let filter: Vec<u16> = "Photos (PNG, JPEG)\0*.png;*.jpg;*.jpeg\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = format!("{title}\0").encode_utf16().collect();
    // SAFETY: the UI thread owns this dialog; all buffers outlive the synchronous call.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = unsafe { GetActiveWindow() };
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrFile = filename.as_mut_ptr();
    dialog.nMaxFile = filename.len() as u32;
    dialog.Flags = OFN_EXPLORER
        | OFN_FILEMUSTEXIST
        | OFN_PATHMUSTEXIST
        | OFN_NOCHANGEDIR
        | OFN_DONTADDTORECENT;
    if unsafe { GetOpenFileNameW(&mut dialog) } != 0 {
        let length = filename
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(filename.len());
        Ok(Some(PathBuf::from(OsString::from_wide(
            &filename[..length],
        ))))
    } else {
        ensure!(
            unsafe { CommDlgExtendedError() } == 0,
            "Could not open the photo chooser. Try dragging a photo onto the Profile page."
        );
        Ok(None)
    }
}

#[cfg(not(windows))]
pub fn choose_photo() -> Result<Option<PathBuf>> {
    anyhow::bail!("Drag a PNG or JPEG onto the Profile page to choose your photo.")
}

#[cfg(not(windows))]
pub fn choose_image(_: &str) -> Result<Option<PathBuf>> {
    anyhow::bail!("Image selection currently supports Windows only.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photo_is_cropped_bounded_and_rendered_as_a_circle() {
        let mut input = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            320,
            160,
            image::Rgb([40, 120, 160]),
        ))
        .write_to(&mut input, image::ImageFormat::Jpeg)
        .unwrap();
        let png = normalize_photo(input.get_ref()).unwrap();
        let image = image::load_from_memory(&png).unwrap();
        assert_eq!((image.width(), image.height()), (256, 256));
        let circle = circular_photo(&png).unwrap();
        assert_eq!(circle.pixels[0].a(), 0);
        assert_eq!(circle.pixels[128 * 256 + 128].a(), 255);
        assert!(normalize_photo(b"not a photo").is_err());
        assert!(normalize_photo(&vec![0; MAX_INPUT_BYTES + 1]).is_err());
        let mut oversized = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(4097, 1)
            .write_to(&mut oversized, image::ImageFormat::Png)
            .unwrap();
        assert!(normalize_photo(oversized.get_ref()).is_err());
    }

    #[test]
    fn personal_fields_and_security_inputs_are_validated() {
        assert!(PersonalDetails::default().validate().is_err());
        let mut details = PersonalDetails {
            full_name: " Alex Morgan ".into(),
            ..Default::default()
        };
        assert_eq!(details.trimmed().display_name(), "Alex Morgan");
        details.preferred_name = "Alex\nAdmin".into();
        assert!(details.validate().is_err());
        assert!(
            validate_password_change("old fixture", "a long new fixture", "a long new fixture")
                .is_ok()
        );
        assert!(validate_password_change("old fixture", "short", "short").is_err());
        assert!(
            validate_password_change("old fixture", "a long new fixture", "different").is_err()
        );
        assert!(
            validate_password_change(
                "same long fixture",
                "same long fixture",
                "same long fixture"
            )
            .is_err()
        );
        assert!(validate_email("person@example.invalid").is_ok());
        assert!(validate_email("person\n@example.invalid").is_err());
        assert!(
            DisplayPreferences {
                scale_percent: 200,
                clock_24h: true,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
