#!/usr/bin/env bash
# Copy the data files the Unity client reads at runtime into StreamingAssets.
#
# Three files, three different owners, and all three were hand-copied once and would have drifted
# quietly: the catalog is what the SIM validates a save against (a stale one fails the server's
# version gate), the Commander track is what the client draws while the server pays from its own
# copy, and the strings table is where every item's display name lives.
#
# Run after changing any of them. `tools/check_online_sync.sh` does the same job for the Godot copy.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dst="$root/unity/ROI/Assets/StreamingAssets"

copy() {
  if cmp -s "$1" "$2"; then echo "  = $(basename "$2")"; else cp "$1" "$2"; echo "  → $(basename "$2") updated"; fi
}

copy "$root/data/game_data.json"        "$dst/game_data.json"
copy "$root/data/commander_rewards.json" "$dst/commander_rewards.json"
copy "$root/godot/i18n/strings.csv"      "$dst/strings.csv"

# The SOUNDS, which live under godot/assets because that is where they were authored. They are
# read at runtime from StreamingAssets exactly as the art sheets are — the same reason: a file
# under StreamingAssets is bytes the client opens, with no import settings between the source and
# what ships, so the two engines cannot end up playing differently processed audio.
mkdir -p "$dst/audio"
for f in "$root"/godot/assets/music/*.mp3 "$root"/godot/assets/sfx/*.mp3; do
  [ -f "$f" ] || continue
  copy "$f" "$dst/audio/$(basename "$f")"
done
# ART the client reads at runtime that is not part of the arena export.
# `tools/export_arena_tables.gd` writes the sprite sheets it knows about — actors and monsters —
# and these three belong to things it does not enumerate: a screen (the card frames) and the
# projectiles in flight.
for f in card_frames.png arrow_sheet.png fireball_sheet.png; do
  [ -f "$root/godot/assets/$f" ] || continue
  copy "$root/godot/assets/$f" "$dst/art/$f"
done

# The STORY files. Both clients play the SAME `.dialogue` sources — a mission that read
# differently in two clients would be two missions — so they are copied, never re-authored.
mkdir -p "$dst/story"
for f in "$root"/godot/story/*.dialogue; do
  [ -f "$f" ] || continue
  copy "$f" "$dst/story/$(basename "$f")"
done
echo "unity data in sync"
