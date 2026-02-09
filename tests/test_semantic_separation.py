import unittest
import sys
import os
sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), '../src')))

from unittest.mock import MagicMock
from fragment_assembler import FragmentAssembler, Fragment

class TestSemanticSeparation(unittest.TestCase):
    def test_semantic_disentangle(self):
        assembler = FragmentAssembler(max_gap=1000)
        
        # Mock classifier
        classifier = MagicMock()
        def mock_classify(text):
            if "trade" in text: return ("Trading", 1.0)
            if "mind" in text: return ("Psychology", 1.0)
            return ("Unknown", 0.0)
        classifier.classify_text.side_effect = mock_classify
        
        # Create interleaved fragments
        # Stream A: Trading (offsets 0, 100)
        f1 = Fragment(offset=0, size=50, data=b"trade profit", links=set(), file_type="txt")
        f2 = Fragment(offset=100, size=50, data=b"trade loss", links=set(), file_type="txt")
        
        # Stream B: Psychology (offset 50) - clearly interleaved
        f3 = Fragment(offset=50, size=50, data=b"mind control", links=set(), file_type="txt")
        
        fragments = [f1, f2, f3]
        
        # Without classifier, it should probably group by offset logic alone
        # But here f3 is perfectly in between f1 and f2.
        # size=50. f1 ends at 50. f3 at 50 is perfect gap=0 match for f1.
        # f3 ends at 100. f2 at 100 is perfect gap=0 match for f3.
        # So physically: f1 -> f3 -> f2 is a perfect stream.
        
        # BUT semantically: f1(Trading) -> f3(Psychology) should be discouraged 
        # if we strictly enforcing semantics.
        # My implementation only adds +30/-30 score.
        # Gap=0 gives +100.
        # Semantic Mismatch gives -30.
        # Net = +70.
        # So it might still link them if gap is perfect.
        
        # Let's make gap slightly larger to trigger semantic decision.
        # f1 ends at 50.
        # f3 starts at 60 (gap 10).
        # f2 starts at 110 (gap from f1 end=60).
        
        f1 = Fragment(offset=0, size=50, data=b"trade profit", links=set(), file_type="txt") # 0-50
        f3 = Fragment(offset=60, size=50, data=b"mind control", links=set(), file_type="txt") # 60-110 (Gap 10 from f1)
        f2 = Fragment(offset=120, size=50, data=b"trade loss", links=set(), file_type="txt")   # 120-170 (Gap 70 from f1, Gap 10 from f3)
        
        # Gap scores:
        # f1->f3 (gap 10). Score ~ 80. Semantic mismatch -30 = 50.
        # f1->f2 (gap 70). Score ~ 80. Semantic match +30 = 110.
        # Expectation: f1 should jump to f2.
        
        fragments = [f1, f3, f2]
        
        streams = assembler.disentangle_cluster(fragments, classifier=classifier)
        
        # Expect 2 streams
        self.assertEqual(len(streams), 2)
        
        # Stream 1 should be f1, f2
        s1 = sorted(streams[0], key=lambda x: x.offset)
        self.assertEqual(len(s1), 2)
        self.assertEqual(s1[0].offset, 0)
        self.assertEqual(s1[1].offset, 120)
        
        # Stream 2 should be f3
        s2 = streams[1]
        self.assertEqual(len(s2), 1)
        self.assertEqual(s2[0].offset, 60)

if __name__ == '__main__':
    unittest.main()
