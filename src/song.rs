use {
    crate::{OrgError, PROPERTY_UNUSED, read_cursor::ReadCursor},
    std::path::Path,
};

#[derive(Copy, Clone, Default)]
pub(crate) struct Event {
    pub(crate) position: u32,
    pub(crate) pitch: u8,
    pub(crate) length: u8,
    pub(crate) volume: u8,
    pub(crate) pan: u8,
}

#[derive(Default)]
pub(crate) struct Channel {
    pub(crate) instrument: u8,
    pub(crate) finetune: u16,
    pub(crate) pizzicato: bool,
    pub(crate) events: Vec<Event>,
}

pub(crate) struct Song {
    pub(crate) tempo_ms: u16,
    pub(crate) repeat_start: u32,
    pub(crate) repeat_end: u32,
    pub(crate) channels: [Channel; 16],
}

impl Default for Song {
    fn default() -> Self {
        let mut this = Self {
            tempo_ms: 125,
            repeat_start: 0,
            repeat_end: 1600,
            channels: Default::default(),
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
    pub(crate) fn read(&mut self, data: &[u8]) -> Result<(), OrgError> {
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
        let _beats = read.next_u8().unwrap();
        let _steps = read.next_u8().unwrap();
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
