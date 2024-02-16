#![feature(let_chains)]
extern crate core;

use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use config_file::FromConfigFile;
use log::{debug, error};
use rodio::{Sink};

use crate::config::Config;
use crate::referential::{Note, Page, Referential};

mod config;
mod referential;
mod audio;
mod midi;

const WHITE_COLOR: u8 = 3;
const GREEN_COLOR: u8 = 87;

// Exclusive notes from Launchpad Mini Mk3, change them at your convenience.
const FIRST_PAGE_NOTE: u8 = 91;
const LAST_PAGE_NOTE: u8 = 92;
const PREV_PAGE_NOTE: u8 = 93;
const NEXT_PAGE_NOTE: u8 = 94;
const END_SESSION_NOTE: u8 = 95;
const STOP_NOTE: u8 = 19;
const SYSTEM_NOTES: [u8; 6] = [FIRST_PAGE_NOTE, LAST_PAGE_NOTE, PREV_PAGE_NOTE, NEXT_PAGE_NOTE, END_SESSION_NOTE, STOP_NOTE];
const BOOKMARK_NOTES: [u8; 7] = [89, 79, 69, 59, 49, 39, 29];

type NoteState = (Note, bool);
type NoteEvent = (u8, bool);

#[derive(Debug, Clone)]
struct AppState {
  pub page: Page,
}

impl AppState {
  pub fn get_page(&self) -> &Page {
    &self.page
  }

