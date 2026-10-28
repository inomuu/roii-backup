# Recruit → Gacha only — แผน retire (2026-09-17)

> user: "recruit เราว่าจะยกเลิกแล้ว" / "ตัวละครเข้าทาง gacha อย่างเดียว" · สถานะ: **แผน ยังไม่ทำ** — รอเคาะ §4

## 1. ที่มีอยู่

| ทาง | event / fn | จ่าย | ปลายทาง |
|---|---|---|---|
| Recruit ลงสนาม | `RecruitMember` → `recruit_member` | Gil `recruit_cost` (100k…20M) + level gate Lv12/18/… | เข้าปาร์ตี้ทันที |
| ซื้อเข้า roster | `BuyCharacter` → `buy_character` | Gil `buy_character_cost` (0/1k…200k — **ยังไม่ ×100**) | benched |
| **Gacha** | server pull → mail → `claim_mail` → `try_grant_item(exemplar_*)` → **`grant_exemplar_unit`** | Crystal (server) | benched Lv1 + Exemplar 0★; dupe → `pending_resonance` |
| Deploy/Bench | `DeployCharacter` / `BenchCharacter` | ฟรี | roster ↔ ปาร์ตี้ (cap 5) |

⇒ ทาง gacha สมบูรณ์อยู่แล้ว (`grant_exemplar_unit` = "the ONE acquisition primitive") — งานคือ**ปิดสองทางที่จ่าย Gil** และเปลี่ยนโมเดล sim-cli

## 2. ที่จะเปลี่ยน (sim)

1. `RecruitMember` / `BuyCharacter` → **silent no-op** (คง variant ไว้ให้ replay เก่า parse — แบบเดียวกับ Sell ใน 0.0.411) · ลบ `recruit_cost` / `recruit_level_req` / `buy_character_cost` + `GilSpend::RecruitField/RecruitRoster` (คง enum variant ไว้ อ่านได้ว่าเคยมี)
2. `recruit_member` เหลือเป็น `pub(crate)` ให้ **sim-cli/dev** ใช้เท่านั้น (ไม่มี event ถึง) — หรือย้ายไปเป็น `grant_exemplar_unit` + deploy ทั้งหมด
3. Story sandbox (`StoryDriver.BuildRoster` ใช้ `BuyCharacter` สร้าง cast) → เปลี่ยนเป็น debug command `give_unit` (sandbox เท่านั้น เซฟไม่ได้อยู่แล้ว) — story เลื่อนอยู่ ทำเมื่อกลับมาแตะ
4. Unity `PartyWindow` ปุ่มซื้อ class ออก · หน้าต่างเหลือ roster (benched จาก gacha) + Deploy/Bench

## 3. โมเดล sim-cli — `auto_gacha` แทน `auto_recruit`

pacing ทุกตัวเลขวันนี้ตั้งบน "ปาร์ตี้ครบ 5 ที่ ~ชม. 12" — ต้องมีตารางว่าผู้เล่น F2P ได้ตัวที่ 2-5 เมื่อไหร่ อิง `gacha-economy-design.md`: F2P 350 crystal/วัน = **1.17 pull/วัน** · SSR 4% flat · SR = ชั้น "accessible" (อัตราต้องเคาะ) · dupe ไม่ให้ตัวใหม่ (Resonance)

ข้อเสนอ (ค่าเริ่ม, ทุกอย่างเป็น data ไม่ใช่ RNG ใน sim-cli — ให้วัดซ้ำได้):

| สมาชิก | มาจาก | เวลา (F2P) |
|---|---|---|
| 1 Seraphine | ให้ฟรีตอนเริ่ม | 0h |
| 2 | **tutorial 10-pull** (เควสต์แรก 3,000 crystal) — SR การันตี 1 ตัว | ~0.5h |
| 3 | 10-pull เดียวกัน (SR ตัวที่ 2 จาก 10 pull ที่ SR rate สูง) | ~0.5h |
| 4 | crystal วัน 1-3 (≈4 pull) — เจอ SR ใหม่ที่ไม่ dupe | ~วัน 3 |
| 5 | crystal ถึง ~วัน 7 (≈8 pull) | ~วัน 7 |

sim-cli: `--party-schedule 0,0.5,0.5,72,168` (ชั่วโมงที่สมาชิก 1-5 เข้า, ค่าเริ่มตามตาราง) → `grant_exemplar_unit` + `deploy` เมื่อถึงเวลา; order ของ unit ตาม `--recruit` เดิม · ไม่มี Gil top-up อีก (recruit เคยเติม Gil ให้ — ตัวเลข `gil_earned` จะสะอาดขึ้น)

ผลที่คาด: ต้นเกมช้าลง (เดี่ยว/คู่ 3 วัน) ⇒ ต้องวัด 7 วันใหม่หลังเปลี่ยน — ตาราง J/K/XP อาจต้องขยับตาม

## 4. ต้องเคาะก่อนทำ

1. ปาร์ตี้เริ่ม = Seraphine คนเดียว หรือ Seraphine + SR ฟรี 1 ตัว
2. tutorial 10-pull มีไหม (3,000 crystal ครั้งเดียว) + การันตี SR
3. SR rate (ยังไม่มีในเอกสาร gacha — มีแต่ SSR 4%)
4. ตัวจาก gacha เข้ามาที่ Lv1 — มี catch-up (XP refund / ระดับปาร์ตี้) ไหม ไม่งั้นตัวที่ 5 วัน 7 เป็น Lv1 ในด่าน 100
5. `buy_character_cost` ยังไม่ ×100 — ถ้า retire ก็ไม่ต้องแตะ
