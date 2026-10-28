# depa Digital Startup S1 (Game) — Application Working Doc

> Round 2: **16–30 June 2026** (BE 2569). Grant: **฿200,000** (เงินให้เปล่า),
> individual Thai national, project ≤ 6 months. Apply via LINE OA
> `@depathailand` → become depa Member → fill form → submit **Pitch Deck
> (5-min, EN)** + **Video Pitching (2-min & 5-min, EN, founder presents)**.
> Authoritative criteria/disbursement: confirm in the LINE OA คู่มือ.

Working language for judges = **English**. Thai notes for you are in
`> blockquotes`. Numbers/scope are a strong first draft — tune to taste.

---

## ▶ START HERE — status & next actions  *(updated 2026-06-19)*

**Ready in this doc (decided / drafted):**
- ✅ Name + branding: **RoI: Reign of Idle** — *Idle Party RPG* (Steam-free)
- ✅ Tagline: *"Reign of Idle — your party never rests."*
- ✅ Pitch deck — 7-slide outline (§1) + credibility framing (§1.5)
- ✅ Video scripts — **2-min** & **5-min**, word-for-word (§2)
- ✅ 6-month project scope + deliverables (§3) · Budget ฿200,000 (§4)
- ✅ Positioning locked: single-player + **async multiplayer** (co-op
  dungeons + marketplace), NOT an MMO; server-light = solo-feasible

**You still need to DO (the actual submission deliverables):**
- [ ] Fill in `[name]` + a one-line founder background (search the doc for `[name]`)
- [ ] Polish the game to **demo-ready** (ask Claude for the "punch list")
- [ ] Record **30–60s gameplay footage** (auto-battle + open Skill Tree)
- [ ] Record **Video Pitch** — do the 5-min, then cut a 2-min (§2)
- [ ] Build the **7-slide deck** from §1
- [ ] LINE OA `@depathailand` → register as **depa Member** → get the
      official **form + คู่มือ** (confirm exact docs / scoring / disbursement)
- [ ] Fill the form + scope (§3) + budget (§4); pay the ฿30–50 fee
- [ ] **Submit before 30 June 2026**

