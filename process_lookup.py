import json

def process_metadata():
    print("Processing bowie_metadata.json to update bowie_lookup.json...")
    
    with open("bowie_metadata.json", "r") as f:
        full_db = json.load(f)
        
    with open("bowie_lookup.json", "r") as f:
        lookup = json.load(f)
        
    name_map = {} # name -> [(rec_mbid, rg_mbid)]
    
    for rg_id, data in full_db.items():
        for track in data.get("tracks", []):
            rec_id = track.get("id")
            rec_name = track.get("title")
            if not rec_id or not rec_name:
                continue
            
            norm_name = rec_name.lower()
            if norm_name not in name_map:
                name_map[norm_name] = []
            
            # Use tuple to avoid duplicates if same recording is in multiple releases (unlikely here but safe)
            entry = (rec_id, rg_id)
            if entry not in name_map[norm_name]:
                name_map[norm_name].append(entry)
                
    lookup["name_map"] = name_map
    
    with open("bowie_lookup.json", "w") as f:
        json.dump(lookup, f)
        
    print(f"Updated bowie_lookup.json with {len(name_map)} track names.")

if __name__ == "__main__":
    process_metadata()
