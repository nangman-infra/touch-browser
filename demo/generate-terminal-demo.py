#!/usr/bin/env python3

from __future__ import annotations

import json
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlparse

from PIL import Image, ImageDraw, ImageFont


REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_PATH = REPO_ROOT / "demo" / "terminal-demo.gif"
WIDTH = 1440
HEIGHT = 810
BACKGROUND = "#0f1720"
PANEL = "#111827"
SURFACE = "#0c1424"
TEXT = "#e5e7eb"
MUTED = "#94a3b8"
ACCENT = "#22c55e"
PROMPT = "#38bdf8"
DIVIDER = "#223047"
OUTLINE = "#1f3148"


@dataclass(frozen=True)
class LayoutMetrics:
    frame_width: int
    frame_height: int
    panel_left: int
    panel_top: int
    panel_right: int
    panel_bottom: int
    panel_radius: int
    title_bar_height: int
    content_left: int
    content_right: int
    content_width: int
    title_y: int
    title_bar_bottom: int
    badge_padding_x: int
    badge_padding_y: int
    badge_radius: int
    command_top_gap: int
    section_gap: int
    divider_gap: int
    content_line_spacing: int
    caption_spacing: int
    response_padding_x: int
    response_padding_y: int
    response_radius: int
    footer_bottom_padding: int


def build_layout_metrics(width: int, height: int) -> LayoutMetrics:
    scale = min(width / 1440, height / 810)
    outer_margin_x = round(36 * scale)
    outer_margin_y = round(38 * scale)
    panel_left = outer_margin_x
    panel_top = outer_margin_y
    panel_right = width - outer_margin_x
    panel_bottom = height - outer_margin_y
    panel_width = panel_right - panel_left
    inner_padding_x = round(40 * scale)
    content_width = min(
        panel_width - (inner_padding_x * 2),
        round(width * 0.72),
    )
    content_left = panel_left + inner_padding_x
    content_right = content_left + content_width
    title_bar_height = round(72 * scale)

    return LayoutMetrics(
        frame_width=width,
        frame_height=height,
        panel_left=panel_left,
        panel_top=panel_top,
        panel_right=panel_right,
        panel_bottom=panel_bottom,
        panel_radius=round(26 * scale),
        title_bar_height=title_bar_height,
        content_left=content_left,
        content_right=content_right,
        content_width=content_width,
        title_y=panel_top + round(14 * scale),
        title_bar_bottom=panel_top + title_bar_height,
        badge_padding_x=round(14 * scale),
        badge_padding_y=round(8 * scale),
        badge_radius=round(14 * scale),
        command_top_gap=round(24 * scale),
        section_gap=round(16 * scale),
        divider_gap=round(14 * scale),
        content_line_spacing=round(12 * scale),
        caption_spacing=round(10 * scale),
        response_padding_x=round(16 * scale),
        response_padding_y=round(14 * scale),
        response_radius=round(18 * scale),
        footer_bottom_padding=round(24 * scale),
    )


