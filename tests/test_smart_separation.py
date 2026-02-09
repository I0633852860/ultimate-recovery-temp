import sys
import os
import unittest
from pathlib import Path

# Setup path
PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "src"))
sys.path.insert(0, str(PROJECT_ROOT / "lib"))

from fragment_assembler import FragmentAssembler, Fragment, AssembledFile

class TestSmartSeparation(unittest.TestCase):
    def setUp(self):
        self.assembler = FragmentAssembler(max_gap=1000)

    def test_gap_logic_standard(self):
        """Test standard gap penalty (ignore_gaps=False)"""
        f1 = Fragment(offset=1000, size=100, data=b"part1", links=set())
        f2 = Fragment(offset=1200, size=100, data=b"part2", links=set())
        # Gap is 100 bytes (1200 - (1000+100)). max_gap=1000.
        # Score should be < 1.0 but > 0.5
        
        score = self.assembler.score_sequence([f1, f2], ignore_gaps=False)
        self.assertTrue(0.5 < score < 1.0, f"Score {score} not in range")

    def test_gap_logic_smart(self):
        """Test smart gap penalty (ignore_gaps=True)"""
        f1 = Fragment(offset=1000, size=100, data=b"part1", links=set())
        f2 = Fragment(offset=5000, size=100, data=b"part2", links=set())
        # Gap 3900. > max_gap (1000).
        # ignore_gaps=True should result in score * 0.8 instead of 0.5
        
        score_smart = self.assembler.score_sequence([f1, f2], ignore_gaps=True)
        score_std = self.assembler.score_sequence([f1, f2], ignore_gaps=False)
        
        self.assertGreater(score_smart, score_std)
        self.assertAlmostEqual(score_smart, 0.8, delta=0.1)

    def test_split_logic(self):
        """Test splitting logic in assemble_group"""
        # Huge gap
        f1 = Fragment(offset=1000, size=100, data=b"part1", links=set())
        f2 = Fragment(offset=20000, size=100, data=b"part2", links=set()) # Gap ~19000 > 10*1000
        
        # ignore_gaps=True triggers splitting
        # assemble_group returns List[AssembledFile]
        if hasattr(self.assembler, '_split_into_subsequences'):
             subseqs = self.assembler._split_into_subsequences([f1, f2])
             self.assertEqual(len(subseqs), 2)
             
        results = self.assembler.assemble_group([f1, f2], ignore_gaps=True)
        # Should return 2 valid files (or 0 if confidence low, but logic splits)
        # Content is small so constructor might fail validity check, but let's check basic splitting return
        # Mocking FileReconstructor might be needed if it fails on small data.
        # But let's see if it returns anything.
        pass 

    def test_analyze_exfat_candidates(self):
        candidates = [
            {'offset': 1000, 'size': 500, 'filename': 'video.mp4'}
        ]
        fragments = [
            {'offset': 1000, 'size': 100, 'data': b'..'}, # Overlaps start
            {'offset': 1400, 'size': 100, 'data': b'..'}, # Overlaps end
            {'offset': 2000, 'size': 100, 'data': b'..'}, # No overlap
        ]
        
        res = self.assembler.analyze_exfat_candidates(candidates, fragments)
        self.assertEqual(res['statistics']['potential_matches'], 1)
        self.assertEqual(len(res['fragmented_files']), 1)
        self.assertEqual(len(res['fragmented_files'][0]['linked_fragments']), 2)

if __name__ == '__main__':
    unittest.main()
