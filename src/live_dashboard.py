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
    """Professional minimal dashboard - ddrescue/smartctl style"""

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

        # Disk map (4 rows x 50 cols)
        self.disk_map_width = 50
        self.disk_map_rows = 4
        self.disk_map = [[0] * self.disk_map_width for _ in range(self.disk_map_rows)]

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

        now = time.time()
        dt = now - self._last_speed_time
        if dt >= 1.0:
            db = position - self._last_speed_bytes
            current_speed = (db / 1024 / 1024) / dt if dt > 0 else 0
            self.speed_history.append(current_speed)
            self._last_speed_time = now
            self._last_speed_bytes = position

    def update_disk_map(self, offset: int, intensity: int):
        """Update disk activity map at the correct position"""
        if self.total_size == 0:
            return
        cell_size = self.total_size // self.disk_map_width
        if cell_size == 0:
            return
        col = min(self.disk_map_width - 1, offset // cell_size)
        # Distribute across rows based on intensity
        row = min(self.disk_map_rows - 1, intensity // 25)
        self.disk_map[row][col] = min(3, self.disk_map[row][col] + 1)

    def add_log(self, message: str):
        """Add log entry (no emoji, just timestamp + message)"""
        timestamp = time.strftime("%H:%M:%S")
        self.activity_log.append(f"  {timestamp}  {message}")

    def render(self) -> Text:
        """Render complete dashboard as plain text"""
        output = Text()

        # Title line
        img_name = self.image_path.split("/")[-1] if "/" in self.image_path else self.image_path
        output.append(
            f"Ultimate Recovery v11.5 - {img_name} -> {self.output_dir}/\n",
            style="bold white",
        )
        output.append("=" * 64 + "\n\n")

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
        if filled < bar_width:
            bar = "=" * filled + ">" + " " * (bar_width - filled - 1)
        else:
            bar = "=" * bar_width
        gb_done = self.bytes_scanned / 1024 / 1024 / 1024
        gb_total = self.total_size / 1024 / 1024 / 1024
        output.append(
            f"  Completed: [{bar}] {progress_pct:.1f}%  ({gb_done:.1f}/{gb_total:.1f} GB)\n"
        )

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
        remaining_bytes = self.total_size - self.bytes_scanned
        eta = (
            remaining_bytes / (avg_speed * 1024 * 1024) if avg_speed > 0 else 0
        )
        output.append(
            f"  Time:      {self._fmt_time(elapsed)} elapsed    |  {self._fmt_time(eta)} remaining\n\n"
        )

        # --- DISK MAP ---
        output.append(
            "[DISK MAP - Activity density, last 60 seconds]\n", style="bold"
        )
        map_chars = [" ", ".", "=", "#"]
        for row in range(self.disk_map_rows):
            output.append("  0%  [")
            for col in range(self.disk_map_width):
                val = self.disk_map[row][col]
                ch = map_chars[min(3, val)]
                if val >= 2:
                    output.append(ch, style="cyan")
                else:
                    output.append(ch, style="dim")
            output.append("] 100%\n")
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
        output.append("=" * 64 + "\n")
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
