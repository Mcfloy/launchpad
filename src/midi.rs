use std::sync::mpsc::Sender;
use log::warn;
use midir::{MidiInputConnection, MidiInputPort};
use crate::{NoteEvent};

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

pub mod actions {
    use std::sync::mpsc::Sender;
    use std::thread;
    use std::time::Duration;
    use crate::{clear_grid, NoteState};

    pub fn end_session(tx_midi: &Sender<NoteState>) {
        clear_grid(&tx_midi, 99);
        thread::sleep(Duration::from_millis(100));
    }
}