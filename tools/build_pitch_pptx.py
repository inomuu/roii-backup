#!/usr/bin/env python3.12
"""Build an editable 16:9 Google-Slides-ready PPTX of the ROI pitch deck.
Mirrors docs/depa-pitch-deck.html. Run with python3.12 (3.14 has a broken pyexpat)."""
from pptx import Presentation
from pptx.util import Inches, Pt, Emu
from pptx.dml.color import RGBColor
from pptx.enum.text import PP_ALIGN, MSO_ANCHOR
from pptx.enum.shapes import MSO_SHAPE
from pptx.oxml.ns import qn

BG     = RGBColor(0x0f,0x16,0x22)
BG2    = RGBColor(0x15,0x26,0x4a)   # title/ask gradient-ish
ACCENT = RGBColor(0x4a,0xa3,0xff)
WHITE  = RGBColor(0xf5,0xf7,0xfa)
SUB    = RGBColor(0xae,0xbf,0xd4)
GREEN  = RGBColor(0x7f,0xd4,0xa0)
GREY   = RGBColor(0x5b,0x6b,0x82)
PANEL  = RGBColor(0x13,0x1e,0x30)
BORDER = RGBColor(0x35,0x50,0x80)
NODE   = RGBColor(0x1b,0x29,0x40)

EMU_W, EMU_H = Inches(13.333), Inches(7.5)
prs = Presentation()
prs.slide_width  = EMU_W
prs.slide_height = EMU_H
BLANK = prs.slide_layouts[6]

def slide(bg=BG):
    s = prs.slides.add_slide(BLANK)
    s.background.fill.solid()
    s.background.fill.fore_color.rgb = bg
    return s

def box(s, x, y, w, h):
    tb = s.shapes.add_textbox(Inches(x), Inches(y), Inches(w), Inches(h))
    tf = tb.text_frame; tf.word_wrap = True
    return tf

def para(tf, text, size, color, bold=False, first=False, space_after=8, align=PP_ALIGN.LEFT, line=1.15):
    p = tf.paragraphs[0] if first else tf.add_paragraph()
    p.alignment = align
    p.space_after = Pt(space_after); p.line_spacing = line
    r = p.add_run(); r.text = text
    f = r.font; f.size = Pt(size); f.color.rgb = color; f.bold = bold
    f.name = "Arial"
    return p

def kicker(s, text):
    para(box(s, 0.83, 0.5, 11.6, 0.5), text.upper(), 14, ACCENT, bold=True, first=True)

def pagenum(s, n):
    para(box(s, 12.2, 6.9, 1.0, 0.4), str(n), 11, GREY, first=True, align=PP_ALIGN.RIGHT)

def rect(s, x, y, w, h, fill=None, line=None, dash=False, line_w=1.0):
    sh = s.shapes.add_shape(MSO_SHAPE.ROUNDED_RECTANGLE, Inches(x), Inches(y), Inches(w), Inches(h))
    sh.shadow.inherit = False
    if fill is None: sh.fill.background()
    else: sh.fill.solid(); sh.fill.fore_color.rgb = fill
    if line is None: sh.line.fill.background()
    else:
        sh.line.color.rgb = line; sh.line.width = Pt(line_w)
        if dash:
            d = sh.line._get_or_add_ln(); pd = d.makeelement(qn('a:prstDash'), {'val':'dash'}); d.append(pd)
    return sh

def placeholder(s, x, y, w, h, label, sub=None):
    rect(s, x, y, w, h, fill=PANEL, line=BORDER, dash=True, line_w=2.5)
    tf = box(s, x+0.2, y+h/2-0.5, w-0.4, 1.0)
    para(tf, label, 16, RGBColor(0x6f,0x86,0xa8), bold=True, first=True, align=PP_ALIGN.CENTER)
    if sub: para(tf, sub, 12, RGBColor(0x6f,0x86,0xa8), align=PP_ALIGN.CENTER)

