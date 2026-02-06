import os

# === CONFIG ===
TILESET_WIDTH = 17  # number of tiles per row
FOLDER = "."        # current folder, or set path like "tiles/"

# === SCRIPT ===
for filename in os.listdir(FOLDER):
    if filename.endswith(".png"):
        name, _ = os.path.splitext(filename)
        try:
            x, y = map(int, name.split("_"))
        except ValueError:
            print(f"Skipping {filename} (invalid name)")
            continue

        tile_id = y // 16 * TILESET_WIDTH + (x // 16) + 1
        new_name = f"{tile_id}.png"
        os.rename(os.path.join(FOLDER, filename), os.path.join(FOLDER, new_name))
        print(f"{filename} → {new_name}")

