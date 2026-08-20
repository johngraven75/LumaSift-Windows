#!/usr/bin/env python3
"""Generate Windows application icons from the committed LumaSift prism source asset."""
from pathlib import Path
from PIL import Image

root = Path(__file__).resolve().parents[1]
source = root / "assets" / "lumasift-prism.png"
icons = root / "src-tauri" / "icons"
icons.mkdir(parents=True, exist_ok=True)
image = Image.open(source).convert("RGBA")
image.save(icons / "icon.png", format="PNG")
image.save(icons / "icon.ico", format="ICO", sizes=[(16, 16), (32, 32), (48, 48), (128, 128), (256, 256)])
