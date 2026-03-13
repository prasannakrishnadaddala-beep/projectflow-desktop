"""Generate all required Tauri icons from icon.png"""
from pathlib import Path
from PIL import Image
import shutil

src = Path("icon.png")
if not src.exists():
    raise FileNotFoundError("icon.png not found")

# Icons go inside src-tauri/icons/ (assembled by workflow step before this runs)
icons_dir = Path("src-tauri/icons")
icons_dir.mkdir(parents=True, exist_ok=True)

img = Image.open(src).convert("RGBA")

for size, name in [(32,"32x32.png"),(128,"128x128.png"),(256,"128x128@2x.png")]:
    img.resize((size,size), Image.LANCZOS).save(icons_dir/name)
    print(f"  created {name}")

shutil.copy(icons_dir/"128x128.png", icons_dir/"icon.icns")
print("  created icon.icns")

img.save(str(icons_dir/"icon.ico"), format="ICO",
         sizes=[(16,16),(32,32),(48,48),(64,64),(128,128),(256,256)])
print("  created icon.ico")
print("Done!")
