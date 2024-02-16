use std::fs;
use crate::{BOOKMARK_NOTES, GREEN_COLOR, WHITE_COLOR};

#[derive(Debug, Copy, Clone)]
pub(crate) struct Note {
    pub note_id: u8,
    pub path: &'static str,
    pub color: u8,
}

impl Note {
    pub fn new(note_id: u8, path: &'static str, color: u8) -> Self {
        Note {
            note_id,
            path,
            color,
        }
    }

    pub fn green(note_id: u8) -> Self {
        Note {
            note_id,
            path: "",
            color: GREEN_COLOR,
        }
    }

    pub fn white(note_id: u8) -> Self {
        Note {
            note_id,
            path: "",
            color: WHITE_COLOR,
        }
    }

    pub fn off(note_id: u8) -> Self {
        Note {
            note_id,
            path: "",
            color: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Page {
    notes: Vec<Note>,
}

impl Page {
    pub fn new(notes: Vec<Note>) -> Self {
        Page {
            notes,
        }
    }

    pub fn get_notes(&self) -> &Vec<Note> {
        &self.notes
    }

    pub fn get_note(&self, note_id: u8) -> Option<Note> {
        self.notes.iter().find(|note| note.note_id == note_id).cloned()
    }
}

pub(crate) struct Referential {
    pages: Vec<Page>,
    current_page: usize,
    current_bookmark: u8,
}

impl Referential {
    pub fn new() -> Self {
        Referential {
            pages: vec![],
            current_page: 0,
            current_bookmark: BOOKMARK_NOTES[0],
        }
    }

    pub fn init(&mut self, folder: String) {
        self.pages = vec![];
        self.current_page = 0;
        let paths = fs::read_dir(folder.as_str()).unwrap();
        for path in paths {
            let path = path.unwrap().path();

            if path.is_file() {
                let mut page = Page::new(vec![]);

                let content = fs::read_to_string(path.clone()).unwrap();
                // Create new for each line in content
                for line in content.lines() {
                    let mut line = line.split(';');
                    let note_id = line.next().unwrap().parse::<u8>().unwrap();
                    let path = String::from(line.next().unwrap());
                    let color = line.next().unwrap().parse::<u8>().unwrap();
                    // We need to leak because the path can live as much as the program
                    page.notes.push(Note::new(note_id, Box::leak(path.into_boxed_str()), color));
                }
                self.pages.push(page);
            }
        }
    }

    pub fn get_nb_pages(&self) -> u8 {
        self.pages.len() as u8
    }

    pub fn get_page(&self, mut page_number: u8) -> Option<&Page> {
        if page_number > self.pages.len() as u8 {
            page_number = self.pages.len() as u8;
        }
        self.pages.get(page_number as usize)
    }

    pub fn previous_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
        }
    }

    pub fn next_page(&mut self) {
        if self.current_page < self.pages.len() - 1 {
            self.current_page += 1;
        }
    }

    pub fn first_page(&mut self) {
        self.current_page = 0;
    }

    pub fn last_page(&mut self) {
        self.current_page = self.pages.len() - 1;
    }

    pub fn get_note(&self, note_id: u8) -> Option<Note> {
        self.pages[self.current_page].get_note(note_id)
    }

    pub fn is_current_bookmark(&self, bookmark_note_id: u8) -> bool {
        return self.current_bookmark == bookmark_note_id;
    }

    pub fn set_current_bookmark(&mut self, bookmark_note_id: u8) {
        self.current_bookmark = bookmark_note_id;
    }
}
