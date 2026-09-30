"""Regenerate GitVibe platform icons from the approved logo artwork.

Requires Pillow: python -m pip install pillow
"""

from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"
SOURCE = ASSETS / "gitvibe-logo-source.png"


def main() -> None:
    artwork = Image.open(SOURCE).convert("RGBA")
    icon = artwork.resize((512, 512), Image.Resampling.LANCZOS)
    icon.save(ASSETS / "gitvibe-icon.png", optimize=True)
    artwork.save(
        ASSETS / "gitvibe.ico",
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )

    # Inno Setup's large wizard image uses a 164:314 aspect ratio.
    banner = Image.new("RGBA", (328, 628), (14, 24, 45, 255))
    lines = Image.new("RGBA", banner.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(lines)
    for offset in (0, 78, 156):
        draw.line([(0, 425 + offset), (328, 177 + offset)], fill=(39, 210, 211, 30), width=2)
        draw.line([(0, 505 + offset), (328, 257 + offset)], fill=(255, 111, 83, 24), width=2)
    banner.alpha_composite(lines)
    badge = artwork.resize((288, 288), Image.Resampling.LANCZOS)
    banner.alpha_composite(badge, (20, 158))
    banner.save(ASSETS / "installer-banner.png", optimize=True)


if __name__ == "__main__":
    main()
