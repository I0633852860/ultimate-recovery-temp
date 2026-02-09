import logging
import shutil
import hashlib
from pathlib import Path
from typing import Dict, List, Optional
from collections import defaultdict
import json

from semantic_classifier import SemanticClassifier

logger = logging.getLogger("SemanticAssembler")

class SemanticAssembler:
    """
    Groups unstructured fragments into semantic categories based on content analysis.
    Scan candidates/rejected files -> Classify -> Group into 07_SEMANTIC_GROUPS.
    """

    def __init__(self, output_dir: Path, classifier: SemanticClassifier):
        self.output_dir = Path(output_dir)
        self.classifier = classifier
        self.groups_dir = self.output_dir / "07_SEMANTIC_GROUPS"
        self.stats = defaultdict(int)

    def process_candidates(self, candidates_dir: Path, min_confidence: float = 0.3):
        """
        Iterates over fragments in candidates_dir, classifies them, and groups them.
        """
        if not candidates_dir.exists():
            logger.warning(f"Candidates directory {candidates_dir} does not exist.")
            return

        logger.info(f"Scanning fragments in {candidates_dir}...")
        
        # Create group directories
        for category in self.classifier.CATEGORIES.keys():
            (self.groups_dir / category).mkdir(parents=True, exist_ok=True)
        (self.groups_dir / "Unknown").mkdir(parents=True, exist_ok=True)

        count = 0
        for cand_dir in candidates_dir.iterdir():
            if not cand_dir.is_dir(): continue
            
            # Look for data file (raw.bin or just any large file)
            # CandidateManager stores in `raw.bin` or recovered file?
            # If scanning temp candidates: raw.bin
            # If scanning rejected: raw.bin
            
            data_file = cand_dir / "raw.bin"
            if not data_file.exists():
                # Fallback: maybe it's a recovered file structure?
                continue

            try:
                # Read start of file for text analysis
                # We only need enough bytes to extract keywords
                with open(data_file, 'rb') as f:
                    head_data = f.read(64 * 1024) # 64KB sample
                
                try:
                    text = head_data.decode('utf-8', errors='ignore')
                except Exception:
                    continue

                category, confidence = self.classifier.classify_text(text)

                if confidence >= min_confidence and category != "Unknown":
                    self._add_to_group(category, cand_dir, data_file, confidence)
                else:
                    # Optional: Group unknowns by similarity? For now, skip or low confidence
                    # self._add_to_group("Unknown", cand_dir, data_file, confidence)
                    pass

                count += 1
                if count % 100 == 0:
                    logger.info(f"Processed {count} fragments...")

            except Exception as e:
                logger.error(f"Error processing {cand_dir}: {e}")

        self._save_report()

    def _add_to_group(self, category: str, cand_dir: Path, data_file: Path, confidence: float):
        """
        Copies the fragment to the group directory.
        """
        group_path = self.groups_dir / category
        
        # Create a filename that preserves some info
        # cand_dir.name is like "cand_<OFFSET>_<HASH>"
        safe_name = f"[{int(confidence*100)}%] {cand_dir.name}.txt" # Assume text for now? 
        # Actually it's binary, but we matched text. Let's keep .bin or .txt
        
        target_file = group_path / safe_name
        
        try:
            shutil.copy(data_file, target_file)
            self.stats[category] += 1
            
            # Also save metadata
            meta = {
                "original_candidate": str(cand_dir),
                "confidence": confidence,
                "category": category
            }
            with open(target_file.with_suffix('.json'), 'w') as f:
                json.dump(meta, f, indent=2)
                
        except Exception as e:
            logger.error(f"Failed to copy to group {category}: {e}")

    def _save_report(self):
        """Saves a summary of organized fragments."""
        report_path = self.groups_dir / "semantic_summary.json"
        with open(report_path, 'w') as f:
            json.dump(self.stats, f, indent=2)
        logger.info(f"Semantic assembly complete. Stats: {dict(self.stats)}")
