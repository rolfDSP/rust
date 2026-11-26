# Microphone Recorder

A small interactive CLI that records audio from the system's default microphone
and writes it to a WAV file. Recording is controlled by single-key commands —
no need to press Enter.

## Features

- Start/stop recording on demand with single keypresses.
- Change the output file name at any time between recordings.
- Default output file: `sound.wav`.
- Works with any default input device's native sample format (`F32`, `I16`,
  or `U16`), converting as needed so the WAV file is always valid.

## Controls

| Key | Action |
|-----|--------|
| `s` | Start recording to the current output file |
| `t` | Stop recording and finalize the WAV file |
| `f` | Prompt for a new output file name (only while not recording) |
| `q` | Stop any active recording (if needed) and quit |

Any other key is ignored.

## Requirements

- Rust 2024 edition toolchain.
- A working audio input device, accessible through the OS's default audio
  backend (ALSA on Linux, Core Audio on macOS, WASAPI on Windows).
- **Linux only:** the ALSA development headers must be installed at build
  time, since `cpal` links against `libasound` via `alsa-sys`:

  ```bash
  sudo apt-get install -y libasound2-dev
  ```

## Dependencies

Declared in `Cargo.toml`:

| Crate | Version | Purpose |
|-------|---------|---------|
| [`cpal`](https://docs.rs/cpal) | 0.18.2 | Cross-platform audio input: enumerates devices, negotiates a stream configuration, and delivers captured audio samples via a callback. |
| [`hound`](https://docs.rs/hound) | 3.5.1 | Minimal WAV file reader/writer. Used here to stream samples straight to disk and finalize the file header on stop. |
| [`crossterm`](https://docs.rs/crossterm) | 0.29.0 | Cross-platform terminal control. Used to put the terminal into raw mode so single keypresses are read immediately, without waiting for Enter. |

## Building and running

```bash
cargo build --release
cargo run --release
```

On startup the program prints the control legend and the current output file
name, then waits for key input:

```
Microphone recorder
  s = start recording
  t = stop recording
  f = change output file name
  q = quit

Current output file: 'sound.wav'
```

Press `s` to start recording. The program prints a confirmation and keeps
capturing audio until `t` is pressed:

```
Recording to 'sound.wav'... press 't' to stop.
Saved recording to 'sound.wav'.
```

Press `f` at any time while *not* recording to change the output file name.
This temporarily leaves raw mode so you can type a normal line of text and
press Enter:

```
Enter new output file name: my_recording.wav
Output file set to 'my_recording.wav'.
```

Press `q` to exit. If a recording is in progress, it is stopped and finalized
first so the WAV file is never left truncated.

## How it works

### Terminal input (`crossterm`)

By default a terminal only delivers input to a program line-by-line, after
the user presses Enter (line-buffered / "cooked" mode). To react to a single
keypress like `s` or `t` immediately, the program puts the terminal into
**raw mode** via `enable_raw_mode()` at startup and reads key events in a
loop with `crossterm::event::read()`.

Raw mode has a side effect: the terminal no longer translates `\n` into
`\r\n`, so every `println!` after `enable_raw_mode()` explicitly includes a
trailing `\r` to keep lines left-aligned.

When the user presses `f`, the program needs normal line-buffered input (to
type a file name and confirm with Enter), so `prompt_for_filename()` calls
`disable_raw_mode()`, reads a line with `std::io::stdin().read_line`, then
re-enables raw mode before returning to the main loop.

Raw mode is always restored to normal (`disable_raw_mode()`) before the
process exits, whether via `q` or Ctrl+C... *(see Known limitations below for
the Ctrl+C caveat)*.

### Audio capture (`cpal`)

`Recorder::start()`:

1. Opens the host's **default input device** (`cpal::default_host()` →
   `default_input_device()`).
2. Queries that device's **default input configuration**
   (`default_input_config()`), which reports the native channel count, sample
   rate, and sample format (`F32`, `I16`, or `U16`) the hardware/driver
   prefers. Using the native format avoids an extra resampling step.
3. Builds a matching `hound::WavSpec` — bit depth and WAV sample-format tag
   are derived from the device's `cpal::SampleFormat` (`F32` → 32-bit float,
   anything else → 16-bit integer).
4. Creates the WAV file (`hound::WavWriter::create`) and wraps it in
   `Arc<Mutex<Option<WavWriter<..>>>>` (`WavWriterHandle`) so it can be
   shared safely with the audio callback, which runs on a dedicated thread
   managed by `cpal`.
5. Builds an input stream with `device.build_input_stream`, dispatching on
   the sample format:
   - **F32** and **I16**: samples are written to the WAV file as-is.
   - **U16**: `cpal` streams are unsigned, but WAV/`hound` only supports
     signed integer or float samples, so each `u16` sample is converted to
     `i16` by subtracting the midpoint offset (`32768`), mapping the
     unsigned range `[0, 65535]` onto the signed range `[-32768, 32767]`
     before writing.
6. Calls `stream.play()` to start capture, and stores the `cpal::Stream`
   handle on the `Recorder` so `is_recording()` can check `self.stream.is_some()`.

The actual sample writing happens in `write_samples`, a small generic helper
invoked from the audio callback on every buffer of incoming samples:

```rust
fn write_samples<T: hound::Sample + Copy>(writer: &WavWriterHandle, data: &[T]) {
    if let Ok(mut guard) = writer.lock() {
        if let Some(w) = guard.as_mut() {
            for &sample in data {
                let _ = w.write_sample(sample);
            }
        }
    }
}
```

It locks the shared writer, and if one is present, writes every sample in the
incoming buffer. Write errors are intentionally swallowed here (`let _ =`) —
a single dropped sample should not crash the audio callback thread; if the
file becomes genuinely unwritable, finalization in `stop()` will surface a
clear error instead.

`Recorder::stop()`:

1. Drops the `cpal::Stream` (`self.stream = None`), which stops audio capture
   and tears down the backend stream.
2. Takes the `hound::WavWriter` out of the shared `Option` and calls
   `.finalize()`, which flushes any buffered data and rewrites the WAV
   header's size fields (RIFF chunk size and `data` chunk size) now that the
   final length is known. Without this step the file would have a
   zero/placeholder size in its header and could be rejected by strict WAV
   readers.

### Why `Arc<Mutex<Option<...>>>`?

The `WavWriter` must be reachable from two places at once: the main thread
(to create it in `start()` and finalize it in `stop()`) and the audio
callback thread spawned internally by `cpal` (to write incoming samples).
`Arc` allows shared ownership across threads, `Mutex` provides exclusive
access for each write, and the `Option` lets the writer be cleanly removed
(`.take()`) and finalized in `stop()` without leaving a stale value behind for a
subsequent `start()`.

## Known limitations

- **Ctrl+C / abrupt termination**: if the process is killed (e.g. `SIGKILL`,
  or a terminal closing) instead of exiting via `q`, `stop()` is never
  called, so the WAV header is never finalized. Raw mode is also not restored
  in that case — if this happens, running `reset` or `stty sane` in the
  terminal afterwards will fix the terminal's state.
- **Device changes**: the input device and its configuration are only
  queried once per `start()` call. If the default input device changes while
  idle, the next `s` press picks up the new device automatically; it does not
  adapt mid-recording.
- **No duration/level feedback**: the CLI does not display elapsed recording
  time or an input level meter — it only prints state transitions.
- **Single input device**: there is no device-selection UI; it always uses
  the OS-reported default input device.

## Testing notes

This was verified end-to-end on Linux by driving the compiled binary through
a pseudo-terminal (so raw-mode key reads behave as they would in a real
terminal) and feeding it synthetic keystrokes:

- `s` → `t` → `q`: produced a valid `RIFF/WAVE` file (confirmed via `file`),
  recorded from the default input device (a USB audio device in this case).
- `f` → `<name>` → `s` → `t` → `q`: confirmed the renamed output file is
  created instead of `sound.wav`, and is likewise a valid WAV file.