extern crate core;


use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

use cpal::traits::HostTrait;
use rodio::Source;

use crate::colors::{GREEN, WHITE};
use crate::referential::{Note, Page};

mod referential;
mod colors;

const PREV_PAGE_NOTE: u8 = 93;
const NEXT_PAGE_NOTE: u8 = 94;
const SESSION_NOTE: u8 = 95;

type NoteState = (Note, bool);

#[derive(Debug, Clone)]
struct AppState {
    pub page: Page
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
    let mut midi_in = midir::MidiInput::new("midir reading input")?;
    midi_in.ignore(midir::Ignore::All);

    println!("Opening connection");
    let in_port = &midi_in.ports()[1];
    let in_port_name = midi_in.port_name(in_port).unwrap();
    println!("Input port: {}", in_port_name);

    let midi_out = midir::MidiOutput::new("midir reading output")?;
    let out_port = &midi_out.ports()[2];
    let out_port_name = midi_out.port_name(out_port).unwrap();
    println!("Output port: {}", out_port_name);
    let mut conn_out = midi_out.connect(out_port, "midir-read-output")?;

    let mut referential = referential::Referential::new();
    referential.init();

    if referential.get_nb_pages() == 0 {
        println!("No pages found");
        return Ok(());
    }

    let current_page: AtomicU8 = AtomicU8::new(0);
    let mut page = referential.get_page(0).unwrap().clone();
    let app_state = Arc::new(Mutex::new(AppState {
        page
    }));


    for note in 1..88 {
        conn_out.send(&[144, note, 0]).unwrap();
    }

    if referential.get_nb_pages() > 1 {
        conn_out.send(&[144, PREV_PAGE_NOTE, WHITE]).unwrap();
        conn_out.send(&[144, NEXT_PAGE_NOTE, WHITE]).unwrap();
    }

    let thread_app_state = app_state.clone();
    for note in thread_app_state.lock().unwrap().page.get_notes().iter() {
        conn_out.send(&[144, note.note_id, note.color]).unwrap();
    }
    conn_out.send(&[144, SESSION_NOTE, WHITE]).unwrap();

    let (tx, rx): (Sender<Note>, Receiver<Note>) = mpsc::channel();
    let (tx_midi, rx_midi): (Sender<NoteState>, Receiver<NoteState>) = mpsc::channel();

    // Thread to manage the midi LEDs
    thread::spawn(move || {
        for event in rx_midi {
            let channel = if event.1 { 145 } else { 144 };
            let color = if event.1 { GREEN } else { event.0.color };
            let note = event.0;
            conn_out.send(&[channel, note.note_id, color]).unwrap();
        }
    });

    let thread_app_state = Arc::clone(&app_state);
    let _conn_in = midi_in.connect(in_port, "midir-read-input", move |_stamp, message, _| {
        let is_on = message[2] == 127;
        let thread_tx = tx.clone();
        let message = [message[0], message[1]];
        let page = thread_app_state.lock().unwrap().get_page().clone();

        if is_on {
            println!("{:?}", message);
            if message[1] == PREV_PAGE_NOTE {
                thread_tx.send(Note::new(PREV_PAGE_NOTE, "", 0)).unwrap();
            }
            if message[1] == NEXT_PAGE_NOTE {
                thread_tx.send(Note::new(NEXT_PAGE_NOTE, "", 0)).unwrap();
            }
            if message[1] == SESSION_NOTE {
                thread_tx.send(Note::new(SESSION_NOTE, "", 0)).unwrap();
            }
            if let Some(note) = page.get_note(message[1]) {
                thread::spawn(move || {
                    thread_tx.send(note).unwrap();
                });
            }
        }
    }, ())?;

    let host = cpal::default_host();
    let output_device = host.default_output_device().expect("Failed to get default output device");
    // let input_device = host.default_input_device().expect("Failed to get default input device");
    let (_stream, handle) = rodio::OutputStream::try_from_device(&output_device).unwrap();

    // Loop to handle the playback
    for note in rx {
        if note.note_id == SESSION_NOTE {
            // Close everything
            let thread_tx_midi = tx_midi;
            for note in 1..99 {
                let note = Note::new(note, "", 0);
                thread_tx_midi.send((note, false)).unwrap();
            }
            thread::sleep(Duration::from_millis(100));
            break;
        }
        if note.note_id == PREV_PAGE_NOTE {
            if current_page.load(Ordering::Relaxed) > 0 {
                current_page.fetch_sub(1, Ordering::Relaxed);
                println!("Current page: {}", current_page.load(Ordering::Relaxed));
                let page = referential.get_page(current_page.load(Ordering::Relaxed))
                    .unwrap().clone();
                let app_state = Arc::clone(&app_state);
                app_state.lock().unwrap().set_page(page.clone());

                let thread_tx_midi = tx_midi.clone();
                for note in 1..88 {
                    let note = Note::new(note, "", 0);
                    thread_tx_midi.send((note, false)).unwrap();
                }

                for note in page.get_notes().iter() {
                    thread_tx_midi.send((*note, false)).unwrap();
                }
            }
            continue;
        }
        if note.note_id == NEXT_PAGE_NOTE {
            if current_page.load(Ordering::Relaxed) >= referential.get_nb_pages() - 1 {
                continue;
            }
            current_page.fetch_add(1, Ordering::Relaxed);
            // let thread_page = Arc::clone(&global_page);
            // let mut page = thread_page.lock().unwrap();
            page = referential.get_page(current_page.load(Ordering::Relaxed))
                .unwrap().clone();
            let app_state = Arc::clone(&app_state);
            app_state.lock().unwrap().set_page(page.clone());

            let thread_tx_midi = tx_midi.clone();
            for note in 1..88 {
                let note = Note::new(note, "", 0);
                thread_tx_midi.send((note, false)).unwrap();
            }

            for note in page.get_notes().iter() {
                thread_tx_midi.send((*note, false)).unwrap();
            }
            continue;
        }
        let file = BufReader::new(File::open(Path::new(note.path)).unwrap());
        let source = rodio::Decoder::new(file).unwrap();
        if let Some(duration) = source.total_duration() {
            // Light on the note and light off after the duration
            let thread_tx_midi = tx_midi.clone();
            thread_tx_midi.send((note, true)).unwrap();
            thread::spawn(move || {
                thread::sleep(duration);
                thread_tx_midi.send((note, false)).unwrap();
            });
        }

        handle.play_raw(source.convert_samples()).unwrap();
    }

    Ok(())
}