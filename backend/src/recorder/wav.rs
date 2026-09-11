//! Fabrication de WAV en memoire a partir de PCM brut s16le mono.

pub const SAMPLE_RATE: u32 = 16_000;
pub const BYTES_PER_SEC: usize = (SAMPLE_RATE as usize) * 2; // s16 mono

/// Encapsule un buffer PCM s16le mono 16 kHz dans un WAV (header 44 octets).
pub fn wav_from_pcm(pcm: &[u8]) -> Vec<u8> {
    let data_len = pcm.len() as u32;
    let byte_rate = SAMPLE_RATE * 2;
    let mut out = Vec::with_capacity(44 + pcm.len());

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // taille du bloc fmt
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits par sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

/// Amplitude max absolue d'un buffer PCM s16le. Sert a sauter les chunks silencieux
/// (Whisper hallucine sur du silence).
pub fn max_amplitude(pcm: &[u8]) -> i32 {
    pcm.chunks_exact(2)
        .map(|c| (i16::from_le_bytes([c[0], c[1]]) as i32).abs())
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_header() {
        let pcm = vec![0u8; 3200];
        let wav = wav_from_pcm(&pcm);
        assert_eq!(wav.len(), 44 + 3200);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]), 3200);
    }

    #[test]
    fn test_max_amplitude() {
        assert_eq!(max_amplitude(&[0, 0, 0, 0]), 0);
        // -1000 en s16le
        let sample = (-1000i16).to_le_bytes();
        assert_eq!(max_amplitude(&[sample[0], sample[1]]), 1000);
    }
}
