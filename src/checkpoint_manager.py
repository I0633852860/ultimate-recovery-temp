#!/usr/bin/env python3
"""
Checkpoint Manager for Ultimate File Recovery v9.5
Reliable checkpoint/resume system (ddrescue-style)
Atomic saves, backup rotation, image validation
"""

import json
import os
import time
import hashlib
from pathlib import Path
from typing import Optional, Dict


class CheckpointManager:
    """
    Reliable checkpoint/resume system (like ddrescue).
    - Atomic writes (temp -> rename)
    - Backup rotation
    - Image hash validation (prevents resume on wrong image)
    - Auto-save interval
    """

    def __init__(self, output_dir: str, image_path: str):
        self.output_dir = Path(output_dir)
        self.image_path = image_path
        self.checkpoint_file = self.output_dir / "CHECKPOINT.json"
        self.backup_file = self.output_dir / "CHECKPOINT.bak"

        # Image signature for validation
        self.image_hash = self._quick_hash(image_path)

        # Auto-save tracking
        self._last_checkpoint_position = 0

    def _quick_hash(self, filepath: str) -> str:
        """Quick hash: first 1MB + file size for fast validation"""
        try:
            size = os.path.getsize(filepath)
            with open(filepath, "rb") as f:
                chunk = f.read(1024 * 1024)  # First 1MB
            raw = f"{size}:".encode() + chunk
            return hashlib.sha256(raw).hexdigest()[:16]
        except Exception:
            return "unknown"

    def save(self, position: int, state: Dict) -> bool:
        """
        Atomic checkpoint save.
        1. Write to .tmp
        2. fsync
        3. Backup old checkpoint
        4. Atomic rename
        Returns True if saved successfully.
        """
        checkpoint_data = {
            "version": "9.5",
            "timestamp": time.time(),
            "timestamp_human": time.strftime("%Y-%m-%d %H:%M:%S"),
            "image_path": self.image_path,
            "image_hash": self.image_hash,
            "position": position,
            "state": state,
        }

        try:
            # Ensure output dir exists
            self.output_dir.mkdir(parents=True, exist_ok=True)

            # Write to temp file first
            temp_file = self.checkpoint_file.with_suffix(".tmp")
            with open(temp_file, "w") as f:
                json.dump(checkpoint_data, f, indent=2)
                f.flush()
                os.fsync(f.fileno())  # Force write to disk

            # Backup old checkpoint
            if self.checkpoint_file.exists():
                try:
                    # os.replace is atomic on POSIX and Win32 (replaces if target exists)
                    os.replace(str(self.checkpoint_file), str(self.backup_file))
                except Exception:
                    pass

            # Atomic rename/replace
            os.replace(str(temp_file), str(self.checkpoint_file))

            return True

        except Exception as e:
            # Silently fail — checkpoint is non-critical
            return False

    def load(self) -> Optional[Dict]:
        """
        Load checkpoint with validation.
        Returns None if no valid checkpoint exists.
        Validates image hash to prevent resume on wrong image.
        """
        if not self.checkpoint_file.exists():
            return None

        try:
            with open(self.checkpoint_file, "r") as f:
                data = json.load(f)

            # Validate image hash
            if data.get("image_hash") != self.image_hash:
                return None

            # Validate image path matches
            if data.get("image_path") != self.image_path:
                return None

            # Validate position is reasonable
            position = data.get("position", 0)
            try:
                image_size = os.path.getsize(self.image_path)
            except OSError:
                image_size = float('inf') # If image is gone/unreadable, we can't strict validate size here, rely on read error later

            if position < 0 or (image_size != float('inf') and position > image_size):
                return None

            return data

        except (json.JSONDecodeError, KeyError, TypeError):
            # Try backup
            if self.backup_file.exists():
                try:
                    with open(self.backup_file, "r") as f:
                        data = json.load(f)
                    
                    # Validate backup too
                    if data.get("image_hash") != self.image_hash:
                        return None
                        
                    position = data.get("position", 0)
                    if position < 0:
                         return None
                         
                    return data
                except Exception:
                    pass

            return None

    def auto_save_interval(
        self, position: int, state: Dict, interval_bytes: int = 100 * 1024 * 1024
    ) -> bool:
        """
        Auto-save every N bytes (default 100MB).
        Returns True if a checkpoint was saved this call.
        """
        if position < 0:
            return False
            
        if position - self._last_checkpoint_position >= interval_bytes:
            success = self.save(position, state)
            if success:
                self._last_checkpoint_position = position
            return success
        return False

    def clear(self):
        """Remove checkpoint files (e.g., after successful completion)"""
        for f in [self.checkpoint_file, self.backup_file]:
            try:
                if f.exists():
                    f.unlink()
            except Exception:
                pass
