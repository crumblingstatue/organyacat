use {
    crate::{
        Interpolation, OrgError, PROPERTY_UNUSED, read_cursor::ReadCursor, song::Song, sound::Sound,
    },
    std::{iter::zip, path::Path},
};

static SIZE_TABLE: [u16; 8] = [256, 256, 128, 128, 64, 32, 16, 8];
static FREQ_TABLE: [u16; 12] = [262, 277, 294, 311, 330, 349, 370, 392, 415, 440, 466, 494];
static PANNING_TABLE: [i16; 13] = [0, 43, 86, 129, 172, 215, 256, 297, 340, 383, 426, 469, 512];

type SndPair = [Sound; 2];

#[derive(Clone, Default)]
struct Melody {
    pitch: u8,
    volume: u8,
    pan: u8,
    index: usize,
    ticks: u32,
    alt: u8,
    muted: bool,
    snd_pairs: [SndPair; 8],
}
impl Melody {
    const fn pitch_alt_sound(&mut self) -> &mut Sound {
        &mut self.snd_pairs[(self.pitch / 12) as usize][self.alt as usize]
    }
}

#[derive(Default)]
struct Percussion {
    pitch: u8,
    volume: u8,
    pan: u8,
    index: usize,
    muted: bool,
    sound: Sound,
}

pub type WaveData = Vec<i8>;

/// Organya music player
pub struct Player {
    song: Song,
    position: u32,
    last_position: u32,
    samples_to_next_tick: f64,
    volume_ramp: u16,
    melodies: [Melody; 8],
    percussions: [Percussion; 8],
    volume: f32,
    sample_rate: u16,
    melody_wave_data: [i8; 25_600],
    percussion_wave_data: [WaveData; 42],
}

impl Default for Player {
    fn default() -> Self {
        let mut this = Self {
            song: Song::default(),
            position: Default::default(),
            last_position: Default::default(),
            samples_to_next_tick: Default::default(),
            volume_ramp: Default::default(),
            melodies: Default::default(),
            percussions: Default::default(),
            volume: Default::default(),
            sample_rate: Default::default(),
            melody_wave_data: [0; _],
            percussion_wave_data: [const { WaveData::new() }; _],
        };
        this.position = 0;
        this.last_position = 0;
        this.samples_to_next_tick = 0.0f64;
        this.set_sample_rate(44_100);
        this.volume = 1.0;
        for melody in &mut this.melodies {
            melody.pitch = PROPERTY_UNUSED;
            melody.volume = 200;
            melody.pan = 6;
            melody.index = 0;
            melody.ticks = 0;
            melody.alt = 0;
            melody.muted = false;
        }
        for perc in &mut this.percussions {
            perc.pitch = PROPERTY_UNUSED;
            perc.volume = 200;
            perc.pan = 6;
            perc.index = 0;
            perc.muted = false;
        }
        this.melody_wave_data.fill(0);
        this.percussion_wave_data.fill_with(WaveData::new);
        this
    }
}

impl Player {
    fn load_instruments(&mut self) {
        let ([lo, hi], []) = self.song.channels.as_chunks::<8>() else {
            unreachable!()
        };
        for (melody, chan) in zip(&mut self.melodies, lo) {
            for (j, sound) in melody.snd_pairs.iter_mut().enumerate() {
                let mut sample_count = usize::from(SIZE_TABLE[j]);
                if chan.pizzicato {
                    sample_count = sample_count.wrapping_mul(4_usize.wrapping_add(j * 4));
                }
                for ch in &mut *sound {
                    ch.init(sample_count, self.sample_rate, self.volume_ramp);
                }
                let mut wave_index = 0;
                for k in 0..sample_count {
                    let sample = self.melody_wave_data
                        [(usize::from(chan.instrument) * 0x100).wrapping_add(wave_index)];
                    sound[1].data[k] = sample;
                    sound[0].data[k] = sample;
                    wave_index = wave_index.wrapping_add(0x100 / usize::from(SIZE_TABLE[j])) & 0xff;
                }
            }
        }
        for (perc, ch) in zip(&mut self.percussions, hi) {
            let percussion_data = &self.percussion_wave_data[usize::from(ch.instrument)];
            perc.sound
                .init(percussion_data.len(), self.sample_rate, self.volume_ramp);
            for (&src, dst) in zip(percussion_data, &mut perc.sound.data) {
                *dst = src.wrapping_add(-128);
            }
        }
    }

