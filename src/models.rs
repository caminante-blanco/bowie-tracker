use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ListenBrainzResponse {
    pub payload: Payload,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PlayingNowResponse {
    pub payload: PlayingNowPayload,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PlayingNowPayload {
    pub listens: Vec<PlayingNowListen>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PlayingNowListen {
    pub track_metadata: TrackMetadata,
    pub playing_now: bool,
}

// ... existing MBReleaseGroupResponse etc ...

// MusicBrainz Models
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MBReleaseGroupResponse {
    pub releases: Vec<MBRelease>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MBRelease {
    #[serde(rename = "track-count")]
    pub track_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Payload {
    pub count: i64,
    pub latest_listen_ts: i64,
    pub listens: Vec<Listen>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Listen {
    pub inserted_at: i64,
    pub listened_at: i64,
    pub recording_msid: String,
    pub track_metadata: TrackMetadata,
    pub user_name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TrackMetadata {
    pub artist_name: String,
    pub track_name: String,
    pub release_name: Option<String>,
    pub additional_info: Option<AdditionalInfo>,
    pub mbid_mapping: Option<MbidMapping>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MbidMapping {
    pub recording_name: Option<String>,
    pub recording_mbid: Option<String>,
    pub artists: Option<Vec<MappedArtist>>,
    pub release_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MappedArtist {
    pub artist_credit_name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AdditionalInfo {
    pub artist_names: Option<Vec<String>>,
    pub recording_mbid: Option<String>,
    pub duration_ms: Option<i64>,
    pub release_group_mbid: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BowieLookup {
    pub recordings: std::collections::HashMap<String, String>, // RecID -> RG_ID
    pub release_groups: std::collections::HashMap<String, (String, Option<String>, usize, Option<String>)>, // RG_ID -> (Title, Art, Count, Type)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BowieDatabase {
    #[serde(flatten)]
    pub release_groups: std::collections::HashMap<String, BowieReleaseGroup>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BowieReleaseGroup {
    pub title: String,
    #[serde(rename = "type")]
    pub release_type: Option<String>,
    pub track_count: usize,
    pub image_url: Option<String>,
    pub tracks: Vec<BowieTrack>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BowieTrack {
    pub id: String, // Recording MBID
    pub title: String,
    pub duration_ms: i64,
}