  pub fn set_page(&mut self, page: Page) {
    self.page = page;
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  // TODO: Load the configuration file from config.yaml, if not found generate a default one
  // Also it should be initialized inside the config module.
  let config = Config::from_config_file("config.yaml").unwrap();
  // TODO: Replace with an internal environment variable
  let is_debug_enabled = config.is_debug_enabled();
  // TODO: Replace with an internal environment variable
  let is_hold_to_play_enabled = config.is_hold_to_play_enabled();

  let mut midi_in = midir::MidiInput::new("midir reading input")?;
  midi_in.ignore(midir::Ignore::All);

  if is_debug_enabled {
    debug!("Available MIDI input ports:");
    for p in midi::get_midi_input_devices() {
      debug!("  - {}", p);
    }
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

  if is_debug_enabled {
    debug!("Available output ports:");
    for p in midi::get_midi_output_devices() {
      debug!("  - {}", p);
    }
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

  if is_debug_enabled {
    debug!("Available output devices:");
    for device in audio::get_output_devices() {
      debug!("  - {}", device);
    }
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

  let mut referential = Referential::new();
  referential.init(String::from("pages"));

  if referential.get_nb_pages() == 0 {
    error!("No pages found");
    return Ok(());
  }

  let current_page: AtomicU8 = AtomicU8::new(0);
  let page = referential.get_page(0).unwrap().clone();
  let app_state = Arc::new(Mutex::new(AppState {
    page
  }));

  let (tx_on, rx_on): (Sender<NoteState>, Receiver<NoteState>) = mpsc::channel();
  let (tx_note, rx_note): (Sender<NoteEvent>, Receiver<NoteEvent>) = mpsc::channel();
  let (tx_midi, rx_midi): (Sender<NoteState>, Receiver<NoteState>) = mpsc::channel();

  refresh_grid(&config, &mut referential, &current_page, &app_state, &tx_midi, true);

  // Activate programmer mode
  let midi_in_device_name = config.get_midi_in_device().unwrap();
  if midi_in_device_name.contains("LPMiniMK3") {
    println!("Programmer mode activated");
    conn_out.send(&[240, 0, 32, 41, 2, 13, 14, 1, 247]).unwrap();
  } else if midi_in_device_name.contains("LPX") {
    conn_out.send(&[240, 0, 32, 41, 2, 12, 127, 247]).unwrap();
  } else {
    println!("No programmer mode available for this device. Please create an issue on the repository.");
  }

  // Thread to manage the midi LEDs
  thread::spawn(move || {
    for event in rx_midi {
      let channel = if event.1 { 145 } else { 144 };
      let color = if event.1 { WHITE_COLOR } else { event.0.color };
      let note = event.0;
      conn_out.send(&[channel, note.note_id, color]).unwrap();
    }
  });

  let thread_app_state = Arc::clone(&app_state);

  // New thread to manage to handle the received events from the midi input
  thread::spawn(move || {
    let thread_app_state = thread_app_state;
    for event in rx_note {
      // Only handle on note
      if event.1 {
        // Check if the note is a system note
        if SYSTEM_NOTES.contains(&event.0) || BOOKMARK_NOTES.contains(&event.0) {
          tx_on.send((Note::off(event.0), event.1)).unwrap();
        }
        if let Some(note) = thread_app_state.lock().unwrap().get_page().get_note(event.0) {
          tx_on.send((note, event.1)).unwrap();
        }
      }
    }
  });

  let mut sinks: HashMap<u8, (Sink, Sink)> = HashMap::new();

  let _conn_in = midi::listen_midi_input(midi_in_device_name, tx_note);

  // Loop to handle the commands
  for (note, is_on) in rx_on {
    if !is_on && is_hold_to_play_enabled {
      if let Some((audio_sink, virtual_sink)) = sinks.remove(&note.note_id) {
        audio_sink.stop();
        virtual_sink.stop();
      }
      continue;
    }
    match note.note_id {
      END_SESSION_NOTE => {
        midi::actions::end_session(&tx_midi);
        break;
      },
      // TODO: Implement the other notes
      _ => {}
    };
    if note.note_id == FIRST_PAGE_NOTE {
      if current_page.load(Ordering::Relaxed) == 0 {
        continue;
      }
      current_page.store(0, Ordering::Relaxed);

      refresh_grid(&config, &mut referential, &current_page, &app_state, &tx_midi, false);
      continue;
    }
    if note.note_id == LAST_PAGE_NOTE {
      if current_page.load(Ordering::Relaxed) == referential.get_nb_pages() - 1 {
        continue;
      }
      current_page.store(referential.get_nb_pages() - 1, Ordering::Relaxed);

      refresh_grid(&config, &mut referential, &current_page, &app_state, &tx_midi, false);
      continue;
    }
    if note.note_id == PREV_PAGE_NOTE {
      if current_page.load(Ordering::Relaxed) == 0 {
        continue;
      }
      current_page.fetch_sub(1, Ordering::Relaxed);

      refresh_grid(&config, &mut referential, &current_page, &app_state, &tx_midi, false);
      continue;
    }
    if note.note_id == NEXT_PAGE_NOTE {
      if current_page.load(Ordering::Relaxed) >= referential.get_nb_pages() - 1 {
        continue;
      }
      current_page.fetch_add(1, Ordering::Relaxed);

      refresh_grid(&config, &mut referential, &current_page, &app_state, &tx_midi, false);
      continue;
    }
    if note.note_id == STOP_NOTE {
      for (audio_sink, virtual_sink) in sinks.values() {
        audio_sink.stop();
        virtual_sink.stop();
      }
      sinks.clear();
      continue;
    }
    if BOOKMARK_NOTES.contains(&note.note_id) {
      let index = BOOKMARK_NOTES.iter().position(|&r| r == note.note_id).unwrap();
      if !config.bookmark_exists(index) {
        continue;
      }
      referential.set_current_bookmark(BOOKMARK_NOTES[index]);
      // Get the bookmark parameter from Config based on index
      let bookmark_path = config.get_bookmark(index).expect("No path found for bookmark");
      referential.init(bookmark_path);
      current_page.store(0, Ordering::Relaxed);

      refresh_grid(&config, &mut referential, &current_page, &app_state, &tx_midi, true);
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

fn refresh_grid(config: &Config, referential: &mut Referential, current_page: &AtomicU8, app_state: &Arc<Mutex<AppState>>, tx_midi: &Sender<NoteState>, with_header: bool) {
  let page = referential.get_page(current_page.load(Ordering::Relaxed))
    .unwrap().clone();
  let app_state = Arc::clone(app_state);
  app_state.lock().unwrap().set_page(page.clone());

  let thread_tx_midi = tx_midi.clone();
  // Clear the grid and right side
  clear_grid(&thread_tx_midi, if with_header { 99 } else { 89 });

  for note in page.get_notes().iter() {
    thread_tx_midi.send((*note, false)).unwrap();
  }

  if with_header {
    if referential.get_nb_pages() > 1 {
      thread_tx_midi.send((Note::new(FIRST_PAGE_NOTE, "", WHITE_COLOR), false)).unwrap();
      thread_tx_midi.send((Note::new(LAST_PAGE_NOTE, "", WHITE_COLOR), false)).unwrap();
      thread_tx_midi.send((Note::new(PREV_PAGE_NOTE, "", WHITE_COLOR), false)).unwrap();
      thread_tx_midi.send((Note::new(NEXT_PAGE_NOTE, "", WHITE_COLOR), false)).unwrap();
    }
    thread_tx_midi.send((Note::new(END_SESSION_NOTE, "", WHITE_COLOR), false)).unwrap();
  }

  thread_tx_midi.send((Note::new(STOP_NOTE, "", WHITE_COLOR), false)).unwrap();

  for (i, bookmark_note) in BOOKMARK_NOTES.iter().enumerate() {
    if !config.bookmark_exists(i) {
      continue;
    }
    if referential.is_current_bookmark(*bookmark_note) {
      thread_tx_midi.send((Note::green(*bookmark_note), false)).unwrap();
    } else {
      thread_tx_midi.send((Note::white(*bookmark_note), false)).unwrap();
    }
  }
}

fn clear_grid(thread_tx_midi: &Sender<NoteState>, max_note: u8) {
  for note in 1..max_note {
    let note = Note::off(note);
    thread_tx_midi.send((note, false)).unwrap();
  }
}