use aws_sdk_polly::model::{Engine, OutputFormat, VoiceId};
use aws_sdk_polly::Client;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use tokio::io::AsyncWriteExt;

const FILENAME: &str = "out.ogg";
const TEXT: &str = "Hello, world!";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let shared_config = aws_config::load_from_env().await;
    let client = Client::new(&shared_config);

    let resp = client
        .synthesize_speech()
        .output_format(OutputFormat::OggVorbis)
        .text(TEXT)
        .voice_id(VoiceId::Joanna)
        .engine(Engine::Neural)
        .send()
        .await?;

    let mut blob = resp
        .audio_stream
        .collect()
        .await
        .expect("failed to read data");

    let mut file = tokio::fs::File::create(FILENAME)
        .await
        .expect("failed to create file");

    file.write_all_buf(&mut blob)
        .await
        .expect("failed to write to file");

    let (_stream, stream_handle) = OutputStream::try_default().expect("failed to create stream");
    let sink = Sink::try_new(&stream_handle).expect("failed to create sink");

    let file = BufReader::new(File::open(FILENAME).expect("failed to open file"));
    let source = Decoder::new_vorbis(file).expect("failed to create decoder");

    sink.append(source);

    sink.sleep_until_end();

    Ok(())
}
