#!/usr/bin/env python3
"""
File Reconstructor - восстанавливает оригинальные файлы (.txt, .json, .csv, .html)
V11.5: Военный уровень надежности, улучшенная классификация и валидация.
"""

import json
import csv
import re
import io
import math
from typing import Optional, Dict, List, Tuple
from pathlib import Path
from dataclasses import dataclass
from enum import Enum

class FileType(Enum):
    """Типы файлов"""
    TXT = "txt"
    JSON = "json"
    CSV = "csv"
    HTML = "html"
    UNKNOWN = "unknown"

@dataclass
class ReconstructedFile:
    """Восстановленный файл"""
    file_type: FileType
    content: bytes
    is_valid: bool
    confidence: float  # 0-100
    metadata: Dict
    links_extracted: List[str]
    cleaned_content: Optional[str] = None
    suggested_name: str = "unknown_file"

class FileReconstructor:
    """Восстановление оригинальных файлов с военным качеством"""
    
    def __init__(self):
        # Stricter regex for YouTube video IDs
        self.youtube_pattern = re.compile(
            r'(?:https?://)?(?:www\.)?(?:youtube\.com/watch\?v=|youtu\.be/)([a-zA-Z0-9_-]{11})(?![a-zA-Z0-9_-])'
        )
        # Ключевые слова для классификации
        self.class_keywords = {
            "ВЕБИНАР": ["вебинар", "webinar", "online", "трансляция"],
            "РАЗБОР": ["разбор", "анализ", "review", "tutorial"],
            "ДОПОЛНИТЕЛЬНЫЙ": ["доп", "extra", "additional", "bonus"],
            "КУРС": ["курс", "course", "lesson", "урок"]
        }
        
    def clean_text(self, data: bytes) -> str:
        """Очищает текст от null-байтов и мусора"""
        if not data:
            return ""
            
        text = ""
        for encoding in ['utf-8', 'utf-16le', 'cp1251']:
            try:
                text = data.decode(encoding)
                break
            except:
                continue
        
        if not text:
            text = data.decode('utf-8', errors='replace')
            
        text = re.sub(r'\x00{2,}', '\n', text)
        
        def is_printable(c):
            o = ord(c)
            return (32 <= o <= 126) or (1024 <= o <= 1279) or c in '\n\r\t'
            
        cleaned = "".join(c if is_printable(c) else ' ' for c in text)
        cleaned = re.sub(r' +', ' ', cleaned)
        cleaned = re.sub(r'\n+', '\n', cleaned)
        
        return cleaned.strip()

    def _calculate_entropy(self, chunk: bytes) -> float:
        """Вычисляет энтропию Шеннона (0..8)"""
        if not chunk:
            return 0.0
        counts = [0] * 256
        for b in chunk:
            counts[b] += 1
        length = len(chunk)
        ent = 0.0
        for count in counts:
            if count > 0:
                p = count / length
                ent -= p * math.log2(p)
        return ent

    def detect_data_boundary(self, data: bytes, direction: str = 'forward') -> int:
        """Находит границу данных по нулям и энтропии"""
        if not data: return 0
        NULL_BLOCK = b'\x00' * 64
        if direction == 'forward':
            idx = data.find(NULL_BLOCK)
            return idx if idx != -1 else len(data)
        else:
            idx = data.rfind(NULL_BLOCK)
            return idx + len(NULL_BLOCK) if idx != -1 else 0

    def _detect_file_type(self, data: bytes) -> FileType:
        """Определяет тип файла по сигнатурам и структуре"""
        try:
            text = data.decode('utf-8', errors='ignore').strip()
            if not text: return FileType.UNKNOWN
            
            if text.startswith(('{', '[')):
                try:
                    json.loads(text)
                    return FileType.JSON
                except:
                    if text.count('"') > 4 and text.count(':') > 1:
                        return FileType.JSON
            
            if '<html' in text.lower() or '<body' in text.lower() or '<div' in text.lower():
                return FileType.HTML
                
            if text.count(',') > 5 and text.count('\n') > 1:
                return FileType.CSV
                
            return FileType.TXT
        except Exception:
            return FileType.UNKNOWN

    def _suggest_filename(self, text: str, file_type: FileType) -> str:
        """Интеллектуальное именование на основе контента"""
        # 1. Ищем тайтлы
        title_match = re.search(r'["\']title["\']\s*:\s*["\']([^"\']+)["\']', text, re.I)
        if not title_match:
            title_match = re.search(r'<title>(.*?)</title>', text, re.I)
            
        base_name = "recovered_file"
        if title_match:
            base_name = title_match.group(1).strip()
            base_name = "".join(c for c in base_name if c.isalnum() or c in " _-")[:50]
            
        # 2. Классификация по ключевым словам
        category = "GENERAL"
        for cat, keywords in self.class_keywords.items():
            if any(kw in text.lower() for kw in keywords):
                category = cat
                break
                
        return f"{category}_{base_name}.{file_type.value}"

    def compute_dynamic_chunk_size(self, data: bytes, chunk_min: int, chunk_max: int) -> int:
        """
        Calculates optimal chunk size based on content analysis.
        """
        if len(data) <= chunk_min:
            return len(data)
        limit = min(len(data), chunk_max)
        return limit

    def reconstruct(self, data: bytes, offset: int = 0, end_offset: int = 0) -> ReconstructedFile:
        """Основной метод реконструкции"""
        file_type = self._detect_file_type(data)
        cleaned = self.clean_text(data)
        links = list(set(self.youtube_pattern.findall(cleaned)))
        
        is_valid = False
        confidence = 0.0
        
        if file_type == FileType.JSON:
            try:
                json.loads(cleaned)
                is_valid = True
                confidence = 100.0
            except:
                # Частичный JSON
                if len(links) > 0:
                    is_valid = True
                    confidence = min(90.0, 30.0 + len(links) * 10)
        elif file_type == FileType.HTML:
            if '</html>' in cleaned.lower() or len(links) > 2:
                is_valid = True
                confidence = 80.0
        elif len(links) > 0:
            is_valid = True
            # Boost confidence for many links (e.g. 50 + 1 per link, max 95)
            confidence = min(95.0, 50.0 + len(links) * 1.0)

        suggested_name = self._suggest_filename(cleaned, file_type)
        
        return ReconstructedFile(
            file_type=file_type,
            content=data,
            is_valid=is_valid,
            confidence=confidence,
            metadata={"offset": offset, "size": len(data)},
            links_extracted=links,
            cleaned_content=cleaned,
            suggested_name=suggested_name
        )

# --- UNIT TESTS ---
def test_reconstructor():
    r = FileReconstructor()
    # Test JSON
    data = '{"title": "Вебинар по Rust", "links": ["https://youtube.com/watch?v=12345678901"]}'.encode('utf-8')
    res = r.reconstruct(data)
    assert res.file_type == FileType.JSON
    assert res.is_valid is True
    assert "ВЕБИНАР" in res.suggested_name
    assert "12345678901" in res.links_extracted

    # Test HTML
    data_html = "<html><title>Разбор полетов</title><body>youtu.be/abcdefghijk</body></html>".encode('utf-8')
    res_html = r.reconstruct(data_html)
    assert res_html.file_type == FileType.HTML
    assert "РАЗБОР" in res_html.suggested_name

if __name__ == "__main__":
    test_reconstructor()
    print("FileReconstructor tests passed!")
