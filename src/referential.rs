use std::fs;

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
}

impl Referential {
    pub fn new() -> Self {
        Referential {
            pages: vec![]
        }
    }

    pub fn init(&mut self, folder: String) {
        self.pages = vec![];
        let paths = fs::read_dir(folder.as_str()).unwrap();
        for path in paths {
            let path = path.unwrap().path();

            if path.is_file() {
                let mut page = Page::new(vec![]);

                let content = fs::read_to_string(path.clone()).unwrap();
                // Create a new for each line in content
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
}
