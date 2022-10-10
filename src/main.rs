use async_std::prelude::*;
use aws_sdk_polly::model::{Engine, OutputFormat, TextType, VoiceId};
use aws_sdk_polly::Client;
use dirs::home_dir;
use regex::Regex;
use rodio::{Decoder, OutputStream, Sink};
use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::error::Error;
use std::fs;
use std::fs::DirEntry;
use std::hash::{Hash, Hasher};
use std::io::BufReader;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use sysinfo::{System, SystemExt};

const APPNAME: &str = "lexicc";

fn process_text(text: String) -> String {
    let quote_pattern = Regex::new(r#"["“](?P<inner>.*?)(?P<punctuation>[\.\?!])?[”"]"#).unwrap();
    let text = quote_pattern.replace_all(&text, "(quote) $inner (end quote)$punctuation");

    let citation_pattern = Regex::new(r#"\[\^?\d+\]"#).unwrap();
    let text = citation_pattern.replace_all(&text, "");

    format!(
        r#"<speak><prosody rate="x-fast"><p>{}</p></prosody></speak>"#,
        text.replace('"', " (quote) ")
            .replace('&', "&amp;")
            .replace('\'', "&apos;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    )
}

fn create_state_dir(name: &str) -> PathBuf {
    let home_path = home_dir().unwrap();
    let state_path = home_path.join(".local/state").join(APPNAME).join(name);
    fs::create_dir_all(&state_path).unwrap();
    state_path
}

fn entries_from(path: &PathBuf) -> Vec<DirEntry> {
    let mut entries: Vec<DirEntry> = fs::read_dir(path).unwrap().map(|r| r.unwrap()).collect();
    entries.sort_by_key(|entry| entry.path());
    entries
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Exit immediately if we're already running
    let s = System::new_all();
    let instances = s.processes_by_exact_name(APPNAME);
    let mut count_instances = 0;
    for _instance in instances {
        count_instances += 1;
        if count_instances > 1 {
            return Ok(());
        }
    }

    let inbox_dir = create_state_dir("inbox");
    let output_dir = create_state_dir("audio");

    let shared_config = aws_config::load_from_env().await;
    let client = Client::new(&shared_config);

    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();
    sink.set_speed(1.15);

    let mut paragraphs: VecDeque<String> = VecDeque::new();

    loop {
        let inbox_entries = entries_from(&inbox_dir);
        if inbox_entries.is_empty() && sink.empty() {
            break;
        }

        for entry in inbox_entries {
            let text = fs::read_to_string(&entry.path())?;

            let hyphenated_line_break_pattern = Regex::new(r#"-\r?\n"#).unwrap();
            let text = hyphenated_line_break_pattern.replace_all(&text, "");

            // This isn't perfect... I'm trying to fix pauses in the middle of
            // sentences for PDFs that are inserting too many line breaks, by
            // identifying whether any particular line break looks like it's in
            // the middle of a sentence. However, sometimes the punctuation I'm
            // using to reject a line break as legitimate is followed by a
            // reference, e.g. .[^2] or .2 and the regex crate I'm using doesn't
            // allow negative lookaheads.
            let unpunctuated_line_break_pattern =
                Regex::new(r#"(?P<character>[^\.\?!])[\[\d\]]*\r?\n"#).unwrap();
            let text = unpunctuated_line_break_pattern.replace_all(&text, "$character ");

            for chunk in text.split('\n') {
                if !chunk.trim().is_empty() {
                    paragraphs.push_back(chunk.to_string());
                }
            }
            fs::remove_file(&entry.path())?;
        }

        while sink.len() < 2 && !paragraphs.is_empty() {
            let processed_text = process_text(paragraphs.pop_front().unwrap());
            let hash = calculate_hash(&processed_text).to_string();

            if !output_dir.join(&hash).exists() {
                let synth_output = client
                    .synthesize_speech()
                    .output_format(OutputFormat::OggVorbis)
                    .voice_id(VoiceId::Joanna)
                    .engine(Engine::Neural)
                    .text_type(TextType::Ssml)
                    .text(processed_text)
                    .send()
                    .await?;

                let blob = synth_output.audio_stream.collect().await?;
                let mut file = async_std::fs::File::create(output_dir.join(&hash)).await?;
                file.write_all(&blob.into_bytes()).await?;
            }

            let file = BufReader::new(fs::File::open(output_dir.join(&hash))?);
            let source = Decoder::new_vorbis(file)?;
            sink.append(source);
        }

        sleep(Duration::from_millis(1000));
    }

    Ok(())
}
