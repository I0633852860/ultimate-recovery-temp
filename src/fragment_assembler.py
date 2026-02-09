#!/usr/bin/env python3
"""
Fragment Assembler - сборка фрагментированных файлов с военным качеством
V11.5: Многоуровневый скоринг (Jaccard + Структура + Энтропия + Офсеты)
"""

import hashlib
import json
import logging
from typing import List, Dict, Optional, Tuple, Set
from dataclasses import dataclass
from pathlib import Path

logger = logging.getLogger(__name__)

@dataclass
class Fragment:
    offset: int
    size: int
    data: bytes
    links: Set[str]
    file_type: str = "unknown"

@dataclass
class AssembledFile:
    fragments: List[Fragment]
    content: bytes
    confidence: float
    is_valid: bool
    file_type: str
    suggested_name: str = ""

    @property
    def total_size(self) -> int:
        return len(self.content)

class FragmentAssembler:
    def __init__(self, max_gap: int = 1024 * 1024, similarity_threshold: float = 0.3):
        self.max_gap = max_gap
        self.similarity_threshold = similarity_threshold
        
    def calculate_jaccard(self, set1: Set[str], set2: Set[str]) -> float:
        if not set1 or not set2: return 0.0
        return len(set1 & set2) / len(set1 | set2)

    def score_sequence(self, frags: List[Fragment], ignore_gaps: bool = False) -> float:
        """
        Evaluates the logical consistency of a fragment sequence.
        1. Checks for overlap (bad).
        2. Checks for gaps (penalized).
        """
        if not frags: return 0.0
        
        score = 1.0
        # Check adjacent fragments
        sorted_frags = sorted(frags, key=lambda x: x.offset)
        
        for i in range(len(sorted_frags) - 1):
            f1, f2 = sorted_frags[i], sorted_frags[i+1]
            gap = f2.offset - (f1.offset + f1.size)
            
            if gap < 0: # Overlap - bad for assembly
                score *= 0.1
            elif gap > self.max_gap:
                if ignore_gaps:
                    # Logic for "SmartSeparation": if gaps are HUGE, we still penalize, 
                    # but not as severely as standard mode.
                    # Standard: 0.5 penalty
                    # Smart: 0.8 (trusting the clusterer)
                    score *= 0.8
                else:
                    score *= 0.5
            else:
                # Smaller gap = higher confidence
                penalty = (gap / self.max_gap) * 0.2
                score *= (1.0 - penalty)
                
        return score

    def disentangle_cluster(self, fragments: List[Fragment]) -> List[List[Fragment]]:
        """
        Stream Solver: Disentangles interleaved fragments into separate streams.
        Uses a greedy approach based on offset continuity and content density.
        """
        if not fragments: return []
        if len(fragments) == 1: return [fragments]
        
        # Sort by offset
        pending = sorted(fragments, key=lambda x: x.offset)
        streams: List[List[Fragment]] = []
        
        while pending:
            current_stream = []
            
            # Start new stream with the earliest fragment
            current_frag = pending.pop(0)
            current_stream.append(current_frag)
            
            # Try to extend this stream
            # Look for the best next fragment in remaining pending items
            while True:
                best_idx = -1
                best_score = -float('inf')
                
                expected_offset = current_frag.offset + current_frag.size
                
                # Search window: Look at next N fragments to find a good fit
                # limit lookahead to avoid O(N^2) on huge sets
                lookahead = min(len(pending), 50) 
                
                for i in range(lookahead):
                    cand = pending[i]
                    
                    # Calculate gap
                    gap = cand.offset - expected_offset
                    
                    # Hard constraints
                    if gap < -4096: # Significant overlap
                         continue
                         
                    # Scoring logic
                    # 1. Gap Score
                    if gap >= self.max_gap:
                         gap_score = -50
                    elif gap < 0:
                         gap_score = -50
                    elif gap == 0:
                        gap_score = 100
                    elif gap < 32 * 1024:
                        gap_score = 80 - (gap / 1024)
                    else:
                        gap_score = 40 - (gap / self.max_gap * 40)
                        
                    # 2. Alignment Score
                    align_score = 10 if (cand.offset % 512 == 0) else 0
                    
                    # 3. Type/Link Continuity
                    if cand.file_type == current_frag.file_type and cand.file_type != "unknown":
                        type_score = 20
                    elif cand.file_type != current_frag.file_type and cand.file_type != "unknown" and current_frag.file_type != "unknown":
                        type_score = -50
                    else:
                        type_score = 0
                    
                    final_score = gap_score + align_score + type_score
                    
                    # print(f"DEBUG: Current={current_frag.offset}, Cand={cand.offset}, Gap={gap}, GapScore={gap_score}, Final={final_score}")
                    
                    if final_score > best_score and final_score > 0:
                        best_score = final_score
                        best_idx = i
                
                if best_idx != -1:
                    # Found a successor
                    next_frag = pending.pop(best_idx)
                    current_stream.append(next_frag)
                    current_frag = next_frag
                else:
                    # End of this stream
                    break
            
            streams.append(current_stream)
            
        return streams

    def _split_into_subsequences(self, fragments: List[Fragment]) -> List[List[Fragment]]:
        # Legacy/Fallback wrapper
        return self.disentangle_cluster(fragments)



    def assemble_group(self, fragments: List[Fragment], ignore_gaps: bool = False) -> List[AssembledFile]:
        """Сборка группы фрагментов. Может вернуть несколько файлов, если обнаружены разрывы."""
        if not fragments: return []
        
        # 1. Split logic for SmartSeparation
        sequences = [fragments]
        if ignore_gaps:
             sequences = self.disentangle_cluster(fragments)
        
        results = []
        
        for seq in sequences:
            if not seq: continue
            
            # Сортируем по офсету
            sorted_frags = sorted(seq, key=lambda x: x.offset)
            
            # Собираем контент
            content = b"".join(f.data for f in sorted_frags)
            
            # Валидация
            from file_reconstructor import FileReconstructor, FileType
            reconstructor = FileReconstructor()
            res = reconstructor.reconstruct(content)
            
            # Итоговый скоринг
            seq_score = self.score_sequence(sorted_frags, ignore_gaps=ignore_gaps)
            final_confidence = (res.confidence / 100.0) * seq_score
            
            if res.is_valid and final_confidence > 0.4: # Lower threshold allowed for assembled
                results.append(AssembledFile(
                    fragments=sorted_frags,
                    content=content,
                    confidence=final_confidence * 100,
                    is_valid=True,
                    file_type=res.file_type.value,
                    suggested_name=res.suggested_name
                ))
                
        return results

    def load_exfat_candidates_from_dir(self, directory: Path) -> List[Dict]:
        """Loads exFAT candidates from the recovery directory."""
        candidates = []
        if not directory.exists():
            return candidates
            
        for meta_file in directory.glob("*.meta.json"):
            try:
                with open(meta_file, 'r') as f:
                    meta = json.load(f)
                    # Find corresponding file
                    # meta file is X.meta.json, file is X
                    # but file might have extension
                    
                    # Heuristic: base name of meta file without .meta.json
                    base_name = meta_file.name.replace(".meta.json", "")
                    
                    # Try to find file with this base name and any extension?
                    # Or just assume it's next to it.
                    # recover.py saves file as "safe_name", then "safe_name.meta.json"
                    
                    # We can use the info in meta to find the file if needed, 
                    # OR just look for files in dir that are NOT .json
                    pass
                    
                # For now just return what we have in meta plus placeholder for data
                # Actually, recover.py expects a list of dicts with 'offset', 'size', etc.
                candidates.append({
                    'offset': int(meta.get('entry_offset', '0'), 16),
                    'size': meta.get('size_bytes', 0),
                    'filename': meta.get('original_filename', ''),
                    'sha256': meta.get('sha256', ''),
                    'is_deleted': meta.get('is_deleted', False),
                    'linked_fragments': [] # Placeholder
                })
            except Exception as e:
                logger.warning(f"Failed to load meta {meta_file}: {e}")
                
        return candidates

    def analyze_exfat_candidates(self, candidates: List[Dict], fragments: List[Dict]) -> Dict:
        """
        Analyzes relationship between exFAT metadata candidates and recovered fragments.
        Returns statistics and potential matches.
        """
        results = {
            'statistics': {
                'potential_matches': 0,
                'confidence_score': 0.0
            },
            'fragmented_files': []
        }
        
        if not candidates or not fragments:
            return results
            
        matches = 0
        total_score = 0.0
        
        # Sort for efficiency
        sorted_frags = sorted(fragments, key=lambda x: x['offset'])
        
        for cand in candidates:
            cand_start = cand['offset']
            cand_end = cand_start + cand['size']
            
            linked_frags = []
            
            # Simple range overlap check
            for frag in sorted_frags:
                frag_start = frag['offset']
                frag_end = frag_start + frag['size']
                
                # Check overlap
                if max(cand_start, frag_start) < min(cand_end, frag_end):
                    linked_frags.append(frag)
                    
            if linked_frags:
                matches += 1
                total_score += 0.8 # Heuristic confidence
                
                cand_copy = cand.copy()
                cand_copy['linked_fragments'] = linked_frags
                results['fragmented_files'].append(cand_copy)
                
        results['statistics']['potential_matches'] = matches
        if matches > 0:
            results['statistics']['confidence_score'] = (total_score / matches) * 100.0
            
        return results

    def assemble_multiple_files(self, fragments: List[Dict]) -> List[AssembledFile]:
        """
        Main entry point for fragment assembly.
        Takes raw cluster fragments and attempts to assemble them into files.
        """
        return self.process_clusters(fragments)

    def _extract_domain(self, link: str) -> str:
        """Simple domain extractor for grouping."""
        try:
            if "youtube.com" in link or "youtu.be" in link: return "youtube"
            if "tiktok.com" in link: return "tiktok"
            if "instagram.com" in link: return "instagram"
            if "facebook.com" in link: return "facebook"
            return "other"
        except:
            return "other"

    def process_clusters(self, fragments: List[Dict]) -> List[AssembledFile]:
        """Иерархическая сборка: группировка -> сборка"""
        # 1. Преобразуем в объекты Fragment
        from file_reconstructor import FileReconstructor
        recon = FileReconstructor()
        
        frag_objs = []
        for f in fragments:
            data = f.get('data', b'')
            links = set(recon.youtube_pattern.findall(recon.clean_text(data)))
            frag_objs.append(Fragment(
                offset=f.get('offset', 0),
                size=len(data),
                data=data,
                links=links,
                file_type=f.get('file_type', 'unknown')
            ))
            
        # 2. Группировка по Jaccard
        groups = []
        used = set()
        
        for i, f1 in enumerate(frag_objs):
            if i in used: continue
            current_group = [f1]
            used.add(i)
            
            for j, f2 in enumerate(frag_objs):
                if j in used: continue
                if self.calculate_jaccard(f1.links, f2.links) >= self.similarity_threshold:
                    current_group.append(f2)
                    used.add(j)
            groups.append(current_group)

        # 3. Smart Separation (Manus Algo) via Rust Accelerator
        results = []
        print("DEBUG: Starting Smart Separation...")
        
        try:
            from rust_accelerator import FragmentClusterer
            import rust_accelerator
            print(f"DEBUG: Rust FragmentClusterer imported.")
            
            # Identify candidates for Smart Clustering
            # Group by domain for better precision
            smart_pools: Dict[str, List[Fragment]] = {}
            standard_groups = []
            
            for group in groups:
                total_links = sum(len(f.links) for f in group)
                if total_links > 0:
                    # Determine dominant domain
                    domains = {}
                    for f in group:
                        for l in f.links:
                            d = self._extract_domain(l)
                            domains[d] = domains.get(d, 0) + 1
                    
                    if domains:
                         main_domain = max(domains, key=domains.get)
                    else:
                         main_domain = "other"
                         
                    if main_domain not in smart_pools: smart_pools[main_domain] = []
                    smart_pools[main_domain].extend(group)
                else:
                    standard_groups.append(group)
            
            # Process Standard Groups
            for group in standard_groups:
                assembled_files = self.assemble_group(group)
                results.extend(assembled_files)
            
            # Process Smart Pools
            for domain, pool in smart_pools.items():
                print(f"DEBUG: Processing Smart Pool for {domain}: {len(pool)} fragments")
                if not pool: continue
                
                clusterer = FragmentClusterer()
                
                # Deduplicate pool by offset
                unique_pool = sorted(list({f.offset: f for f in pool}.values()), key=lambda x: x.offset)
                
                for f in unique_pool:
                     clusterer.add_fragment(f.offset, f.data, list(f.links))
                
                # Tune clusterer based on domain/density? (Optional future step)
                
                rust_clusters = clusterer.cluster_fragments()
                print(f"DEBUG: {domain} -> {len(rust_clusters)} clusters")
                
                for cluster_indices in rust_clusters:
                    cluster_frags = [unique_pool[idx] for idx in cluster_indices]
                    
                    # Sort and Deduplicate again (paranoia)
                    cluster_frags = sorted(list({f.offset: f for f in cluster_frags}.values()), key=lambda x: x.offset)
                    
                    # Assemble with ignore_gaps=True (gap-aware splitting inside)
                    assembled_files = self.assemble_group(cluster_frags, ignore_gaps=True)
                    results.extend(assembled_files)
                        
        except ImportError as e:
            print(f"DEBUG: Rust Import Error: {e}")
            logger.warning(f"Rust FragmentClusterer not available: {e}. Using fallback.")
            
            # Fallback: strict per-group assembly ONLY (No "Super Group")
            for group in groups:
                 assembled_files = self.assemble_group(group, ignore_gaps=False) 
                 results.extend(assembled_files)

        print(f"DEBUG: Total results: {len(results)}")
        return results

# --- UNIT TESTS ---
def test_assembler():
    fa = FragmentAssembler()
    f1 = Fragment(offset=1000, size=50, data=b'{"title": "Test", "links": [', links={"v1"})
    f2 = Fragment(offset=2000, size=50, data=b'"https://youtube.com/watch?v=v1"]}', links={"v1"})
    
    assembled = fa.assemble_group([f1, f2])
    assert assembled is not None
    assert assembled.is_valid is True
    assert b"Test" in assembled.content
    assert assembled.confidence > 70

if __name__ == "__main__":
    test_assembler()
    print("FragmentAssembler tests passed!")
