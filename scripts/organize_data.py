
import shutil
import logging
from pathlib import Path
from src.semantic_classifier import SemanticClassifier

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("Organizer")

def organize_files(source_dir: str):
    source = Path(source_dir)
    if not source.exists():
        logger.error(f"Source directory {source} does not exist")
        return

    # Initialize classifier (will use default keywords if no training data found)
    classifier = SemanticClassifier(source)
    
    # Create category directories
    for cat in classifier.CATEGORIES.keys():
        (source / cat).mkdir(exist_ok=True)
        
    (source / "Unknown").mkdir(exist_ok=True)

    # Group files by YouTube ID (or stem if no ID found)
    import re
    from collections import defaultdict
    groups = defaultdict(list)
    
    # Regex for YouTube ID: [11 chars] at end of stem (mostly)
    # But sometimes filename is just ID.
    # Let's be flexible: look for [...........]
    id_pattern = re.compile(r"\[([a-zA-Z0-9_-]{11})\]")

    logger.info(f"Scanning files in {source}...")
    
    files_found = 0
    for f in source.iterdir():
        if f.is_dir(): continue
        
        # Extract ID
        match = id_pattern.search(f.name)
        if match:
            video_id = match.group(1)
            groups[video_id].append(f)
        else:
            # Fallback: group by stem (without last extension)
            # This handles files without ID in brackets, if any.
            # But be careful: "foo.en.vtt" -> stem "foo.en" -> group "foo.en"
            # "foo.info.json" -> stem "foo.info" -> group "foo.info"
            # This splits groups!
            # If no ID, try to strip known extensions?
            name = f.name
            for ext in ['.info.json', '.en.vtt', '.ru.vtt', '.vtt', '.srt', '.webp', '.jpg', '.mp4', '.mkv', '.webm']:
                if name.endswith(ext):
                    name = name[:-len(ext)]
                    break
            else:
                 # Last resort: just stem
                 name = f.stem
            
            groups[f"NOID_{name}"].append(f)
        
        files_found += 1
            
    logger.info(f"Found {files_found} files in {len(groups)} groups.")
    
    moved_count = 0
    
    for group_id, files in groups.items():
        category = "Unknown"
        
        # 1. Try to classify using .info.json
        info_json = next((f for f in files if f.name.endswith('.info.json')), None)
        
        if info_json:
            try:
                import json
                with open(info_json, 'r', encoding='utf-8') as f:
                    data = json.load(f)
                    # title + description
                    text = f"{data.get('title', '')} {data.get('description', '')}"
                    cat, conf = classifier.classify_text(text)
                    if conf > 0.2:
                        category = cat
            except Exception as e:
                logger.warning(f"Failed to read info.json for {group_id}: {e}")
        
        # 2. If Unknown, try .vtt
        if category == "Unknown":
            vtt_files = [f for f in files if f.suffix == '.vtt']
            for vtt in vtt_files:
                try:
                    text = classifier._read_vtt(vtt)
                    cat, conf = classifier.classify_text(text)
                    if conf > 0.2:
                        category = cat
                        break
                except Exception:
                    pass
        
        # Move all files in group
        target_dir = source / category
        for f in files:
            try:
                # If target file exists, overwrite or skip? shutil.move overwrites on overwrite_existing (Unix rename)
                # But if different filesystem, copy+del.
                # Just move.
                target_path = target_dir / f.name
                shutil.move(str(f), str(target_path))
                # logger.info(f"Moved {f.name} -> {category}") # Too spammy?
            except Exception as e:
                logger.error(f"Failed to move {f.name} to {category}: {e}")
        
        moved_count += 1
        if moved_count % 100 == 0:
            logger.info(f"Organized {moved_count} groups...")

    logger.info(f"Finished. Organized {moved_count} groups.")


if __name__ == "__main__":
    import sys
    source_dir = sys.argv[1] if len(sys.argv) > 1 else "semantic_training"
    organize_files(source_dir)
