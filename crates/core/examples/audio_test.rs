//! Diagnostic: verify rodio can play audio on this system.
//!
//! Run with: cargo run -p glottisdale-core --example audio_test

use rodio::{buffer::SamplesBuffer, OutputStream, Sink};
use std::time::Duration;

fn main() {
    println!("=== Glottisdale Audio Diagnostic ===\n");

    // 1. Open audio device
    println!("1. Opening default audio device...");
    let (stream, handle) = match OutputStream::try_default() {
        Ok(pair) => {
            println!("   OK: Audio device opened successfully");
            pair
        }
        Err(e) => {
            eprintln!("   FAIL: {}", e);
            eprintln!("\n   Your system may not have a working audio output device.");
            std::process::exit(1);
        }
    };

    // 2. Create sink
    println!("2. Creating audio sink...");
    let sink = match Sink::try_new(&handle) {
        Ok(s) => {
            println!("   OK: Sink created");
            s
        }
        Err(e) => {
            eprintln!("   FAIL: {}", e);
            std::process::exit(1);
        }
    };

    // 3. Generate a 1-second 440Hz sine wave at 44100 Hz (native rate)
    println!("3. Playing 1s 440Hz tone at 44100 Hz (native rate)...");
    let sr = 44100u32;
    let samples: Vec<f32> = (0..sr)
        .map(|i| {
            (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr as f64).sin() as f32 * 0.5
        })
        .collect();
    let source = SamplesBuffer::new(1, sr, samples);
    sink.append(source);
    println!("   >> You should hear a beep NOW <<");
    sink.sleep_until_end();
    println!("   Done (sink empty: {})", sink.empty());

    std::thread::sleep(Duration::from_millis(200));

    // 4. Test at 16000 Hz (our project's sample rate, requires resampling)
    println!("4. Playing 1s 440Hz tone at 16000 Hz (resampled)...");
    let sr2 = 16000u32;
    let samples2: Vec<f32> = (0..sr2)
        .map(|i| {
            (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr2 as f64).sin() as f32 * 0.5
        })
        .collect();
    let sink2 = Sink::try_new(&handle).expect("Failed to create second sink");
    let source2 = SamplesBuffer::new(1, sr2, samples2);
    sink2.append(source2);
    println!("   >> You should hear a beep NOW <<");
    sink2.sleep_until_end();
    println!("   Done (sink empty: {})", sink2.empty());

    std::thread::sleep(Duration::from_millis(200));

    // 5. Test from a background thread (same pattern as PlaybackEngine)
    println!("5. Playing from background thread (PlaybackEngine pattern)...");
    let (tx, rx) = std::sync::mpsc::channel::<Vec<f32>>();

    let handle_clone = {
        // We need a new stream for the background thread since OutputStream
        // isn't Send. This matches PlaybackEngine which creates its own.
        std::thread::spawn(move || {
            let (_bg_stream, bg_handle) = OutputStream::try_default().expect("BG: open audio");
            let bg_sink = Sink::try_new(&bg_handle).expect("BG: create sink");

            if let Ok(samples) = rx.recv() {
                let source = SamplesBuffer::new(1, 16000, samples);
                bg_sink.append(source);
                println!("   >> You should hear a beep NOW (from background thread) <<");
                bg_sink.sleep_until_end();
                println!("   Done from background thread");
            }
        })
    };

    let sr3 = 16000u32;
    let samples3: Vec<f32> = (0..sr3)
        .map(|i| {
            (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr3 as f64).sin() as f32 * 0.5
        })
        .collect();
    tx.send(samples3).unwrap();
    handle_clone.join().unwrap();

    // Keep stream alive until we're done
    drop(stream);

    println!("\n=== Audio diagnostic complete ===");
    println!("If you heard 3 beeps, audio is working correctly.");
    println!("If you heard 0 beeps, rodio/cpal cannot output audio on this system.");
    println!("If you heard some but not all, note which ones worked.");
}
