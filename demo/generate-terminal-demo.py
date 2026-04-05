#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_PATH = REPO_ROOT / "demo" / "terminal-demo.gif"
WIDTH = 1280
HEIGHT = 720
BACKGROUND = "#0f1720"
PANEL = "#111827"
TEXT = "#e5e7eb"
MUTED = "#94a3b8"
ACCENT = "#22c55e"
PROMPT = "#38bdf8"


def main() -> None:
    build_cli()

    compact = run_cli(
        [
            "compact-view",
            "https://docs.aws.amazon.com/lambda/latest/dg/welcome.html",
            "--allow-domain",
            "docs.aws.amazon.com",
        ]
    )
    read_view = run_cli(
        [
            "read-view",
            "https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html",
            "--main-only",
            "--allow-domain",
            "docs.aws.amazon.com",
        ],
        raw=True,
    )
    extract = run_cli(
        [
            "extract",
            "https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html",
            "--allow-domain",
            "docs.aws.amazon.com",
            "--claim",
            "The maximum timeout for a Lambda function is 15 minutes.",
            "--verifier-command",
            "node scripts/example-verifier.mjs",
        ]
    )

    compact_json = json.loads(compact)
    extract_json = json.loads(extract)["extract"]["output"]
    outcome = extract_json["claimOutcomes"][0]
    citation = extract_json["evidenceSupportedClaims"][0]["citation"]["url"]

    read_lines = [
        "# Lambda quotas",
        "",
        find_first_line(read_view, "Function timeout:") or "Function timeout: 900 seconds (15 minutes).",
    ]
    compact_lines = [
        "{",
        f'  "approxTokens": {compact_json.get("approxTokens", 0)},',
        f'  "lineCount": {compact_json.get("lineCount", 0)},',
        f'  "compactText": "{truncate(compact_json.get("compactText", ""), 86)}"',
        "}",
    ]
    extract_lines = [
        "{",
        f'  "verdict": "{outcome.get("verdict", "unknown")}",',
        f'  "verificationVerdict": "{outcome.get("verificationVerdict", "none")}",',
        f'  "supportScore": {extract_json["evidenceSupportedClaims"][0].get("supportScore", 0)},',
        f'  "citation": "{citation}"',
        "}",
    ]

    frames = [
        render_terminal_frame(
            "touch-browser",
            "$ touch-browser compact-view https://docs.aws.amazon.com/lambda/latest/dg/welcome.html --allow-domain docs.aws.amazon.com",
            compact_lines,
            "Step 1: low-token routing surface decides where to browse next.",
        ),
        render_terminal_frame(
            "touch-browser",
            "$ touch-browser read-view https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html --main-only --allow-domain docs.aws.amazon.com",
            read_lines,
            "Step 2: readable markdown confirms the exact source sentence.",
        ),
        render_terminal_frame(
            "touch-browser",
            "$ touch-browser extract https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html --allow-domain docs.aws.amazon.com --claim \"The maximum timeout for a Lambda function is 15 minutes.\" --verifier-command 'node scripts/example-verifier.mjs'",
            extract_lines,
            "Step 3: final claim outcome carries evidence, verification, and citation.",
        ),
    ]

    save_gif(frames, OUTPUT_PATH)
    print(OUTPUT_PATH)


def build_cli() -> None:
    binary = REPO_ROOT / "target" / "debug" / "touch-browser"
    if binary.exists():
        return
    subprocess.run(
        ["cargo", "build", "-q", "-p", "touch-browser-cli"],
        cwd=REPO_ROOT,
        check=True,
    )


def run_cli(args: list[str], raw: bool = False) -> str:
    binary = REPO_ROOT / "target" / "debug" / "touch-browser"
    result = subprocess.run(
        [str(binary), *args],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=True,
    )
    return result.stdout if raw else result.stdout


def render_terminal_frame(title: str, command: str, lines: list[str], caption: str) -> Image.Image:
    image = Image.new("RGB", (WIDTH, HEIGHT), BACKGROUND)
    draw = ImageDraw.Draw(image)

    title_font = load_font(34)
    body_font = load_font(23)
    caption_font = load_font(24)

    panel_rect = (60, 80, WIDTH - 60, HEIGHT - 120)
    draw.rounded_rectangle(panel_rect, radius=24, fill=PANEL)
    draw.rounded_rectangle((60, 80, WIDTH - 60, 132), radius=24, fill="#0b1220")
    draw.text((92, 95), title, fill=TEXT, font=title_font)
    draw.text((92, 160), "$", fill=PROMPT, font=body_font)
    command_x = 120
    draw.multiline_text(
        (command_x, 160),
        wrap_text(command, 84),
        fill=TEXT,
        font=body_font,
        spacing=10,
    )
    draw.multiline_text(
        (92, 270),
        "\n".join(lines),
        fill=TEXT,
        font=body_font,
        spacing=12,
    )
    draw.text((92, HEIGHT - 90), caption, fill=ACCENT, font=caption_font)
    draw.text(
        (WIDTH - 420, HEIGHT - 90),
        "read-view / compact-view / extract",
        fill=MUTED,
        font=caption_font,
    )
    return image


def wrap_text(text: str, width: int) -> str:
    words = text.split(" ")
    lines: list[str] = []
    current = []
    current_len = 0
    for word in words:
        projected = current_len + len(word) + (1 if current else 0)
        if projected > width:
            lines.append(" ".join(current))
            current = [word]
            current_len = len(word)
        else:
            current.append(word)
            current_len = projected
    if current:
        lines.append(" ".join(current))
    return "\n".join(lines)


def truncate(text: str, limit: int) -> str:
    return text if len(text) <= limit else text[: limit - 3] + "..."


def find_first_line(text: str, prefix: str) -> str | None:
    for line in text.splitlines():
        if prefix in line:
            return line.strip()
    return None


def load_font(size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    for candidate in [
        "/System/Library/Fonts/Supplemental/Menlo.ttc",
        "/System/Library/Fonts/SFNSMono.ttf",
        "/Library/Fonts/Menlo.ttf",
    ]:
        if Path(candidate).exists():
            return ImageFont.truetype(candidate, size=size)
    return ImageFont.load_default()


def save_gif(frames: list[Image.Image], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    first, *rest = frames
    first.save(
        path,
        save_all=True,
        append_images=rest,
        duration=[1400, 1600, 2200],
        loop=0,
        optimize=False,
    )


if __name__ == "__main__":
    main()
