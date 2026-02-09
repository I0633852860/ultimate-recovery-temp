#!/usr/bin/env python3
"""
Hotkey Controller for Ultimate File Recovery v9.5
Non-blocking keyboard input for live controls
Supports: [P]ause, [S]kip, [V]iew, [C]heckpoint, [Q]uit
"""

import threading
import sys
import select
import os


class HotkeyController:
    """
    Non-blocking keyboard input for hotkeys.
    Runs a daemon thread that listens for single-character keypresses.
    Works in both raw terminal and non-TTY environments (graceful fallback).
    """

    def __init__(self):
        self.paused = False
        self.skip_requested = False
        self.view_requested = False
        self.checkpoint_requested = False
        self.quit_requested = False

        self._running = False
        self._thread = None
        self._old_settings = None
        self._is_tty = False

    def start(self):
        """Start listening for hotkeys in a background thread"""
        # Only enable hotkeys if stdin is a real terminal
        try:
            self._is_tty = hasattr(sys.stdin, "fileno") and os.isatty(sys.stdin.fileno())
        except Exception:
            self._is_tty = False

        if not self._is_tty:
            return  # No TTY — hotkeys disabled silently

        self._running = True
        self._thread = threading.Thread(target=self._listen_loop, daemon=True)
        self._thread.start()

    def stop(self):
        """Stop listening and restore terminal settings"""
        self._running = False
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=1)
        self._restore_terminal()

    def _listen_loop(self):
        """Listen for keypresses in raw/cbreak mode"""
        try:
            import termios
            import tty
        except ImportError:
            return  # Not available on this platform

        try:
            self._old_settings = termios.tcgetattr(sys.stdin)
            tty.setcbreak(sys.stdin.fileno())
        except Exception:
            return

        try:
            while self._running:
                # Use select with timeout to avoid busy-waiting
                # Increased timeout to 0.5s to reduce CPU usage (Issue #23)
                rlist, _, _ = select.select([sys.stdin], [], [], 0.5)
                if rlist:
                    try:
                        char = sys.stdin.read(1)
                        if not char:  # EOF
                            break
                        char = char.lower()
                    except Exception:
                        break

                    if char == "p":
                        self.paused = not self.paused
                    elif char == "s":
                        self.skip_requested = True
                    elif char == "v":
                        self.view_requested = True
                    elif char == "c":
                        self.checkpoint_requested = True
                    elif char == "q":
                        self.quit_requested = True
                        self._running = False
        finally:
            self._restore_terminal()

    def _restore_terminal(self):
        """Restore original terminal settings"""
        if self._old_settings is not None:
            try:
                import termios

                termios.tcsetattr(sys.stdin, termios.TCSADRAIN, self._old_settings)
            except Exception:
                pass
            self._old_settings = None

    def reset_flags(self):
        """Reset one-shot flags after they have been handled"""
        self.skip_requested = False
        self.view_requested = False
        self.checkpoint_requested = False
