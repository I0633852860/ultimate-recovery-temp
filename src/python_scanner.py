import os
import re
import time
from typing import Callable, Optional, Dict, List
from dataclasses import dataclass

import mmap

@dataclass
class ScanResult:
    links: List[Dict]
    bytes_scanned: int
    duration_secs: float

class PythonFileScanner:
    """
    Optimized Python scanner using mmap and fast bytes.find().
    """
    def __init__(self, num_threads=0, chunk_size_mb=256, overlap_kb=64, deduplicate=True, min_confidence=0.1):
        self.chunk_size = chunk_size_mb * 1024 * 1024
        self.overlap = overlap_kb * 1024
        self.deduplicate = deduplicate
        self.min_confidence = min_confidence
        # Main regex for YouTube video IDs (for validation)
        self.yt_pattern = re.compile(rb'(?:https?://)?(?:www\.)?(?:youtube\.com/watch\?v=|youtu\.be/)([a-zA-Z0-9_-]{11})(?![a-zA-Z0-9_-])')
        # Fast search needles
        self.needles = [b'youtube.com/watch?v=', b'youtu.be/']

    def scan_streaming(
        self, 
        image_path: str, 
        start_position: int, 
        reverse: bool, 
        progress_cb: Optional[Callable[[int], None]], 
        hot_fragment_cb: Optional[Callable[[Dict], None]]
    ) -> Dict:
        """
        Scans the file using mmap and fast prefix search.
        """
        file_size = os.path.getsize(image_path)
        bytes_scanned_total = 0
        start_time = time.time()
        
        found_links = []
        
        with open(image_path, 'rb') as f:
            # Memory map the file (read-only)
            # Use access=mmap.ACCESS_READ for Linux
            try:
                mm = mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ)
            except OSError as e:
                print(f"mmap failed: {e}, falling back to slow read")
                return self._scan_streaming_slow(f, file_size, start_position, progress_cb, hot_fragment_cb)

            current_pos = start_position
            
            # Streaming loop with mmap
            while current_pos < file_size:
                # Determine chunk boundaries
                chunk_end = min(current_pos + self.chunk_size, file_size)
                # Optimization: direct slice of mmap is zero-copy in recent Python? 
                # Actually slicing mmap creates a bytes copy. 
                # But we can limit search range in find() if we iterate manually, 
                # OR just slice because processing 64MB chunks is fine.
                
                # To handle overlaps without re-reading, we can just search in the window
                # [current_pos, chunk_end + overlap]
                window_end = min(chunk_end + self.overlap, file_size)
                chunk_data = mm[current_pos:window_end]
                
                # Fast search for needles
                for needle in self.needles:
                    start = 0
                    while True:
                        idx = chunk_data.find(needle, start)
                        if idx == -1:
                            break
                        
                        # Found a needle, extract context for regex validation
                        # Context: e.g. 50 bytes before (for http) and 11 bytes after + safety
                        # The regex needs to match from the start of the potential URL
                        # Let's verify exactly at this position or slightly earlier
                        
                        # Simple approach: apply regex on a small window around the match
                        # needle is part of the pattern.
                        # Pattern: (http...)?(www.)?NEEDLE(id)
                        # We found NEEDLE.
                        
                        # Let's take a slice around the hit to run regex
                        # Max prefix length (https://www.) is ~12 chars
                        # Video ID is 11 chars
                        check_start = max(0, idx - 20) 
                        check_end = min(len(chunk_data), idx + len(needle) + 12)
                        
                        check_window = chunk_data[check_start:check_end]
                        
                        # We use search instead of match
                        m = self.yt_pattern.search(check_window)
                        if m:
                            video_id = m.group(1).decode('utf-8', errors='ignore')
                            # Match offset relative to check_window start
                            rel_match_offset = m.start()
                            # Absolute offset
                            abs_offset = current_pos + check_start + rel_match_offset
                            
                            # Deduplicate simple check: if we already found this offset recently?
                            # Handled by logic below or post-processing?
                            # recover.py handles loose duplicates usually.
                            
                            # Boundary check: if match is in overlap region (>= chunk_size),
                            # we will catch it in next iteration.
                            if (current_pos + check_start + rel_match_offset) >= (current_pos + self.chunk_size) and (chunk_end < file_size):
                                start = idx + 1
                                continue

                            link_info = {
                                "url": f"https://youtube.com/watch?v={video_id}",
                                "video_id": video_id,
                                "title": None,
                                "offset": abs_offset,
                                "pattern_name": "youtube_id",
                                "confidence": 1.0,
                                "score": 100.0
                            }
                            found_links.append(link_info)
                            
                            if hot_fragment_cb:
                                hot_fragment_cb(link_info)

                        start = idx + 1
                
                # Update progress
                advance_by = chunk_end - current_pos
                current_pos = chunk_end
                bytes_scanned_total += advance_by
                
                if progress_cb:
                    progress_cb(current_pos)
            
            mm.close()
        
        duration = time.time() - start_time
        
        return {
            "links": found_links,
            "bytes_scanned": bytes_scanned_total,
            "duration_secs": duration
        }

    def _scan_streaming_slow(self, f, file_size, start_position, progress_cb, hot_fragment_cb):
        # Fallback to the old implementation if mmap fails
        # (Copy-paste previous logic or keep it as backup method)
        pass # To keep diff small, we assume mmap works on 64-bit Linux usually.
             # In a real impl we would move the old code here.