def title_only(s, text, size=40, top=1.2):
    para(box(s, 0.83, top, 11.6, 1.4), text, size, WHITE, bold=True, first=True, line=1.1)

def bullets(s, items, top=2.2, size=18, width=11.6, sa=10):
    tf = box(s, 0.83, top, width, 7.5-top-0.6)
    for i, it in enumerate(items):
        para(tf, "▸  " + it, size, RGBColor(0xe6,0xed,0xf6), first=(i==0), space_after=sa, line=1.25)

# ---------------- content (mirrors depa-pitch-deck.html) ----------------
# 1 GAME EXAMPLE
s = slide(BG2)
kicker(s, "Idle ARPG · PC / Steam")
para(box(s, 0.83, 1.0, 11.6, 1.4), "ROI: Reign of Idle", 52, WHITE, bold=True, first=True, line=1.05)
para(box(s, 0.83, 2.3, 11.6, 0.8), "Build your party. Leave it running. Win while you work.", 22, SUB, first=True)
placeholder(s, 0.83, 3.3, 11.67, 3.2, "▶ Gameplay screenshot / GIF here", "(drop a capture of the running game)")
para(box(s, 0.83, 6.95, 8, 0.4), "depa Digital Startup Fund — Pitch Deck", 11, GREY, first=True)

# 2 GENRE
s = slide(); kicker(s, "Genre"); title_only(s, "Idle Action-RPG · party-building.", 36)
para(box(s, 0.83, 2.6, 11.6, 2.5),
     "An idle ARPG where you build a party and it auto-fights for you — Ragnarok-style depth, idle-game ease.",
     32, WHITE, bold=True, first=True, line=1.25); pagenum(s, 2)

# 3 FEATURES (rows)
s = slide(); kicker(s, "Features"); title_only(s, "At a glance.", 34)
rows = [("Platform","PC (Steam) — Windows & macOS; architected for online from the start."),
        ("Audience","Targeting Teen (IARC); core players 20–40, working adults who idle-play alongside their day."),
        ("Key feature","Deep team composition + a Desktop Companion that lives on your screen."),
        ("Core gameplay","An auto-battling party you optimize — skills, gear, elements."),
        ("Content","10 classes · elemental matchups · 20+ enemies · 3 bosses · class advancement.")]
y = 2.3
for lbl, val in rows:
    para(box(s, 0.83, y, 2.6, 0.9), lbl.upper(), 15, GREEN, bold=True, first=True)
    para(box(s, 3.5, y, 9.0, 0.9), val, 18, RGBColor(0xe6,0xed,0xf6), first=True, line=1.2)
    y += 0.86
pagenum(s, 3)

# 4 WHAT PLAYERS DO
s = slide(); kicker(s, "What Players Do"); title_only(s, "Plan deep — play effortless.", 34)
bullets(s, ["Recruit & compose a party across 10 classes.",
            "Allocate stats & skills; equip gear to power up the party.",
            "Exploit elemental rock-paper-scissors against each enemy.",
            "Push harder zones, bosses, and class advancements as the party auto-battles.",
            "Run it small as a Desktop Companion while you work."]); pagenum(s, 4)

# 5 UNIQUE GAMEPLAY
s = slide(); kicker(s, "Unique Gameplay"); title_only(s, "What sets us apart.", 34)
bullets(s, ["Team-composition strategy — you tune a whole party (classes, synergy, elements), not one hero.",
            "Desktop Companion — the game lives on your screen all day, not in a tab you forget.",
            "Real ARPG depth — elemental matchups + cross-class skill synergy.",
            "Player-driven economy — trade and sell the gear you farm on the Steam Market; loot has real, player-set value.",
            "Deterministic core — fair, reproducible gameplay; the foundation for future server-side anti-cheat verification."], sa=9); pagenum(s, 5)

