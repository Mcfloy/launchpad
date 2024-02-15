#![feature(let_chains)]
extern crate core;

use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

use config_file::FromConfigFile;
use rodio::{Sink};

use crate::config::Config;
use crate::referential::{Note, Page, Referential};

mod config;
mod referential;
mod audio;

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
    println!("Available MIDI input ports:");
    for p in midi_in.ports() {
      println!("\t- {}", midi_in.port_name(&p).unwrap());
    }
  }

  let mut current_bookmark: u8 = 0;

  let midi_in_device_name = config.get_midi_in_device().unwrap();
  let mut potential_in_port = None;
  for p in midi_in.ports() {
    let port_name = midi_in.port_name(&p).unwrap();
    if port_name.eq_ignore_ascii_case(midi_in_device_name.as_str()) {
      potential_in_port = Some(p);

      println!("Selected input port: {}", port_name);
    }
  }

  let midi_in_port = potential_in_port.as_ref()
    .expect("No midi input port found, have you plugged your midi device ?");

  let midi_out = midir::MidiOutput::new("midir reading output")?;

  if is_debug_enabled {
    println!("Available output ports:");
    for p in midi_out.ports() {
      println!("\t- {}", midi_out.port_name(&p).unwrap());
    }
  }

  let mut potential_out_port = None;
  for p in midi_out.ports() {
    let port_name = midi_out.port_name(&p).unwrap();
    if port_name.eq_ignore_ascii_case(config.get_midi_out_device().unwrap().as_str()) {
      potential_out_port = Some(p);

      println!("Selected output port: {}", port_name);
    }
  }

  let midi_out_port = potential_out_port.as_ref()
    .expect("No midi output port found, have you plugged your midi device ?");

  let mut conn_out = midi_out.connect(midi_out_port, "midir-read-output")?;

  let (_output_stream, output_handle);

  match audio::select_output_device(config.get_output_device().unwrap()) {
    Some((stream, handle)) => {
      _output_stream = stream;
      output_handle = handle;
    }
    None => {
      println!("No audio device found");
      return Ok(());
    }
  }

  if is_debug_enabled {
    println!("Available output devices:");
    for device in audio::get_output_devices() {
      println!("\t- {}", device);
    }
  }

  let (_virtual_stream, virtual_handle);
  match audio::select_output_device(&config.get_virtual_device().unwrap()) {
    Some((stream, handle)) => {
      _virtual_stream = stream;
      virtual_handle = handle;
    }
    None => {
      println!("No virtual device found");
      return Ok(());
    }
  }

  let mut referential = Referential::new();
  referential.init(String::from("pages"));

  if referential.get_nb_pages() == 0 {
    println!("No pages found");
    return Ok(());
  }

  let current_page: AtomicU8 = AtomicU8::new(0);
  let page = referential.get_page(0).unwrap().clone();
  let app_state = Arc::new(Mutex::new(AppState {
    page
  }));

  let (tx_on, rx_on): (Sender<NoteState>, Receiver<NoteState>) = mpsc::channel();
  let (tx_midi, rx_midi): (Sender<NoteState>, Receiver<NoteState>) = mpsc::channel();

  refresh_grid(&config, &mut current_bookmark, &mut referential, &current_page, &app_state, &tx_midi, true);

  // Activate programmer mode
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
  let mut sinks: HashMap<u8, (Sink, Sink)> = HashMap::new();
  // Listen to the midi inputs
  let _conn_in = midi_in.connect(midi_in_port, "midir-read-input", move |_stamp, message, _| {
    let is_on = message[2] == 127;
    let message = [message[0], message[1]];
    let page = thread_app_state.lock().unwrap().get_page().clone();

    if is_on {
      if is_debug_enabled {
        println!("Pressed {:?}", message);
      }
      let thread_tx = tx_on.clone();
      if SYSTEM_NOTES.contains(&message[1]) || BOOKMARK_NOTES.contains(&message[1]) {
        thread_tx.send((Note::new(message[1], "", 0), is_on)).unwrap();
      }
      if let Some(note) = page.get_note(message[1]) {
        thread_tx.send((note, is_on)).unwrap();
      }
    } else if is_hold_to_play_enabled {
      let thread_tx = tx_on.clone();
      if let Some(note) = page.get_note(message[1]) {
        thread_tx.send((note, is_on)).unwrap();
      }
    }
  }, ())?;


  // Loop to handle the commands
  for (note, is_on) in rx_on {
    if !is_on && is_hold_to_play_enabled {
      if let Some((audio_sink, virtual_sink)) = sinks.remove(&note.note_id) {
        audio_sink.stop();
        virtual_sink.stop();
      }
      continue;
    }
    if note.note_id == END_SESSION_NOTE {
      // Close everything
      let thread_tx_midi = tx_midi;
      clear_grid(&thread_tx_midi, 99);
      thread::sleep(Duration::from_millis(100));
      break;
    }
    if note.note_id == FIRST_PAGE_NOTE {
      if current_page.load(Ordering::Relaxed) == 0 {
        continue;
      }
      current_page.store(0, Ordering::Relaxed);

      refresh_grid(&config, &mut current_bookmark, &mut referential, &current_page, &app_state, &tx_midi, false);
      continue;
    }
    if note.note_id == LAST_PAGE_NOTE {
      if current_page.load(Ordering::Relaxed) == referential.get_nb_pages() - 1 {
        continue;
      }
      current_page.store(referential.get_nb_pages() - 1, Ordering::Relaxed);

      refresh_grid(&config, &mut current_bookmark, &mut referential, &current_page, &app_state, &tx_midi, false);
      continue;
    }
    if note.note_id == PREV_PAGE_NOTE {
      if current_page.load(Ordering::Relaxed) == 0 {
        continue;
      }
      current_page.fetch_sub(1, Ordering::Relaxed);

      refresh_grid(&config, &mut current_bookmark, &mut referential, &current_page, &app_state, &tx_midi, false);
      continue;
    }
    if note.note_id == NEXT_PAGE_NOTE {
      if current_page.load(Ordering::Relaxed) >= referential.get_nb_pages() - 1 {
        continue;
      }
      current_page.fetch_add(1, Ordering::Relaxed);

      refresh_grid(&config, &mut current_bookmark, &mut referential, &current_page, &app_state, &tx_midi, false);
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
      current_bookmark = BOOKMARK_NOTES[index];
      // Get the bookmark parameter from Config based on index
      let bookmark_path = config.get_bookmark(index).expect("No path found for bookmark");
      referential.init(bookmark_path);
      current_page.store(0, Ordering::Relaxed);

      refresh_grid(&config, &mut current_bookmark, &mut referential, &current_page, &app_state, &tx_midi, true);
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

fn refresh_grid(config: &Config, current_bookmark: &mut u8, referential: &mut Referential, current_page: &AtomicU8, app_state: &Arc<Mutex<AppState>>, tx_midi: &Sender<NoteState>, with_header: bool) {
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
  signal_bookmarks(config, current_bookmark, thread_tx_midi);
}

fn clear_grid(thread_tx_midi: &Sender<NoteState>, max_note: u8) {
  for note in 1..max_note {
    let note = Note::off(note);
    thread_tx_midi.send((note, false)).unwrap();
  }
}

fn signal_bookmarks(config: &Config, current_bookmark: &mut u8, thread_tx_midi: Sender<NoteState>) {
  for (i, bookmark_note) in BOOKMARK_NOTES.iter().enumerate() {
    if !config.bookmark_exists(i) {
      continue;
    }
    if *current_bookmark == *bookmark_note {
      thread_tx_midi.send((Note::green(*bookmark_note), false)).unwrap();
    } else {
      thread_tx_midi.send((Note::white(*bookmark_note), false)).unwrap();
    }
  }
}