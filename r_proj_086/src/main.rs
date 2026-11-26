use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::event::{read, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

type WavWriterHandle = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

struct Recorder {
    filename: String,
    writer: WavWriterHandle,
    stream: Option<cpal::Stream>,
}

impl Recorder {
    fn new() -> Self {
        Self {
            filename: "sound.wav".to_string(),
            writer: Arc::new(Mutex::new(None)),
            stream: None,
        }
    }

    fn is_recording(&self) -> bool {
        self.stream.is_some()
    }

    fn start(&mut self) {
        if self.is_recording() {
            println!("Already recording.\r");
            return;
        }

        let host = cpal::default_host();
        let Some(device) = host.default_input_device() else {
            println!("No input device available.\r");
            return;
        };

        let supported_config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                println!("Failed to get input config: {e}\r");
                return;
            }
        };

        let sample_format = supported_config.sample_format();
        let config: cpal::StreamConfig = supported_config.into();

        let spec = hound::WavSpec {
            channels: config.channels,
            sample_rate: config.sample_rate,
            bits_per_sample: match sample_format {
                cpal::SampleFormat::F32 => 32,
                _ => 16,
            },
            sample_format: match sample_format {
                cpal::SampleFormat::F32 => hound::SampleFormat::Float,
                _ => hound::SampleFormat::Int,
            },
        };

        let writer = match hound::WavWriter::create(&self.filename, spec) {
            Ok(w) => w,
            Err(e) => {
                println!("Failed to create '{}': {e}\r", self.filename);
                return;
            }
        };
        *self.writer.lock().unwrap() = Some(writer);
        let writer_handle = self.writer.clone();

        let err_fn = |err| eprintln!("Stream error: {err}\r");

        let stream_result = match sample_format {
            cpal::SampleFormat::F32 => device.build_input_stream(
                config.clone(),
                move |data: &[f32], _: &_| write_samples(&writer_handle, data),
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                config.clone(),
                move |data: &[i16], _: &_| write_samples(&writer_handle, data),
                err_fn,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_input_stream(
                config.clone(),
                move |data: &[u16], _: &_| {
                    let converted: Vec<i16> =
                        data.iter().map(|&s| (s as i32 - 32768) as i16).collect();
                    write_samples(&writer_handle, &converted)
                },
                err_fn,
                None,
            ),
            other => {
                println!("Unsupported sample format: {other:?}\r");
                *self.writer.lock().unwrap() = None;
                return;
            }
        };

        let stream = match stream_result {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to build input stream: {e}\r");
                *self.writer.lock().unwrap() = None;
                return;
            }
        };

        if let Err(e) = stream.play() {
            println!("Failed to start stream: {e}\r");
            *self.writer.lock().unwrap() = None;
            return;
        }

        self.stream = Some(stream);
        println!("Recording to '{}'... press 't' to stop.\r", self.filename);
    }

    fn stop(&mut self) {
        if !self.is_recording() {
            println!("Not currently recording.\r");
            return;
        }

        self.stream = None; // dropping stops and closes the cpal stream

        if let Some(writer) = self.writer.lock().unwrap().take() {
            match writer.finalize() {
                Ok(()) => println!("Saved recording to '{}'.\r", self.filename),
                Err(e) => println!("Failed to finalize wav file: {e}\r"),
            }
        }
    }

    fn set_filename(&mut self, name: String) {
        if self.is_recording() {
            println!("Cannot change file name while recording.\r");
            return;
        }
        self.filename = name;
        println!("Output file set to '{}'.\r", self.filename);
    }
}

fn write_samples<T: hound::Sample + Copy>(writer: &WavWriterHandle, data: &[T]) {
    if let Ok(mut guard) = writer.lock() {
        if let Some(w) = guard.as_mut() {
            for &sample in data {
                let _ = w.write_sample(sample);
            }
        }
    }
}

fn prompt_for_filename() -> io::Result<String> {
    disable_raw_mode()?;
    print!("\nEnter new output file name: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    enable_raw_mode()?;
    Ok(input.trim().to_string())
}

fn main() -> io::Result<()> {
    println!("Microphone recorder");
    println!("  s = start recording");
    println!("  t = stop recording");
    println!("  f = change output file name");
    println!("  q = quit");
    println!();

    let mut recorder = Recorder::new();
    println!("Current output file: '{}'", recorder.filename);

    enable_raw_mode()?;

    loop {
        if let Event::Key(key_event) = read()? {
            if key_event.kind != KeyEventKind::Press {
                continue;
            }
            match key_event.code {
                KeyCode::Char('s') => recorder.start(),
                KeyCode::Char('t') => recorder.stop(),
                KeyCode::Char('f') => {
                    let name = prompt_for_filename()?;
                    if !name.is_empty() {
                        recorder.set_filename(name);
                    } else {
                        println!("File name unchanged.\r");
                    }
                }
                KeyCode::Char('q') => {
                    if recorder.is_recording() {
                        recorder.stop();
                    }
                    break;
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    println!("Goodbye.");
    Ok(())
}