**Timeline (deadline 30 Jun · you review Fri 26 Jun):**
| When | Do |
|---|---|
| now → ~25 Jun | polish game to demo-ready (also = playtest progress) |
| **Fri 26 Jun** | record video + gameplay footage; build the deck |
| 27–29 Jun | fill the depa form; final check |
| 30 Jun | **submit** (don't wait for the last hour) |

Related: `anti-cheat-economy-strategy-v2.md` (long-term economy plan — out of
this grant's scope, but it's the moat story you can reference in the pitch).

---

## 0. One-liner (memorize this)
**"An idle, auto-battling party RPG with the depth of a classic
Ragnarok-style class system — built on a deterministic, replayable engine
so it can grow from single-player into a fair, player-driven trading
economy."**

> **Positioning — say this, not that.** This is a **single-player RPG with
> async multiplayer** — NOT an MMO / shared real-time world. The multiplayer
> is two idle-native systems:
> 1. **Async co-op dungeons:** players queue; the server forms a party and
>    runs **one authoritative, deterministic simulation** of the shared run;
>    each player gets a **result + watchable replay**. No real-time presence
>    needed.
> 2. **A shared player-driven marketplace** (async item trading).
>
> *Never say "MMO/MMORPG"* (reads as a solo dev over-promising). Say
> "async multiplayer," "co-op dungeons," "player-driven marketplace." This
> framing is the *reason* it's solo-feasible and server-light — lean on it.

---

## 1. Pitch Deck (5-min, EN) — 7 slides

### Slide 1 — Title
- **RoI: Reign of Idle** — *Idle Party RPG*
  (brand mark = **RoI**; store/search name = the full **Reign of Idle**)
- Tagline: **"Reign of Idle — your party never rests."**
- One-liner (above). Your name, "Solo developer, Thailand". Date.
> Name locked: **Reign of Idle**. Hooks — *Reign Of* = an **RO** wink
> (Ragnarok-nostalgia, our target audience), *Reign* = power fantasy,
> *Idle* = clear genre + store search. (RoI = a quiet easter egg; the loud
> "Return on Investment" version belongs to the tycoon game *Rise of
> Industry* — different genre, so it's only a wink, not the headline.)
> Steam: the full title "Reign of Idle" is currently unused.

### Slide 2 — Problem / Opportunity
- Idle/auto-battler games are huge and growing (low-friction, play-anywhere),
  **but most are shallow** — number-go-up with thin combat.
- Classic MMORPGs (Ragnarok Online) have **deep, beloved class/skill systems**
  — but demand hours of active grinding.
- **Gap:** an idle game with *real* RPG class depth. Especially resonant in
  **Thailand & SEA**, where Ragnarok is a nostalgia powerhouse.

### Slide 3 — The Game (core loop)
- Build a **party of up to 5 RO-style classes** (Swordsman, Mage, Archer,
  Thief, Acolyte) that **auto-farm, auto-cast skills, collect loot, and clear
  dungeon bosses** — even while you're away.
- Depth comes from **builds**: a learnable **skill tree** per class, stat
  allocation, gear + refine, element matchups — not twitch input.
- Core loop: *auto-farm → loot & XP → level up → spend skill/stat points →
  stronger party → harder zone/boss → repeat.*
> โชว์ GIF/คลิป prototype จริงตรงนี้ — auto-battle + เปิดหน้า Skill Tree.

### Slide 4 — What makes it different (moat)
- **RO-grade systems in idle form:** iRO Renewal-accurate ASPD/combat
  formulas, per-class skill trees, transient buff/debuff engine (DoT, slows,
  party auras), weapon-type-gated masteries.
- **Deterministic, event-sourced core (Rust):** every battle is reproducible
  from a seed + inputs. This is the foundation for **anti-cheat and a fair
  player-driven marketplace later** — a defensible technical edge most idle
  games lack.
- **Async co-op dungeons — a novel "idle × multiplayer" technique:** players
  **queue**, the server runs **one deterministic party simulation** of the
  shared run, then everyone gets the **result + a watchable replay**. Real
  co-op **without real-time co-presence** (idle-friendly), **one sim per run**
  (server-light), and **server-authoritative** (cheat-proof). This is *only*
  possible because the engine is deterministic — it's the differentiator.
- **Offline-first, server-light by design** → realistic for a solo dev (see
  §1.5): no always-on game servers; near-zero infra cost.
- Clean split: **Rust sim-core** (rules) + **Godot client** (presentation).

### Slide 5 — Traction (proof it's real)
- **Playable single-player prototype today**: 5 classes, full skill trees,
  auto-battle, loot, bosses, party.
- **64 commits in 5 days** of focused build; tested + deterministic
  (golden state-hash regression tests).
- Living design docs (GDD, combat formulas, skill catalog) + roadmap.
> ใส่ screenshot ของเกม + กราฟ commit / git log เป็นหลักฐาน.

### Slide 6 — Market & plan
- **Target:** Thai/SEA idle-RPG & Ragnarok-nostalgia players; mobile + PC.
- **Phase plan:** SP prototype → **playtest core loop** → soft launch →
  (later) the **async multiplayer layer**: **co-op dungeons** (queue → server
  runs the deterministic party run → replay/result) + a **shared player
  marketplace** + cosmetics. *Not a real-time MMO — players keep their own
  instances; they team up via queued runs and trade in a common market.*
- **This grant funds:** turning the prototype into a **polished, playtestable
  vertical slice** (the validation gate before scaling).

### Slide 7 — The Ask & Team
- **Ask:** ฿200,000 depa S1 grant for a **6-month vertical-slice project**.
- **Use:** art/audio/tools/playtest (see budget). **No servers** — the
  funded scope is single-player & offline, so the grant goes entirely into
  the game, not infrastructure.
- **Team:** [You] — solo developer; full-stack (Rust engine + Godot client +
  game design). Background: [1 line].
- Contact + thank you.

---

## 1.5 Credibility — why a solo dev can deliver this (and won't blow up on server cost)
> ใส่จุดนี้ใน deck/วิดีโอ (สั้นๆ) — กันกรรมการคิดว่า solo dev "โม้ทำ MMO".
> สาระคือ **scope วินัย + architecture ที่จงใจให้ถูก**.

- **The grant project is single-player & offline.** The 6-month deliverable
  is a polished SP vertical slice — **it needs no game servers at all**, so
  there is **฿0 server cost** during the project and the whole grant goes
  into the game itself. This is deliberately scoped, not hand-wavy.
- **The async multiplayer layer (co-op dungeons + player marketplace) is the
  long-term vision — explicitly OUT of this grant's scope**, and a
  *separately funded* later stage. We're not asking ฿200k to build an MMO;
  we're proving the single-player core loop first.
- **Even the future online stage is server-light by design.** Because the
  sim is **deterministic + event-sourced**, players run their own game
  **client-side**; the server only **replays events on-demand** to validate
  (anti-cheat / settle economy trades) — it does **NOT** mirror every
  player's simulation in real time. That is the cost difference between a
  classic always-on MMO (expensive, team-scale) and this model (cheap,
  solo-feasible). *(See `docs/anti-cheat-and-server-model.md`.)*
- **Genre fit:** single-player **idle RPGs are a proven solo/tiny-team
  shippable genre** (e.g. Melvor Idle, IdleOn) — not a 100-person MMO. The
  scope matches what one focused developer can finish and polish.
- **Reuse over reinvention:** Godot (free engine) for the client + a small,
  tested Rust core — no bespoke netcode/engine to build for this slice.

**One-line version for the pitch:** *"This grant funds a single-player
slice — zero server cost. And because the engine is deterministic and
replay-based, even the future player marketplace stays cheap to run, which is
exactly why one developer can realistically build toward it."*

---

## 2. Video Pitch — scripts (EN, founder on camera + screen recording)

### 2-min version (~280 words)
> พูดช้าๆ ชัดๆ ~140 wpm. โชว์หน้าจอเกมจริงระหว่างพูด.

"Hi, I'm **[name]**, a solo game developer from Thailand. I'm building
**Reign of Idle** — an **idle party RPG** with the depth of a classic
Ragnarok-style class system.

Most idle games are shallow: you just watch numbers grow. And classic
MMORPGs have amazing class depth, but they demand hours of active grinding.
I'm closing that gap — a game you can leave running, where the real fun is
**building your party**.

[Screen: auto-battle] You assemble up to five RO-inspired classes —
Swordsman, Mage, Archer, Thief, Acolyte. They **auto-farm, auto-cast their
skills, collect loot, and clear bosses** on their own. [Screen: skill tree]
The depth is in the build: each class has a **learnable skill tree**, stats,
gear, and element matchups.

Under the hood it runs on a **deterministic Rust engine** — every battle is
reproducible from a seed. That's my technical edge: it's the foundation for
**anti-cheat and a fair player-driven marketplace** down the road.

This is real: I already have a **playable prototype** with five classes and
full skill trees, built and tested over an intense sprint.

And I'm scoping it honestly. **This isn't an MMO** — every player runs their
own game. The multiplayer comes later and it's idle-native: players **queue
for co-op dungeons that the server runs once and replays back to them**, plus
a **shared marketplace**. This grant funds a **single-player slice with zero
server cost**, and because the engine is deterministic and replay-based, even
that multiplayer stays cheap to run — which is why one developer can
realistically build it.

With depa's support, I'll turn this into a **polished, playtestable slice**
over six months — the milestone that proves the core loop is fun before I
scale it. Thank you."

### 5-min version (~650 words, word-for-word)
> `[SCREEN: …]` = roll the screen recording; otherwise you're on camera.
> ~140 wpm, calm and clear. Maps to beats: hook · problem · demo · moat ·
> traction · plan · ask.

"Hi, I'm **[name]**, a solo game developer from Thailand, and I'm building
**Reign of Idle** — an idle party RPG with the depth of a classic
Ragnarok-style class system. My one line for it: *a game you can leave
running, where the real fun is building your party* — on an engine that lets
it grow, fairly and cheaply, into something multiplayer.

Let me tell you why I'm making it. Idle games are everywhere — easy to pick
up, play anywhere — but most are shallow: you just watch numbers go up. On
the other side, classic MMORPGs like Ragnarok Online have deep, beloved
class and skill systems — but they demand hours of active grinding every
day. There's a gap in the middle: an idle game with *real* RPG depth. And
it's timely here — in Thailand and Southeast Asia, Ragnarok is a nostalgia
powerhouse; millions of us grew up on those classes.

[SCREEN: auto-battle] So here's the game. You build a party of up to five
Ragnarok-inspired classes — Swordsman, Mage, Archer, Thief, and Acolyte.
They auto-farm, auto-cast their skills, collect loot, and clear dungeon
bosses on their own — even while you're away. [SCREEN: loot + level-up] The
core loop is simple and satisfying: your party farms, loot and experience
drop, they level up — [SCREEN: open Skill Tree, learn/upgrade a skill] and
then *you* make the real decisions. Every class has a learnable skill tree.
You spend points to learn and raise skills, allocate stats, equip gear — and
your party gets stronger, so you push into harder zones and tougher bosses.
The depth is in the build, not in twitch reflexes.

Now, what makes this defensible. First, the systems are real:
Ragnarok-accurate combat — the actual attack-speed and element formulas —
per-class skill trees, and a buff-and-debuff engine with damage-over-time
and party auras. Second, my technical edge: the whole simulation runs on a
**deterministic, event-sourced engine in Rust** — every battle is
reproducible from a seed. That's the foundation for anti-cheat and a fair
player economy later. And it makes the multiplayer realistic for one
developer. This is **not an MMO** — every player runs their own game. When
they want to team up, they **queue for a co-op dungeon**, and the server
runs *one* deterministic simulation of that shared run, then sends everyone
the result and a replay. Real co-op, no real-time servers, and it's
cheat-proof because the server is the authority. That's a multiplayer game
one person can actually build.

And this is real today. I already have a **playable prototype** — five
classes, full skill trees, auto-battle, loot, bosses, a party — built and
tested in an intense sprint, with deterministic regression tests so the
game's math can't silently break, plus living design docs and a roadmap.

My players are idle-RPG and Ragnarok-nostalgia fans in Thailand and
Southeast Asia, on PC and mobile. The plan: finish a polished single-player
slice, **playtest the core loop** with real players, soft-launch, and then
add the async multiplayer — co-op dungeons and a player marketplace. This
grant funds the most important step: turning the prototype into a polished,
playtestable slice — the moment that proves the loop is fun before I scale.

So that's my ask: a **200,000-baht depa S1 grant** for a six-month
vertical-slice project. It goes entirely into the game — art, audio, tools,
and a closed playtest — because the funded scope is single-player and
offline, with **zero server cost**. I'm **[name]**, I'm building **Reign of
Idle**, and I'd be honored to build it with depa's support. Thank you."

---

## 3. Project scope — 6 months (what you'll DELIVER)
> ทุน S1 = performance-based: ต้องส่งมอบตาม scope นี้ (ไม่ใช่แค่ roadmap,
> ไม่ใช่เกมจบสมบูรณ์). ตั้ง scope ที่ทำเสร็จได้จริง = **vertical slice**.

**Goal:** a polished, playtestable single-player vertical slice proving the
core loop is fun.

**Deliverables (proposed milestones):**
- **M1 (mo. 1–2):** Content slice — 3–4 zones + 2 bosses; all 5 classes
  playable with their skill trees; balance pass on combat/ASPD/skills.
- **M2 (mo. 3–4):** Game-feel polish — art pass (sprites/UI/icons), SFX +
  music, hit/skill feedback, save/offline-progress.
- **M3 (mo. 5):** Onboarding + UX — tutorial/first-session flow, skill-tree
  & inventory UI polish, settings.
- **M4 (mo. 6):** **Closed playtest** with N external testers + feedback
  report; demo build + trailer; post-mortem on the core loop.

**Final outputs:** playable demo build, gameplay trailer/clip, playtest
report, updated design docs. *(Online/economy/marketplace = explicitly OUT
of scope for this grant — that's the next stage.)*

---

## 4. Budget — ฿200,000 over 6 months (draft)
> ปรับสัดส่วนตามจริง + ตามที่ฟอร์ม depa ขอ (มักให้แจกแจงรายหมวด).

| Category | ฿ | Notes |
|---|---:|---|
| Art — sprites, UI, skill icons, environments | 80,000 | Biggest gap for a code-first solo dev; contract a 2D artist |
| Audio — music + SFX | 30,000 | Licensed pack or a contract composer |
| Tools / licenses / assets | 20,000 | Godot is free; asset packs, fonts, software, build/signing |
| Playtest — recruiting + incentives | 20,000 | External tester rewards, survey tools |
| Trailer / marketing assets | 25,000 | Demo trailer, store/press kit, screenshots |
| Contingency / misc | 25,000 | Buffer for overruns |
| **Total** | **200,000** | |

> ถ้าฟอร์มต้องการ matching fund / in-kind (แรงคุณเอง) ให้ใส่ "founder labor
> (in-kind)" เป็นสัดส่วนหลักของ R&D ด้วย.

---

## 5. Checklist (do now — ~13 days)
- [ ] Add LINE OA `@depathailand`; register as **depa Member**; get the
      **official S1 คู่มือ + form** (confirm exact criteria, documents,
      disbursement, any matching fund).
- [ ] Lock the **game name**.
- [ ] Record **30–60s of clean gameplay** (auto-battle + skill tree) for the
      deck + video.
- [ ] Finalize **Pitch Deck (5-min, EN)** from §1.
- [ ] Record **Video Pitch 2-min & 5-min (EN)** from §2 (founder on camera +
      screen demo).
- [ ] Fill the depa form + **6-month scope (§3) + budget (§4)**.
- [ ] Submit before **30 June 2026**.

> ข้อควรเช็คในคู่มือ: จ่ายเงินเป็นงวด(milestone)หรือก้อนเดียว · ต้องมี
> matching fund ไหม · เกณฑ์ให้คะแนน(น้ำหนักแต่ละด้าน) · เอกสารบังคับอื่นๆ
> (สำเนาบัตร ปชช., bank, ฯลฯ).
