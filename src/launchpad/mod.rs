pub enum Launchpad {
  MiniMk3,
  X
}

impl Launchpad {
  pub(crate) fn first_page_note(&self) -> u8 {
    match self {
      Launchpad::MiniMk3 => 91,
      Launchpad::X => 91
    }
  }

  pub(crate) fn last_page_note(&self) -> u8 {
    match self {
      Launchpad::MiniMk3 => 92,
      Launchpad::X => 92
    }
  }

  pub(crate) fn prev_page_note(&self) -> u8 {
    match self {
      Launchpad::MiniMk3 => 93,
      Launchpad::X => 93
    }
  }

  pub(crate) fn next_page_note(&self) -> u8 {
    match self {
      Launchpad::MiniMk3 => 94,
      Launchpad::X => 94
    }
  }

  pub(crate) fn end_session_note(&self) -> u8 {
    match self {
      Launchpad::MiniMk3 => 95,
      Launchpad::X => 95
    }
  }

  pub(crate) fn stop_note(&self) -> u8 {
    match self {
      Launchpad::MiniMk3 => 19,
      Launchpad::X => 19
    }
  }

  pub(crate) fn system_notes<'n>(&self) -> &'n[u8] {
    match self {
      Launchpad::MiniMk3 => &[91, 92, 93, 94, 95, 19],
      Launchpad::X => &[91, 92, 93, 94, 95, 19]
    }
  }

  pub(crate) fn bookmark_notes<'n>(&self) -> &'n[u8] {
    match self {
      Launchpad::MiniMk3 => &[89, 79, 69, 59, 49, 39, 29],
      Launchpad::X => &[89, 79, 69, 59, 49, 39, 29]
    }
  }

  pub(crate) fn programmer_mode_command<'command>(&self) -> &'command[u8] {
    match self {
      Launchpad::MiniMk3 => &[240, 0, 32, 41, 2, 13, 14, 1, 247],
      Launchpad::X => &[240, 0, 32, 41, 2, 12, 127, 247]
    }
  }

  pub fn get_launchpad(name: &str) -> Launchpad {
    if name.contains("LPMiniMK3") {
      Launchpad::MiniMk3
    } else if name.contains("LPX") {
      Launchpad::X
    } else {
      panic!("Launchpad not found")
    }
  }
}
