
import json
import logging
import re
from pathlib import Path
from typing import Dict, List, Set, Tuple, Optional
from collections import Counter

logger = logging.getLogger(__name__)

class SemanticClassifier:
    """
    Classifies content based on keywords extracted from subtitles/metadata.
    Builds a 'Target Map' of known lost files (Size -> Metadata).
    """
    
    CATEGORIES = {
        "Trading": ["trading", "forex", "crypto", "bitcoin", "chart", "price", "market", "analysis", "trade", "profit", "loss", "stop loss", "take profit", "indicator", "strategy", "трейдинг", "крипта", "биткоин", "график", "цена", "рынок", "анализ", "торговля", "профит", "лосс", "стратегия", "сделка", "разбор", "lipovoy", "cherny", "burla", "shevchenko", "parshikov", "липовой", "черный", "чёрный", "бурла", "шевченко", "паршиков", "смирнов"],
        "Masterclasses": ["masterclass", "mk", "workshop", "tutorial", "course", "guide", "learn", "how to", "series", "bootcamp", "training", "мк", "мастеркласс", "мастер-класс", "курсы", "обучение"],
        "Webinars": ["webinar", "class", "lesson", "module", "introduction", "summary", "zoom", "meeting", "presentation", "slide", "qa", "questions", "вебинар", "урок", "занятие", "введение", "зум", "встреча", "презентация", "вопросы"],
        "Psychology": ["psychology", "mindset", "emotion", "fear", "greed", "discipline", "control", "brain", "bias", "behavior", "cognitive", "mental", "психология", "мышление", "эмоции", "страх", "жадность", "дисциплина", "контроль", "мозг", "поведение"]
    }

    def __init__(self, training_dir: Path):
        self.training_dir = Path(training_dir)
        self.target_map: Dict[int, Dict] = {} # Size -> Metadata
        self.keyword_index: Dict[str, Counter] = {} # Category -> Word Freq
        self._load_data()

    def _load_data(self):
        """Loads metadata and builds keyword index from training directory."""
        if not self.training_dir.exists():
            logger.warning(f"Training directory {self.training_dir} does not exist.")
            return

        # 1. Build Target Map from .info.json files
        for info_file in self.training_dir.glob("**/*.info.json"):
            try:
                with open(info_file, 'r', encoding='utf-8') as f:
                    data = json.load(f)
                    
                # Extract key metadata
                file_size = data.get('filesize')
                if not file_size:
                    # Try to estimate or skip?
                    continue
                    
                self.target_map[file_size] = {
                    'id': data.get('id'),
                    'title': data.get('title'),
                    'duration': data.get('duration'),
                    'category': self._determine_category_by_path(info_file)
                }
                
                # Update keyword index if category is known
                category = self.target_map[file_size]['category']
                if category and category in self.CATEGORIES:
                     self._update_keywords(category, data.get('title', '') + " " + data.get('description', ''))
                     
            except Exception as e:
                logger.warning(f"Failed to load {info_file}: {e}")

        # 2. Process subtitles for deeper keyword analysis
        # (Optional, improved accuracy)
        for sub_file in self.training_dir.glob("**/*.vtt"):
             # Find corresponding category
             category = self._determine_category_by_path(sub_file)
             if category and category in self.CATEGORIES:
                 content = self._read_vtt(sub_file)
                 self._update_keywords(category, content)

    def _determine_category_by_path(self, path: Path) -> str:
        """Heuristic: check parent directory name."""
        for part in path.parts:
            if part in self.CATEGORIES:
                return part
        return "Unknown"

    def _read_vtt(self, path: Path) -> str:
        """Simple VTT text extractor."""
        text = []
        try:
            with open(path, 'r', encoding='utf-8') as f:
                for line in f:
                    if "-->" in line: continue
                    if line.strip().isdigit(): continue
                    if line.strip() == "WEBVTT": continue
                    if not line.strip(): continue
                    text.append(line.strip())
        except Exception:
            pass
        return " ".join(text)

    def _update_keywords(self, category: str, text: str):
        if category not in self.keyword_index:
            self.keyword_index[category] = Counter()
            
        words = re.findall(r'\w+', text.lower())
        # Filter stopwords (very basic list)
        stopwords = {'the', 'a', 'an', 'and', 'or', 'but', 'is', 'are', 'was', 'were', 'to', 'for', 'of', 'in', 'on', 'at', 'with', 'by', 'this', 'that', 'it', 'you', 'i', 'we', 'they'}
        
        filtered = [w for w in words if w not in stopwords and len(w) > 3]
        self.keyword_index[category].update(filtered)

    def classify_text(self, text: str) -> Tuple[str, float]:
        """
        Classifies input text into one of the categories.
        Returns (Category, Confidence).
        """
        if not text: return ("Unknown", 0.0)
        
        scores = {cat: 0.0 for cat in self.CATEGORIES}
        words = re.findall(r'\w+', text.lower())
        
        total_words = len(words)
        if total_words == 0: return ("Unknown", 0.0)

        # 1. Check strict keywords (high weight)
        for word in words:
            for cat, keywords in self.CATEGORIES.items():
                if word in keywords:
                    scores[cat] += 5.0
        
        # 2. Check learned frequency (lower weight)
        # Normalize scores
        max_score = 0
        best_cat = "Unknown"
        
        for cat, score in scores.items():
            if score > max_score:
                max_score = score
                best_cat = cat
                
        # Simple confidence
        confidence = min(max_score / (total_words * 0.5 + 1), 1.0) 
        
        return best_cat, confidence

    def get_target_info(self, size: int) -> Optional[Dict]:
        """Checks if a file size matches a known target."""
        # Allow small tolerance? exFAT is cluster aligned, original size might be exact.
        # But recovered file might be padded.
        # Check exact match first
        if size in self.target_map:
            return self.target_map[size]
            
        # Check tolerance (+/- 4KB for cluster padding?)
        # Actually, metadata 'filesize' is exact. Recovered file content length might be larger due to padding?
        # FragmentAssembler trims? No.
        
        return None
