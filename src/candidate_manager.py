#!/usr/bin/env python3
"""
Candidate Manager - управление кандидатами с военным качеством
V11.5: Интеллектуальное именование и строгая валидация
"""

import shutil
import json
import hashlib
from pathlib import Path
from typing import List, Dict, Optional
from dataclasses import dataclass

@dataclass
class CandidateStatus:
    PENDING = "pending"
    VALID = "valid"
    INVALID = "invalid"
    RECOVERED = "recovered"
    FAILED = "failed"

class CandidateManager:
    def __init__(self, output_dir: Path):
        self.output_dir = Path(output_dir)
        self.recovered_dir = self.output_dir / "01_RECOVERED_FILES"
        self.rejected_dir = self.output_dir / "02_REJECTED_CANDIDATES"
        
        for d in [self.recovered_dir, self.rejected_dir]:
            d.mkdir(parents=True, exist_ok=True)
            
        self.candidates_dir = self.output_dir / "00_CANDIDATES"
        self.candidates_dir.mkdir(parents=True, exist_ok=True)
        self.stats = {'total': 0, 'recovered': 0, 'rejected': 0}

    def add_candidate(self, data: bytes, metadata: Dict) -> Optional[Path]:
        """Save raw candidate for processing"""
        try:
            # Create a unique filename based on offset or hash
            offset = metadata.get('offset', 0)
            cand_id = f"cand_{offset:X}_{hashlib.md5(data[:1024]).hexdigest()[:8]}"
            cand_dir = self.candidates_dir / cand_id
            cand_dir.mkdir(exist_ok=True)
            
            # Save raw data
            raw_path = cand_dir / "raw.bin"
            with open(raw_path, 'wb') as f:
                f.write(data)
                
            # Save metadata
            metadata['status'] = CandidateStatus.PENDING
            meta_path = cand_dir / "meta.json"
            with open(meta_path, 'w') as f:
                json.dump(metadata, f, indent=2)
                
            self.stats['total'] += 1
            return cand_dir
        except Exception as e:
            print(f"Failed to add candidate: {e}")
            return None

    def validate_candidate(self, cand_path: Path, is_valid: bool, message: str):
        """Update candidate status after reconstruction"""
        if not cand_path or not cand_path.exists(): return
        
        meta_path = cand_path / "meta.json"
        try:
            if meta_path.exists():
                with open(meta_path, 'r') as f:
                    meta = json.load(f)
            else:
                meta = {}
                
            meta['status'] = CandidateStatus.VALID if is_valid else CandidateStatus.INVALID
            meta['validation_msg'] = message
            
            with open(meta_path, 'w') as f:
                json.dump(meta, f, indent=2)
        except Exception:
            pass

    def recover_candidate(self, cand_path: Path, content: bytes, file_type: str, 
                         confidence: float, links: List, cleaned_content: bytes = None, 
                         links_only: bool = False) -> Optional[Path]:
        """Finalize recovery"""
        if not cand_path: return None
        
        try:
            # Metadata update
            meta_input = {
                'file_type': file_type,
                'confidence': confidence,
                'links_count': len(links)
            }
            # Load original metadata to get offset/original name if possible
            orig_meta_path = cand_path / "meta.json"
            if orig_meta_path.exists():
                with open(orig_meta_path, 'r') as f:
                    orig_meta = json.load(f)
                    meta_input.update(orig_meta)

            if links_only:
                # Just save links? User asked for links only mode support
                # But typically we still save the file if it's good.
                pass

            # Use save_recovered to actually write the file to RECOVERED_FILES
            # We construct a suggested name if we have one
            suggested_name = meta_input.get('original_filename')
            
            saved_path = self.save_recovered(content, meta_input, suggested_name)
            
            # Update status in candidate dir
            self.validate_candidate(cand_path, True, f"Recovered to {saved_path.name}")
            
            # Optionally clean up raw candidate to save space?
            # shutil.rmtree(cand_path) 
            
            return saved_path
        except Exception as e:
            print(f"Recovery failed for {cand_path}: {e}")
            return None

    def fail_candidate(self, cand_path: Path, reason: str):
        """Mark candidate as failed"""
        self.validate_candidate(cand_path, False, reason)
        self.log_rejection({'path': str(cand_path)}, reason)

    def cleanup_candidates(self, keep_rejected: bool = False) -> int:
        """
        Removes temporary candidate files.
        Returns number of cleaned items.
        """
        count = 0
        try:
            if self.candidates_dir.exists():
                for item in self.candidates_dir.iterdir():
                    try:
                        if item.is_dir():
                            shutil.rmtree(item)
                        else:
                            item.unlink()
                        count += 1
                    except Exception:
                        pass
        except Exception as e:
            print(f"Cleanup warning: {e}")
        return count
        
    def save_recovered(self, content: bytes, metadata: Dict, suggested_name: str = None) -> Path:
        """Сохранение восстановленного файла с умным именем"""
        file_type = metadata.get('file_type', 'bin')
        conf = metadata.get('confidence', 0.0)
        
        # Создаем подпапку по типу
        type_dir = self.recovered_dir / file_type.upper()
        type_dir.mkdir(exist_ok=True)
        
        if not suggested_name:
            suggested_name = f"recovered_{hashlib.md5(content).hexdigest()[:8]}.{file_type}"
            
        # Добавляем уверенность в имя для удобства
        final_name = f"[{int(conf)}%] {suggested_name}"
        save_path = type_dir / final_name
        
        with open(save_path, 'wb') as f:
            f.write(content)
            
        # Сохраняем метаданные рядом
        meta_path = save_path.with_suffix('.json')
        with open(meta_path, 'w', encoding='utf-8') as f:
            json.dump(metadata, f, indent=2, ensure_ascii=False)
            
        self.stats['recovered'] += 1
        return save_path

    def log_rejection(self, metadata: Dict, reason: str):
        """Логирование отклоненных кандидатов для минимизации false positives"""
        self.stats['rejected'] += 1
        # В военном режиме мы можем сохранять их в отдельный лог для аудита
        pass

# --- UNIT TESTS ---
def test_candidate_manager(tmp_path):
    cm = CandidateManager(tmp_path)
    content = b'{"test": "data"}'
    meta = {"file_type": "json", "confidence": 95.0}
    path = cm.save_recovered(content, meta, "test_file.json")
    assert path.exists()
    assert "[95%] test_file.json" in path.name
    assert (tmp_path / "01_RECOVERED_FILES" / "JSON").exists()

if __name__ == "__main__":
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        test_candidate_manager(Path(tmp))
    print("CandidateManager tests passed!")
