//! Database logic for Ziggy.
//! 
//! MIT License
//! 
//! Copyright (c) 2024 RustyNova (Original Logic)
//! 
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//! 
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//! 
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use rexie::{Rexie, TransactionMode, ObjectStore, Index, Direction};
use std::collections::HashMap;
use crate::models::{Listen, MbidMapping};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CanonicalMapping {
    pub msid: String,
    pub recording_name: String,
    pub artist_name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AlbumMetadata {
    pub release_group_mbid: String,
    pub track_count: usize,
}

pub async fn init_db() -> Result<Rexie, String> {
    let rexie = Rexie::builder("bowie_tracker_db")
        .version(3) // Incremented for album_metadata store
        .add_object_store(
            ObjectStore::new("listens")
                .key_path("inserted_at")
                .add_index(Index::new("listened_at", "listened_at"))
        )
        .add_object_store(
            ObjectStore::new("mappings")
                .key_path("msid")
        )
        .add_object_store(
            ObjectStore::new("album_metadata")
                .key_path("release_group_mbid")
        )
        .build()
        .await
        .map_err(|e| e.to_string())?;
    Ok(rexie)
}

pub async fn add_listens(db: &Rexie, listens: Vec<Listen>) -> Result<(), String> {
    let transaction = db.transaction(&["listens", "mappings"], TransactionMode::ReadWrite)
        .map_err(|e| e.to_string())?;
    let listens_store = transaction.store("listens").map_err(|e| e.to_string())?;
    let mappings_store = transaction.store("mappings").map_err(|e| e.to_string())?;

    for listen in listens {
        let listen_js = serde_wasm_bindgen::to_value(&listen)
            .map_err(|e| format!("Serialization error: {}", e))?;
        listens_store.put(&listen_js, None).await.map_err(|e| e.to_string())?;

        if let Some(mapping) = &listen.track_metadata.mbid_mapping {
            let artist_name = mapping.artists.as_ref()
                .and_then(|a| a.first())
                .map(|a| a.artist_credit_name.clone());

            if let (Some(rec_name), Some(artist)) = (&mapping.recording_name, artist_name) {
                let canonical = CanonicalMapping {
                    msid: listen.recording_msid.clone(),
                    recording_name: rec_name.clone(),
                    artist_name: artist,
                };
                let mapping_js = serde_wasm_bindgen::to_value(&canonical)
                    .map_err(|e| format!("Serialization error: {}", e))?;
                
                mappings_store.put(&mapping_js, None).await.map_err(|e| e.to_string())?;
            }
        }
    }
    
    transaction.done().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_all_listens(db: &Rexie) -> Result<Vec<Listen>, String> {
    let transaction = db.transaction(&["listens"], TransactionMode::ReadOnly).map_err(|e| e.to_string())?;
    let store = transaction.store("listens").map_err(|e| e.to_string())?;
    let all = store.get_all(None, None, None, None).await.map_err(|e| e.to_string())?;
    
    let mut listens = Vec::new();
    for (_key, value) in all { 
        let listen: Listen = serde_wasm_bindgen::from_value(value)
            .map_err(|e| format!("Deserialization error: {}", e))?;
        listens.push(listen);
    }
    Ok(listens)
}

pub async fn get_max_timestamp(db: &Rexie) -> Result<Option<i64>, String> {
    let transaction = db.transaction(&["listens"], TransactionMode::ReadOnly).map_err(|e| e.to_string())?;
    let store = transaction.store("listens").map_err(|e| e.to_string())?;
    let index = store.index("listened_at").map_err(|e| e.to_string())?;
    
    let latest = index.get_all(None, Some(1), None, Some(Direction::Prev))
        .await
        .map_err(|e| e.to_string())?;

    if let Some((_key, value)) = latest.into_iter().next() {
        let listen: Listen = serde_wasm_bindgen::from_value(value)
            .map_err(|e| format!("Deserialization error: {}", e))?;
        return Ok(Some(listen.listened_at));
    }

    Ok(None)
}

pub async fn save_album_metadata(db: &Rexie, meta: AlbumMetadata) -> Result<(), String> {
    let transaction = db.transaction(&["album_metadata"], TransactionMode::ReadWrite).map_err(|e| e.to_string())?;
    let store = transaction.store("album_metadata").map_err(|e| e.to_string())?;
    let js_val = serde_wasm_bindgen::to_value(&meta).map_err(|e| e.to_string())?;
    store.put(&js_val, None).await.map_err(|e| e.to_string())?;
    transaction.done().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_all_album_metadata(db: &Rexie) -> Result<HashMap<String, usize>, String> {
    let transaction = db.transaction(&["album_metadata"], TransactionMode::ReadOnly).map_err(|e| e.to_string())?;
    let store = transaction.store("album_metadata").map_err(|e| e.to_string())?;
    let all = store.get_all(None, None, None, None).await.map_err(|e| e.to_string())?;
    
    let mut map = HashMap::new();
    for (_key, value) in all {
        let meta: AlbumMetadata = serde_wasm_bindgen::from_value(value).map_err(|e| e.to_string())?;
        map.insert(meta.release_group_mbid, meta.track_count);
    }
    Ok(map)
}
