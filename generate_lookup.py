import json

CANONICAL_ALBUMS = {
    "david bowie", "space oddity", "the man who sold the world", "hunky dory",
    "the rise and fall of ziggy stardust and the spiders from mars", "aladdin sane",
    "pin ups", "diamond dogs", "young americans", "station to station", "low",
    "\"heroes\"", "lodger", "scary monsters (and super creeps)", "let's dance",
    "tonight", "never let me down", "black tie white noise", "the buddha of suburbia",
    "outside", "earthling", "hours", "heathen", "reality", "the next day", "blackstar",
    "toy"
}

def build_lookup():
    print("Building two-tier lookup database (Canonical Prioritization)...")
    with open("bowie_metadata.json", "r") as f:
        full_db = json.load(f)

    lookup = {
        "recordings": {},
        "release_groups": {},
        "track_durations": {}
    }
    
    def rg_score(rg_id, data):
        title = data.get("title", "").lower()
        score = 0
        
        # Check if it's a known canonical studio album
        # We use a partial match because some titles might have (remastered) etc.
        is_canonical = False
        for canon in CANONICAL_ALBUMS:
            if canon in title:
                is_canonical = True
                break
        
        if is_canonical:
            score += 1000
            # Within canonical, prefer original (no extra tags)
            score -= len(title)
        else:
            # Deprioritize others
            if "live" in title: score -= 200
            if "collection" in title: score -= 100
            if "best of" in title: score -= 100
            if "anthology" in title: score -= 100
            if "greatest hits" in title: score -= 100
            if "best" == title: score -= 500 # Handle the "Best" RG
            score -= len(title)
            
        if data.get("image_url"): score += 50
        
        return score

    sorted_rgs = sorted(
        full_db.items(),
        key=lambda x: rg_score(x[0], x[1]),
        reverse=True
    )

    for rg_id, data in sorted_rgs:
        title = data.get("title")
        image = data.get("image_url")
        rg_type = data.get("type")
        count = data.get("track_count", 11)
        
        lookup["release_groups"][rg_id] = [title, image, count, rg_type]

        for track in data.get("tracks", []):
            rec_id = track.get("id")
            if not rec_id: continue
            
            # Populate track durations
            duration = track.get("duration_ms")
            if duration:
                lookup["track_durations"][rec_id] = duration

            if rec_id not in lookup["recordings"]:
                lookup["recordings"][rec_id] = rg_id

    with open("bowie_lookup.json", "w") as f:
        json.dump(lookup, f)
    
    print(f"Database ready. Recordings: {len(lookup['recordings'])}, Groups: {len(lookup['release_groups'])}, Durations: {len(lookup['track_durations'])}")

if __name__ == "__main__":
    build_lookup()