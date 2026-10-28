# Headless compositor: rebuild a ChomBomb-family sheet from a raw 16x7 Chomb sheet.
#
# The band-3 `chomb_sheet.png` (5 rows: idle/walk/hurt/attack/explosion, 32px, 7 cols)
# was cherry-picked from the raw 224x512 Chomb1.png. That recipe wasn't saved, so this
# script RE-DERIVES it by pixel-matching each built row against the 16 source rows of
# Chomb1.png, then applies the SAME row mapping to a chosen variant sheet (Chomb2.png =
# teal "Toxic Chom") to emit a matching 224x160 sheet.
#
# Run:
#   Godot --headless -s tools/build_chomb_variant.gd -- <raw_variant.png> <out.png>
extends SceneTree

const CELL := 32
const RAW := "/Users/lunalieneray/Downloads/ChomBombs (full) v1.1/ChomBombs/Chomb All/Chomb1.png"
const BUILT := "/Users/lunalieneray/idle_mmorpg/godot/assets/chomb_sheet.png"

func _load(path: String) -> Image:
	var img: Image
	if path.begins_with("res://"):
		img = load(path).get_image()
	else:
		img = Image.new()
		var err := img.load(path)
		if err != OK:
			push_error("load failed: %s (%d)" % [path, err])
			return null
	img.convert(Image.FORMAT_RGBA8)
	return img

# Sum of absolute per-channel diff between two full rows (all 7 cols) of two sheets.
func _row_diff(a: Image, ra: int, b: Image, rb: int, cols: int) -> int:
	var total := 0
	for x in range(cols * CELL):
		for dy in range(CELL):
			var ca := a.get_pixel(x, ra * CELL + dy)
			var cb := b.get_pixel(x, rb * CELL + dy)
			total += int(abs(ca.r - cb.r) * 255) + int(abs(ca.g - cb.g) * 255) \
				+ int(abs(ca.b - cb.b) * 255) + int(abs(ca.a - cb.a) * 255)
	return total

func _init() -> void:
	var args := OS.get_cmdline_user_args()
	if args.size() < 2:
		push_error("usage: -- <raw_variant.png> <out.png>")
		quit(1); return
	var variant_path: String = args[0]
	var out_path: String = args[1]

	var src := _load(RAW)          # Chomb1 (orange) — same palette as BUILT
	var built := _load(BUILT)      # the 5-row target we must reproduce
	var variant := _load(variant_path)  # Chomb2 (teal) — same 16x7 layout
	if src == null or built == null or variant == null:
		quit(1); return

	var built_rows := built.get_height() / CELL
	var src_rows := src.get_height() / CELL
	var cols := built.get_width() / CELL

	# For each built row, find the best-matching source row.
	var mapping: Array[int] = []
	for br in range(built_rows):
		var best := -1
		var best_diff := 1 << 62
		for sr in range(src_rows):
			var d := _row_diff(built, br, src, sr, cols)
			if d < best_diff:
				best_diff = d; best = sr
		mapping.append(best)
		print("built row %d  <-  source row %d  (diff %d)" % [br, best, best_diff])

	# Emit the variant sheet using the same row mapping.
	var out := Image.create(cols * CELL, built_rows * CELL, false, Image.FORMAT_RGBA8)
	out.fill(Color(0, 0, 0, 0))
	for br in range(built_rows):
		var sr: int = mapping[br]
		var region := Rect2i(0, sr * CELL, cols * CELL, CELL)
		out.blit_rect(variant, region, Vector2i(0, br * CELL))
	var err := out.save_png(out_path)
	print("saved %s (%dx%d)  err=%d" % [out_path, out.get_width(), out.get_height(), err])
	quit(0)
