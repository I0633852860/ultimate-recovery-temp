#!/usr/bin/env python3
"""
Professional Report Generator - создает детальные отчеты ВСЕГДА
"""

from pathlib import Path
from typing import List, Dict, Optional
from datetime import datetime
import json

class ProfessionalReportGenerator:
    """Генератор профессиональных отчетов"""
    
    def __init__(self, output_dir: Path):
        self.output_dir = Path(output_dir)
        self.reports_dir = self.output_dir / "reports"
        self.reports_dir.mkdir(parents=True, exist_ok=True)
    
    def generate_full_report(self, scan_results: Dict, clusters: List, 
                           recovered_files: List[Dict], failure_reasons: List[str] = None) -> Path:
        """
        Генерирует полный HTML отчет
        
        Отчет создается ВСЕГДА, даже если ничего не найдено
        """
        
        timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
        report_path = self.reports_dir / f"recovery_report_{timestamp}.html"
        
        # Определяем статус
        success = len(recovered_files) > 0
        
        html = self._generate_html(scan_results, clusters, recovered_files, failure_reasons, success)
        
        with open(report_path, 'w', encoding='utf-8') as f:
            f.write(html)
        
        # Также сохраняем JSON версию
        json_path = report_path.with_suffix('.json')
        json_data = {
            'timestamp': timestamp,
            'scan_results': scan_results,
            'clusters': [self._cluster_to_dict(c) for c in clusters] if clusters else [],
            'recovered_files': recovered_files,
            'failure_reasons': failure_reasons or [],
            'success': success
        }
        
        with open(json_path, 'w') as f:
            json.dump(json_data, f, indent=2)
        
        return report_path
    
    def _generate_html(self, scan_results: Dict, clusters: List, 
                      recovered_files: List[Dict], failure_reasons: List[str], success: bool) -> str:
        """Генерирует HTML отчет"""
        
        # Статус
        status_color = "green" if success else "red"
        status_text = "SUCCESS" if success else "NO FILES RECOVERED"
        status_icon = "✅" if success else "❌"
        
        html = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Ultimate File Recovery Report</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            padding: 20px;
            color: #333;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            border-radius: 20px;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3);
            overflow: hidden;
        }}
        .header {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 40px;
            text-align: center;
        }}
        .header h1 {{
            font-size: 2.5em;
            margin-bottom: 10px;
        }}
        .status {{
            display: inline-block;
            padding: 10px 30px;
            background: {status_color};
            color: white;
            border-radius: 50px;
            font-weight: bold;
            font-size: 1.2em;
            margin-top: 20px;
        }}
        .section {{
            padding: 30px 40px;
            border-bottom: 1px solid #eee;
        }}
        .section:last-child {{ border-bottom: none; }}
        .section h2 {{
            color: #667eea;
            margin-bottom: 20px;
            font-size: 1.8em;
            border-left: 5px solid #667eea;
            padding-left: 15px;
        }}
        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            margin: 20px 0;
        }}
        .stat-card {{
            background: linear-gradient(135deg, #f5f7fa 0%, #c3cfe2 100%);
            padding: 20px;
            border-radius: 15px;
            text-align: center;
            box-shadow: 0 5px 15px rgba(0,0,0,0.1);
        }}
        .stat-value {{
            font-size: 2.5em;
            font-weight: bold;
            color: #667eea;
            margin: 10px 0;
        }}
        .stat-label {{
            color: #666;
            font-size: 0.9em;
            text-transform: uppercase;
            letter-spacing: 1px;
        }}
        .file-list {{
            list-style: none;
        }}
        .file-item {{
            background: #f8f9fa;
            padding: 15px;
            margin: 10px 0;
            border-radius: 10px;
            border-left: 4px solid #667eea;
        }}
        .file-item:hover {{
            background: #e9ecef;
            transform: translateX(5px);
            transition: all 0.3s;
        }}
        .file-name {{
            font-weight: bold;
            color: #667eea;
            font-size: 1.1em;
        }}
        .file-meta {{
            color: #666;
            font-size: 0.9em;
            margin-top: 5px;
        }}
        .chart-container {{
            position: relative;
            height: 300px;
            margin: 20px 0;
        }}
        .failure-box {{
            background: #fff3cd;
            border-left: 4px solid #ffc107;
            padding: 20px;
            border-radius: 10px;
            margin: 20px 0;
        }}
        .failure-box h3 {{
            color: #856404;
            margin-bottom: 15px;
        }}
        .failure-box ul {{
            list-style-position: inside;
            color: #856404;
        }}
        .cluster-card {{
            background: #e7f3ff;
            padding: 15px;
            margin: 10px 0;
            border-radius: 10px;
            border-left: 4px solid #2196F3;
        }}
        .footer {{
            background: #f8f9fa;
            padding: 20px;
            text-align: center;
            color: #666;
            font-size: 0.9em;
        }}
        .badge {{
            display: inline-block;
            padding: 5px 15px;
            border-radius: 20px;
            font-size: 0.85em;
            font-weight: bold;
            margin: 0 5px;
        }}
        .badge-success {{ background: #28a745; color: white; }}
        .badge-warning {{ background: #ffc107; color: #333; }}
        .badge-danger {{ background: #dc3545; color: white; }}
        .badge-info {{ background: #17a2b8; color: white; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>{status_icon} Ultimate File Recovery Report</h1>
            <p>Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}</p>
            <div class="status">{status_text}</div>
        </div>
        
        <div class="section">
            <h2>📘 User Guide (How to read results)</h2>
            <div class="failure-box" style="background: #e7f3ff; border-left-color: #2196F3; color: #333;">
                <h3 style="color: #667eea;">🔍 Understanding Recovery Output</h3>
                <p>The system recovers data from raw disk sectors. Depending on the file type, you will see different outputs:</p>
                <ul style="margin-left: 20px; margin-top: 10px; color: #444;">
                    <li><strong>TXT/UNKNOWN</strong>: These are raw fragments. 
                        <ul>
                            <li><code>_raw.bin</code>: The exact bytes from the disk (may contain binary noise/nulls).</li>
                            <li><code>_clean.txt</code>: A readable, filtered version with most noise removed.</li>
                        </ul>
                    </li>
                    <li><strong>JSON/CSV/HTML</strong>: Validated files with preserved or repaired structure.</li>
                    <li><strong>.json (metadata)</strong>: Every file has a matching JSON file containing offsets and <strong>extracted YouTube links</strong>.</li>
                </ul>
                <p style="margin-top: 10px;"><strong>Always check the matching <code>.json</code> file if a binary file looks unreadable!</strong></p>
            </div>
        </div>

        <div class="section">
            <h2>📊 Scan Statistics</h2>
            <div class="stats-grid">
                <div class="stat-card">
                    <div class="stat-label">Image Size</div>
                    <div class="stat-value">{scan_results.get('image_size_mb', 0)} MB</div>
                </div>
                <div class="stat-card">
                    <div class="stat-label">Bytes Scanned</div>
                    <div class="stat-value">{scan_results.get('bytes_scanned_mb', 0)} MB</div>
                </div>
                <div class="stat-card">
                    <div class="stat-label">Candidates Found</div>
                    <div class="stat-value">{scan_results.get('candidates_found', 0)}</div>
                </div>
                <div class="stat-card">
                    <div class="stat-label">Files Recovered</div>
                    <div class="stat-value">{len(recovered_files)}</div>
                </div>
                <div class="stat-card">
                    <div class="stat-label">Scan Time</div>
                    <div class="stat-value">{scan_results.get('scan_time_sec', 0):.1f}s</div>
                </div>
                <div class="stat-card">
                    <div class="stat-label">Avg Speed</div>
                    <div class="stat-value">{scan_results.get('avg_speed_mbps', 0):.1f} MB/s</div>
                </div>
            </div>
        </div>
"""
        
        # Кластеры
        if clusters:
            html += """
        <div class="section">
            <h2>🗺️ Data Clusters</h2>
            <p>Found concentrations of YouTube links in the following areas:</p>
"""
            for i, cluster in enumerate(clusters, 1):
                html += f"""
            <div class="cluster-card">
                <strong>Cluster #{i}</strong><br>
                Offset: 0x{cluster.start_offset:X} - 0x{cluster.end_offset:X}<br>
                Size: {cluster.size // 1024} KB<br>
                Links: {cluster.link_count}<br>
                Density: {cluster.density:.2f} links/KB
            </div>
"""
            html += "</div>"
        
        # Восстановленные файлы
        if recovered_files:
            html += """
        <div class="section">
            <h2>✅ Recovered Files</h2>
            <ul class="file-list">
"""
            for file_info in recovered_files:
                file_type = file_info.get('file_type', 'unknown')
                confidence = file_info.get('confidence', 0)
                links_count = len(file_info.get('links', []))
                
                badge_class = "badge-success" if confidence >= 80 else "badge-warning" if confidence >= 50 else "badge-danger"
                
                html += f"""
                <li class="file-item">
                    <div class="file-name">{file_info.get('filename', 'unknown')}</div>
                    <div class="file-meta">
                        <span class="badge {badge_class}">Confidence: {confidence:.0f}%</span>
                        <span class="badge badge-info">Type: {file_type.upper()}</span>
                        <span class="badge badge-info">Links: {links_count}</span>
                        <span class="badge badge-info">Size: {file_info.get('size_kb', 0)} KB</span>
                    </div>
                    <div class="file-meta" style="margin-top: 10px;">
                        SHA256: <code>{file_info.get('sha256', 'N/A')[:16]}...</code>
                    </div>
                </li>
"""
            html += """
            </ul>
        </div>
"""
        
        # Причины неудачи
        if not success and failure_reasons:
            html += """
        <div class="section">
            <div class="failure-box">
                <h3>❌ Why No Files Were Recovered</h3>
                <ul>
"""
            for reason in failure_reasons:
                html += f"                    <li>{reason}</li>\n"
            
            html += """
                </ul>
                <p style="margin-top: 15px; font-weight: bold;">
                    Recommendations:
                </p>
                <ul>
                    <li>Try different size ranges (--target-size-min/max)</li>
                    <li>Enable reverse scanning (--reverse)</li>
                    <li>Disable metadata scanning (--skip-metadata)</li>
                    <li>Check if the disk image is correct</li>
                </ul>
            </div>
        </div>
"""
        
        # График
        if recovered_files:
            html += """
        <div class="section">
            <h2>📈 Recovery Statistics</h2>
            <div class="chart-container">
                <canvas id="recoveryChart"></canvas>
            </div>
        </div>
        
        <script>
            const ctx = document.getElementById('recoveryChart').getContext('2d');
            new Chart(ctx, {
                type: 'bar',
                data: {
                    labels: ['Candidates', 'Validated', 'Recovered', 'Failed'],
                    datasets: [{
                        label: 'Count',
                        data: [""" + str(scan_results.get('candidates_found', 0)) + """, 
                               """ + str(len(recovered_files)) + """, 
                               """ + str(len(recovered_files)) + """, 
                               """ + str(scan_results.get('candidates_found', 0) - len(recovered_files)) + """],
                        backgroundColor: [
                            'rgba(54, 162, 235, 0.8)',
                            'rgba(255, 206, 86, 0.8)',
                            'rgba(75, 192, 192, 0.8)',
                            'rgba(255, 99, 132, 0.8)'
                        ],
                        borderColor: [
                            'rgba(54, 162, 235, 1)',
                            'rgba(255, 206, 86, 1)',
                            'rgba(75, 192, 192, 1)',
                            'rgba(255, 99, 132, 1)'
                        ],
                        borderWidth: 2
                    }]
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    scales: {
                        y: { beginAtZero: true }
                    }
                }
            });
        </script>
"""
        
        html += """
        <div class="footer">
            <p><strong>Ultimate File Recovery v10.0</strong> | Production Ready | AI-Powered</p>
            <p>Powered by Rust + Python | SIMD Optimized</p>
        </div>
    </div>
</body>
</html>
"""
        
        return html
    
    def _cluster_to_dict(self, cluster) -> Dict:
        """Конвертирует кластер в словарь"""
        return {
            'start_offset': cluster.start_offset,
            'end_offset': cluster.end_offset,
            'size': cluster.size,
            'density': cluster.density,
            'link_count': cluster.link_count,
            'links': cluster.links
        }


if __name__ == "__main__":
    # Тест
    generator = ProfessionalReportGenerator(Path("test_reports"))
    
    # Тест успешного восстановления
    scan_results = {
        'image_size_mb': 500,
        'bytes_scanned_mb': 500,
        'candidates_found': 7,
        'scan_time_sec': 42.5,
        'avg_speed_mbps': 11.7
    }
    
    recovered_files = [
        {
            'filename': 'recovered_0001.json',
            'file_type': 'json',
            'confidence': 95.0,
            'links': ['https://youtube.com/watch?v=abc123'],
            'size_kb': 15,
            'sha256': 'abc123def456'
        }
    ]
    
    report_path = generator.generate_full_report(scan_results, [], recovered_files)
    print(f"Success report: {report_path}")
    
    # Тест неудачного восстановления
    failure_reasons = [
        "No YouTube URL patterns found in scanned data",
        "All candidates were smaller than minimum size (10 KB)",
        "Disk image may be corrupted or encrypted"
    ]
    
    report_path = generator.generate_full_report(scan_results, [], [], failure_reasons)
    print(f"Failure report: {report_path}")
