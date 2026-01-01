use bowie_tracker::models::ListenBrainzResponse;
use bowie_tracker::analytics::is_bowie;
use reqwest::Client;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --example verify_user <username> [token]");
        std::process::exit(1);
    }

    let user = &args[1];
    let token = args.get(2);

    println!("--- Headless Verification for user: {} ---", user);

    let url = format!("https://api.listenbrainz.org/1/user/{}/listens?count=100", user);
    println!("Fetching from: {}", url);

    let client = Client::new();
    let mut req = client.get(&url);
    if let Some(t) = token {
        req = req.header("Authorization", format!("Token {}", t));
    }

    let resp = req.send().await?;

    if !resp.status().is_success() {
        eprintln!("API Error: {}", resp.status());
        return Ok(())
    }

    let json: ListenBrainzResponse = resp.json().await?;
    let listens = json.payload.listens;

    println!("Fetched {} listens.", listens.len());

    let bowie_listens: Vec<_> = listens.iter().filter(|l| is_bowie(l)).collect();
    println!("Found {} David Bowie listens in the last 100 tracks.", bowie_listens.len());

    println!("\n--- Data Preview (MBID Mapping Only) ---");
    for listen in bowie_listens.iter().take(5) {
        let raw_track = &listen.track_metadata.track_name;
        let display_track = listen.track_metadata.mbid_mapping
            .as_ref()
            .and_then(|m| m.recording_name.clone())
            .unwrap_or_else(|| raw_track.clone());
        
        let raw_artist = &listen.track_metadata.artist_name;
        let mapped_artist = listen.track_metadata.mbid_mapping
            .as_ref()
            .and_then(|m| m.artists.as_ref())
            .and_then(|a| a.first())
            .map(|a| &a.artist_credit_name)
            .unwrap_or(raw_artist);

        println!("Original: {} - {}", raw_artist, raw_track);
        println!("Display : {} - {}", mapped_artist, display_track);
        println!("---------------------------------------------------");
    }

    if bowie_listens.is_empty() {
        println!("No Bowie tracks found.");
    } else {
        println!("Verification Complete.");
    }

    Ok(())
}