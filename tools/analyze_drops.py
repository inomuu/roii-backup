import json

d = json.load(open("data/game_data.json"))
mons = d.get("monsters", [])


def analyze(m):
    drops = m.get("drops", [])
    gear = sum(e["weight"] for e in drops
               if e["item_id"] != "nothing" and not e["item_id"].startswith("junk"))
    junk = sum(e["weight"] for e in drops if e["item_id"].startswith("junk"))
    noth = sum(e["weight"] for e in drops if e["item_id"] == "nothing")
    tot = gear + junk + noth
    return gear, junk, noth, tot


seen = set()
for m in mons:
    g, j, n, t = analyze(m)
    key = (g, j, n)
    if key in seen:
        continue
    seen.add(key)
    boss = "boss" in m.get("id", "")
    pct = 100 * g / t if t else 0
    tag = "[BOSS]" if boss else ""
    print(f'{m["id"]:14} gear={g:4} junk={j:5} noth={n:5} tot={t:5} gear%={pct:5.2f} {tag}')
