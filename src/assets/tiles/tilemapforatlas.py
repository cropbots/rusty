from PIL import Image
import json, math, os, sys

# === CONFIG ===
TILE_SIZE = 16              # width/height of each tile in px
ATLAS_COLS = 16             # how many tiles across per row
INPUT_DIR = "."         # folder containing the individual PNGs
OUTPUT_IMAGE = "tileset.png"
OUTPUT_JSON = "tileset.json"

# ================================
def main():
    files = sorted(
        [f for f in os.listdir(INPUT_DIR) if f.lower().endswith(".png")],
        key=lambda x: int(os.path.splitext(x)[0])
    )

    total = len(files)
    rows = math.ceil(total / ATLAS_COLS)
    atlas_w = ATLAS_COLS * TILE_SIZE
    atlas_h = rows * TILE_SIZE

    atlas = Image.new("RGBA", (atlas_w, atlas_h), (0, 0, 0, 0))
    mapping = {}

    for i, file in enumerate(files):
        tile_id = os.path.splitext(file)[0]
        img = Image.open(os.path.join(INPUT_DIR, file)).convert("RGBA")
        x = (i % ATLAS_COLS) * TILE_SIZE
        y = (i // ATLAS_COLS) * TILE_SIZE
        atlas.paste(img, (x, y))

        mapping[f"tile-{tile_id}"] = {
            "x": x,
            "y": y,
            "width": TILE_SIZE,
            "height": TILE_SIZE
        }

    atlas.save(OUTPUT_IMAGE)
    with open(OUTPUT_JSON, "w") as f:
        json.dump(mapping, f, indent=2)

    print(f"✅ Atlas created: {OUTPUT_IMAGE}")
    print(f"✅ Mapping saved: {OUTPUT_JSON}")
    print(f"Tiles packed: {total} ({ATLAS_COLS}x{rows})")

if __name__ == "__main__":
    main()

