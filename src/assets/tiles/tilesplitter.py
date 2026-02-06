from PIL import Image
import os
import argparse

def split_tileset(image_path, output_dir, tile_width, tile_height, skip_empty=False):
    # Load the image
    tileset = Image.open(image_path).convert("RGBA")
    width, height = tileset.size

    # Calculate grid size
    tiles_x = width // tile_width
    tiles_y = height // tile_height

    os.makedirs(output_dir, exist_ok=True)
    print(f"Splitting {image_path} into {tiles_x * tiles_y} tiles...")

    count = 0
    for y in range(tiles_y):
        for x in range(tiles_x):
            # Crop each tile
            left = x * tile_width
            upper = y * tile_height
            right = left + tile_width
            lower = upper + tile_height
            tile = tileset.crop((left, upper, right, lower))

            # Skip empty tiles if requested
            if skip_empty:
                # Check if tile has any non-transparent pixel
                bbox = tile.getbbox()
                if bbox is None:
                    continue  # fully transparent tile, skip it

            # Save with coordinate-based filename
            filename = f"{x * tile_width}_{y * tile_height}.png"
            tile.save(os.path.join(output_dir, filename))
            count += 1

    print(f"✅ Done! Saved {count} tiles to '{output_dir}'.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Split a tileset image into separate tiles.")
    parser.add_argument("image", help="Path to the tileset image (PNG, etc.)")
    parser.add_argument("output", help="Folder to save tiles into")
    parser.add_argument("--tilewidth", type=int, default=16, help="Tile width in pixels")
    parser.add_argument("--tileheight", type=int, default=16, help="Tile height in pixels")
    parser.add_argument("--skip-empty", action="store_true", help="Skip fully transparent tiles")

    args = parser.parse_args()

    split_tileset(args.image, args.output, args.tilewidth, args.tileheight, args.skip_empty)

