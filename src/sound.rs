use crate::Interpolation;

#[derive(Clone, Default)]
pub struct Sound {
    pub(crate) data: Vec<i8>,
    samples: [f32; 8],
    position: usize,
    sub_position: f32,
    pub(crate) frequency: u16,
    position_increment: f32,
    ring: i8,
    playing: bool,
    looping: bool,
    volume: f32,
    pan_left: f32,
    pan_right: f32,
    volume_left: f32,
    volume_right: f32,
    target_volume_left: f32,
    target_volume_right: f32,
    volume_ticks: u16,
    total_samples: u32,
    silence_timer: u8,
}

impl Sound {
    pub(crate) fn init(&mut self, sample_count: usize, sample_rate: u16, volume_ramp: u16) {
        self.data = vec![0; sample_count];
        self.samples.fill(0.0);
        self.position = 0;
        self.sub_position = 0.0;
        self.silence_timer = 0;
        self.total_samples = 0;
        self.frequency = 22050;
        self.volume = 1.0;
        self.pan_left = 1.0;
        self.pan_right = 1.0;
        self.set_frequency(22050, sample_rate);
        self.set_volume(0, volume_ramp);
        self.set_pan(0, volume_ramp);
        self.volume_left = self.target_volume_left;
        self.volume_right = self.target_volume_right;
        self.playing = false;
        self.looping = false;
        self.volume_ticks = 0;
        self.ring = 0;
    }

    pub(crate) fn set_frequency(&mut self, frequency: u16, out_sample_rate: u16) {
        self.frequency = frequency;
        self.position_increment = f32::from(self.frequency) / f32::from(out_sample_rate);
    }

    pub(crate) fn set_volume(&mut self, mut volume_db: i16, out_vol_ramp: u16) {
        volume_db = volume_db.clamp(-10000, 0);
        self.volume = f32::powf(10.0, f32::from(volume_db) / 2000.0);
        self.target_volume_left = self.volume * self.pan_left;
        self.target_volume_right = self.volume * self.pan_right;
        if self.total_samples == 0 {
            self.volume_left = self.target_volume_left;
            self.volume_right = self.target_volume_right;
            self.volume_ticks = 0;
        } else {
            self.volume_ticks = out_vol_ramp;
        }
    }

    pub(crate) fn set_pan(&mut self, mut pan_db: i16, out_vol_ramp: u16) {
        if pan_db < 0 {
            if pan_db < -10000 {
                pan_db = -10000;
            }
            self.pan_left = 1.0;
            self.pan_right = f32::powf(10.0, f32::from(pan_db) / 2000.0);
        } else {
            pan_db = -pan_db;
            if pan_db < -10000 {
                pan_db = -10000;
            }
            self.pan_left = f32::powf(10.0, f32::from(pan_db) / 2000.0);
            self.pan_right = 1.0;
        }
        self.target_volume_left = self.volume * self.pan_left;
        self.target_volume_right = self.volume * self.pan_right;
        if self.total_samples == 0 {
            self.volume_left = self.target_volume_left;
            self.volume_right = self.target_volume_right;
            self.volume_ticks = 0;
        } else {
            self.volume_ticks = out_vol_ramp;
        }
    }

    pub(crate) const fn play(&mut self, looping: bool) {
        if !self.playing {
            self.position = 0;
            if self.silence_timer == 0 {
                self.sub_position = 0.0;
            }
        }
        self.playing = true;
        self.looping = looping;
    }

    pub(crate) const fn stop(&mut self) {
        self.playing = false;
        self.silence_timer = 8;
    }

    pub(crate) fn write_sample(
        &mut self,
        [out_l, out_r]: &mut [f32; 2],
        interpolation: Interpolation,
    ) {
        if !(self.playing || self.silence_timer > 0) {
            return;
        }
        if self.volume_ticks > 0 {
            self.volume_left +=
                (self.target_volume_left - self.volume_left) / f32::from(self.volume_ticks);
            self.volume_right +=
                (self.target_volume_right - self.volume_right) / f32::from(self.volume_ticks);
            self.volume_ticks -= 1;
        }
        let sample_mixed = self.interpolate(interpolation);
        *out_l += sample_mixed * self.volume_left;
        *out_r += sample_mixed * self.volume_right;
        let last_position = self.position;
        self.sub_position += self.position_increment;
        // Sub position is positive, and truncation is intended here
        #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        {
            self.position += self.sub_position as usize;
        }
        self.sub_position %= 1.0;
        if self.position > last_position {
            for i in 0..(self.position - last_position) {
                self.ring = (self.ring + 1).wrapping_rem(8);
                let sample = &mut self.samples[usize::try_from(self.ring).unwrap()];
                if self.playing {
                    if self.looping {
                        *sample = f32::from(
                            (&self.data)[(last_position + i).wrapping_rem(self.data.len())],
                        ) / 128.0;
                    } else {
                        *sample = if last_position + i >= self.data.len() {
                            0.0
                        } else {
                            f32::from((self.data)[last_position + i]) / 128.0
                        };
                    }
                } else {
                    *sample = 0.0;
                    self.silence_timer = self.silence_timer.saturating_sub(1);
                }
            }
        }
        self.total_samples += 1;
        if self.playing {
            if self.position >= self.data.len() {
                if self.looping {
                    self.position = (self.position).wrapping_rem(self.data.len());
                } else {
                    self.playing = false;
                    self.silence_timer = 8;
                }
            }
        } else {
            self.position = 0;
        }
    }
    fn interpolate(&self, interpolation: Interpolation) -> f32 {
        match interpolation {
            Interpolation::None => self.samples[usize::try_from(self.ring).unwrap()],
            Interpolation::Lagrange => self.interpolate_lagrange(),
        }
    }

    fn interpolate_lagrange(&self) -> f32 {
        let margin = self.ring.wrapping_sub(2);
        let idx = usize::try_from(if margin > 8 {
            margin - 1 - 8
        } else if (margin - 1) < 0 {
            (margin - 1) + 8
        } else {
            margin - 1
        })
        .unwrap();
        let sample_a = self.samples[idx];
        let idx = usize::try_from(if margin >= 8 {
            margin - 8
        } else if margin < 0 {
            margin + 8
        } else {
            margin
        })
        .unwrap();
        let sample_b = self.samples[idx];
        let idx = usize::try_from(if margin + 1 >= 8 {
            margin + 1 - 8
        } else if (margin + 1) < 0 {
            (margin + 1) + 8
        } else {
            margin + 1
        })
        .unwrap();
        let sample_c = self.samples[idx];
        let idx = usize::try_from(if margin + 2 >= 8 {
            margin + 2 - 8
        } else if (margin + 2) < 0 {
            (margin + 2) + 8
        } else {
            margin + 2
        })
        .unwrap();
        let sample_d = self.samples[idx];
        let c0 = sample_b;
        let c1 = (1.0f32 / 6.0).mul_add(
            -sample_d,
            (1.0f32 / 2.0).mul_add(-sample_b, (1.0f32 / 3.0).mul_add(-sample_a, sample_c)),
        );
        let c2 = (1.0f32 / 2.0).mul_add(sample_a + sample_c, -sample_b);
        let c3 = (1.0f32 / 6.0).mul_add(sample_d - sample_a, 1.0 / 2.0 * (sample_b - sample_c));
        c3.mul_add(self.sub_position, c2)
            .mul_add(self.sub_position, c1)
            .mul_add(self.sub_position, c0)
    }
}