    fn write_sample(&mut self, out: &mut [f32; 2], interpolation: Interpolation) {
        out[0] = 0.0;
        out[1] = 0.0;
        if self.samples_to_next_tick <= 0.0 {
            self.tick();
        }
        self.samples_to_next_tick -= 1.;
        for melody in &mut self.melodies {
            for sound in &mut melody.snd_pairs {
                sound[0].write_sample(out, interpolation);
                sound[1].write_sample(out, interpolation);
            }
        }
        for perc in &mut self.percussions {
            perc.sound.write_sample(out, interpolation);
        }
        out[0] *= self.volume;
        out[1] *= self.volume;
    }
    /// Read a soundbank file, which contains the samples required for playback.
    ///
    /// # Errors
    ///
    /// Returns [`OrgError::Malformed`] if the bank data is too short.
    pub fn read_soundbank(&mut self, bank_data: &[u8]) -> Result<(), OrgError> {
        let len = self.melody_wave_data.len();
        if bank_data.len() < len + 42 * 4 {
            return Err(OrgError::Malformed);
        }
        self.melody_wave_data
            .copy_from_slice(bytemuck::cast_slice(&bank_data[..len]));
        let mut read = ReadCursor(&bank_data[len..]);
        for wave_data in &mut self.percussion_wave_data {
            let Some(len) = read.next_u32_le() else {
                return Err(OrgError::Malformed);
            };
            if len != 0 {
                *wave_data = bytemuck::cast_slice(read.next_n_bytes(len as usize)).to_vec();
            }
        }
        Ok(())
    }
    /// Load a soundbank from a file. See [`Self::read_soundbank`].
    ///
    /// # Errors
    ///
    /// - Returns [`std::io::Error`] if reading the file failed.
    /// - Returns [`OrgError::Malformed`] if the bank data is too short.
    pub fn load_soundbank_file(&mut self, file_path: &Path) -> Result<(), OrgError> {
        let buffer = std::fs::read(file_path)?;
        self.read_soundbank(&buffer)
    }

    const fn set_sample_rate(&mut self, sample_rate: u16) {
        self.sample_rate = sample_rate;
        self.volume_ramp = sample_rate / 250;
    }

    /// Reads Organya song data and seeks to the beginning
    ///
    /// # Errors
    ///
    /// Returns [`OrgError::Malformed`] if the data can't be interpreted as Organya.
    pub fn read_song(&mut self, song_data: &[u8]) -> Result<(), OrgError> {
        self.song.read(song_data)?;
        self.seek(0);
        self.load_instruments();
        Ok(())
    }
    /// Reads Organya song from a file and seeks to the beginning
    ///
    /// # Errors
    ///
    /// - Returns [`std::io::Error`] if reading the file failed.
    /// - Returns [`OrgError::Malformed`] if the data can't be interpreted as Organya.
    pub fn load_song_file(&mut self, file_path: &Path) -> Result<(), OrgError> {
        self.song.load_file(file_path)?;
        self.seek(0);
        self.load_instruments();
        Ok(())
    }

    fn seek(&mut self, position: u32) {
        self.last_position = position;
        self.position = position;
        for (i, melody) in self.melodies.iter_mut().enumerate() {
            melody.index = 0;
            for (j, event) in self.song.channels[i].events.iter().enumerate() {
                if self.position <= event.position {
                    melody.index = j;
                    break;
                }
            }
        }
        for (i, perc) in self.percussions.iter_mut().enumerate() {
            perc.index = 0;
            let ch = &self.song.channels[8 + i];
            for (j, event) in ch.events.iter().enumerate() {
                if self.position <= event.position {
                    perc.index = j;
                    break;
                }
            }
        }
    }

