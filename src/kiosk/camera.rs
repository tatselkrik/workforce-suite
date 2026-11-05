//! Memory-only camera preview/capture. No image is read from a user file or the clipboard.
use anyhow::{Result, ensure};
use image::{DynamicImage, RgbImage};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

pub struct Frame {
    pub image: RgbImage,
    pub captured_at: Instant,
}
impl Frame {
    pub fn photo(&self) -> Result<Vec<u8>> {
        ensure!(
            self.captured_at.elapsed() <= Duration::from_secs(2),
            "Camera preview is stale. Wait for a fresh frame."
        );
        ensure!(
            self.image.width() >= 80 && self.image.height() >= 80,
            "Camera image is too small."
        );
        for edge in [640, 480, 320] {
            let image = DynamicImage::ImageRgb8(self.image.clone()).thumbnail(edge, edge);
            let mut bytes = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 70)
                .encode_image(&image)?;
            if bytes.len() <= 65536 {
                return Ok(bytes);
            }
        }
        anyhow::bail!("Camera photo could not fit the evidence limit.")
    }
}
pub struct Camera {
    pub frames: mpsc::Receiver<Result<Frame, String>>,
    stop: Arc<AtomicBool>,
}
impl Camera {
    pub fn start() -> Self {
        let (sender, frames) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::spawn(move || {
            if let Err(error) = capture(sender.clone(), flag) {
                let _ = sender.try_send(Err(format!(
                    "Camera unavailable: {error}. Check Windows camera permissions or contact HR."
                )));
            }
        });
        Self { frames, stop }
    }
}
impl Drop for Camera {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn rgb_frame(data: &[u8], width: u32, height: u32, stride: i32) -> Result<RgbImage> {
    ensure!(
        width > 0 && height > 0 && width <= 4096 && height <= 2160,
        "Unsupported camera dimensions."
    );
    let row = usize::try_from(stride.unsigned_abs())?;
    ensure!(
        row >= width as usize * 4
            && data.len()
                >= row
                    .checked_mul(height as usize)
                    .ok_or_else(|| anyhow::anyhow!("Invalid camera stride."))?,
        "Incomplete camera frame."
    );
    let mut image = RgbImage::new(width, height);
    for y in 0..height {
        let source = if stride < 0 { height - 1 - y } else { y };
        let start = source as usize * row;
        for x in 0..width {
            let at = start + x as usize * 4;
            image.put_pixel(x, y, image::Rgb([data[at + 2], data[at + 1], data[at]]));
        }
    }
    Ok(image)
}
#[cfg(not(windows))]
fn capture(_: mpsc::SyncSender<Result<Frame, String>>, _: Arc<AtomicBool>) -> Result<()> {
    anyhow::bail!("Office webcam capture requires Windows.")
}

#[cfg(windows)]
fn capture(sender: mpsc::SyncSender<Result<Frame, String>>, stop: Arc<AtomicBool>) -> Result<()> {
    use windows::Win32::{Media::MediaFoundation::*, System::Com::*};
    struct Runtime;
    impl Drop for Runtime {
        fn drop(&mut self) {
            unsafe {
                let _ = MFShutdown();
                CoUninitialize();
            }
        }
    }
    // SAFETY: all COM interfaces and their buffers remain on this dedicated capture thread.
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        if let Err(e) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
            CoUninitialize();
            return Err(e.into());
        }
        let _runtime = Runtime;
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1)?;
        let attributes =
            attributes.ok_or_else(|| anyhow::anyhow!("Camera attributes unavailable."))?;
        attributes.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )?;
        let mut devices = std::ptr::null_mut();
        let mut count = 0;
        MFEnumDeviceSources(&attributes, &mut devices, &mut count)?;
        ensure!(count > 0 && !devices.is_null(), "No webcam is connected.");
        let first = (*devices).clone();
        for item in std::slice::from_raw_parts_mut(devices, count as usize) {
            drop(item.take());
        }
        CoTaskMemFree(Some(devices.cast()));
        let source: IMFMediaSource = first
            .ok_or_else(|| anyhow::anyhow!("Webcam unavailable."))?
            .ActivateObject()?;
        let mut options = None;
        MFCreateAttributes(&mut options, 1)?;
        let options = options.unwrap();
        options.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
        let reader = MFCreateSourceReaderFromMediaSource(&source, &options)?;
        let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        reader.SetStreamSelection(stream, true)?;
        let output = MFCreateMediaType()?;
        output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
        reader.SetCurrentMediaType(stream, None, &output)?;
        while !stop.load(Ordering::Relaxed) {
            let mut sample = None;
            let mut flags = 0;
            reader.ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
            let Some(sample) = sample else {
                continue;
            };
            let format = reader.GetCurrentMediaType(stream)?;
            let size = format.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let width = (size >> 32) as u32;
            let height = size as u32;
            let stride = format
                .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                .map(|v| v as i32)
                .unwrap_or((width * 4) as i32);
            let buffer = sample.ConvertToContiguousBuffer()?;
            let mut pointer = std::ptr::null_mut();
            let mut length = 0;
            buffer.Lock(&mut pointer, None, Some(&mut length))?;
            let image = if pointer.is_null() {
                Err(anyhow::anyhow!("Camera returned an empty buffer."))
            } else {
                rgb_frame(
                    std::slice::from_raw_parts(pointer, length as usize),
                    width,
                    height,
                    stride,
                )
            };
            let unlock = buffer.Unlock();
            let image = image?;
            unlock?;
            match sender.try_send(Ok(Frame {
                image,
                captured_at: Instant::now(),
            })) {
                Err(mpsc::TrySendError::Disconnected(_)) => break,
                _ => {}
            }
            std::thread::sleep(Duration::from_millis(80));
        }
        let _ = source.Shutdown();
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padded_and_negative_stride_frames_are_bounded() {
        let raw = [
            0, 0, 255, 255, 0, 255, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255, 255, 255, 255, 255, 0, 0,
            0, 0,
        ];
        let frame = rgb_frame(&raw, 2, 2, 12).unwrap();
        assert_eq!(frame.get_pixel(0, 0).0, [255, 0, 0]);
        let reversed = rgb_frame(&raw, 2, 2, -12).unwrap();
        assert_eq!(reversed.get_pixel(0, 0).0, [0, 0, 255]);
        assert!(rgb_frame(&raw[..5], 2, 2, 12).is_err());
        assert!(rgb_frame(&raw, 5000, 2, 20000).is_err());
    }
    #[test]
    fn only_fresh_bounded_camera_images_can_be_captured() {
        let mut frame = Frame {
            image: RgbImage::new(640, 480),
            captured_at: Instant::now(),
        };
        let bytes = frame.photo().unwrap();
        assert!(bytes.len() < 65536);
        assert!(image::load_from_memory(&bytes).is_ok());
        frame.captured_at = Instant::now() - Duration::from_secs(3);
        assert!(frame.photo().is_err());
    }
}
