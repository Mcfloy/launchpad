#![feature(let_chains)]
extern crate core;

use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::{env, thread};

use config_file::FromConfigFile;
use log::{debug, error};
use rodio::{Sink};

use crate::config::Config;
use crate::launchpad::Launchpad;
use crate::referential::{Note, Referential};

mod config;
mod referential;
mod audio;
mod midi;
mod launchpad;

const WHITE_COLOR: u8 = 3;
const GREEN_COLOR: u8 = 87;

type NoteState = (Note, bool);
type NoteEvent = (u8, bool);

fn main() -> Result<(), Box<dyn Error>> {
  // TODO: Load the configuration file from config.yaml, if not found generate a default one
  // Also it should be initialized inside the config module.
  let config = Config::from_config_file("config.yaml").unwrap();

  if config.is_debug_enabled() {
    env::set_var("RUST_LOG", "debug");
  }

  let is_hold_to_play_enabled = config.is_hold_to_play_enabled();
  env::set_var("HOLD_TO_PLAY", is_hold_to_play_enabled.to_string());

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

  let (_output_stream, output_handle);

  match audio::select_output_device(config.get_output_device().unwrap()) {
    Some((stream, handle)) => {
      _output_stream = stream;
      output_handle = handle;
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

  let (_virtual_stream, virtual_handle);
  match audio::select_output_device(&config.get_virtual_device().unwrap()) {
    Some((stream, handle)) => {
      _virtual_stream = stream;
      virtual_handle = handle;
    }
    None => {
      error!("No virtual device found");
      return Ok(());
    }
  }

  let mut referential = Referential::new(&launchpad);
  referential.init(String::from("pages"));

  let referential_mutex = Arc::new(Mutex::new(referential));

  if referential_mutex.lock().unwrap().get_nb_pages() == 0 {
    error!("No pages found");
    return Ok(());
  }

  let (tx_on, rx_on): (Sender<NoteState>, Receiver<NoteState>) = mpsc::channel();
  let (tx_note, rx_note): (Sender<NoteEvent>, Receiver<NoteEvent>) = mpsc::channel();
  let (tx_midi, rx_midi): (Sender<NoteState>, Receiver<NoteState>) = mpsc::channel();

  midi::refresh_grid(&launchpad, &config, &mut referential_mutex.lock().unwrap(), &tx_midi, true);

  // Activate programmer mode
  conn_out.send(launchpad.programmer_mode_command()).unwrap();

  // Thread to manage the midi LEDs
  thread::spawn(move || {
    for event in rx_midi {
      let channel = if event.1 { 145 } else { 144 };
      let color = if event.1 { WHITE_COLOR } else { event.0.color };
      let note = event.0;
      conn_out.send(&[channel, note.note_id, color]).unwrap();
    }
  });

  let referential_clone = referential_mutex.clone();
  let launchpad_clone = launchpad.clone();
  // New thread to manage to handle the received events from the midi input
  thread::spawn(move || {
    for event in rx_note {
      // Only handle on note
      let referential = referential_clone.lock().unwrap();
      if event.1 {
        // Check if the note is a system note
        if launchpad_clone.system_notes().contains(&event.0) || launchpad_clone.bookmark_notes().contains(&event.0) {
          tx_on.send((Note::off(event.0), event.1)).unwrap();
        }
        if let Some(note) = &referential.get_note(event.0) {
          tx_on.send((*note, event.1)).unwrap();
        }
      }
    }
  });

  let mut sinks: HashMap<u8, (Sink, Sink)> = HashMap::new();

  let _conn_in = midi::listen_midi_input(midi_in_device_name, tx_note);

  // Loop to handle the commands
  let referential_clone = referential_mutex.clone();
  for (note, is_on) in rx_on {
    if !is_on && is_hold_to_play_enabled {
      if let Some((audio_sink, virtual_sink)) = sinks.remove(&note.note_id) {
        audio_sink.stop();
        virtual_sink.stop();
      }
      continue;
    }
    if note.note_id == launchpad.end_session_note() {
      midi::actions::end_session(&tx_midi);
      break;
    }
    if note.note_id == launchpad.stop_note() {
      midi::actions::stop_note(&mut sinks);
      continue;
    }
    if note.note_id == launchpad.first_page_note() {
      let mut referential = referential_clone.lock().unwrap();
      referential.first_page();

      midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, false);
      continue;
    }
    if note.note_id == launchpad.last_page_note() {
      let mut referential = referential_clone.lock().unwrap();
      referential.last_page();

      midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, false);
      continue;
    }
    if note.note_id == launchpad.prev_page_note() {
      let mut referential = referential_clone.lock().unwrap();
      referential.previous_page();

      midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, false);
      continue;
    }
    if note.note_id == launchpad.next_page_note() {
      let mut referential = referential_clone.lock().unwrap();
      referential.next_page();

      midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, false);
      continue;
    }
    let bookmark_notes = launchpad.bookmark_notes();
    if bookmark_notes.contains(&note.note_id) {
      let index = bookmark_notes.iter().position(|&r| r == note.note_id).unwrap();
      if !config.bookmark_exists(index) {
        continue;
      }
      let mut referential = referential_mutex.lock().unwrap();
      referential.set_current_bookmark(bookmark_notes[index]);
      // Get the bookmark parameter from Config based on index
      let bookmark_path = config.get_bookmark(index).expect("No path found for bookmark");
      referential.init(bookmark_path);

      midi::refresh_grid(&launchpad, &config, &mut referential, &tx_midi, true);
      continue;
    }

    let (audio_sink, duration) = audio::play_sound(&output_handle, note.path, 1.0);
    let (virtual_sink, _duration) = audio::play_sound(&virtual_handle, note.path, 0.1);

    if !is_hold_to_play_enabled && let Some(duration) = duration {
      // Light on the note and light off after the duration
      let thread_tx_midi = tx_midi.clone();
      thread_tx_midi.send((note, true)).unwrap();
      thread::spawn(move || {
        thread::sleep(duration);
        thread_tx_midi.send((note, false)).unwrap();
      });
    }

    sinks.insert(note.note_id, (audio_sink, virtual_sink));
  }

  Ok(())
}