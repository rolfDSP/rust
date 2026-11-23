"""
analytics.py - a small Python "plugin" that is embedded into the Rust binary.

The Rust crate includes this file at *compile time* with `include_str!`, so the
final executable does not need this file on disk. At run time the source code
is handed to the embedded CPython interpreter via `PyModule::from_code`, which
turns it into a regular Python module object named `analytics`.

Everything defined here (constants, functions, classes) can then be accessed
from Rust with `getattr` / `call1` / `call_method1`.
"""

import re
import statistics
from collections import Counter

# A plain module-level constant. Rust reads it with `module.getattr("SCRIPT_VERSION")`.
SCRIPT_VERSION = "1.0.0"

_WORD_RE = re.compile(r"[a-zA-ZäöüÄÖÜß']+")


def word_frequencies(text, top_n=5):
    """Return the `top_n` most common words as a list of (word, count) tuples.

    Words are compared case-insensitively. Ties are ordered alphabetically so
    the result is deterministic (important for tests).
    """
    if top_n < 0:
        raise ValueError("top_n must not be negative")
    words = [w.lower().strip("'") for w in _WORD_RE.findall(text)]
    words = [w for w in words if w]
    counts = Counter(words)
    ordered = sorted(counts.items(), key=lambda item: (-item[1], item[0]))
    return ordered[:top_n]


def describe(numbers):
    """Return descriptive statistics of a sequence of numbers as a dict.

    Raises `statistics.StatisticsError` (a subclass of `ValueError`) when the
    sequence is empty. `stdev` is `None` when fewer than two values are given.
    """
    data = list(numbers)
    if not data:
        raise statistics.StatisticsError("describe() requires at least one value")
    return {
        "count": len(data),
        "mean": statistics.fmean(data),
        "median": statistics.median(data),
        "stdev": statistics.stdev(data) if len(data) >= 2 else None,
        "min": min(data),
        "max": max(data),
    }


def render_report(title, rows):
    """Render a simple fixed-width text table.

    `rows` is a list of (label, value) tuples; values are formatted with two
    decimal places.
    """
    if not rows:
        return f"{title}\n(no data)"
    width = max(len(title), *(len(label) for label, _ in rows))
    lines = [title, "=" * (width + 12)]
    for label, value in rows:
        lines.append(f"{label:<{width}} {value:>11.2f}")
    return "\n".join(lines)


class TemperatureLog:
    """A stateful Python object that Rust creates, keeps and mutates."""

    def __init__(self, location):
        self.location = location
        self._readings = []

    def add(self, celsius):
        if celsius < -273.15:
            raise ValueError(f"{celsius} °C is below absolute zero")
        self._readings.append(float(celsius))

    def average(self):
        if not self._readings:
            return None
        return statistics.fmean(self._readings)

    def to_fahrenheit(self):
        return [c * 9.0 / 5.0 + 32.0 for c in self._readings]

    def __len__(self):
        return len(self._readings)

    def __repr__(self):
        return f"TemperatureLog(location={self.location!r}, readings={len(self)})"
