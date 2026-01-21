#![forbid(unsafe_code)]

use {
    organyacat::{Interpolation, Player},
    std::{
        error::Error,
        io::{IsTerminal, Write},
    },
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let sb_path = args.next().expect("Need soundbank file");
    let org_path = args.next().expect("Need org file");
    let mut buffer: [f32; 256] = [0.0; _];
    let mut player = Player::default();

    player.load_soundbank_file(sb_path.as_ref())?;
    player.load_song_file(org_path.as_ref())?;

    let mut writer = std::io::stdout().lock();
    if writer.is_terminal() {
        return Err("Pipe me to 44 Khz 32 bit float stereo audio sink".into());
    }

    loop {
        player.write_next(&mut buffer, Interpolation::Lagrange);
        writer.write_all(bytemuck::cast_slice_mut(&mut buffer))?;
    }
}
