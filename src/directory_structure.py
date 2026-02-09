"""
Directory Structure Manager
Организует выходные файлы по понятным категориям
"""

from pathlib import Path
from datetime import datetime
import json

class DirectoryManager:
    """Управление структурой выходных директорий (V10.0 Updated)"""
    
    def __init__(self, base_output_dir: str):
        self.base_dir = Path(base_output_dir)
        self.session_id = datetime.now().strftime("%Y%m%d_%H%M%S")
        
        # Создаем структуру
        self.dirs = {
            # Восстановленные файлы по типам
            'json': self.base_dir / '01_RECOVERED_FILES' / 'JSON',
            'csv': self.base_dir / '01_RECOVERED_FILES' / 'CSV',
            'txt': self.base_dir / '01_RECOVERED_FILES' / 'TXT',
            'html': self.base_dir / '01_RECOVERED_FILES' / 'HTML',
            'originals': self.base_dir / '01_RECOVERED_FILES' / 'ORIGINALS',
            'full_original_exfat': self.base_dir / '00_FULL_ORIGINAL_EXFAT',
            'other': self.base_dir / '01_RECOVERED_FILES' / 'OTHER',
            
            # Собранные из фрагментов
            'assembled': self.base_dir / '02_ASSEMBLED_FROM_FRAGMENTS',
            
            # Извлеченные ссылки
            'links': self.base_dir / '03_EXTRACTED_LINKS',
            
            # Метаданные
            'metadata': self.base_dir / '04_METADATA',
            
            # Отчеты
            'reports': self.base_dir / '05_REPORTS',
            
            # Временные (кандидаты)
            'candidates_validated': self.base_dir / '06_TEMP_CANDIDATES' / 'validated',
            'candidates_rejected': self.base_dir / '06_TEMP_CANDIDATES' / 'rejected',
            'candidates_failed': self.base_dir / '06_TEMP_CANDIDATES' / 'failed',
        }
        
    def create_structure(self):
        """Создать все директории"""
        for dir_path in self.dirs.values():
            dir_path.mkdir(parents=True, exist_ok=True)
            
        # Создать README в каждой категории
        self._create_category_readmes()
        
        # Создать главный индекс
        self._create_main_index()
        
    def _create_category_readmes(self):
        """Создать README в каждой категории (V10.0 Update)"""
        
        readmes = {
            '00_FULL_ORIGINAL_EXFAT': """# 📦 Full Original exFAT Files

Files fully recovered from exFAT filesystem metadata.

## How it works:
The scanner reads exFAT directory entries (including deleted ones),
follows the FAT cluster chain (or contiguous allocation), and
extracts the complete original file with its original filename.

## What you get:
- **Original filenames** preserved from the filesystem
- **Full file content** — not chunks, but complete files
- **Matching `.meta.json`** with forensic metadata (offset, cluster, chain type, SHA256)

## When this folder is empty:
If no exFAT filesystem was detected in the image, or all cluster chains
were damaged, the system falls back to chunk-based recovery in `01_RECOVERED_FILES/`.
""",
            '01_RECOVERED_FILES': """# 📁 Recovered Files
 
Original files successfully recovered from disk.

## Text/Unknown Recovery (v10.0+):
For TXT and UNKNOWN files, the system now provides two versions:
- `_raw.bin`: The original binary data exactly as found on disk. Use this for forensic analysis.
- `_clean.txt`: A filtered, readable version of the text. Large empty blocks and binary noise have been removed.

## Subfolders:
- **ORIGINALS/** - Professional Forensic Recovery (Original names + Full FAT chains)
- **JSON/** - JSON files containing YouTube links
- **CSV/** - CSV data tables
- **TXT/** - Text files (Clean + Raw)
- **HTML/** - HTML pages
- **OTHER/** - Other formats

## File Naming:
`recovered_NNNN_<type>_<size>.<ext>`

## Metadata:
Every file has a matching `.json` file containing:
- SHA256 hash
- Disk offset
- Quality score (Confidence)
- **Extracted YouTube links** (Check this if the file is unreadable!)
""",
            
            '02_ASSEMBLED_FROM_FRAGMENTS': """# 🧩 Assembled from Fragments

Files reconstructed by merging multiple fragments found on disk.

## Naming Format:
`assembled_<group>_<fragments>frags_<size>.<ext>`

## Metadata:
- List of all fragment offsets used
- Assembly confidence score
- SHA256 of the final file
""",
            
            '03_EXTRACTED_LINKS': """# 🔗 Extracted Links

YouTube links extracted from data where full file reconstruction was not possible or in --links-only mode.

## Files:
- `all_links.txt`: Consolidated list of all unique YouTube IDs and URLs found in this session.
- `links_extracted_<offset>.json`: Specific links found at a particular disk location.
""",
            
            '04_METADATA': """# 📊 Metadata

Technical details about the recovery session.

## Key Files:
- `session_info.json`: Session parameters and results summary.
- `disk_map.json`: Visualization of data distribution on the disk.
- `clusters.json`: Detailed information about identified data clusters.
""",
            
            '05_REPORTS': """# 📄 Reports

Professional recovery reports.

## Main Report:
- `recovery_report_<timestamp>.html`: Open this in any web browser for a detailed analysis, charts, and file list.
""",
            
            '06_TEMP_CANDIDATES': """# 🔍 Temporary Candidates

Intermediate files currently being processed or validated.

**⚠️ This folder is automatically cleaned up after the session ends.**
"""
        }
        
        for dir_name, content in readmes.items():
            readme_path = self.base_dir / dir_name / 'README.md'
            readme_path.write_text(content, encoding='utf-8')
            
    def _create_main_index(self):
        """Создать главный индексный файл (V10.0 Updated)"""
        
        index_content = f"""# 🎯 Ultimate File Recovery V10.0 - Session Results
 
**Session ID**: {self.session_id}  
**Date**: {datetime.now().strftime("%Y-%m-%d %H:%M:%S")}

---

## 📂 Directory Structure

### 00_FULL_ORIGINAL_EXFAT/ 📦
**Complete files recovered from exFAT filesystem with original names**

These are full files extracted by following exFAT cluster chains.
Check `.meta.json` for forensic details.

### 01_RECOVERED_FILES/ 📁
**Recovered original files (Validated)**

Files are organized by type. For TXT/UNKNOWN, look for the `_clean.txt` version for readability.

### 02_ASSEMBLED_FROM_FRAGMENTS/ 🧩
**Reconstructed fragmented files**

Combined multi-part data into single files with integrity checks.

### 03_EXTRACTED_LINKS/ 🔗
**YouTube Link Repository**

Check `all_links.txt` for a complete list of unique links found across the entire disk.

### 04_METADATA/ 📊
**Technical Session Data**

Logs, disk maps, and performance statistics.

### 05_REPORTS/ 📄
**Professional Analysis Reports**

Open `recovery_report_*.html` in your browser for interactive charts and deep-dive analysis.

---

## 🚀 Quick Navigation

### Where are my files?
👉 `01_RECOVERED_FILES/` - Main storage for recovered data.

### Where is the summary report?
👉 `05_REPORTS/recovery_report_*.html` - Open in browser.

### I see unreadable symbols?
👉 Check the matching `.json` file for extracted links, or look for the `_clean.txt` version.

### Where is the full link list?
👉 `03_EXTRACTED_LINKS/all_links.txt`

---

## ℹ️ Support
Refer to the `README.md` in each folder for more details. 
**Good luck with your recovery! 🍀**
"""
        
        index_path = self.base_dir / 'INDEX.md'
        index_path.write_text(index_content, encoding='utf-8')
        
    def get_path(self, category: str) -> Path:
        """Получить путь к категории"""
        return self.dirs.get(category, self.base_dir)
        
    def save_session_info(self, info: dict):
        """Сохранить информацию о сессии"""
        info['session_id'] = self.session_id
        info['created_at'] = datetime.now().isoformat()
        
        session_file = self.dirs['metadata'] / 'session_info.json'
        session_file.write_text(json.dumps(info, indent=2, ensure_ascii=False), encoding='utf-8')
