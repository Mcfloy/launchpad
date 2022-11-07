extern crate core;

use std::collections::HashMap;
use std::error::Error;
use std::io::{BufReader, stdin};
use std::rc::Rc;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use cpal::traits::{DeviceTrait, HostTrait};
use rodio::Sink;

mod backend;
mod frontend;

fn main() -> Result<(), Box<dyn Error>> {

    println!("Launching midir");
    let mut midi_in = midir::MidiInput::new("midir reading input")?;
    midi_in.ignore(midir::Ignore::None);

    println!("Opening connection");
    let in_port = &midi_in.ports()[1];
    let in_port_name = midi_in.port_name(in_port).unwrap();

    println!("Launching cpal");
    let host = cpal::default_host();
    let input_device = host.default_input_device().expect("Failed to get default input device");
    let output_device = host.default_output_device().expect("Failed to get default output device");

    println!("Listening to output device: {}", output_device.name().unwrap());
    let (tx, rx): (Sender<(u8, u8)>, Receiver<(u8, u8)>) = mpsc::channel();

    let playing_sounds: Arc<Mutex<HashMap<(u8, u8), rodio::Sink>>> = Arc::new(Mutex::new(HashMap::new()));

    let _conn_in = midi_in.connect(in_port, "midir-read-input", move |stamp, message, _| {
        println!("{:?}", message);
        let is_on = message[2] == 127;
        let thread_tx = tx.clone();
        let message = [message[0], message[1]];

        if is_on {
            let file = std::fs::File::open("assets_music.mp3");
            if let Ok(file) = file {
                thread::spawn(move || {
                    thread_tx.send((message[0], message[1])).unwrap();
                    drop(message);
                });
            }
        }
    }, ())?;

    let sinks: HashMap<[u8;2], Sink> = HashMap::new();

    let mut handles: Vec<JoinHandle<()>> = vec![];

    let (thread_tx, thread_rx): (Sender<JoinHandle<()>>, Receiver<JoinHandle<()>>) = mpsc::channel();

    thread::spawn(move || {
        for note_id in rx {
            if let Some(sink) = playing_sounds.lock().unwrap().remove(&note_id) {
                println!("Stop sink");
                sink.stop();
            } else {
                let host = cpal::default_host();
                let output_device = host.default_output_device().expect("Failed to get default output device");
                let (_stream, handle) = rodio::OutputStream::try_from_device(&output_device).unwrap();

                println!("Start sink");
                let handle = thread::spawn(move || {
                    let file = std::fs::File::open("pew.mp3").unwrap();
                    let sink = Sink::try_new(&handle).unwrap();
                    let source = rodio::Decoder::new(BufReader::new(file)).unwrap();
                    sink.append(source);
                    println!("Playing sound");
                    sink.sleep_until_end();
                });

                println!("Insert sink");
                handle.join().unwrap();
            }
        }
    });

    for handle in thread_rx {
        println!("Joining thread");
        handle.join().unwrap();
    }

    println!("Launching sink");

    stdin().read_line(&mut String::new())?;

    Ok(())
}