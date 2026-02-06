from PIL import Image
import os
import math

# === Config ===
TILE_SIZE = 16
CHUNK_SIZE = 16
MAP_COLS = TILE_SIZE
MAP_ROWS = TILE_SIZE
OUTPUT_DIR = "."
TILE_DIR = "."
GRASS_TILE_ID = 24

os.makedirs(OUTPUT_DIR, exist_ok=True)

# Load the single grass tile once
grass_tile = Image.open(os.path.join(TILE_DIR, f"{GRASS_TILE_ID}.png")).convert("RGBA")

chunk_cols = math.ceil(MAP_COLS / CHUNK_SIZE)
chunk_rows = math.ceil(MAP_ROWS / CHUNK_SIZE)

for cy in range(chunk_rows):
    for cx in range(chunk_cols):
        chunk_img = Image.new("RGBA", (CHUNK_SIZE * TILE_SIZE, CHUNK_SIZE * TILE_SIZE))
        for y in range(CHUNK_SIZE):
            for x in range(CHUNK_SIZE):
                px = x * TILE_SIZE
                py = y * TILE_SIZE
                chunk_img.paste(grass_tile, (px, py), grass_tile)
        chunk_path = os.path.join(OUTPUT_DIR, f"grass-chunk-{cx}-{cy}.png")
        chunk_img.save(chunk_path)

print("✅ Grass chunk images baked successfully!")