# 6 CONCEPT / LOOK & FEEL
s = slide(); kicker(s, "Concept · Look & Feel"); title_only(s, "A cozy pixel-art ARPG you live alongside.", 30)
tf = box(s, 0.83, 2.05, 6.7, 4.6)
para(tf, "A charming, readable pixel-art fantasy world. A party of heroes auto-battles through colorful dungeons of slimes, beasts and bosses — drawn top-down so every fight reads at a glance, even shrunk into the Desktop Companion.",
     19, SUB, first=True, line=1.3, space_after=16)
for b in ["Retro pixel-art — nostalgic & instantly readable.",
          "Colorful, characterful monsters + elemental variety.",
          "Clean top-down arena that stays clear at any window size."]:
    para(tf, "▸  " + b, 17, RGBColor(0xe6,0xed,0xf6), space_after=8, line=1.2)
placeholder(s, 7.9, 2.05, 4.6, 2.15, "Concept art / in-game")
placeholder(s, 7.9, 4.35, 4.6, 2.15, "Concept art / in-game")
pagenum(s, 6)

# 7 BUSINESS MODEL
s = slide(); kicker(s, "Business Model"); title_only(s, "Free-to-play live-service — multiple revenue streams.", 30)
bullets(s, ["Battle Pass & seasons — the proven F2P engine: recurring revenue from cosmetics + progression rewards each season.",
            "Convenience & cosmetics — optional QoL (extra stash, auto-farm slots, time-savers) and customization. Power is earned through play and accelerated — sold indirectly via time, never as a direct stat-stick.",
            "Player-driven marketplace (Steam Market) — players trade farmed gear; we earn the platform's transaction cut. A real, self-sustaining economy — and the path to power without grinding for paying players."], top=2.0, size=17, sa=12); pagenum(s, 7)

# 8 TEAM
s = slide(); kicker(s, "Team"); title_only(s, "A shipped, credible team.", 34)
members = [("Nuttapong Ketam","Developer — design & engineering","Shipped Shima Bird (Japanese-learning app) & Kanji Striker (mobile kanji game)"),
           ("Kampol Sae-Iew","Artist — characters / monsters / world","Zheza Fighter · Zombie Heroes · Neko Odyssey · Banjo Tactic · Eternity Breaker"),
           ("Inomuu","UI Artist — experienced","UI / UX visual design")]
x = 0.83
for name, role, cred in members:
    tf = box(s, x, 2.2, 3.7, 1.9)
    para(tf, name, 21, WHITE, bold=True, first=True, space_after=3)
    para(tf, role, 15, GREEN, bold=True, space_after=6)
    para(tf, cred, 13, SUB, line=1.25)
    placeholder(s, x, 4.3, 3.7, 2.1, "Portfolio / work samples")
    x += 3.92
pagenum(s, 8)

# 9 TOTAL BUDGET
s = slide(BG2); kicker(s, "Total Expected Budget")
para(box(s, 0.83, 1.9, 11.6, 1.8), "฿200,000", 90, ACCENT, bold=True, first=True, line=1.0)
para(box(s, 0.83, 4.0, 11.6, 2.0),
     "Grant request (non-repayable) to deliver the complete offline core in 6 months. "
     "Software-development labor is self-funded and not part of the request.", 22, SUB, first=True, line=1.3)
pagenum(s, 9)

# 10 6-MONTH GOAL
s = slide(); kicker(s, "6-Month Goal · for ฿200,000"); title_only(s, "A complete, online-ready offline core.", 32)
bullets(s, ["10 classes (5 tier-1 + 5 tier-2 / advancement) with full skill kits.",
            "20+ enemies, 3 bosses, level path & tuned balance.",
            "VFX/SFX pass + UI/UX polish.",
            "Tested via in-house QA + private alpha; Steam page live to gather wishlists.",
            "Success targets: ≥1,000 Steam wishlists · ≥30 private-alpha testers · all 10 classes balance-verified · a stable build with no critical blockers."], top=2.1, size=17, sa=9)
para(box(s, 0.83, 6.5, 11.6, 0.6), "Built online-ready — the full live game is the next phase.", 16, GREEN, bold=True, first=True)
pagenum(s, 10)

