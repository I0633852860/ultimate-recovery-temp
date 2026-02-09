#!/usr/bin/env python3
"""
Live Dashboard Module for Ultimate File Recovery v9.5
Professional minimal dashboard - ddrescue/smartctl style
No emoji, no fancy borders, strict monochrome ASCII
"""

from rich.text import Text
from collections import deque
import time


class LiveDashboard:
    """Professional minimal dashboard - ddrescue/smartctl style (Linear Block Map)"""

    def __init__(self, total_size: int, image_path: str, output_dir: str):
        self.total_size = total_size
        self.image_path = image_path
        self.output_dir = output_dir

        # State
        self.current_position = 0
        self.bytes_scanned = 0
        self.start_time = time.time()
        self.fragments_found = 0
        self.recovered_files = 0
        self.is_reverse = False
        self.paused = False

        # Statistics
        self.speed_history = deque(maxlen=60)
        self.activity_log = deque(maxlen=5)

        # Disk map (Linear 200 blocks)
        self.disk_map_width = 50
        self.disk_map_rows = 4
        self.total_blocks = self.disk_map_width * self.disk_map_rows
        # 0: Unscanned, 1: Scanned, 2: Found Data, 3: Hot/Recent
        self.disk_map = [0] * self.total_blocks

        # Top candidate
        self.top_candidate = None
        self.hot_clusters = 0

        # Target files (for early-exit display)
        self.target_files = 0

        # Last speed calculation
        self._last_speed_time = time.time()
        self._last_speed_bytes = 0

    def update_position(self, position: int):
        """Update scan position and calculate speed"""
        self.current_position = position
        self.bytes_scanned = position

        # Mark blocks as scanned (1)
        if self.total_size > 0:
            pct = position / self.total_size
            blocks_scanned = int(pct * self.total_blocks)
            # Fill up to current position
            if self.is_reverse:
                # Fill from end back
                 start_idx = self.total_blocks - blocks_scanned
                 for i in range(start_idx, self.total_blocks):
                     if self.disk_map[i] == 0: self.disk_map[i] = 1
            else:
                # Fill from start forward
                 for i in range(blocks_scanned):
                     if self.disk_map[i] == 0: self.disk_map[i] = 1

        now = time.time()
        dt = now - self._last_speed_time
        if dt >= 1.0:
            db = abs(position - self._last_speed_bytes)
            current_speed = (db / 1024 / 1024) / dt if dt > 0 else 0
            self.speed_history.append(current_speed)
            self._last_speed_time = now
            self._last_speed_bytes = position

    def update_disk_map(self, offset: int, intensity: int):
        """Update disk activity map with FOUND DATA (2)"""
        if self.total_size == 0: return

        # Calculate block index
        block_idx = int((offset / self.total_size) * self.total_blocks)
        if 0 <= block_idx < self.total_blocks:
            # Mark as found data (2) or hot (3)
            # Intensity logic? Just mark as found.
            self.disk_map[block_idx] = 2

    def add_log(self, message: str):
        """Add log entry (no emoji, just timestamp + message)"""
        timestamp = time.strftime("%H:%M:%S")
        self.activity_log.append(f"  {timestamp}  {message}")

    def render(self) -> Text:
        """Render complete dashboard as plain text with colors"""
        output = Text()

        # Title line
        img_name = self.image_path.split("/")[-1] if "/" in self.image_path else self.image_path
        output.append(
            f"Ultimate Recovery v11.5 - {img_name} -> {self.output_dir}/\n",
            style="bold white",
        )
        output.append("═" * 64 + "\n\n")

        # --- SCAN PROGRESS ---
        output.append("[SCAN PROGRESS]", style="bold")
        if self.paused:
            output.append("  ** PAUSED **", style="bold cyan")
        output.append("\n")

        pos_hex = f"0x{self.current_position:X}"
        total_hex = f"0x{self.total_size:X}"
        mode = "(reverse mode)" if self.is_reverse else ""
        output.append(f"  Position:  {pos_hex} / {total_hex}  {mode}\n")

        # Progress bar
        progress_pct = (
            (self.bytes_scanned / self.total_size * 100) if self.total_size > 0 else 0
        )
        bar_width = 40
        filled = int(bar_width * progress_pct / 100)
        
        # Unicode Bar
        bar_char = "█"
        empty_char = "░"
        bar = bar_char * filled + empty_char * (bar_width - filled)
        
        gb_done = self.bytes_scanned / 1024 / 1024 / 1024
        gb_total = self.total_size / 1024 / 1024 / 1024
        
        # Color based on progress?
        output.append(f"  Completed: [")
        output.append(bar, style="green")
        output.append(f"] {progress_pct:.1f}%  ({gb_done:.1f}/{gb_total:.1f} GB)\n")

        # Speed
        current_speed = self.speed_history[-1] if self.speed_history else 0
        avg_speed = (
            sum(self.speed_history) / len(self.speed_history)
            if self.speed_history
            else 0
        )
        output.append(
            f"  Speed:     current {current_speed:.1f} MB/s  |  average {avg_speed:.1f} MB/s\n"
        )

        # Time
        elapsed = time.time() - self.start_time
        # Simplified ETA calculation
        remaining_bytes = self.total_size - self.bytes_scanned
        eta = (
            remaining_bytes / (avg_speed * 1024 * 1024) if avg_speed > 0 else 0
        )
        output.append(
            f"  Time:      {self._fmt_time(elapsed)} elapsed    |  {self._fmt_time(eta)} remaining\n\n"
        )

        # --- DISK MAP ---
        output.append(
            "[DISK MAP - Linear Surface Scan]\n", style="bold"
        )
        # Legend
        output.append("  Legend: ")
        output.append("░ Unscanned  ", style="dim")
        output.append("▒ Scanned  ", style="cyan")
        output.append("█ Found Data\n", style="bold green")

        # Render Blocks
        block_chars = ["░", "▒", "█", "█"]
        styles = ["dim", "cyan", "bold green", "bold red"]
        
        for row in range(self.disk_map_rows):
            start_idx = row * self.disk_map_width
            end_idx = start_idx + self.disk_map_width
            output.append("  [")
            for i in range(start_idx, end_idx):
                val = self.disk_map[i]
                output.append(block_chars[val], style=styles[val])
            output.append("]\n")
        output.append("\n")

        # --- STATISTICS ---
        output.append("[STATISTICS]\n", style="bold")
        output.append(
            f"  Fragments:      {self.fragments_found:<10} Clusters:        {self.hot_clusters}\n"
        )
        if self.top_candidate:
            output.append(
                f"  Top candidate:  0x{self.top_candidate['offset']:X} "
                f"(confidence {self.top_candidate['score']:.1f}%)\n"
            )
        target_str = ""
        if self.target_files > 0:
            target_str = f"    (target: {self.target_files}, early-exit enabled)"
        output.append(f"  Recovered:      {self.recovered_files} files{target_str}\n")
        checkpoint_gb = self.bytes_scanned / 1024 / 1024 / 1024
        output.append(f"  Checkpoint:     auto-saved at {checkpoint_gb:.1f} GB\n\n")

        # --- LOG ---
        output.append("[LOG]\n", style="bold")
        for entry in self.activity_log:
            output.append(entry + "\n", style="dim")
        if not self.activity_log:
            output.append("  (no events yet)\n", style="dim")
        output.append("\n")

        # --- FOOTER ---
        output.append("═" * 64 + "\n")
        output.append(
            "Controls: [P]ause  [S]kip  [V]iew  [C]heckpoint  [Q]uit\n",
            style="dim",
        )

        return output

    def _fmt_time(self, seconds: float) -> str:
        """Format time as HH:MM:SS"""
        if seconds < 0:
            seconds = 0
        h = int(seconds // 3600)
        m = int((seconds % 3600) // 60)
        s = int(seconds % 60)
        return f"{h:02d}:{m:02d}:{s:02d}"
