"""
Index Generator - создает удобный индекс всех восстановленных файлов
"""

from pathlib import Path
from datetime import datetime
import json
from typing import List, Dict

class IndexGenerator:
    """Генератор индексного файла для навигации"""
    
    def __init__(self, output_dir: Path):
        self.output_dir = output_dir
        
    def generate_html_index(self, recovered_files: List[Dict]) -> str:
        """Создать HTML индекс всех файлов"""
        
        # Группируем по типам
        by_type = {}
        for file_info in recovered_files:
            file_type = file_info.get('type', 'other')
            if file_type not in by_type:
                by_type[file_type] = []
            by_type[file_type].append(file_info)
            
        html = f"""<!DOCTYPE html>
<html lang="ru">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Индекс восстановленных файлов</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            padding: 20px;
            min-height: 100vh;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            border-radius: 20px;
            padding: 40px;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3);
        }}
        h1 {{
            color: #667eea;
            margin-bottom: 10px;
            font-size: 2.5em;
        }}
        .subtitle {{
            color: #666;
            margin-bottom: 30px;
            font-size: 1.1em;
        }}
        .stats {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            margin-bottom: 40px;
        }}
        .stat-card {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 20px;
            border-radius: 15px;
            text-align: center;
        }}
        .stat-number {{
            font-size: 3em;
            font-weight: bold;
            margin-bottom: 5px;
        }}
        .stat-label {{
            font-size: 0.9em;
            opacity: 0.9;
        }}
        .section {{
            margin-bottom: 40px;
        }}
        .section-title {{
            color: #667eea;
            font-size: 1.8em;
            margin-bottom: 20px;
            padding-bottom: 10px;
            border-bottom: 3px solid #667eea;
        }}
        .file-grid {{
            display: grid;
            gap: 15px;
        }}
        .file-card {{
            background: #f8f9fa;
            border-left: 4px solid #667eea;
            padding: 20px;
            border-radius: 10px;
            transition: all 0.3s;
        }}
        .file-card:hover {{
            transform: translateX(5px);
            box-shadow: 0 5px 15px rgba(102, 126, 234, 0.3);
        }}
        .file-name {{
            font-weight: bold;
            color: #333;
            margin-bottom: 10px;
            font-size: 1.1em;
        }}
        .file-details {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
            gap: 10px;
            margin-top: 10px;
        }}
        .detail {{
            font-size: 0.9em;
            color: #666;
        }}
        .detail-label {{
            font-weight: bold;
            color: #667eea;
        }}
        .badge {{
            display: inline-block;
            padding: 5px 15px;
            border-radius: 20px;
            font-size: 0.85em;
            font-weight: bold;
            margin-right: 10px;
        }}
        .badge-json {{ background: #28a745; color: white; }}
        .badge-csv {{ background: #17a2b8; color: white; }}
        .badge-txt {{ background: #ffc107; color: #333; }}
        .badge-html {{ background: #dc3545; color: white; }}
        .badge-assembled {{ background: #6f42c1; color: white; }}
        .quality-bar {{
            height: 8px;
            background: #e9ecef;
            border-radius: 4px;
            overflow: hidden;
            margin-top: 5px;
        }}
        .quality-fill {{
            height: 100%;
            background: linear-gradient(90deg, #28a745 0%, #ffc107 50%, #dc3545 100%);
            transition: width 0.3s;
        }}
        .links-preview {{
            background: #fff;
            border: 1px solid #dee2e6;
            border-radius: 5px;
            padding: 10px;
            margin-top: 10px;
            max-height: 100px;
            overflow-y: auto;
            font-size: 0.85em;
            color: #495057;
        }}
        .footer {{
            text-align: center;
            margin-top: 40px;
            padding-top: 20px;
            border-top: 2px solid #e9ecef;
            color: #666;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🎯 Индекс восстановленных файлов</h1>
        <div class="subtitle">Сессия: {datetime.now().strftime("%Y-%m-%d %H:%M:%S")}</div>
        
        <div class="stats">
            <div class="stat-card">
                <div class="stat-number">{len(recovered_files)}</div>
                <div class="stat-label">Всего файлов</div>
            </div>
            <div class="stat-card">
                <div class="stat-number">{len(by_type)}</div>
                <div class="stat-label">Типов файлов</div>
            </div>
            <div class="stat-card">
                <div class="stat-number">{sum(f.get('links_count', 0) for f in recovered_files)}</div>
                <div class="stat-label">YouTube ссылок</div>
            </div>
        </div>
"""
        
        # Добавляем секции по типам
        type_badges = {
            'json': 'badge-json',
            'csv': 'badge-csv',
            'txt': 'badge-txt',
            'html': 'badge-html',
            'assembled': 'badge-assembled'
        }
        
        type_titles = {
            'json': '📄 JSON Файлы',
            'csv': '📊 CSV Таблицы',
            'txt': '📝 Текстовые файлы',
            'html': '🌐 HTML Страницы',
            'assembled': '🧩 Собранные из фрагментов'
        }
        
        for file_type, files in sorted(by_type.items()):
            html += f"""
        <div class="section">
            <div class="section-title">{type_titles.get(file_type, f'📁 {file_type.upper()}')}</div>
            <div class="file-grid">
"""
            for file_info in files:
                file_name = file_info.get('filename', 'unknown')
                size_kb = file_info.get('size_kb', 0)
                quality = file_info.get('quality', 0)
                links_count = file_info.get('links_count', 0)
                offset = file_info.get('offset', 0)
                sha256 = file_info.get('sha256', '')[:16] + '...'
                
                html += f"""
                <div class="file-card">
                    <div class="file-name">
                        <span class="badge {type_badges.get(file_type, '')}">{file_type.upper()}</span>
                        {file_name}
                    </div>
                    <div class="file-details">
                        <div class="detail">
                            <span class="detail-label">Размер:</span> {size_kb} KB
                        </div>
                        <div class="detail">
                            <span class="detail-label">Ссылок:</span> {links_count}
                        </div>
                        <div class="detail">
                            <span class="detail-label">Офсет:</span> 0x{offset:X}
                        </div>
                        <div class="detail">
                            <span class="detail-label">SHA256:</span> {sha256}
                        </div>
                    </div>
                    <div class="detail">
                        <span class="detail-label">Качество:</span> {quality}/100
                        <div class="quality-bar">
                            <div class="quality-fill" style="width: {quality}%"></div>
                        </div>
                    </div>
                </div>
"""
            
            html += """
            </div>
        </div>
"""
        
        html += f"""
        <div class="footer">
            <p>🚀 Ultimate File Recovery v9.0</p>
            <p>Создано: {datetime.now().strftime("%Y-%m-%d %H:%M:%S")}</p>
        </div>
    </div>
</body>
</html>
"""
        
        return html
        
    def save_index(self, recovered_files: List[Dict]):
        """Сохранить HTML индекс"""
        html = self.generate_html_index(recovered_files)
        index_path = self.output_dir / 'INDEX.html'
        index_path.write_text(html, encoding='utf-8')
        return index_path
