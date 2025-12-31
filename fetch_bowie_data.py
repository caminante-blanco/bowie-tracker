import requests
import json
import time
import sys
import os

ARTIST_MBID = "5441c29d-3602-4898-b1a1-b77fa23b8e50"
USER_AGENT = "ZiggyTracker/0.3.2 ( walkercwhite@gmail.com )"
BASE_URL = "https://musicbrainz.org/ws/2"
OUTPUT_FILE = "bowie_metadata.json"
PROGRESS_FILE = ".processed_releases.json"

session = requests.Session()
session.headers.update({
    "User-Agent": USER_AGENT,
    "Accept": "application/json"
})

def fetch_json(url, params=None):
    for attempt in range(5):
        try:
            time.sleep(1.1)
            resp = session.get(url, params=params, timeout=25)
            if resp.status_code == 200:
                return resp.json()
            elif resp.status_code == 503:
                time.sleep(15)
            else:
                time.sleep(5)
        except Exception as e:
            time.sleep(3 * (attempt + 1))
    return None

def get_all_recordings():
    if os.path.exists(OUTPUT_FILE):
        with open(OUTPUT_FILE, "r") as f:
            final_db = json.load(f)
    else:
        final_db = {}

    if os.path.exists(PROGRESS_FILE):
        with open(PROGRESS_FILE, "r") as f:
            processed_rel_ids = set(json.load(f))
    else:
        processed_rel_ids = set()

    print("Fetching all releases for David Bowie (with release-groups)...")
    all_releases = []
    offset = 0
    while True:
        data = fetch_json(f"{BASE_URL}/release", {
            "artist": ARTIST_MBID,
            "inc": "release-groups", 
            "fmt": "json",
            "limit": 100,
            "offset": offset
        })
        if not data or not data.get("releases"):
            break
        batch = data["releases"]
        all_releases.extend(batch)
        print(f"  Found {len(all_releases)} releases...")
        if len(batch) < 100:
            break
        offset += 100

    print(f"Processing {len(all_releases)} releases to extract all possible MBIDs and Art...")
    
    start_time = time.time()
    processed_this_run = 0

    for i, rel in enumerate(all_releases):
        rel_id = rel["id"]
        rg = rel.get("release-group", {})
        rg_id = rg.get("id")
        
        if rel_id in processed_rel_ids:
            continue
            
        if not rg_id:
            continue

        processed_this_run += 1
        elapsed = time.time() - start_time
        avg = elapsed / processed_this_run
        eta = avg * (len(all_releases) - i)
        
        print(f"[{i+1}/{len(all_releases)}] {rel['title']} ({rel_id}) | ETA: {int(eta//60)}m")
        
        details = fetch_json(f"{BASE_URL}/release/{rel_id}", {"inc": "recordings", "fmt": "json"})
        if not details:
            continue

        if rg_id not in final_db:
            final_db[rg_id] = {
                "title": rg.get("title", rel["title"]),
                "type": rg.get("primary-type"),
                "track_count": 0,
                "image_url": None,
                "tracks": []
            }

        # Set image if not already set or null, and this release has one
        if (not final_db[rg_id].get("image_url")) and rel.get("cover-art-archive", {}).get("front"):
            final_db[rg_id]["image_url"] = f"https://coverartarchive.org/release/{rel_id}/front-250"

        existing_track_ids = {t["id"] for t in final_db[rg_id]["tracks"]}
        
        current_rel_track_count = 0
        for medium in details.get("media", []):
            for track in medium.get("tracks", []):
                current_rel_track_count += 1
                rec = track.get("recording", {})
                rec_id = rec.get("id")
                if rec_id and rec_id not in existing_track_ids:
                    final_db[rg_id]["tracks"].append({
                        "id": rec_id,
                        "title": track.get("title", rec.get("title")),
                        "duration_ms": track.get("length") or rec.get("length") or 0
                    })
                    existing_track_ids.add(rec_id)
        
        final_db[rg_id]["track_count"] = max(final_db[rg_id].get("track_count", 0), current_rel_track_count)

        processed_rel_ids.add(rel_id)
        
        if processed_this_run % 10 == 0:
            with open(OUTPUT_FILE + ".tmp", "w") as f:
                json.dump(final_db, f, indent=2)
            os.replace(OUTPUT_FILE + ".tmp", OUTPUT_FILE)
            with open(PROGRESS_FILE, "w") as f:
                json.dump(list(processed_rel_ids), f)

    with open(OUTPUT_FILE, "w") as f:
        json.dump(final_db, f, indent=2)
    with open(PROGRESS_FILE, "w") as f:
        json.dump(list(processed_rel_ids), f)

    print("Exhaustive fetch complete.")

if __name__ == "__main__":
    get_all_recordings()
