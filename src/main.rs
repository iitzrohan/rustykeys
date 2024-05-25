use input::{event::keyboard::KeyboardEventTrait, event::Event, Libinput, LibinputInterface};
use libc::{O_RDONLY, O_RDWR, O_WRONLY};
use rodio::{Decoder, OutputStream, Sink, Source};
use serde_json;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::os::unix::{fs::OpenOptionsExt, io::OwnedFd};
use std::path::Path;
use std::time::Duration;

struct Interface;

impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        std::fs::OpenOptions::new()
            .custom_flags(flags)
            .read((flags & O_RDONLY != 0) | (flags & O_RDWR != 0))
            .write((flags & O_WRONLY != 0) | (flags & O_RDWR != 0))
            .open(path)
            .map(|file| file.into())
            .map_err(|err| err.raw_os_error().unwrap())
    }

    fn close_restricted(&mut self, fd: OwnedFd) {
        drop(std::fs::File::from(fd));
    }
}

#[derive(serde::Deserialize)]
struct SoundPack {
    id: String,
    name: String,
    key_define_type: String,
    includes_numpad: bool,
    sound: String,
    defines: HashMap<String, Vec<u32>>,
}

fn load_sound_pack(path: &str) -> Result<SoundPack, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let sound_pack: SoundPack = serde_json::from_reader(reader)?;
    Ok(sound_pack)
}

fn play_sound(file_path: &Path, start_time: u32, duration: u32) -> Result<(), Box<dyn Error>> {
    println!(
        "Attempting to play sound segment from {:?} (start: {}, duration: {})",
        file_path, start_time, duration
    );

    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let file = File::open(file_path)?;
    let source = Decoder::new(BufReader::new(file))?
        .skip_duration(Duration::from_millis(start_time as u64))
        .take_duration(Duration::from_millis(duration as u64));

    sink.append(source);
    sink.sleep_until_end();

    println!(
        "Finished playing sound segment from {:?} (start: {}, duration: {})",
        file_path, start_time, duration
    );
    Ok(())
}

fn main() {
    let sound_pack_folder = "audio/cherrymx-black-abs";
    let sound_pack = load_sound_pack(&format!("{}/config.json", sound_pack_folder)).unwrap();

    let mut sound_files = HashMap::new();
    for (key, _) in &sound_pack.defines {
        let sound_file_path = format!("{}/{}", sound_pack_folder, sound_pack.sound);
        sound_files.insert(key.clone(), sound_file_path);
    }

    if !sound_files.contains_key("default") {
        sound_files.insert(
            "default".to_string(),
            format!("{}/{}", sound_pack_folder, sound_pack.sound),
        );
    }

    let mut input = Libinput::new_with_udev(Interface);
    input.udev_assign_seat("seat0").unwrap();

    loop {
        input.dispatch().unwrap();
        for event in &mut input {
            if let Event::Keyboard(keyboard_event) = event {
                let key_code = keyboard_event.key().to_string();
                println!("Got key event: {:?}", key_code);

                if let Some(segment) = sound_pack.defines.get(&key_code) {
                    if segment.len() == 2 {
                        let start_time = segment[0];
                        let duration = segment[1];
                        let sound_file_path = sound_files
                            .get(&key_code)
                            .unwrap_or_else(|| &sound_files["default"]);

                        if let Err(e) = play_sound(Path::new(sound_file_path), start_time, duration)
                        {
                            eprintln!("Failed to play sound segment: {}", e);
                        }
                    }
                }
            }
        }
    }
}
