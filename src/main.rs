use aws_sdk_polly::model::{Engine, OutputFormat, TextType, VoiceId};
use aws_sdk_polly::Client;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::DirEntry;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

const APPNAME: &str = "lexicc";

fn process_text(text: &str) -> String {
    format!("<speak><prosody rate=\"x-fast\">{}</prosody></speak>", text)
}

fn create_state_dir(name: &str) -> PathBuf {
    let home_path = dirs::home_dir().unwrap();
    let state_path = home_path.join(".local/state").join(APPNAME).join(name);
    std::fs::create_dir_all(&state_path).unwrap();
    state_path
}

fn entries_from(path: &PathBuf) -> Vec<DirEntry> {
    let mut entries: Vec<DirEntry> = std::fs::read_dir(path)
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    entries.sort_by_key(|entry| entry.path());
    entries
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inbox_dir = create_state_dir("inbox");
    let output_dir = create_state_dir("audio");

    let shared_config = aws_config::load_from_env().await;
    let client = Client::new(&shared_config);

    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    loop {
        let inbox_entries = entries_from(&inbox_dir);
        if inbox_entries.is_empty() && sink.empty() {
            break;
        }

        for entry in inbox_entries {
            let text = std::fs::read_to_string(&entry.path())?;
            let processed_text = process_text(&text);

            let synth_output = client
                .synthesize_speech()
                .output_format(OutputFormat::OggVorbis)
                .voice_id(VoiceId::Joanna)
                .engine(Engine::Neural)
                .text_type(TextType::Ssml)
                .text(processed_text)
                .send()
                .await?;

            let mut blob = synth_output.audio_stream.collect().await?;
            let mut file = tokio::fs::File::create(output_dir.join(entry.file_name())).await?;
            file.write_all_buf(&mut blob).await?;

            let file = BufReader::new(std::fs::File::open(output_dir.join(entry.file_name()))?);
            let source = Decoder::new_vorbis(file)?;
            sink.append(source);

            std::fs::remove_file(&entry.path())?;
        }

        std::thread::sleep(Duration::from_millis(1000));
    }

    Ok(())
}
