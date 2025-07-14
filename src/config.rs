use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    midi_in_device: Option<String>,
    midi_out_device: Option<String>,
    output_device: Option<String>,
    virtual_device: Option<String>,
    bookmark_1: Option<String>,
    bookmark_2: Option<String>,
    bookmark_3: Option<String>,
    bookmark_4: Option<String>,
    bookmark_5: Option<String>,
    bookmark_6: Option<String>,
    bookmark_7: Option<String>,
    hold_to_play: bool,
}

impl Config {
    // TODO: Create an init function

    pub fn get_midi_in_device(&self) -> Option<&str> {
        self.midi_in_device.as_deref()
    }

    pub fn get_midi_out_device(&self) -> Option<&str> {
        self.midi_out_device.as_deref()
    }

    pub fn get_output_device(&self) -> Option<&str> {
        self.output_device.as_deref()
    }

    pub fn get_virtual_device(&self) -> Option<String> {
        self.virtual_device.clone()
    }

    pub fn get_bookmark(&self, index: usize) -> Option<String> {
        match index {
            0 => self.bookmark_1.clone(),
            1 => self.bookmark_2.clone(),
            2 => self.bookmark_3.clone(),
            3 => self.bookmark_4.clone(),
            4 => self.bookmark_5.clone(),
            5 => self.bookmark_6.clone(),
            6 => self.bookmark_7.clone(),
            _ => None
        }
    }

    pub fn bookmark_exists(&self, index: usize) -> bool {
        match index {
            0 => self.bookmark_1.is_some(),
            1 => self.bookmark_2.is_some(),
            2 => self.bookmark_3.is_some(),
            3 => self.bookmark_4.is_some(),
            4 => self.bookmark_5.is_some(),
            5 => self.bookmark_6.is_some(),
            6 => self.bookmark_7.is_some(),
            _ => false
        }
    }

    pub fn is_hold_to_play_enabled(&self) -> bool {
        self.hold_to_play
    }

    pub fn swap_hold_to_play(&mut self) {
        self.hold_to_play = !self.hold_to_play;
    }
}