# 11 PRODUCTION PERIOD (timeline)
s = slide(); kicker(s, "Total Production Period"); title_only(s, "Phased — funded scope is 6 months.", 32)
tl = [("Now","Playable prototype already built (9 classes, element system, advancement, pet/trap, Desktop Companion)"),
      ("Months 1–6","Grant scope (this proposal): 10 classes (5+5), 20+ enemies, 3 bosses, VFX/SFX, balancing, private alpha + Steam page"),
      ("Phase 2","Online services, Steam Market backend, commercial launch (~6–12 months after)")]
x = 0.83
for m, t in tl:
    rect(s, x, 2.4, 3.8, 2.6, fill=NODE, line=ACCENT, line_w=2.0)
    tf = box(s, x+0.25, 2.6, 3.3, 2.3)
    para(tf, m, 18, ACCENT, bold=True, first=True, space_after=8)
    para(tf, t, 14, RGBColor(0xdb,0xe6,0xf3), line=1.3)
    x += 4.0
para(box(s, 0.83, 5.4, 11.6, 1.0),
     "With the ฿200,000 grant, the game becomes a playable offline product — not a project left hanging.", 18, SUB, first=True)
pagenum(s, 11)

# 12 ENGINE
s = slide(); kicker(s, "Engine · Architecture"); title_only(s, "A predictable, cheat-resistant core — our technical edge.", 30)
bullets(s, ["Predictable engine — the whole game runs on a custom core where the same actions always give the exact same result.",
            "Logic separated from graphics — the visual layer only shows the game, it can't change it (no room for client-side cheats).",
            "Every save is a replay — a recording of the player's actions a server can re-run and verify exactly → cheating is detectable.",
            "Online-ready from day one — built to plug into live online services later."]); pagenum(s, 12)

# 13 COMPETITORS
s = slide(); kicker(s, "Competitors"); title_only(s, "A rival proved demand — and the gap.", 32)
bullets(s, ["TBH: Task Bar Hero — recently validated strong demand for taskbar-idle games (reportedly a top Steam launch).",
            "Legend of Idleon — a true idle MMO on Steam (multi-character progression) proving the PC/Steam idle audience.",
            "Melvor Idle — proved deep idle-RPG demand (a genre hit).",
            "Our edge: deeper play (team-composition, elemental depth) + a verifiable core for fair play + an economy built to last."]); pagenum(s, 13)

# 14 MARKETS
s = slide(); kicker(s, "Markets & Target Audience"); title_only(s, "Idle isn't niche — it tops Steam.", 32)
para(box(s, 0.83, 2.1, 5.6, 1.4), "$13–14B", 44, ACCENT, bold=True, first=True)
para(box(s, 0.83, 3.0, 5.6, 1.0), "global idle-games market (2025) → ~$25–35B by 2034 (~9–10% CAGR)", 14, SUB, first=True, line=1.25)
para(box(s, 6.7, 2.1, 5.6, 1.4), "~130M", 44, ACCENT, bold=True, first=True)
para(box(s, 6.7, 3.0, 5.6, 1.0), "Steam monthly active users; Ragnarok community in the tens of millions", 14, SUB, first=True, line=1.25)
bullets(s, ["Primary target: global PC / Steam idle players — working adults 20–40 who want RPG depth in limited play time.",
            "Secondary target: Ragnarok & classic-MMORPG fans (SEA + global) chasing build depth without the grind.",
            "Go-to-market: creator/streamer-led (the Desktop Companion is built-in shareable content) + a Steam Market economy that drives word-of-mouth & retention."], top=3.9, size=16, sa=7)
para(box(s, 0.83, 6.4, 11.6, 0.6),
     "Sources: idle-games market — DataIntelo ($14.2B→$34.8B, 10.5% CAGR) & MarketIntelo ($13.2B→$24.8B, 8.7% CAGR), 2025; Steam metrics — Valve published figures.",
     12, RGBColor(0x6f,0x86,0xa8), first=True); pagenum(s, 14)

