
import unittest
from pathlib import Path
from src.semantic_classifier import SemanticClassifier
import tempfile
import json
import os

class TestSemanticClassifier(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.test_dir.name)
        
        # Create structure
        (self.root / "Trading").mkdir()
        (self.root / "Psychology").mkdir()
        
        # Create dummy info.json
        self.info1 = {
            "id": "vid1",
            "title": "Bitcoin Trading Strategy",
            "filesize": 1000,
            "duration": 60,
            "description": "Learn how to trade crypto"
        }
        with open(self.root / "Trading/video1.info.json", "w") as f:
            json.dump(self.info1, f)
            
        # Create dummy vtt
        with open(self.root / "Trading/video1.en.vtt", "w") as f:
            f.write("WEBVTT\n\n00:00:01.000 --> 00:00:05.000\nHello traders, today we analyze the market.\n")

    def tearDown(self):
        self.test_dir.cleanup()

    def test_load_and_target_map(self):
        classifier = SemanticClassifier(self.root)
        
        # Check target map
        self.assertIn(1000, classifier.target_map)
        info = classifier.target_map[1000]
        self.assertEqual(info['title'], "Bitcoin Trading Strategy")
        self.assertEqual(info['category'], "Trading")

    def test_classification(self):
        classifier = SemanticClassifier(self.root)
        
        text = "I want to open a long position on bitcoin because the market is bullish"
        cat, conf = classifier.classify_text(text)
        
        self.assertEqual(cat, "Trading")
        self.assertGreater(conf, 0.0)

    def test_unknown_classification(self):
        classifier = SemanticClassifier(self.root)
        text = "I like to eat apples and bananas"
        cat, conf = classifier.classify_text(text)
        # Should be Unknown or low confidence
        if cat != "Unknown":
            self.assertLess(conf, 0.5)

if __name__ == '__main__':
    unittest.main()