def main() -> None:
    build_cli()

    session_file = Path("/tmp/tb-demo.json")
    cleanup_session_file(session_file)

    search = json.loads(
        run_cli(
            [
                "search",
                "iana example domains",
                "--engine",
                "google",
            ]
        )
    )
    first_open = json.loads(
        run_cli(
            [
                "open",
                "https://www.iana.org/help/example-domains",
                "--browser",
                "--session-file",
                str(session_file),
            ]
        )
    )
    second_open = json.loads(
        run_cli(
            [
                "open",
                "https://www.iana.org/domains/reserved",
                "--browser",
                "--session-file",
                str(session_file),
            ]
        )
    )
    session_state = json.loads(session_file.read_text())["session"]["state"]
    session_extract = json.loads(
        run_cli(
            [
                "session-extract",
                "--session-file",
                str(session_file),
                "--claim",
                "Example domains are maintained for documentation purposes.",
            ]
        )
    )
    session_synthesis = run_cli(
        [
            "session-synthesize",
            "--session-file",
            str(session_file),
            "--format",
            "markdown",
        ],
        raw=True,
    )

    top_result = search["search"]["results"][0]
    second_title = second_open["output"]["source"]["title"]
    extract_json = session_extract["extract"]["output"]
    outcome = extract_json["claimOutcomes"][0]
    synthesis_lines = [
        "# Session Synthesis",
        "",
        find_first_line(session_synthesis, "- Session ID:")
        or "- Session ID: scliopen001",
        find_first_line(session_synthesis, "- Snapshots:") or "- Snapshots: 2",
        "- Visited URLs: 2",
        "",
        "## Synthesized Notes",
        find_first_line(session_synthesis, "As described in RFC 2606")
        or "As described in RFC 2606 and RFC 6761, example domains are maintained for documentation purposes.",
    ]

    search_lines = [
        "{",
        f'  "query": "{search["query"]}",',
        f'  "status": "{search["search"]["status"]}",',
        f'  "topDomain": "{top_result["domain"]}",',
        f'  "nextAction": "{search["search"]["nextActionHints"][0]["action"]}"',
        "}",
    ]
    second_open_lines = [
        "{",
        f'  "title": "{second_title}",',
        f'  "snapshots": {len(session_state["snapshotIds"])},',
        f'  "visitedCount": {len(session_state["visitedUrls"])},',
        f'  "currentHost": "{urlparse(session_state["currentUrl"]).netloc}"',
        "}",
    ]
    extract_lines = [
        "{",
        f'  "verdict": "{outcome.get("verdict", "unknown")}",',
        f'  "supportScore": {extract_json["evidenceSupportedClaims"][0].get("supportScore", 0)},',
        f'  "sourceLabel": "{extract_json["evidenceSupportedClaims"][0]["citation"]["sourceLabel"]}",',
        "}",
    ]

    frames = [
        render_terminal_frame(
            "touch-browser",
            "touch-browser search \"iana example domains\" --engine google",
            search_lines,
            "Step 1: search ranks browser-backed candidates and returns the next action the agent can take.",
        ),
        render_terminal_frame(
            "touch-browser",
            "touch-browser open https://www.iana.org/domains/reserved --browser --session-file /tmp/tb-demo.json",
            second_open_lines,
            "Step 2: the same session file accumulates multiple official pages for later synthesis.",
        ),
        render_terminal_frame(
            "touch-browser",
            "touch-browser session-extract --session-file /tmp/tb-demo.json --claim \"Example domains are maintained for documentation purposes.\"",
            extract_lines,
            "Step 3: session-extract returns a cited verdict from the persisted browser session.",
        ),
        render_terminal_frame(
            "touch-browser",
            "touch-browser session-synthesize --session-file /tmp/tb-demo.json --format markdown",
            synthesis_lines,
            "Step 4: session-synthesize turns the multi-page session into reviewable notes for downstream agents.",
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


def cleanup_session_file(path: Path) -> None:
    if path.exists():
        path.unlink()
    context_dir = Path(f"{path}.browser-context")
    if context_dir.exists():
        shutil.rmtree(context_dir)


def render_terminal_frame(title: str, command: str, lines: list[str], caption: str) -> Image.Image:
    image = Image.new("RGB", (WIDTH, HEIGHT), BACKGROUND)
    draw = ImageDraw.Draw(image)
    layout = build_layout_metrics(WIDTH, HEIGHT)

    font_scale = min(WIDTH / 1440, HEIGHT / 810)
    title_font = load_font(round(34 * font_scale))
    badge_font = load_font(round(16 * font_scale))
    command_font = load_font(round(22 * font_scale))
    body_font = load_font(round(22 * font_scale))
    caption_font = load_font(round(18 * font_scale))

    command_line_height = line_height(draw, command_font)
    body_line_height = line_height(draw, body_font)
    caption_line_height = line_height(draw, caption_font)

    panel_rect = (
        layout.panel_left,
        layout.panel_top,
        layout.panel_right,
        layout.panel_bottom,
    )
    draw.rounded_rectangle(panel_rect, radius=layout.panel_radius, fill=PANEL)
    draw.rounded_rectangle(
        (panel_rect[0], panel_rect[1], panel_rect[2], layout.title_bar_bottom),
        radius=layout.panel_radius,
        fill="#0b1220",
    )
    draw.text(
        (layout.content_left, layout.title_y),
        title,
        fill=TEXT,
        font=title_font,
    )

    badge_text = "read-view / compact-view / extract"
    badge_width = (
        int(draw.textlength(badge_text, font=badge_font))
        + layout.badge_padding_x * 2
    )
    badge_height = badge_font.size + layout.badge_padding_y * 2
    badge_x = panel_rect[2] - layout.content_left + panel_rect[0] - badge_width
    badge_y = panel_rect[1] + round(12 * font_scale)
    draw.rounded_rectangle(
        (badge_x, badge_y, badge_x + badge_width, badge_y + badge_height),
        radius=layout.badge_radius,
        fill="#101a2e",
        outline=OUTLINE,
        width=1,
    )
    draw.text(
        (badge_x + layout.badge_padding_x, badge_y + layout.badge_padding_y - 1),
        badge_text,
        fill=MUTED,
        font=badge_font,
    )

    command_text = command.removeprefix("$ ").strip()
    command_lines = wrap_command_block(
        draw,
        command_text,
        command_font,
        layout.content_width,
    )
    command_height = block_height(
        len(command_lines),
        command_line_height,
        layout.content_line_spacing,
    )
    body_lines = wrap_code_block(draw, lines, body_font, layout.content_width)
    caption_lines = wrap_text_to_width(
        draw,
        caption,
        caption_font,
        layout.content_width,
        continuation_prefix="",
    )

    natural_body_height = block_height(
        len(body_lines),
        body_line_height,
        layout.content_line_spacing,
    )
    response_height = max(
        round(160 * font_scale),
        natural_body_height + (layout.response_padding_y * 2),
    )
    footer_height = block_height(
        len(caption_lines),
        caption_line_height,
        layout.caption_spacing,
    )
    natural_cluster_height = (
        command_height
        + layout.divider_gap
        + 1
        + layout.section_gap
        + response_height
        + layout.section_gap
        + layout.divider_gap
        + 1
        + footer_height
    )
    available_cluster_height = (
        panel_rect[3]
        - layout.footer_bottom_padding
        - (layout.title_bar_bottom + layout.command_top_gap)
    )
    cluster_offset = max(
        0,
        round((available_cluster_height - natural_cluster_height) * 0.18),
    )
    command_y = layout.title_bar_bottom + layout.command_top_gap + cluster_offset
    divider_y = command_y + command_height + layout.divider_gap
    response_top = divider_y + layout.section_gap
    response_rect = (
        layout.content_left - layout.response_padding_x,
        response_top,
        layout.content_right + layout.response_padding_x,
        response_top + response_height,
    )
    footer_y = response_rect[3] + layout.section_gap + layout.divider_gap

    draw_multiline_lines(
        draw,
        layout.content_left,
        command_y,
        command_lines,
        command_font,
        layout.content_line_spacing,
        TEXT,
        prompt_color=PROMPT,
    )
    draw.line(
        (layout.content_left, divider_y, layout.content_right, divider_y),
        fill=DIVIDER,
        width=1,
    )
    draw.rounded_rectangle(
        response_rect,
        radius=layout.response_radius,
        fill=SURFACE,
        outline=OUTLINE,
        width=1,
    )

    body_max_height = max(
        0,
        response_height - (layout.response_padding_y * 2),
    )
    body_lines = fit_lines_to_height(
        body_lines,
        body_max_height,
        body_line_height,
        layout.content_line_spacing,
    )
    body_y = response_rect[1] + layout.response_padding_y
    draw_multiline_lines(
        draw,
        layout.content_left,
        body_y,
        body_lines,
        body_font,
        layout.content_line_spacing,
        TEXT,
    )

    draw.line(
        (
            layout.content_left,
            footer_y - layout.divider_gap,
            layout.content_right,
            footer_y - layout.divider_gap,
        ),
        fill=DIVIDER,
        width=1,
    )
    draw_multiline_lines(
        draw,
        layout.content_left,
        footer_y,
        caption_lines,
        caption_font,
        layout.caption_spacing,
        ACCENT,
    )
    return image


def wrap_command_block(
    draw: ImageDraw.ImageDraw,
    command: str,
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
    max_width: int,
) -> list[tuple[str, bool]]:
    prompt_prefix = "$ "
    prompt_width = int(draw.textlength(prompt_prefix, font=font))
    wrapped = wrap_text_to_width(
        draw,
        command,
        font,
        max_width - prompt_width,
        continuation_prefix="  ",
    )
    if not wrapped:
        return [(prompt_prefix, True)]

    lines = []
    for index, line in enumerate(wrapped):
        prefix = prompt_prefix if index == 0 else "  "
        lines.append((prefix + line, index == 0))
    return lines


def wrap_code_block(
    draw: ImageDraw.ImageDraw,
    lines: list[str],
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
    max_width: int,
) -> list[str]:
    wrapped: list[str] = []
    for line in lines:
        wrapped.extend(
            wrap_text_to_width(
                draw,
                line,
                font,
                max_width,
                continuation_prefix="  ",
                preserve_indent=True,
            )
        )
    return wrapped


def wrap_text_to_width(
    draw: ImageDraw.ImageDraw,
    text: str,
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
    max_width: int,
    continuation_prefix: str = "  ",
    preserve_indent: bool = False,
) -> list[str]:
    if not text:
        return [""]

    wrapped_lines: list[str] = []
    for raw_line in text.splitlines() or [""]:
        if raw_line == "":
            wrapped_lines.append("")
            continue

        indent = ""
        content = raw_line
        if preserve_indent:
            stripped = raw_line.lstrip(" ")
            indent = raw_line[: len(raw_line) - len(stripped)]
            content = stripped

        prefixes = [indent, indent + continuation_prefix]
        current_prefix = prefixes[0]
        remaining = content

        while remaining:
            allowed_width = max_width - int(draw.textlength(current_prefix, font=font))
            if draw.textlength(current_prefix + remaining, font=font) <= max_width:
                wrapped_lines.append(current_prefix + remaining)
                break

            split_at = find_wrap_position(draw, remaining, font, allowed_width)
            chunk = remaining[:split_at].rstrip()
            wrapped_lines.append(current_prefix + chunk)
            remaining = remaining[split_at:].lstrip()
            current_prefix = prefixes[1]

    return wrapped_lines


def find_wrap_position(
    draw: ImageDraw.ImageDraw,
    text: str,
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
    max_width: int,
) -> int:
    if max_width <= 0:
        return 1

    fallback = 1
    for index in range(1, len(text) + 1):
        sample = text[:index]
        if draw.textlength(sample, font=font) <= max_width:
            fallback = index
            continue
        break

    if fallback >= len(text):
        return len(text)

    for index in range(fallback, 0, -1):
        if text[index - 1].isspace() or text[index - 1] in "/_-.:,)]}\"'":
            return max(1, index)

    return max(1, fallback)


def fit_lines_to_height(
    lines: list[str],
    max_height: int,
    line_height_value: int,
    spacing: int,
) -> list[str]:
    if not lines:
        return [""]

    fitted: list[str] = []
    current_height = 0
    for line in lines:
        additional = line_height_value if not fitted else line_height_value + spacing
        if current_height + additional > max_height:
            if fitted:
                fitted[-1] = truncate(fitted[-1], max(8, len(fitted[-1]) - 1))
            return fitted
        fitted.append(line)
        current_height += additional

    return fitted


def draw_multiline_lines(
    draw: ImageDraw.ImageDraw,
    x: int,
    y: int,
    lines: list[str] | list[tuple[str, bool]],
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
    spacing: int,
    color: str,
    prompt_color: str | None = None,
) -> None:
    current_y = y
    height = line_height(draw, font)
    for line in lines:
        if isinstance(line, tuple):
            text, highlight_prompt = line
            if prompt_color and highlight_prompt and text.startswith("$ "):
                draw.text((x, current_y), "$", fill=prompt_color, font=font)
                offset = int(draw.textlength("$ ", font=font))
                draw.text((x + offset, current_y), text[2:], fill=color, font=font)
            else:
                draw.text((x, current_y), text, fill=color, font=font)
        else:
            draw.text((x, current_y), line, fill=color, font=font)
        current_y += height + spacing


def line_height(
    draw: ImageDraw.ImageDraw,
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
) -> int:
    box = draw.textbbox((0, 0), "Ag", font=font)
    return box[3] - box[1]


def block_height(line_count: int, line_height_value: int, spacing: int) -> int:
    if line_count <= 0:
        return 0
    return (line_count * line_height_value) + ((line_count - 1) * spacing)


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
    durations = [1400] * len(frames)
    if durations:
        durations[-1] = 2200
    first.save(
        path,
        save_all=True,
        append_images=rest,
        duration=durations,
        loop=0,
        optimize=False,
    )


if __name__ == "__main__":
    main()