# 15 PRODUCTION BREAKDOWN
s = slide(); kicker(s, "Production Breakdown · 6 Months"); title_only(s, "From prototype to a complete core.", 32)
pb = [("M1–2","Core polish · 10 classes · enemy-skill system"),
      ("M3–4","20+ enemies · 3 bosses · level path & balance"),
      ("M5","VFX/SFX · UI/UX · in-house + private alpha"),
      ("M6","Offline core done · Steam page · wishlist")]
x = 0.83
for m, t in pb:
    rect(s, x, 2.5, 2.85, 2.5, fill=NODE, line=ACCENT, line_w=2.0)
    tf = box(s, x+0.2, 2.7, 2.5, 2.2)
    para(tf, m, 17, ACCENT, bold=True, first=True, space_after=8)
    para(tf, t, 13, RGBColor(0xdb,0xe6,0xf3), line=1.3)
    x += 3.0
para(box(s, 0.83, 5.5, 11.6, 0.8),
     "Developer-built engineering throughout; art & audio outsourced to local creators.", 16, GREEN, bold=True, first=True)
pagenum(s, 15)

# 16 BUDGET BREAKDOWN (table)
s = slide(); kicker(s, "Budget Breakdown"); title_only(s, "฿200,000 — where it goes.", 32)
data = [("Item","THB"),
        ("Art — local artists: ~10 class sets, 20+ monsters, 3 bosses, UI & VFX sprites","150,000"),
        ("Audio — local SFX & music production","20,000"),
        ("Software / dev tools — AI coding-assist & licenses (dev productivity, not asset generation)","20,000"),
        ("Steam Direct fee + miscellaneous","10,000"),
        ("Total","200,000")]
rowsn = len(data)
tbl_sh = s.shapes.add_table(rowsn, 2, Inches(0.83), Inches(2.2), Inches(11.6), Inches(3.4))
tbl = tbl_sh.table
tbl.columns[0].width = Inches(9.6); tbl.columns[1].width = Inches(2.0)
for ri,(a,b) in enumerate(data):
    for ci,val in enumerate((a,b)):
        c = tbl.cell(ri,ci)
        c.fill.solid(); c.fill.fore_color.rgb = NODE if (ri==0 or ri==rowsn-1) else BG
        c.margin_left=Inches(0.12); c.margin_top=Inches(0.06); c.margin_bottom=Inches(0.06)
        c.vertical_anchor = MSO_ANCHOR.MIDDLE
        p = c.text_frame.paragraphs[0]; p.alignment = PP_ALIGN.RIGHT if ci==1 else PP_ALIGN.LEFT
        r = p.add_run(); r.text = val
        r.font.size = Pt(15); r.font.name="Arial"
        r.font.bold = (ri==0 or ri==rowsn-1)
        r.font.color.rgb = (ACCENT if ri==0 else WHITE)
para(box(s, 0.83, 6.4, 11.6, 0.6),
     "85% (฿170K) funds local art & audio hiring; software-engineering labor is self-funded.", 15, GREEN, bold=True, first=True)
pagenum(s, 16)

# 17 SUMMARY
s = slide(RGBColor(0x16,0x3a,0x2a)); kicker(s, "Summary"); title_only(s, "Why ROI: Reign of Idle wins.", 32)
bullets(s, ["Proven market (idle tops Steam) + a clear gap rivals left open.",
            "Differentiated — deep team-composition + Desktop Companion + a real, multi-stream F2P live-service.",
            "De-risked — working prototype + a shipped, credible team.",
            "Scalable — F2P live-service + original IP for the global market.",
            "The ask: ฿200,000 to finish the online-ready core in 6 months."], top=2.1, size=18, sa=9)
para(box(s, 0.83, 6.5, 11.6, 0.6), "ROI: Reign of Idle — build your party, leave it running.", 17, GREEN, bold=True, first=True)
pagenum(s, 17)

prs.save("docs/depa-pitch-deck.pptx")
print("saved docs/depa-pitch-deck.pptx —", len(prs.slides.__iter__.__self__._sldIdLst), "slides")
