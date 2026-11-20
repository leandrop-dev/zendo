#!/usr/bin/env python3
"""Seeds a throwaway home with games and covers for the FPS harness.

The window draws the grid from whatever the library scan finds; with no real
library on a test machine, this writes `count` `.desktop` games and a cover
for each into a home of its own. Then run the shell there with `ZENDO_FPS=1`
and it scrolls itself through the list, showing the frame rate and the worst
half second.

    scripts/seed-library.py --home /tmp/zendo-fps --count 2700

    env -i PATH="$PATH" HOME=/tmp/zendo-fps \\
        XDG_DATA_HOME=/tmp/zendo-fps/.local/share \\
        XDG_CACHE_HOME=/tmp/zendo-fps/.cache \\
        XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-1 \\
        ZENDO_FPS=1 target/debug/zendo

Only a test home should be used: the games point at `/bin/true`.
"""

import argparse
from pathlib import Path

from PIL import Image, ImageDraw

ENTRY = """[Desktop Entry]
Type=Application
Name=Seeded Game {number}
Exec=/bin/true
Categories=Game;
"""


def cover(path: Path, number: int) -> None:
    hue = number * 37
    red = (hue * 3) % 200 + 40
    green = (hue * 5) % 200 + 40
    blue = (hue * 7) % 200 + 40
    image = Image.new("RGB", (180, 270), (red, green, blue))
    draw = ImageDraw.Draw(image)
    draw.rectangle((8, 8, 172, 262), outline=(255, 255, 255))
    draw.text((16, 16), f"#{number:04d}", fill=(255, 255, 255))
    image.save(path, "JPEG", quality=75)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--home", required=True, type=Path)
    parser.add_argument("--count", type=int, default=2700)
    arguments = parser.parse_args()

    applications = arguments.home / ".local/share/applications"
    covers = arguments.home / ".cache/zendo/covers"
    applications.mkdir(parents=True, exist_ok=True)
    covers.mkdir(parents=True, exist_ok=True)

    for number in range(1, arguments.count + 1):
        name = f"zendo-seed-{number:04d}"
        (applications / f"{name}.desktop").write_text(
            ENTRY.format(number=f"{number:04d}"), encoding="utf-8"
        )
        # The id is `desktop:<file name without .desktop>`; the cover's name
        # is the id with anything outside [A-Za-z0-9._-] as `_`.
        cover(covers / f"desktop_{name}.jpg", number)

    print(f"{arguments.count} games in {arguments.home}")


if __name__ == "__main__":
    main()
