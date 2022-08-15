use aws_sdk_polly::model::{OutputFormat, VoiceId};
use aws_sdk_polly::{Client, Error};
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let shared_config = aws_config::load_from_env().await;
    let client = Client::new(&shared_config);
    synthesize(&client, "this is some text").await
}

async fn synthesize(client: &Client, text: &str) -> Result<(), Error> {
    let resp = client
        .synthesize_speech()
        .output_format(OutputFormat::Mp3)
        .text(text)
        .voice_id(VoiceId::Joanna)
        .send()
        .await?;

    // Get MP3 data from response and save it
    let mut blob = resp
        .audio_stream
        .collect()
        .await
        .expect("failed to read data");

    let parts: Vec<&str> = text.split('.').collect();
    let out_file = format!("{}{}", String::from(parts[0]), ".mp3");

    let mut file = tokio::fs::File::create(out_file)
        .await
        .expect("failed to create file");

    file.write_all_buf(&mut blob)
        .await
        .expect("failed to write to file");

    Ok(())
}
