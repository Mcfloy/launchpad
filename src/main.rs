extern crate core;

use std::error::Error;
use std::fs::File;
use std::io::{BufReader};
use std::path::Path;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use config_file::FromConfigFile;

use cpal::Device;
use cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Sink, Source};

use crate::colors::{GREEN, WHITE};
use crate::config::Config;
use crate::referential::{Note, Page};

mod config;
mod referential;
mod colors;

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
    let config = Config::from_config_file("config.yaml").unwrap();
    let is_debug_enabled = config.is_debug_enabled();

    let mut midi_in = midir::MidiInput::new("midir reading input")?;
    midi_in.ignore(midir::Ignore::All);

    if is_debug_enabled {
        println!("Available input ports:");
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

    let midi_in_port = potential_in_port.as_ref().unwrap();

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

    let midi_out_port = potential_out_port.as_ref().unwrap();
    let mut conn_out = midi_out.connect(midi_out_port, "midir-read-output")?;

    let host = cpal::default_host();
    let output_device = host.default_output_device().expect("Failed to get default output device");
    let (_stream, main_handle) = rodio::OutputStream::try_from_device(&output_device).unwrap();

    if is_debug_enabled {
        println!("Available virtual devices:");
        for device in host.output_devices().unwrap() {
            println!("\t- {}", device.name().unwrap());
        }
    }

    let mut virtual_device: Option<Device> = None;
    let expected_out_device_name = config.get_virtual_device().unwrap();
    for device in host.output_devices().unwrap() {
        if device.name().unwrap() == expected_out_device_name.as_str() {
            virtual_device = Some(device);
        }
    }
    if virtual_device.is_none() {
        println!("No virtual device found");
        return Ok(());
    }
    let (_stream, virtual_handle) = rodio::OutputStream::try_from_device(&virtual_device.unwrap()).unwrap();

    let mut referential = referential::Referential::new();
    referential.init(String::from("pages"));

    if referential.get_nb_pages() == 0 {
        println!("No pages found");
        return Ok(());
    }

    let current_page: AtomicU8 = AtomicU8::new(0);
    let mut page = referential.get_page(0).unwrap().clone();
    let app_state = Arc::new(Mutex::new(AppState {
        page
    }));


    for note in 1..89 {
        conn_out.send(&[144, note, 0]).unwrap();
    }

    if referential.get_nb_pages() > 1 {
        conn_out.send(&[144, FIRST_PAGE_NOTE, WHITE]).unwrap();
        conn_out.send(&[144, LAST_PAGE_NOTE, WHITE]).unwrap();
        conn_out.send(&[144, PREV_PAGE_NOTE, WHITE]).unwrap();
        conn_out.send(&[144, NEXT_PAGE_NOTE, WHITE]).unwrap();
    }

    for note in app_state.lock().unwrap().page.get_notes().iter() {
        conn_out.send(&[144, note.note_id, note.color]).unwrap();
    }
    conn_out.send(&[144, END_SESSION_NOTE, WHITE]).unwrap();
    conn_out.send(&[144, STOP_NOTE, WHITE]).unwrap();

    for (i, bookmark_note) in BOOKMARK_NOTES.iter().enumerate() {
        if config.bookmark_exists(i) {
            conn_out.send(&[144, *bookmark_note, WHITE]).unwrap();
        }
    }

    let (tx, rx): (Sender<Note>, Receiver<Note>) = mpsc::channel();
    let (tx_midi, rx_midi): (Sender<NoteState>, Receiver<NoteState>) = mpsc::channel();

    // Thread to manage the midi LEDs
    thread::spawn(move || {
        for event in rx_midi {
            let channel = if event.1 { 145 } else { 144 };
            let color = if event.1 { WHITE } else { event.0.color };
            let note = event.0;
            conn_out.send(&[channel, note.note_id, color]).unwrap();
        }
    });

    let thread_app_state = Arc::clone(&app_state);
    // Listen to the midi inputs
    let _conn_in = midi_in.connect(midi_in_port, "midir-read-input", move |_stamp, message, _| {
        let is_on = message[2] == 127;
        let thread_tx = tx.clone();
        let message = [message[0], message[1]];
        let page = thread_app_state.lock().unwrap().get_page().clone();

        if is_on {
            if is_debug_enabled {
                println!("{:?}", message);
            }
            if SYSTEM_NOTES.contains(&message[1]) || BOOKMARK_NOTES.contains(&message[1]) {
                thread_tx.send(Note::new(message[1], "", 0)).unwrap();
            }
            if let Some(note) = page.get_note(message[1]) {
                thread_tx.send(note).unwrap();
            }
        }
    }, ())?;


    let mut sinks: Vec<Sink> = vec![];

    // Loop to handle the commands
    for note in rx {
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
            let page = referential.get_page(current_page.load(Ordering::Relaxed))
                .unwrap().clone();
            let app_state = Arc::clone(&app_state);
            app_state.lock().unwrap().set_page(page.clone());

            let thread_tx_midi = tx_midi.clone();
            // Clear the grid and right side
            clear_grid(&thread_tx_midi, 89);

            for note in page.get_notes().iter() {
                thread_tx_midi.send((*note, false)).unwrap();
            }

            thread_tx_midi.send((Note::new(STOP_NOTE, "", WHITE), false)).unwrap();
            signal_bookmarks(&config, &mut current_bookmark, thread_tx_midi);
            continue;
        }
        if note.note_id == LAST_PAGE_NOTE {
            if current_page.load(Ordering::Relaxed) == referential.get_nb_pages() - 1 {
                continue;
            }
            current_page.store(referential.get_nb_pages() - 1, Ordering::Relaxed);
            let page = referential.get_page(current_page.load(Ordering::Relaxed))
                .unwrap().clone();
            let app_state = Arc::clone(&app_state);
            app_state.lock().unwrap().set_page(page.clone());

            let thread_tx_midi = tx_midi.clone();
            // Clear the grid and right side
            clear_grid(&thread_tx_midi, 89);

            for note in page.get_notes().iter() {
                thread_tx_midi.send((*note, false)).unwrap();
            }

            thread_tx_midi.send((Note::new(STOP_NOTE, "", WHITE), false)).unwrap();
            signal_bookmarks(&config, &mut current_bookmark, thread_tx_midi);
            continue;
        }
        if note.note_id == PREV_PAGE_NOTE {
            if current_page.load(Ordering::Relaxed) > 0 {
                current_page.fetch_sub(1, Ordering::Relaxed);
                let page = referential.get_page(current_page.load(Ordering::Relaxed))
                    .unwrap().clone();
                let app_state = Arc::clone(&app_state);
                app_state.lock().unwrap().set_page(page.clone());

                let thread_tx_midi = tx_midi.clone();
                clear_grid(&thread_tx_midi, 89);

                for note in page.get_notes().iter() {
                    thread_tx_midi.send((*note, false)).unwrap();
                }
                thread_tx_midi.send((Note::new(STOP_NOTE, "", WHITE), false)).unwrap();
                signal_bookmarks(&config, &mut current_bookmark, thread_tx_midi);
            }
            continue;
        }
        if note.note_id == NEXT_PAGE_NOTE {
            if current_page.load(Ordering::Relaxed) >= referential.get_nb_pages() - 1 {
                continue;
            }
            current_page.fetch_add(1, Ordering::Relaxed);
            page = referential.get_page(current_page.load(Ordering::Relaxed))
                .unwrap().clone();
            let app_state = Arc::clone(&app_state);
            app_state.lock().unwrap().set_page(page.clone());

            let thread_tx_midi = tx_midi.clone();
            // Clear the grid and right side
            for note in 1..89 {
                let note = Note::new(note, "", 0);
                thread_tx_midi.send((note, false)).unwrap();
            }

            for note in page.get_notes().iter() {
                thread_tx_midi.send((*note, false)).unwrap();
            }
            thread_tx_midi.send((Note::new(STOP_NOTE, "", WHITE), false)).unwrap();
            signal_bookmarks(&config, &mut current_bookmark, thread_tx_midi);

            continue;
        }
        if note.note_id == STOP_NOTE {
            for sink in sinks.iter() {
                sink.stop();
            }
            sinks = vec![];

            let thread_tx_midi = tx_midi.clone();
            // Clear the grid and right side
            for note in 1..89 {
                let note = Note::new(note, "", 0);
                thread_tx_midi.send((note, false)).unwrap();
            }

            let app_state = Arc::clone(&app_state);
            let state = app_state.lock().unwrap();
            let page = state.get_page().clone();
            for note in page.get_notes().iter() {
                thread_tx_midi.send((*note, false)).unwrap();
            }
            thread_tx_midi.send((Note::new(STOP_NOTE, "", WHITE), false)).unwrap();

            signal_bookmarks(&config, &mut current_bookmark, thread_tx_midi);

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

            let page_number = current_page.load(Ordering::Relaxed);
            current_page.fetch_sub(page_number, Ordering::Relaxed);

            referential.init(bookmark_path);
            page = referential.get_page(current_page.load(Ordering::Relaxed))
                .unwrap().clone();
            let app_state = Arc::clone(&app_state);
            app_state.lock().unwrap().set_page(page.clone());

            let thread_tx_midi = tx_midi.clone();
            // Clear the grid and right side
            for note in 1..99 {
                let note = Note::new(note, "", 0);
                thread_tx_midi.send((note, false)).unwrap();
            }

            for note in page.get_notes().iter() {
                thread_tx_midi.send((*note, false)).unwrap();
            }

            if referential.get_nb_pages() > 1 {
                thread_tx_midi.send((Note::new(FIRST_PAGE_NOTE, "", WHITE), false)).unwrap();
                thread_tx_midi.send((Note::new(LAST_PAGE_NOTE, "", WHITE), false)).unwrap();
                thread_tx_midi.send((Note::new(PREV_PAGE_NOTE, "", WHITE), false)).unwrap();
                thread_tx_midi.send((Note::new(NEXT_PAGE_NOTE, "", WHITE), false)).unwrap();
            }
            thread_tx_midi.send((Note::new(END_SESSION_NOTE, "", WHITE), false)).unwrap();
            thread_tx_midi.send((Note::new(STOP_NOTE, "", WHITE), false)).unwrap();
            signal_bookmarks(&config, &mut current_bookmark, thread_tx_midi);

            continue;
        }
        let file = BufReader::new(File::open(Path::new(note.path)).unwrap());
        let source = rodio::Decoder::new(file).unwrap()
            .amplify(0.25);
        if let Some(duration) = source.total_duration() {
            // Light on the note and light off after the duration
            let thread_tx_midi = tx_midi.clone();
            thread_tx_midi.send((note, true)).unwrap();
            thread::spawn(move || {
                thread::sleep(duration);
                thread_tx_midi.send((note, false)).unwrap();
            });
        }

        let sink = Sink::try_new(&main_handle).unwrap();
        sink.append(source);
        sinks.push(sink);

        let file = BufReader::new(File::open(Path::new(note.path)).unwrap());
        let source = rodio::Decoder::new(file).unwrap()
            .amplify(0.1);

        let sink = Sink::try_new(&virtual_handle).unwrap();
        sink.append(source);
        sinks.push(sink);
    }

    Ok(())
}

fn clear_grid(thread_tx_midi: &Sender<NoteState>, max_note: u8) {
    for note in 1..max_note {
        let note = Note::new(note, "", 0);
        thread_tx_midi.send((note, false)).unwrap();
    }
}

fn signal_bookmarks(config: &Config, current_bookmark: &mut u8, thread_tx_midi: Sender<NoteState>) {
    for (i, bookmark_note) in BOOKMARK_NOTES.iter().enumerate() {
        if !config.bookmark_exists(i) {
            continue;
        }
        if *current_bookmark == *bookmark_note {
            thread_tx_midi.send((Note::new(*bookmark_note, "", GREEN), false)).unwrap();
        } else {
            thread_tx_midi.send((Note::new(*bookmark_note, "", WHITE), false)).unwrap();
        }
    }
}