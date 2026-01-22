use {
    crate::{OrgError, PROPERTY_UNUSED, read_cursor::ReadCursor},
    std::path::Path,
};

/// An event that happens in the song
///
/// Some properties are optional. If their value is [`PROPERTY_UNUSED`], they are ignored.
#[derive(Copy, Clone, Default)]
pub struct Event {
    /// When in the song this event happens
    pub position: u32,
    /// The pitch of the note to play
    pub pitch: u8,
    /// Length of the note to play
    pub length: u8,
    /// Volume change
    pub volume: u8,
    /// Pan change
    pub pan: u8,
}

/// A channel that plays an independent stream of audio that will be mixed together
/// with other channels to produce the final output.
///
/// There are 8 melody channels, and 8 drum channels.
#[derive(Default)]
pub struct Channel {
    /// The index of the instrument in the instrument bank
    ///
    /// Wave and melody channels have separate instrument banks.
    pub instrument: u8,
    pub(crate) finetune: u16,
    pub(crate) pizzicato: bool,
    /// The list of events for this channel
    pub events: Vec<Event>,
}

/// An Organya song
pub struct Song {
    /// Tempo of the song
    pub tempo_ms: u16,
    /// The point at which the song starts repeating
    pub repeat_start: u32,
    /// The point at which the song ends
    pub repeat_end: u32,
    /// The 16 channels of the song. There are 8 melody, and 8 drum channels.
    pub channels: [Channel; 16],
    /// Beats per measure
    pub beats_per_measure: u8,
    /// Steps per beat
    pub steps_per_beat: u8,
}

impl Default for Song {
    fn default() -> Self {
        let mut this = Self {
            tempo_ms: 125,
            repeat_start: 0,
            repeat_end: 1600,
            channels: Default::default(),
            beats_per_measure: 1,
            steps_per_beat: 1,
        };
        let ([lo, hi], []) = this.channels.as_chunks_mut::<8>() else {
            unreachable!()
        };
        let mut ins = 0;
        for ch in lo {
            ch.instrument = ins;
            ch.finetune = 1000;
            ins += 11;
        }
        for ch in hi {
            ch.finetune = 1000;
        }
        this.channels[8].instrument = 0;
        this.channels[9].instrument = 2;
        this.channels[10].instrument = 5;
        this.channels[11].instrument = 6;
        this.channels[12].instrument = 4;
        this.channels[13].instrument = 8;
        this
    }
}

impl Song {
    /// Read the song from raw bytes
    ///
    /// # Panics
    ///
    /// - May panic on I/O errors
    pub fn read(&mut self, data: &[u8]) -> Result<(), OrgError> {
        if data.len() < 114 {
            return Err(OrgError::Malformed);
        }
        let mut read = ReadCursor(data);
        if read.next_bytes() != Some(b"Org-") {
            return Err(OrgError::Malformed);
        }
        let version = ((read.next_u8().unwrap()) - b'0') * 10 + ((read.next_u8().unwrap()) - b'0');
        if !(1..=3).contains(&version) {
            return Err(OrgError::Malformed);
        }
        self.tempo_ms = read.next_u16_le().unwrap();
        self.beats_per_measure = read.next_u8().unwrap();
        self.steps_per_beat = read.next_u8().unwrap();
        self.repeat_start = read.next_u32_le().unwrap();
        self.repeat_end = read.next_u32_le().unwrap();
        for (i, ch) in self.channels.iter_mut().enumerate() {
            ch.finetune = read.next_u16_le().unwrap();
            ch.instrument = read.next_u8().unwrap();
            let pizzicato = read.next_u8().unwrap();
            ch.pizzicato = if version > 1 { pizzicato == 1 } else { false };
            if i < 8 && ch.instrument >= 100 || i >= 8 && ch.instrument >= 42 {
                ch.instrument = 0;
            }
            let event_count = read.next_u16_le().unwrap();
            ch.events = vec![Event::default(); usize::from(event_count)];
        }
        for ch in &mut self.channels {
            let len = ch.events.len();
            for (j, evt) in ch.events.iter_mut().enumerate() {
                evt.position = read.u32_le_at(j * 4);
                evt.pitch = read.u8_at((len * 4) + j);
                evt.length = read.u8_at((len * 5) + j);
                evt.volume = read.u8_at((len * 6) + j);
                evt.pan = read.u8_at((len * 7) + j);
                if evt.pitch >= 96 {
                    evt.pitch = PROPERTY_UNUSED;
                }
                if evt.length == 0 {
                    evt.length = 1;
                }
                if evt.pan > 12 && evt.pan != PROPERTY_UNUSED {
                    evt.pan = 6;
                }
            }
            read.skip(len * 8);
        }
        Ok(())
    }

    pub(crate) fn load_file(&mut self, file_path: &Path) -> Result<(), OrgError> {
        let buffer = std::fs::read(file_path)?;
        self.read(&buffer)
    }
}
