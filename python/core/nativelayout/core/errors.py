"""Errors of the core (port of NativeLayoutError.kt)."""
from __future__ import annotations


class NativeLayoutError(Exception):
    def __init__(self, message: str, line: int, context: str,
                 word: str | None = None, suggestion: str | None = None):
        super().__init__(message)
        self.message = message
        self.line = line
        self.context = context
        self.word = word
        self.suggestion = suggestion

    def format(self) -> str:
        out = f"Error: line {self.line}\n{self.context}\n\n{self.message}"
        if self.suggestion is not None:
            out += f"\n\nSuggestion: {self.suggestion}"
        return out


def extract_line(source: str, line_number: int) -> str:
    lines = source.split("\n")
    i = line_number - 1
    return lines[i].strip() if 0 <= i < len(lines) else ""
