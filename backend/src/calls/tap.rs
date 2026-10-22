//! What the SFU forwarded, written down for a test to inspect.
//!
//! Off unless `losos-calls --tap DIR` is given, which only the VM test does.
//! It writes each video stream as an IVF file (`video-p<pid>-<mid>.ivf`,
//! the container `ffmpeg` and `vpxdec` read) and each audio stream as
//! length-prefixed frames (`audio-p<pid>-<mid>.bin`), exactly the bytes
//! str0m handed the SFU. Since the browsers encrypt every frame before it
//! leaves them, these files are what an SFU operator could see: a decoder
//! must fail on them.

use super::admit::ParticipantId;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

/// Frames kept per stream; a test needs seconds, not the whole call.
pub const FRAMES_PER_STREAM: u32 = 900;
/// Streams kept in all, so a long test cannot fill the disk.
pub const MAX_STREAMS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Audio,
    Video,
}

struct Stream {
    out: BufWriter<File>,
    frames: u32,
}

pub struct Tap {
    dir: PathBuf,
    streams: HashMap<(ParticipantId, String), Stream>,
}

impl Tap {
    pub fn new(dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Ok(Tap {
            dir,
            streams: HashMap::new(),
        })
    }

    /// One frame from `pid`'s stream `mid`, at RTP time `pts`.
    pub fn record(&mut self, pid: ParticipantId, mid: &str, kind: Kind, pts: u64, frame: &[u8]) {
        let key = (pid, mid.to_owned());
        if !self.streams.contains_key(&key) {
            if self.streams.len() >= MAX_STREAMS {
                return;
            }
            match self.open(pid, mid, kind) {
                Ok(s) => {
                    self.streams.insert(key.clone(), s);
                }
                Err(e) => {
                    tracing::warn!("tap: cannot open a file for p{pid} {mid}: {e}");
                    return;
                }
            }
        }
        let Some(s) = self.streams.get_mut(&key) else {
            return;
        };
        if s.frames >= FRAMES_PER_STREAM {
            return;
        }
        let res = match kind {
            Kind::Video => write_ivf_frame(&mut s.out, pts, frame),
            Kind::Audio => s
                .out
                .write_all(&(frame.len() as u32).to_le_bytes())
                .and_then(|_| s.out.write_all(frame)),
        };
        s.frames += 1;
        if let Err(e) = res.and_then(|_| s.out.flush()) {
            tracing::warn!("tap: write failed: {e}");
        }
    }

    fn open(&self, pid: ParticipantId, mid: &str, kind: Kind) -> std::io::Result<Stream> {
        let safe: String = mid.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let name = match kind {
            Kind::Video => format!("video-p{pid}-{safe}.ivf"),
            Kind::Audio => format!("audio-p{pid}-{safe}.bin"),
        };
        let mut out = BufWriter::new(File::create(self.dir.join(name))?);
        if kind == Kind::Video {
            write_ivf_header(&mut out)?;
        }
        Ok(Stream { out, frames: 0 })
    }
}

/// The 32-byte IVF file header: VP8, a 90 kHz clock to match RTP video.
/// The size is a guess; decoders read it from the frames.
fn write_ivf_header(w: &mut impl Write) -> std::io::Result<()> {
    let mut h = Vec::with_capacity(32);
    h.extend_from_slice(b"DKIF");
    h.extend_from_slice(&0u16.to_le_bytes()); // version
    h.extend_from_slice(&32u16.to_le_bytes()); // header size
    h.extend_from_slice(b"VP80");
    h.extend_from_slice(&640u16.to_le_bytes());
    h.extend_from_slice(&480u16.to_le_bytes());
    h.extend_from_slice(&90_000u32.to_le_bytes()); // time base denominator
    h.extend_from_slice(&1u32.to_le_bytes()); // numerator
    h.extend_from_slice(&0u32.to_le_bytes()); // frame count, unknown
    h.extend_from_slice(&0u32.to_le_bytes()); // unused
    w.write_all(&h)
}

fn write_ivf_frame(w: &mut impl Write, pts: u64, frame: &[u8]) -> std::io::Result<()> {
    w.write_all(&(frame.len() as u32).to_le_bytes())?;
    w.write_all(&pts.to_le_bytes())?;
    w.write_all(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_is_ivf_and_audio_is_length_prefixed() {
        let dir = std::env::temp_dir().join(format!("losos-tap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut tap = Tap::new(dir.clone()).unwrap();
        tap.record(3, "1", Kind::Video, 9000, b"frame-one");
        tap.record(3, "1", Kind::Video, 12000, b"two");
        tap.record(3, "0", Kind::Audio, 960, b"opus");

        let v = std::fs::read(dir.join("video-p3-1.ivf")).unwrap();
        assert_eq!(&v[0..4], b"DKIF");
        assert_eq!(&v[8..12], b"VP80");
        assert_eq!(v.len(), 32 + 12 + 9 + 12 + 3);
        assert_eq!(u32::from_le_bytes(v[32..36].try_into().unwrap()), 9);
        assert_eq!(u64::from_le_bytes(v[36..44].try_into().unwrap()), 9000);
        assert_eq!(&v[44..53], b"frame-one");

        let a = std::fs::read(dir.join("audio-p3-0.bin")).unwrap();
        assert_eq!(a, [4, 0, 0, 0, b'o', b'p', b'u', b's']);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_stream_stops_at_its_cap_and_names_stay_in_the_dir() {
        let dir = std::env::temp_dir().join(format!("losos-tap-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut tap = Tap::new(dir.clone()).unwrap();
        for i in 0..FRAMES_PER_STREAM + 10 {
            tap.record(1, "../x", Kind::Audio, i as u64, b"a");
        }
        let a = std::fs::read(dir.join("audio-p1-x.bin")).unwrap();
        assert_eq!(a.len() as u32, FRAMES_PER_STREAM * 5);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
