use std::sync::mpsc::Sender;
use log::warn;
use midir::{MidiInputConnection, MidiInputPort};
use crate::{NoteEvent, NoteState};
use crate::config::Config;
use crate::launchpad::Launchpad;
use crate::referential::{Note, Referential};

pub fn get_midi_input_devices() -> Vec<String> {
    let midi_in = midir::MidiInput::new("launchpad-soundpad").unwrap();
    let mut devices = vec![];
    for port in midi_in.ports() {
        let port_name = midi_in.port_name(&port).unwrap();
        devices.push(port_name);
    }
    devices
}

pub fn get_midi_output_devices() -> Vec<String> {
    let midi_out = midir::MidiOutput::new("launchpad-soundpad").unwrap();
    let mut devices = vec![];
    for port in midi_out.ports() {
        let port_name = midi_out.port_name(&port).unwrap();
        devices.push(port_name);
    }
    devices
}

pub fn select_midi_output_device(name: &str) -> Option<midir::MidiOutputConnection> {
    let midi_out = midir::MidiOutput::new("launchpad-soundpad").unwrap();
    for port in midi_out.ports() {
        let port_name = midi_out.port_name(&port).unwrap();
        if port_name.eq(name) {
            return midi_out.connect(&port, "launchpad-soundpad-output").ok();
        }
    }
    None
}

pub fn listen_midi_input(name: &str, tx_on: Sender<NoteEvent>) -> Option<MidiInputConnection<()>> {
    let midi_in = midir::MidiInput::new("launchpad-soundpad").unwrap();
    let mut found_port: Option<MidiInputPort> = None;
    for port in midi_in.ports() {
        let port_name = midi_in.port_name(&port).unwrap();
        if port_name.eq(name) {
            found_port = Some(port);
            break;
        }
    }
    if let Some(port) = found_port {
        let conn = midi_in.connect(&port, "launchpad-soundpad-input", move |_stamp: u64, message: &[u8], _| {
            let tx_on = tx_on.clone();
            let is_on = message[2] == 127;
            if let Err(error) = tx_on.send((message[1], is_on)) {
                warn!("Error sending midi message: {}", error);
            }
        }, ()).ok();
        return conn;
    }
    None
}

pub fn refresh_grid(launchpad: &Launchpad, config: &Config, referential: &mut Referential, tx_midi: &Sender<NoteState>, with_header: bool) {
    let thread_tx_midi = tx_midi.clone();
    // Clear the grid and right side
    clear_grid(&thread_tx_midi, if with_header { 99 } else { 89 });

    for note in referential.current_page().get_notes().iter() {
        thread_tx_midi.send((*note, false)).unwrap();
    }

    if with_header {
        if referential.get_nb_pages() > 1 {
            thread_tx_midi.send((Note::white(launchpad.first_page_note()), false)).unwrap();
            thread_tx_midi.send((Note::white(launchpad.last_page_note()), false)).unwrap();
            thread_tx_midi.send((Note::white(launchpad.prev_page_note()), false)).unwrap();
            thread_tx_midi.send((Note::white(launchpad.next_page_note()), false)).unwrap();
        }
        thread_tx_midi.send((Note::white(launchpad.end_session_note()), false)).unwrap();
    }

    thread_tx_midi.send((Note::white(launchpad.stop_note()), false)).unwrap();

    for (i, bookmark_note) in launchpad.bookmark_notes().iter().enumerate() {
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

pub fn clear_grid(thread_tx_midi: &Sender<NoteState>, max_note: u8) {
    for note in 1..max_note {
        let note = Note::off(note);
        thread_tx_midi.send((note, false)).unwrap();
    }
}

pub mod actions {
    use std::collections::HashMap;
    use std::sync::mpsc::Sender;
    use std::thread;
    use std::time::Duration;
    use rodio::Sink;
    use crate::{NoteState};
    use crate::midi::clear_grid;

    pub fn end_session(tx_midi: &Sender<NoteState>) {
        clear_grid(&tx_midi, 99);
        thread::sleep(Duration::from_millis(100));
    }

    pub fn stop_note(sinks: &mut HashMap<u8, (Sink, Sink)>) {
        for (audio_sink, virtual_sink) in sinks.values() {
            audio_sink.stop();
            virtual_sink.stop();
        }
        sinks.clear();
    }
}