    fn tick(&mut self) {
        self.tick_melodies();
        self.tick_percussions();
        self.last_position = self.position;
        self.position += 1;
        if self.position >= self.song.repeat_end {
            let lp = self.last_position;
            self.seek(self.song.repeat_start);
            self.last_position = lp;
        }
        self.samples_to_next_tick +=
            f64::from(self.sample_rate) * f64::from(self.song.tempo_ms) / 1000.0;
    }

    fn tick_percussions(&mut self) {
        for (perc, ch) in zip(&mut self.percussions, self.song.channels.iter().skip(8)) {
            if perc.muted {
                continue;
            }
            let Some(event) = &ch.events.get(perc.index) else {
                continue;
            };
            if self.position != event.position {
                continue;
            }
            if event.pitch != PROPERTY_UNUSED {
                perc.sound.stop();
                perc.pitch = event.pitch;
                perc.sound
                    .set_frequency(u16::from(perc.pitch) * 800 + 100, self.sample_rate);
                perc.sound.play(false);
            }
            if event.volume != PROPERTY_UNUSED {
                perc.volume = event.volume;
                perc.sound.set_volume(
                    (i16::from(perc.volume) * 100 / 0x7f - 0xff) * 8,
                    self.volume_ramp,
                );
            }
            if event.pan != PROPERTY_UNUSED {
                perc.pan = event.pan;
                perc.sound.set_pan(
                    (PANNING_TABLE[usize::from(perc.pan)] - 0x100) * 10,
                    self.volume_ramp,
                );
            }
            perc.index += 1;
        }
    }

    fn tick_melodies(&mut self) {
        for (melody, ch) in zip(&mut self.melodies, &self.song.channels) {
            if melody.index < ch.events.len() && !melody.muted {
                let event = &ch.events[melody.index];
                if self.position == event.position {
                    if event.pitch != PROPERTY_UNUSED {
                        if melody.pitch != PROPERTY_UNUSED {
                            if !ch.pizzicato {
                                melody.pitch_alt_sound().play(false);
                            }
                            melody.alt ^= 1;
                        }
                        melody.pitch = event.pitch;
                        melody.ticks = u32::from(event.length);
                        for (j, snd_pair) in melody.snd_pairs.iter_mut().enumerate() {
                            for snd in snd_pair {
                                let tbl_size = i32::from(SIZE_TABLE[j]);
                                let tbl_freq =
                                    i32::from(FREQ_TABLE[usize::from(melody.pitch % 12)]);
                                let finetune_modded = i32::from(ch.finetune) - 1000;
                                let snd_freq = tbl_size * tbl_freq * (1 << j) / 8 + finetune_modded;
                                snd.set_frequency(
                                    u16::try_from(snd_freq).unwrap(),
                                    self.sample_rate,
                                );
                            }
                        }
                        melody.pitch_alt_sound().play(!ch.pizzicato);
                    }
                    if event.volume != PROPERTY_UNUSED {
                        melody.volume = event.volume;
                        if melody.pitch != PROPERTY_UNUSED {
                            let vol = (i16::from(melody.volume) * 100 / 0x7f - 0xff) * 8;
                            melody.pitch_alt_sound().set_volume(vol, self.volume_ramp);
                        }
                    }
                    if event.pan != PROPERTY_UNUSED {
                        melody.pan = event.pan;
                        if melody.pitch != PROPERTY_UNUSED {
                            let pan = (PANNING_TABLE[usize::from(melody.pan)] - 0x100) * 10;
                            melody.pitch_alt_sound().set_pan(pan, self.volume_ramp);
                        }
                    }
                    melody.index += 1;
                }
            }
            if melody.ticks == 0 {
                if melody.pitch != PROPERTY_UNUSED && !ch.pizzicato {
                    melody.pitch_alt_sound().play(false);
                    melody.pitch = PROPERTY_UNUSED;
                }
            } else {
                melody.ticks -= 1;
            }
        }
    }

    /// Advance the song, and write 32 bit floating point samples to `out_buf`.
    pub fn write_next(&mut self, out_buf: &mut [f32], interpolation: Interpolation) {
        for chk in out_buf.as_chunks_mut().0 {
            self.write_sample(chk, interpolation);
        }
    }
}
