#!/usr/bin/env python3
"""
Cluster Analyzer - находит участки диска с концентрацией данных
"""

from typing import List, Dict, Tuple
from dataclasses import dataclass
import re

@dataclass
class Cluster:
    """Кластер данных на диске"""
    start_offset: int
    end_offset: int
    density: float  # Плотность ссылок (ссылок на KB)
    link_count: int
    links: List[str]
    
    @property
    def size(self) -> int:
        return self.end_offset - self.start_offset
    
    @property
    def center(self) -> int:
        return (self.start_offset + self.end_offset) // 2


class ClusterAnalyzer:
    """Анализатор кластеров данных"""
    
    def __init__(self, window_size: int = 1):  # 1 byte window to prevent merging
        self.window_size = window_size
        self.min_density = 0.5  # Минимум 0.5 ссылок на KB
        
    def find_clusters(self, candidates: List[Dict]) -> List[Cluster]:
        """Находит кластеры из кандидатов"""
        
        if not candidates:
            return []
        
        # Сортируем по offset
        sorted_cands = sorted(candidates, key=lambda x: x.get('offset', 0))
        
        clusters = []
        current_cluster_cands = []
        
        for i, cand in enumerate(sorted_cands):
            offset = cand.get('offset', 0)
            
            if not current_cluster_cands:
                current_cluster_cands.append(cand)
                continue
            
            # Проверяем расстояние до предыдущего
            prev_offset = current_cluster_cands[-1].get('offset', 0)
            distance = offset - prev_offset
            
            if distance <= self.window_size:
                # Добавляем в текущий кластер
                current_cluster_cands.append(cand)
            else:
                # Создаем кластер из кандидатов
                cluster = self._create_cluster(current_cluster_cands)
                if cluster:
                    clusters.append(cluster)
                
                # Начинаем новый кластер
                current_cluster_cands = [cand]
        
        # Обрабатываем последний кластер
        cluster = self._create_cluster(current_cluster_cands)
        if cluster:
            clusters.append(cluster)
        
        return clusters
    
    def _create_cluster(self, candidates: List[Dict]) -> Cluster:
        """Создает кластер из кандидатов"""
        
        if not candidates:
            return None
        
        # Определяем границы
        # Определяем границы
        start_offset = min(c.get('offset', 0) for c in candidates)
        # Use fixed 4KB granularity to avoid massive artificial clusters from speculative sizes
        end_offset = max(c.get('offset', 0) + 256 for c in candidates)

        
        # Собираем ссылки
        links = []
        for cand in candidates:
            # Вариант 1: Ссылка уже в метаданных (от recover.py)
            if 'url' in cand or 'video_id' in cand:
                if cand.get('url'): links.append(cand['url'])
                if cand.get('video_id'): links.append(cand['video_id'])
                continue

            # Вариант 2: Извлекаем из данных (если они есть)
            data = cand.get('data', b'')
            if isinstance(data, bytes) and data:
                # Use replace to avoid losing data completely
                text = data.decode('utf-8', errors='replace')
            
                # Ищем YouTube ссылки (с границами слов)
                youtube_urls = re.findall(
                    r'(?:\bhttps?://)?(?:\bwww\.)?(?:youtube\.com/watch\?v=|youtu\.be/)([a-zA-Z0-9_-]{11})(?![a-zA-Z0-9_-])',
                    text
                )
                links.extend(youtube_urls)
        
        # Удаляем дубликаты
        links = list(set(links))
        
        # Вычисляем плотность по SPAN (расстоянию между крайними ссылками),
        # а не по размеру предполагаемого файла, чтобы не разбавлять плотность
        span_kb = (end_offset - start_offset) / 1024
        
        # Если span очень маленький (одна точка), берем минимальный размер (например 4KB)
        # иначе плотность улетает в бесконечность
        calc_size_kb = max(span_kb, 4.0) 
        
        if calc_size_kb <= 0:
             density = 0.0
        else:
             density = len(links) / calc_size_kb

        
        # Проверяем минимальную плотность
        if density < self.min_density:
            return None
        
        return Cluster(
            start_offset=start_offset,
            end_offset=end_offset,
            density=density,
            link_count=len(links),
            links=links
        )
    
    def merge_overlapping_clusters(self, clusters: List[Cluster]) -> List[Cluster]:
        """Объединяет перекрывающиеся кластеры"""
        
        if not clusters:
            return []
        
        # Сортируем по start_offset
        sorted_clusters = sorted(clusters, key=lambda c: c.start_offset)
        
        merged = []
        current = sorted_clusters[0]
        
        for next_cluster in sorted_clusters[1:]:
            # Проверяем перекрытие
            if next_cluster.start_offset <= current.end_offset:
                # Объединяем
                current = Cluster(
                    start_offset=current.start_offset,
                    end_offset=max(current.end_offset, next_cluster.end_offset),
                    density=(current.density + next_cluster.density) / 2,
                    link_count=current.link_count + next_cluster.link_count,
                    links=list(set(current.links + next_cluster.links))
                )
            else:
                merged.append(current)
                current = next_cluster
        
        merged.append(current)
        return merged
    
    def rank_clusters(self, clusters: List[Cluster]) -> List[Cluster]:
        """Ранжирует кластеры по качеству"""
        
        def cluster_score(cluster: Cluster) -> float:
            # Оценка = плотность * количество ссылок * log(размер)
            import math
            size_factor = math.log(cluster.size / 1024 + 1)
            return cluster.density * cluster.link_count * size_factor
        
        return sorted(clusters, key=cluster_score, reverse=True)


if __name__ == "__main__":
    # Тест
    analyzer = ClusterAnalyzer()
    
    test_candidates = [
        {'offset': 1000, 'size': 1024, 'data': b'youtube.com/watch?v=abc123'},
        {'offset': 2000, 'size': 1024, 'data': b'youtube.com/watch?v=def456'},
        {'offset': 3000, 'size': 1024, 'data': b'youtube.com/watch?v=ghi789'},
        {'offset': 100000, 'size': 1024, 'data': b'youtube.com/watch?v=jkl012'},
    ]
    
    clusters = analyzer.find_clusters(test_candidates)
    print(f"Found {len(clusters)} clusters:")
    for i, cluster in enumerate(clusters, 1):
        print(f"  {i}. Offset: 0x{cluster.start_offset:X}-0x{cluster.end_offset:X}, "
              f"Size: {cluster.size // 1024} KB, Links: {cluster.link_count}, "
              f"Density: {cluster.density:.2f}")
