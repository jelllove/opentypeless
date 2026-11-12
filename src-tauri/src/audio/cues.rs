#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingCue {
    Start,
    Stop,
}

pub fn wav(cue: RecordingCue) -> Vec<u8> {
    let sample_rate = 24_000u32;
    let count = 4_320usize; // 180 ms, quiet attack/release with no clicks.
    let frequencies = match cue {
        RecordingCue::Start => [660.0, 880.0],
        RecordingCue::Stop => [660.0, 440.0],
    };
    let mut pcm = Vec::with_capacity(count * 2);
    for index in 0..count {
        let note = usize::from(index >= count / 2);
        let local = index % (count / 2);
        let progress = local as f64 / ((count / 2 - 1) as f64);
        let envelope = (std::f64::consts::PI * progress).sin().powi(2);
        let signal = (2.0 * std::f64::consts::PI * frequencies[note] * local as f64
            / sample_rate as f64)
            .sin();
        let sample = (signal * envelope * 0.18 * i16::MAX as f64) as i16;
        pcm.extend_from_slice(&sample.to_le_bytes());
    }
    crate::stt::whisper_compat::WhisperCompatProvider::build_wav(&pcm, sample_rate)
}

pub fn play(cue: RecordingCue) {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
        let bytes = wav(cue);
        let _ = PlaySoundW(
            bytes.as_ptr().cast(),
            std::ptr::null_mut(),
            SND_MEMORY | SND_NODEFAULT | SND_SYNC,
        );
    }
    #[cfg(not(target_os = "windows"))]
    {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let device = cpal::default_host()
                .default_output_device()
                .ok_or("No output device")?;
            let supported = device.default_output_config()?;
            if supported.sample_format() != cpal::SampleFormat::F32 {
                return Ok(());
            }
            let rate = supported.sample_rate() as f64;
            let channels = supported.channels() as usize;
            let config: cpal::StreamConfig = supported.into();
            let frequencies = match cue {
                RecordingCue::Start => [660.0, 880.0],
                RecordingCue::Stop => [660.0, 440.0],
            };
            let frames = (rate * 0.18) as usize;
            let mut index = 0usize;
            let stream = device.build_output_stream(
                config,
                move |data: &mut [f32], _| {
                    for frame in data.chunks_mut(channels) {
                        let value = if index < frames {
                            let note = usize::from(index >= frames / 2);
                            let local = index % (frames / 2);
                            let progress = local as f64 / (frames / 2 - 1) as f64;
                            ((2.0 * std::f64::consts::PI * frequencies[note] * local as f64 / rate)
                                .sin()
                                * (std::f64::consts::PI * progress).sin().powi(2)
                                * 0.18) as f32
                        } else {
                            0.0
                        };
                        frame.fill(value);
                        index += 1;
                    }
                },
                |_| {},
                None,
            )?;
            stream.play()?;
            std::thread::sleep(std::time::Duration::from_millis(220));
            drop(stream);
            Ok(())
        })();
        if result.is_err() {
            tracing::debug!("Recording cue playback unavailable");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_distinct_short_pcm_waves_with_silent_edges() {
        let start = wav(RecordingCue::Start);
        let stop = wav(RecordingCue::Stop);
        assert!(start.starts_with(b"RIFF"));
        assert_ne!(start, stop);
        assert!(start.len() < 20_000);
        assert_eq!(&start[44..46], &[0, 0]);
        assert_eq!(&start[start.len() - 2..], &[0, 0]);
    }
}
