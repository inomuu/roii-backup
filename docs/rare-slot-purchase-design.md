# Rare Slot Purchase — design (permanent capacity upgrade)

**Status:** วางแผน (ยังไม่ลงมือ). ต่อยอดจาก rare-loot bag ที่มีอยู่ (`state.rare_bag` / `rare_bag_cap` / `rare_threshold`).
**Companion:** `gear-systems-design.md`, `monetary-policy.md`, `character-skins-idea.md` (โมเดล cash entitlement),
`anti-cheat-and-server-model.md` (client-sim + server-verify), `steam-market-requirements.md`.

## 1. สิ่งที่ขาย

**อัปเกรดความจุ rare bag แบบถาวร** — ไม่ใช่ค่าไถ่รายชิ้น.

- base = **3 ช่อง** (`RARE_BAG_BASE_CAP`)
- ซื้อ ($2–3) → **+3 ถาวร = 6 ช่อง** ใช้ได้ตลอดไป (เป็น "ได้ค่าอัปเกรดจริง" ไม่ใช่จับของเป็นตัวประกัน)
- การ **hold 24 ชม.** เป็นแค่ **จังหวะ upsell** ตอน overflow — ไม่ใช่ตัวสินค้า

## 2. Flow

```
rare drop (rarity ≥ rare_threshold) + rare bag เต็ม
  → ไม่ auto-sell   (เดิม: overflow_recover ขาย/หาย)
  → เข้า pending_rare (สูงสุด N ชิ้น) + server เริ่มจับเวลา 24 ชม.
  → UI เด้ง upsell: "ได้ <item> แต่ช่อง rare เต็ม — อัปเป็น 6 ช่อง $X"
      ├─ ซื้อ  → server ยืนยัน entitlement → cap 3→6 → pending ไหลเข้า bag → เคลมได้ปกติ
      └─ ครบ 24 ชม. ไม่ซื้อ → server สั่ง expire → ของหลุด (auto-sell คืน Gil / หาย — ดู §6)
```

pending item **เห็นได้แต่เคลมไม่ได้** จนกว่าจะซื้อ (cap ขยายแล้ว pending ค่อย drain เข้า bag).

## 3. Sim ↔ Server split (จุดสำคัญสุด)

sim เป็น **integer-only + wall-clock-free** — 24 ชม. จริง นับใน sim ไม่ได้ (พัง determinism + golden).
แบ่งความรับผิดชอบให้ตรงเส้น client-sim / server-authority:

| ส่วน | อยู่ที่ | เหตุผล |
|---|---|---|
| roll rare drop | **sim** (deterministic) | draw order เดิม ไม่แตะ |
| routing "bag เต็ม → pending_rare" | **sim** (branch ใน `bank_to_inventory_inner`) | เป็น code path ล้วน; item identity ต้อง deterministic (verify ได้) |
| `pending_rare: Vec<Item>` | **sim state** (serialized, ไม่ skip) | ของถูก roll ฝั่ง client แล้ว — ต้องอยู่ใน save ให้ replay/verify เห็น |
| นับ 24 ชม. + entitlement ($) + expiry | **server** | wall-clock + เงินจริง = server-authoritative เท่านั้น |
| ขยาย cap / เคลม / expire | **server-authored InputEvent** ลง log | apply ลง save → replay ยัง consistent (แบบเดียวกับ mail-grant) |

**หลัก:** item เกิด+อยู่ใน sim (deterministic), แต่ **timer/เงิน/การปลดล็อก = server** ยิง event เข้ามา.

## 4. State + events ใหม่

sim-core (bump VERSION + re-bless goldens):
- `GameState.pending_rare: Vec<Item>` — rare ที่ค้างเพราะ bag เต็ม (serialize, skip-if-empty).
- `RARE_BAG_PURCHASED_CAP = 6` (หรือ field `rare_bag_cap` ที่ entitlement เขียน) — `rare_bag_cap()` คืน max(base, purchased).
- `PENDING_RARE_CAP` — เพดาน pending พร้อมกัน (กันบวมถ้าไม่ซื้อ/ไม่หมดเวลา).

InputEvents (authority):
- `RaiseRareBagCap { to: u32 }` — **Server** authority (entitlement เงินจริง). handler: ตั้ง cap + drain `pending_rare` → `rare_bag` จนเต็ม cap ใหม่.
- `ExpirePendingRare { idx: u32 }` — **Server** authority. handler: เอา pending[idx] ออก → overflow policy (auto-sell/หาย).
- (มีแล้ว) `MoveRareToInventory` ใช้เคลมจาก `rare_bag` ตามปกติ **หลัง** pending drain เข้า bag.

## 5. Reconcile กับของเดิม

- `rare_bag_cap` เป็น field upgradeable อยู่แล้วแต่ **ยังไม่มี event อัป** — feature นี้คือ event แรกที่อัปมัน (ทาง cash).
- ถ้าอนาคตมี track **Gil/soul** อัป cap ด้วย → ต้อง reconcile 2 แหล่ง (cash-track vs soft-track). ตอนนี้กำหนด **ช่อง +3 นี้ = cash-only**.
- routing เดิม (§2 ของ drops.rs) แทรก step ใหม่ **ก่อน** `overflow_recover`: `if rare && bag เต็ม && pending ไม่เต็ม → push pending; return`.

## 6. Economy + monetization

- ช่อง = **pure cash sink** ไม่ mint Gil → ผ่านเทสต์ monetary-policy ("เพิ่ม Gil/hr ไหม" = ไม่). ✅
- expire แล้ว **auto-sell คืน Gil บางส่วน** (นุ่ม, ได้อะไรกลับ) หรือ **หายเลย** (FOMO แรง) — **ต้องเลือก**. แนะนำคืน Gil: ลดความรู้สึก exploit โดยไม่ทำลาย FOMO (ยังเสียของ rare จริง).
- pattern "hold 24 ชม." อยู่ฝั่ง aggressive — ทำให้ช่อง**ถาวร +3** + expire คืน Gil = ได้ค่าอัปเกรดจริง ไม่ใช่ค่าไถ่.

## 7. Open decisions

1. expire → auto-sell คืน Gil (แนะนำ) หรือ หายเลย?
2. ซื้อได้กี่ครั้ง — ครั้งเดียว 3→6 จบ? หรือมี tier ต่อ (6→9…) ?
3. `PENDING_RARE_CAP` เท่าไหร่ (pending ค้างพร้อมกันได้กี่ชิ้นก่อนตัวเก่าสุดเริ่มหมดเวลา/หลุด)?
4. server จับเวลาเริ่มนับตอนไหน — ตอน drop (save-push แจ้ง) หรือตอน player เห็น prompt?
5. ราคาจริง + ผูกกับ Steam entitlement ยังไง (ตาม steam-market-requirements: released game, authenticator).

## 8. ทำเมื่อไหร่

หลังมี **server จริง + Steam auth** (ตาม production-server-plan) — เป็นฟีเจอร์เงินจริง = "second-launch weapon" ไม่ใช่ day-1.
โค้ด sim ฝั่ง `pending_rare` + routing branch ทำล่วงหน้าได้ (deterministic, ไม่ต้องรอ server); ส่วน entitlement/timer รอ server.
