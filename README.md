# Launchpad

Rust application to play sounds/music from a launchpad (tested with a Launchpad Mini MK3), with unlimited possibilities.

You will be able to assign each note of the grid to a sound and play it by pressing the corresponding button.
You can play multiple sounds at the same time, and also stop them directly.
You can assign up to 64 notes per page, and unlimited pages (as long as you have enough memory) with a pagination system.
To help you organize your sounds, you can also assign bookmarks to folders containing the pages.

## Getting started

The launchpad application is compiled using Rust, but you can also use the precompiled binaries.
You will need to configure a config file and also install a software to provide an output device for the application to use.

In my case, I used Virtual Audio Cable, but you can use any other software that provides an output device.

### Config file
You will need to configure the config.yaml file that is located in the same folder as the binary.

```yaml
# Path: config.yaml
midi_in_device: MIDIIN2 (LPMiniMK3 MIDI) # The input interface for the launchpad
midi_out_device: MIDIOUT2 (LPMiniMK3 MIDI) # The output interface for the launchpad
virtual_device: CABLE Input (VB-Audio Virtual Cable) # The virtual device that will be used to play sounds
bookmark_1: # The name of the bookmarks
bookmark_2: 
bookmark_3:
bookmark_4:
bookmark_5:
bookmark_6:
bookmark_7:
debug_mode: true # If true, the application will print debug messages such as the available midi devices
```

**Note:** The midi_in_device and midi_out_device are the names of the devices that are available on your system.
You can find them by running the application with the `debug_mode` set to true.

Also by default the referential will look for a folder named `pages`, as such it is recommended to create a folder named `pages` in the same folder as the binary.
The bookmarks can be used to categorize your pages.

### Pages

A page is a file containing the configuration of the notes that can be played on the soundpad.
Below is an example on how to configure a page.

The application will list the files and create the pages based on the order of the files.

```
# Path: pages/0
11;pew.mp3;13
```

The first column is the note number, from 11 to 79, note that the numbers `19`, `29`, `39`, `49`, `59`, `69`, `79` are not available.
The second column is the path to the sound file, can be absolute or relative.
The third column is the color of the note, from 0 to 127.

![](colors.png)

## Usage
Once you've configured the page and the config file, you can launch the binary and start playing sounds.