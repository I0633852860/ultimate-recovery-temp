
import unittest
from src.fragment_assembler import FragmentAssembler, Fragment
import logging

class TestStreamSolver(unittest.TestCase):
    def setUp(self):
        self.assembler = FragmentAssembler()

    def test_simple_interleaved_streams(self):
        # Create two streams of fragments: A and B
        # Stream A: 1000, 3000, 5000 (size 1000 each)
        # Stream B: 2000, 4000, 6000 (size 1000 each)
        
        f_a1 = Fragment(offset=1000, size=1000, data=b'A'*1000, links=set(), file_type="A")
        f_b1 = Fragment(offset=2000, size=1000, data=b'B'*1000, links=set(), file_type="B")
        f_a2 = Fragment(offset=3000, size=1000, data=b'A'*1000, links=set(), file_type="A")
        f_b2 = Fragment(offset=4000, size=1000, data=b'B'*1000, links=set(), file_type="B")
        f_a3 = Fragment(offset=5000, size=1000, data=b'A'*1000, links=set(), file_type="A")

        fragments = [f_a1, f_b1, f_a2, f_b2, f_a3]
        
        streams = self.assembler.disentangle_cluster(fragments)
        
        self.assertEqual(len(streams), 2)
        
        s1 = streams[0]
        s2 = streams[1]
        
        s1_types = set(f.file_type for f in s1)
        s2_types = set(f.file_type for f in s2)
        
        self.assertTrue(len(s1_types) == 1)
        self.assertTrue(len(s2_types) == 1)
        self.assertNotEqual(s1_types, s2_types)

    def test_gap_preference(self):
        f1 = Fragment(offset=1000, size=1000, data=b'A', links=set(), file_type="A")
        f2 = Fragment(offset=2000, size=1000, data=b'A', links=set(), file_type="A")
        f3 = Fragment(offset=5000, size=1000, data=b'B', links=set(), file_type="B")
        
        fragments = [f1, f3, f2] # Interleaved input order
        
        self.assembler.max_gap = 1000 # Make gap tolerance small
        
        streams = self.assembler.disentangle_cluster(fragments)
        self.assertEqual(len(streams), 2)
        
        # Verify streams content
        # One stream should have 2 fragments (A), the other 1 (B)
        lengths = sorted([len(s) for s in streams])
        self.assertEqual(lengths, [1, 2])

if __name__ == '__main__':
    logging.basicConfig(level=logging.INFO)
    unittest.main()
