extern crate core;

use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, mpsc};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use config_file::FromConfigFile;
use log::{debug, error};
use rodio::Sink;

use crate::config::Config;
use crate::launchpad::Launchpad;
use crate::referential::Referential;

mod config;
mod referential;
mod audio;
mod midi;
mod launchpad;

const WHITE_COLOR: u8 = 3;
const GREEN_COLOR: u8 = 87;
const VOICE_VOLUME: f32 = 1.0;
const LOOPBACK_VOLUME: f32 = 0.10;

// Midi Output Event for the launchpad is a tuple of 3 elements:
// - The note id
// - The color
// - A boolean to know if the light should blink or not
type MidiOutputEvent = (u8, u8, bool);
type NoteEvent = (u8, bool);

fn main() -> Result<(), Box<dyn Error>> {

  // TODO: Load the configuration file from config.yaml, if not found generate a default one
  // Also it should be initialized inside the config module.
  let mut config = Config::from_config_file("config.yaml").unwrap();

  env_logger::init();

  let mut midi_in = midir::MidiInput::new("midir reading input")?;
  midi_in.ignore(midir::Ignore::All);

  debug!("Available MIDI input ports:");
  for p in midi::get_midi_input_devices() {
    debug!("  - {}", p);
  }

  let mut conn_out;

  match midi::select_midi_output_device(config.get_midi_out_device().unwrap()) {
    Some(conn) => {
      conn_out = conn;
    }
    _ => {
      error!("No midi output port found, have you plugged your midi device ?");
      return Ok(());
    }
  }

  let midi_in_device_name = config.get_midi_in_device().unwrap();
  let launchpad = Arc::new(Launchpad::get_launchpad(midi_in_device_name));

  debug!("Available output ports:");
  for p in midi::get_midi_output_devices() {
    debug!("  - {}", p);
  }

  let output_handle;

  match audio::select_output_device(config.get_output_device().unwrap()) {
    Some(stream) => {
      output_handle = stream;
    }
    None => {
      error!("No audio device found");
      return Ok(());
    }
  }

  debug!("Available output devices:");
  for device in audio::get_output_devices() {
    debug!("  - {}", device);
  }

  let virtual_handle;
  match audio::select_output_device(&config.get_virtual_device().unwrap()) {
    Some(handle) => {
      virtual_handle = handle;
    }
    None => {
      error!("No virtual device found");
      return Ok(());
    }
  }

  let mut referential = Referential::new(&launchpad);
  referential.init(String::from("pages"));

  if referential.get_nb_pages() == 0 {
    error!("No pages found");
    return Ok(());
  }

  let (tx_note, rx_note): (Sender<NoteEvent>, Receiver<NoteEvent>) = mpsc::channel();
  let (tx_midi, rx_midi): (Sender<MidiOutputEvent>, Receiver<MidiOutputEvent>) = mpsc::channel();

  midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, true);

  // Activate programmer mode
  conn_out.send(launchpad.programmer_mode_command()).unwrap();

  // Thread to manage the midi LEDs
  thread::spawn(move || {
    for (note_id, color, should_blink) in rx_midi {
      let channel = if should_blink { 145 } else { 144 };
      let color = if should_blink { WHITE_COLOR } else { color };
      conn_out.send(&[channel, note_id, color]).unwrap();
    }
  });

  let _conn_in = midi::listen_midi_input(midi_in_device_name, tx_note);
  let mut sinks: HashMap<u8, (Sink, Sink)> = HashMap::new();

  // Main loop to receive the midi events, also block the main thread from exiting.
  for (note_id, is_on) in rx_note {
    if !is_on && config.is_hold_to_play_enabled() {
      if let Some((audio_sink, virtual_sink)) = sinks.remove(&note_id) {
        audio_sink.stop();
        virtual_sink.stop();
      }
      continue;
    }
    if is_on {
      if note_id == launchpad.end_session_note() {
        midi::actions::end_session(&tx_midi);
        break;
      }
      if note_id == launchpad.stop_note() {
        midi::actions::stop_note(&mut sinks);
        continue;
      }
      if note_id == launchpad.first_page_note() {
        referential.first_page();

        midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, false);
        continue;
      }
      if note_id == launchpad.last_page_note() {
        referential.last_page();

        midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, false);
        continue;
      }
      if note_id == launchpad.prev_page_note() {
        referential.previous_page();

        midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, false);
        continue;
      }
      if note_id == launchpad.next_page_note() {
        referential.next_page();

        midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, false);
        continue;
      }
      if note_id == launchpad.swap_hold_mode_note() {
        config.swap_hold_to_play();

        midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, true);
        continue;
      }
      let bookmark_notes = launchpad.bookmark_notes();
      if bookmark_notes.contains(&note_id) {
        let index = bookmark_notes.iter().position(|&r| r == note_id).unwrap();
        if !config.bookmark_exists(index) {
          continue;
        }
        referential.set_current_bookmark(bookmark_notes[index]);
        // Get the bookmark parameter from Config based on index
        let bookmark_path = config.get_bookmark(index).expect("No path found for bookmark");
        referential.init(bookmark_path);

        midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, true);
        continue;
      }
      if let Some(note) = &referential.get_note(note_id) {
        // Clone to ensure the value won't be freed until it's no longer used.
        // As without it,
        // the note would be tied with the temporary value from &referential.get_note(note_id)
        let note = note.clone();
        let (audio_sink, duration) = audio::play_sound(&output_handle, note.path, VOICE_VOLUME);
        let (virtual_sink, _duration) = audio::play_sound(&virtual_handle, note.path, LOOPBACK_VOLUME);

        if !config.is_hold_to_play_enabled() && let Some(duration) = duration {
          // Light on the note and light off after the duration
          let thread_tx_midi = tx_midi.clone();
          thread_tx_midi.send((note.note_id, note.color, true)).unwrap();
          thread::spawn(move || {
            thread::sleep(duration);
            thread_tx_midi.send((note.note_id, note.color, false)).unwrap();
          });
        }

        sinks.insert(note.note_id, (audio_sink, virtual_sink));
      }
    }
  }

  Ok(())
}