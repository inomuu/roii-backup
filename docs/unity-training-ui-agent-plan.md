# Training floating UI — implementation handoff

วันที่ 2026-09-17 · แผนและ handoff สำหรับ agent รับงานต่อ

## สถานะ implementation รอบนี้

ลงโค้ด MVP ตาม Phase A–C ใน worktree ปัจจุบันแล้ว: `TrainingModel`, `TrainingPanel`, `UiNumber`, ปุ่มล่างสามช่อง, verified `Train` event, loc และ `-roitrainingsmoke` probe ถูกเพิ่มแล้ว การตรวจที่ผ่านคือ Unity batch compile แบบ isolated (ไม่มี `error CS`), `cargo test -p sim-core training` (4 ผ่าน), `check_loc_keys.py` และ CSV สองชุดตรงกัน

สิ่งที่ยังต้องทำใน Unity Editor/native standalone: เปิด smoke probe จริงในทั้ง Battle/Companion, ตรวจภาพ/การ scroll/การแสดงเลขยาวบนเครื่อง, และให้เจ้าของ master stage/commit หลังตรวจ diff เพราะ session นี้ไม่มีสิทธิ์เขียน git index หรือสร้าง worktree ref

ตรวจโค้ดบน master `e5db2c9b` (sim 0.0.415) แล้ว แต่ตอนเริ่มงานให้ใช้ master ล่าสุดที่ commit แล้ว ไม่ pin กลับมารุ่นนี้ งาน Unity เท่านั้น; Godot เลิกพัฒนาแล้ว

## 1. เป้าหมายและขอบเขตที่ตกลง

- เพิ่ม **Training ที่แถวปุ่มล่างของหน้าตัวละคร + กระเป๋า แถวเดียวกับ Blacksmith** ไม่เพิ่มแถบบนหรือหน้าจอเต็ม
- กดแล้วเปิดแผงรองทางขวาเหมือน Forge/Socket; ใช้ได้ทั้งหน้า Battle และ floating UI ของ Companion
- ฝึก 23 ช่อง ใช้ Gil ตาม host; อัปเกรดถาวรใช้ร่วมทั้งปาร์ตี้ ไม่แยกตามตัวละครที่กำลังเลือก
- รองรับยอดเงินจำนวนมากตั้งแต่ parser → model → affordability → ข้อความ โดยไม่เสียหลักตัวเลข
- งานรอบแรกคือ Phase A–C ด้านล่าง พร้อมหลักฐานว่าใช้งานจริง ไม่เปลี่ยน balance, backend หรือสูตร training
- คงสไตล์สุขุมของ UI ปัจจุบันที่อิง WuWa; TBH/WoG เป็นเรฟการจัดวางและพฤติกรรมหน้าต่าง ไม่ต้อง gen art ใหม่เพื่อให้ MVP เสร็จ

อ่านก่อนเริ่ม: `CLAUDE.md`, `TODO.md` ส่วนที่เกี่ยวข้อง, `docs/ui-handoff-2026-09-17.md`, `docs/training-system-plan.md`, `docs/unity-floating-forge-implementation.md`, `docs/testing-notes.md` และ `docs/gem-system-design.md` §8 หากแตะ Forge/Socket

เมื่อเอกสารเก่าขัดกับโค้ด ให้ยึด host ปัจจุบัน: Training 23 ช่อง, ราคาผ่าน Gil ×100 แล้ว, ไม่มี NPC Sell, starter ใหม่ Seraphine คนเดียว อย่าสร้าง fixture ที่คาดว่ามี Rapis ตั้งแต่ต้น

## 2. เตรียมงานโดยไม่ชน session อื่น

1. ตรวจ `git status`, `git worktree list`, commit ล่าสุดของ master และ instructions ของ worktree ก่อนแก้ไฟล์
2. แยก worktree/branch เช่น `codex/training-floating-ui` จาก master ล่าสุด ห้ามสลับ branch ของ `/Users/lunalieneray/idle_mmorpg` ที่อีก session ใช้อยู่; ไม่เริ่มจาก gem-ui branch เก่าที่ merge ไปแล้ว
3. ใช้ Unity project ใน worktree ของตัวเอง พร้อม local test save แยก ตรวจ path ของ process ก่อนเปิด/ปิด Editor หรือ standalone
4. เตรียม catalog และ native plugin ให้ตรง checkout ตามสคริปต์ปัจจุบัน (`tools/unity_data.sh`, `tools/unity_dylib.sh`); restart เฉพาะ Editor ของตัวเองเพื่อโหลด dylib ใหม่ ห้าม overwrite binary ที่ process กำลังใช้อยู่ด้วย plain copy
5. ไม่ deploy, ไม่ล้างเซฟจริง, ไม่ทำ transaction บนบัญชีจริง และไม่ merge เข้า master ในงานนี้ ส่ง commit ให้เจ้าของ master รวมภายหลัง

การตรวจรอบวางแผนพบไฟล์ Godot `grass_roi.tres` แก้ค้างอยู่ ไม่ใช่งานนี้ อย่า revert/stage รวม

## 3. Layout และ navigation

```text
┌────── Character + Inventory ──────┐  ┌──────── Training ────────┐
│ ตัวละคร / ช่องสวมใส่ / กระเป๋า     │  │ Training             ×  │
│                                  │  │ ใช้ร่วมทั้งปาร์ตี้        │
│                                  │  │ Gil         1,234,567   │
│                                  │  │ เลเวลฝึกรวม         37  │
│                                  │  │ หลัก | Attunement | Ward│
│                                  │  │ Might          Lv 5/50 │
│                                  │  │ โบนัสรวม / เพิ่มต่อ Lv │
│                                  │  │ ต้องการรวม 50 (ตอนนี้37)│
│                                  │  │ [Train · 38,000 Gil]   │
│                                  │  │ ... scroll ...        │
├──────────┬──────────┬─────────────┤  │                       │
│Blacksmith│ Training │ Socket Gem │  │                       │
└──────────┴──────────┴─────────────┘  └───────────────────────┘
                  gameplay ยังเดินต่อด้านล่าง
```

- เริ่มจากขนาดเดิม: แผงหลัก 500×560, รอง 320×560, gap 10; ความกว้างรวม 830 หน่วย แบ่งแถวล่างเป็นสามปุ่มไม่ทับกัน ใช้ label Blacksmith ใน navigation โดยยังเปิด Fusion Forge เดิม
- Header อยู่กับที่; เฉพาะรายการ scroll แบ่งกลุ่ม **หลัก 11 / Attunement 6 / Ward 6** ด้วยปุ่มเลือกกลุ่ม เก็บตำแหน่ง scroll ของแต่ละกลุ่มเมื่อ refresh
- มุมขวาบนมี toggle **Upgradeable First**; เมื่อเปิดจะเรียงช่องที่กด Train ได้ขึ้นก่อนภายในกลุ่มเดิม โดยไม่เปลี่ยนลำดับข้อมูลหรือจำนวนช่อง
- แต่ละรายการเรียง ชื่อ + `Lv x/max` → โบนัสรวม → โบนัสต่อเลเวล → เหตุผลที่ยังซื้อไม่ได้ถ้ามี → ปุ่มฝึก ใช้พื้นที่แนวตั้งเพิ่มตามข้อความ ไม่บีบ 23 ช่องให้เห็นทั้งหมดพร้อมกัน
- ปุ่มฝึกราคาใหญ่มากแบ่งเป็นสองบรรทัดได้: `Train` แล้ว `18,446,744,073,709,551,615 Gil`; ห้ามตัดราคาด้วย `…` หรือย่อฟอนต์จนอ่านไม่ได้
- กด Training ซ้ำปิดแผงรอง; กดปุ่มอื่นเปลี่ยนแผงรอง มีแผงขวา active ได้หนึ่งอัน กากบาท Training ปิดเฉพาะแผงนี้
- รักษา Back/Esc เดิม: ยกเลิก drag → confirmation → result → full stats → แผงรอง → หน้าต่างหลัก คลิกนอกแผงตามพฤติกรรม modal เดิมและไม่ส่งคลิกทะลุ
- สลับแผง/ขนาดให้เปลี่ยนทันที ไม่ใช้ animation ยืดหน้าต่าง; clamp ทั้งกลุ่มในขอบจอ แจ้ง native companion layout ว่าขนาดเปลี่ยน
- Companion คงพื้นหลังโปร่งใส, UI อยู่เหนือ gameplay, game scale 1×–4× และ UI scale 1×/1.5×/2× ทำงานต่อ ถ้าจอเล็กให้ fit ตามระบบเดิมโดยปุ่มฝึกยังเข้าถึงได้
- Combat, damage numbers และ Auto Fusion ต้องทำงานต่อเมื่อเปิด Training หรือปิดหน้าต่างทั้งหมด การแสดงข้อความสำเร็จห้าม pause sim

## 4. Contract และสถานะที่ UI ต้องแสดง

แหล่งจริงคือ `crates/sim-host/src/session.rs` query **`training_rules`** และ `crates/sim-core/src/training.rs`

| ฟิลด์ | ใช้ทำอะไร |
|---|---|
| `slots[].id` | ส่ง Train และหา `TRAIN_<id>`; เก็บตัวเลือกด้วย id ไม่ใช้ลำดับแถว |
| `level`, `max_level` | `Lv x/max`; แต่ละช่อง cap ต่างกัน ห้าม hard-code 50 |
| `text`, `text_per_level` | โบนัสรวมและต่อระดับจาก host; รองรับ null ตามรายละเอียดด้านล่าง |
| `value_now`, `per_level`, `stat`, `element` | ชนิดข้อมูลสำหรับกรณีแสดงผลพิเศษ ไม่ใช้คำนวณผลการฝึก |
| `next_cost` | จำนวนเต็ม u64 หรือ null; ราคาซื้อจริง ไม่คำนวณใหม่จาก cost curve |
| `gate_open`, `gate_total_needed` | gate ของแถวนั้น; แสดง “ต้องการรวม {needed} (ตอนนี้ {total})” |
| `total_levels` | ยอดระดับรวมสำหรับ header และ gate ใช้ตัวนี้ ไม่แทนด้วย `levels_bought` |
| `stash_slots` | โบนัสความจุจาก Training ตาม host ไม่ใช่จำนวนช่องฐานทั้งหมด |

กลุ่มหลักตามลำดับ: `might`, `vitality`, `fortitude`, `precision`, `lethality`, `giant_slayer`, `sprint`, `acceleration`, `regeneration`, `shatter`, `satchel`; กลุ่มธาตุใช้ `attune_*` และ `ward_*` ทั้ง Neutral/Fire/Water/Thunder/Holy/Shadow โดย Neutral แสดง Physical ไม่เพิ่ม Swiftness ที่ยังไม่ได้ implement

**Null ที่ต้องจัดการจริง:** `text` ของช่อง stat ที่ Lv0 เป็น null (ยังไม่มีโบนัส); ให้มีข้อความ localized ว่ายังไม่มีโบนัสและยังเห็น `text_per_level` ส่วน Satchel ไม่มี StatMod จึงไม่มี text ทั้งสองค่า ให้แสดง `value_now`/`per_level` เป็นจำนวนช่องตามชนิด stat และตรวจความหมายกับ inventory capacity ของ host ไม่แปะ `%` ให้ทุกค่า

parser ต้องแยก `next_cost: null` ออกจากราคา 0 และแยก field หายออกจากข้อมูลปกติ ห้ามพึ่ง default numeric 0 จาก JsonUtility แล้วเปิดปุ่มซื้อฟรี ถ้า query เป็น `{}`, slot หาย, JSON ไม่สมบูรณ์ หรือ session ไม่มี ให้ disable action พร้อมข้อความ unavailable และ retry เมื่อข้อมูลพร้อม

สถานะปุ่มตามลำดับ: unavailable/pending → maxed → gate locked → Gil ไม่พอ → พร้อมฝึก เก็บเหตุผลให้อ่านได้แม้ปุ่ม disabled; capped slot ไม่แสดงราคาเก่า

### ส่ง action หนึ่งครั้งต่อคลิก

1. อ่าน snapshot/session ปัจจุบันก่อนส่ง ตรวจ id, availability, gate และ Gil แบบ integer เพื่อลดการกดที่ใช้ไม่ได้
2. ส่ง `{"Train":{"slot_id":"might"}}` ผ่าน `SimDriver` เดิม ไม่สร้าง game session คู่ขนานเพื่อใช้เงินจริง และไม่แก้ level/Gil ใน UI ล่วงหน้า
3. ใช้แนวทาง `ApplyVerifiedInputs(expectedSession, inputs, verifyResult)` ที่มีอยู่: ตรวจผล preflight ว่า level ของ slot เป้าหมายเพิ่มหนึ่งจริง และ replay ผ่าน live driver เดิม ค่า bool ที่รับ input ได้อย่างเดียวไม่พิสูจน์ว่าฝึกสำเร็จ เพราะ event อาจ no-op
4. Guard pending/re-entry จนทราบผล ไม่ queue หลายการซื้อจาก double-click ขณะรายการเดิมยังประมวลผล ไม่มี hold-to-repeat/Buy Max ใน MVP
5. หลังสำเร็จ refresh Gil, ทุกแถวที่ gate อาจเปลี่ยน, total levels, stat/capacity และเลเวลจาก host แสดง highlight/`+1` สั้นๆ ไม่ใช้ full-screen result หรือ confirmation ทุกครั้ง
6. ถ้า rejected/stale/unavailable ให้ refresh พร้อมเหตุผล ไม่มีเสียง/เอฟเฟกต์สำเร็จ ปิด/reset callback เก่าหากเปลี่ยน session หรือโหลดเซฟใหม่

Auto Fusion อาจเพิ่ม Gil ระหว่างเปิดหน้า และ action อื่นอาจใช้เงินไปก่อน จึงต้องตรวจจาก state ตอนส่งจริง ไม่ถือราคากับยอดตอนเปิดหน้าต่างเป็น transaction ที่รับรองแล้ว

## 5. ตัวเลขใหญ่: กติกาเดียวทั้งระบบที่แตะ

เพิ่ม helper กลาง เช่น `UiNumber` (ตรวจของเดิมก่อนสร้างซ้ำ) แยก `Exact` กับ `Compact` ชัดเจน

| ตำแหน่ง | รูปแบบ |
|---|---|
| ราคาฝึก / ค่าใช้จ่าย / tooltip / confirmation | เลขเต็มคั่นหลักแบบ `N0`: `1,234,567,890` |
| Forge payout ใน preview และผลจริงใน result | เลขเต็ม `+1,234,567,890 Gil`; wrap แยกบรรทัดได้ |
| Gil ในแผง Training | ให้เลขเต็มเป็นค่าเริ่มต้น เผื่อความกว้างและบรรทัดตั้งแต่แรก |
| wallet แบบแถบแคบ / damage number ในงานถัดไป | เลขย่อ K/M/B/T/Qa/Qi; hover **และ click** เปิดค่าเต็มได้สำหรับ wallet |

- C# เงิน/ราคาใช้ `ulong` ตาม Rust `u64` ตั้งแต่ deserialize; signed stat/damage ใช้ชนิด signed ของ contract นั้น ไม่ force ทุกค่าเป็น unsigned
- ห้ามแปลงเงินผ่าน `int`, `float`, `double` หรือ JSON parser ที่คืน number เป็น double; ต้องรักษาค่าเหนือ 2^53 แบบเป๊ะ
- ห้ามหาร Gil ด้วย 100: host ส่งเงินจริงหลัง rebalance แล้ว Formatting ต้องไม่เปลี่ยนมูลค่า
- Compact ใช้ integer/string หรือ decimal arithmetic ที่ไม่ overflow; สูงสุดสองตำแหน่งทศนิยม ตัดทศนิยมลงเพื่อไม่แสดงเงินมากกว่ามีจริง และเลือก suffix จากค่าจริง ตัวอย่าง `999,999 → 999.99K`, `1,000,000 → 1M`
- ตรวจ cap/suffix ถึง `ulong.MaxValue` (ประมาณ 18.44Qi); เงินพอหรือไม่เทียบค่าจริงเท่านั้น ห้ามเทียบเลขย่อ/string และห้าม multiply `ulong` จน overflow ระหว่าง format
- locale EN/TH ต้องอ่าน comma grouping และจุดทศนิยมได้แน่นอน ใช้ policy ของโปรเจคหรือกำหนดอย่างชัดเจน ไม่ปล่อยเครื่องที่ทดสอบเปลี่ยนรูปแบบโดยบังเอิญ
- TMP ใช้ font fallback ไทยที่มีอยู่, ตัวเลขขนาดอ่านได้/tabular ถ้าฟอนต์รองรับ, ไม่เปลี่ยนความกว้างคอลัมน์ทุก tick; tooltip ค่าเต็มต้องไม่หลุด native floating window
- รองรับถึงขอบเขตชนิดข้อมูลของ host ไม่สัญญาว่ารองรับเงินไม่จำกัด หากอนาคตเกิน u64 ต้องเปลี่ยน contract ก่อน

## 6. แผนแก้โค้ดตามลำดับ

### Phase A — formatter และ read/action model

- เพิ่ม helper formatting พร้อม boundary tests และ parser/read model ของ Training เช่น `UI/Floating/TrainingReadModel.cs`; เพิ่ม action adapter แยกจากการวาดแถว
- รักษา schema null-aware โดยใช้ JSON utilities ที่โปรเจคมีอยู่หรือ adapter ขนาดเล็ก ไม่เพิ่ม dependency ใหญ่เพื่อ query เดียว
- อ่านข้อมูลตอนเปิด/หลัง action/เปลี่ยน session และ refresh จำกัดความถี่ตามระบบเดิม ไม่มี FFI query ต่อแถวต่อเฟรม

### Phase B — Training panel ทั้งสอง context

- เพิ่ม `UI/Floating/TrainingPanel.cs` ให้เข้ากับ `ForgeUi`, `UiScroll`, `Loc` และ Canvas เดิม ไม่สร้าง fullscreen scene หรือ controller สำหรับ Companion อีกชุด
- ต่อ `FloatingInventoryHost.cs`: ปุ่มล่าง, panel visibility, `Side`, `RefreshNow`, `WindowWidth`, `Back`, `LayoutChanged`, position/clamp และ session teardown
- ปัจจุบัน `ActiveSide` เป็น int 0=None, 1=Forge, 2=Socket; เพิ่ม Training โดยมีชื่อกลาง/enum หรือ constant ที่ชัดเจน ตรวจทุก branch ที่ใช้อยู่ รวม `FloatingInventoryHost.Items.cs` และ input handler
- ออกจาก Forge ต้อง reset draft เดิมและ cancel drag/tooltip ไม่ค้าง selection; เมื่อ Training เปิด right-click item ต้องไม่ถูกส่งไป Forge/Socket ที่ซ่อนอยู่ ให้ยังทำหน้าที่ equip ตามเงื่อนไข/confirmation เดิม และแยก pending action guard ให้ชัด
- ไม่เปลี่ยนความหมาย Full Stats ที่ใช้แผงรองเดิม ไม่ให้ Training กับ Full Stats ซ้อนตำแหน่งกัน
- Reuse row objects ระหว่าง refresh; ไม่สร้าง 23 แถวใหม่ทุก 0.4 วินาที ไม่ reset scroll, hover หรือ focus
- ใช้ `TRAIN_*`, `UI_TRAINING`, `UI_TRAIN_LEVEL`, `UI_TRAIN_BUY`, `UI_TRAIN_MAXED`, `UI_TRAIN_PER_LEVEL`; เพิ่มเฉพาะ loc ที่ขาด เช่น กลุ่ม, gate, party-wide, pending, unavailable, no bonus, capacity และ navigation Blacksmith
- แม้เลิก Godot แล้ว `godot/i18n/strings.csv` ยังเป็นตาราง loc กลางใน repo ต้องตรงกับ `unity/ROI/Assets/StreamingAssets/strings.csv`; ไม่พัฒนา Godot UI เพิ่ม

### Phase C — เชื่อมรายได้ Forge และส่งมอบ

- ใช้ formatter เดียวกับ wallet ที่เกี่ยวข้องและ Forge: preview ใช้ `fuse_preview.payout_gil`, result ใช้ `forge_receipt.gil` จริง ผลล้มเหลวต้องไม่ใช้หัวข้อสำเร็จ ไม่สร้าง payout สำหรับ gem fusion ที่ไม่มี Gil
- ทำ payout ให้อ่านเด่นโดยไม่เปลี่ยน animation/result flow ที่ผู้ใช้รับไว้แล้ว
- แก้ `Perf/FloatingForgeSmoke.Drag.cs` ที่ใช้ `SellInventoryItem`/`SellAllJunk` ซึ่ง retired: เปลี่ยนเป็น fixture/action ที่ยังมีจริงและยังพิสูจน์จุดประสงค์เดิม ห้ามแค่ลบ assertion เพื่อให้ผ่าน
- จบด้วย native verification, รูปทั้ง Battle/Companion, commit เฉพาะงาน และบันทึกข้อจำกัดให้ master อ่าน

## 7. Acceptance checklist

- [ ] Unity compile ไม่มี error; เปิด native standalone ใน Companion ได้จริง ไม่อ้าง Editor screenshot อย่างเดียวว่า transparency ใช้ได้
- [ ] กด Training จากแถวเดียวกับ Blacksmith ได้ทั้ง Battle/Companion; 23 ช่องครบ, กลุ่ม 11/6/6, ชื่อ EN/TH, cap ต่างกัน, Lv0 และ Satchel ไม่เป็นช่องว่าง/เปอร์เซ็นต์ผิด
- [ ] ฝึกสำเร็จเพิ่มหนึ่ง level หัก Gil ตาม host เป๊ะ และโบนัส/ความจุมีผลจริง; เปลี่ยนสมาชิกยังเห็น training ชุดเดียวกัน
- [ ] Gil ไม่พอ, gate ปิด, maxed, query/session unavailable ไม่ใช้เงินและไม่แสดง success; ฝึกช่องอื่นข้าม total gate แล้วแถวที่ล็อกเปิดทันที
- [ ] parser + formatter ทดสอบ 0, 999, 1,000, 999,999, 1,000,000, 2,147,483,647, 9,007,199,254,740,993, `long.MaxValue`, `ulong.MaxValue` รวมราคา null/missing; affordability ทดสอบ cost−1 / cost / cost+1 ที่ไม่ overflow
- [ ] ไม่มีราคา/ยอดสำคัญถูกตัดที่ UI scale 1×/1.5×/2×; เลข u64 สูงสุดอ่านเต็มได้ ภาษาไทยไม่ทับปุ่มหรือแถวข้างๆ
- [ ] double-click/pending และเปลี่ยน session ไม่สร้าง action ซ้ำ; refresh ไม่ดึง scroll กลับบนสุด
- [ ] Combat tick, damage และ Auto Fusion เดินต่อระหว่าง Training; ทดสอบ Auto Fusion เพิ่ม Gil แล้วราคาที่ซื้อได้อัปเดต และหน้า Forge ไม่ถูกเปิดเองจนบัง Training
- [ ] Drag/drop, right-click equip/socket/forge, ยืนยันอุปกรณ์ที่มีเจ้าของ, result dismissal และ Full Stats ไม่ regress; ออกจาก Forge ไม่มี draft ค้าง
- [ ] Native Companion: transparent, scale สลับทันที, UI อยู่เหนือ gameplay, titlebar drag/clamp, tooltip และ Esc/กากบาททำงาน เปิด–ปิด Companion ไม่ freeze/crash
- [ ] `python3 tools/check_loc_keys.py` ผ่าน และ `cmp godot/i18n/strings.csv unity/ROI/Assets/StreamingAssets/strings.csv` ผ่าน; ตรวจ dynamic `TRAIN_*` ทั้ง 23 ด้วย เพราะ literal-key check อย่างเดียวไม่ครอบคลุม

เพิ่ม tests เฉพาะ formatter/parser ที่เสี่ยงและ integration action จริงตาม `docs/testing-notes.md`; Rust tests อย่างเดียวไม่ยืนยัน UI ถ้าไม่ได้แก้ sim ไม่ต้อง bless goldens หรือเปลี่ยน VERSION เพื่องาน presentation

Build helper ปัจจุบัน `unity/ROI/Assets/Editor/BuildFloatingForge.cs` เขียนไป `/private/tmp/roi_floating_forge/ROI.app` ซึ่งใช้ร่วมกัน: ก่อนใช้ ให้ปรับให้รับ output path เฉพาะงานหรือประสานไม่ทับ process อื่น ทดสอบด้วย isolated dev fixture และ native plugin ของ checkout นี้ ห้ามใช้ save จริงเป็น smoke fixture

## 8. งานถัดไป แยกจาก acceptance รอบนี้

| ลำดับ | งาน | ข้อควรระวัง |
|---|---|---|
| P2 | Break damage เด่นกว่าปกติทั้ง Battle/Companion | ใช้ `HitKind::Break`; อย่าคำนวณ damage ใหม่ใน UI หรือเปลี่ยน VFX ที่รับไว้โดยไม่มีภาพเทียบ |
| P2 | First-session cadence/callout | เอกสารเสนอ Fusion ≥6 ชิ้น → Training ด่าน 5 → Gem 8 → Sigil 13 แต่เป็นข้อเสนอ UI ไม่ใช่ sim gate; รอบนี้ให้ Training เข้าถึงได้และเตรียม visibility hook แยก อย่าสร้าง progression lock เอง |
| P3 | Regalia Lv x/60, Bestiary stage_min/max + weakness, Dungeon entry เก่า | ตรวจ contract ล่าสุดก่อนแก้ label/entry |
| P3 | Party slot callout ตั๋ว Summon ×10 | ยังไม่แสดงว่าได้รับของถ้าไม่มี receipt/reward จริง |
| P3 | `Story/StoryDriver.cs` ใช้ `BuyCharacter` ที่ retired | sandbox ต้องใช้ debug grant ไม่เรียก gacha จริงเพื่อสร้าง cast |
| P3 | ไล่ formatter ให้ครบหน้าจอเศรษฐกิจอื่น | รอบนี้ต้องครบ Training + wallet ที่เกี่ยวข้อง + Forge; ที่เหลือทำเป็นรายการตรวจ ไม่ claim ว่าแก้ทุกหน้าจอแล้ว |

## 9. สิ่งที่ส่งกลับให้ master

ระบุ branch/base/head, ไฟล์ที่เปลี่ยน, sim/catalog/native version ที่ทดสอบจริง, คำสั่งและผลตรวจ, รูป Training ในทั้งสอง context, หลักฐานซื้อ/ไม่ซื้อเมื่อ gate ปิด และ known limitations อัปเดตเอกสาร implementation โดยไม่อ้างว่า test ผ่านหากยังไม่ได้รัน

ไม่รวม `docs/previews/`, cache, save, build output หรือไฟล์ของ session อื่นใน commit; ไม่ใช้ blanket restore ล้างการแก้ไฟล์ที่ Unity สร้างโดยยังไม่ดู diff ให้เจ้าของ master เป็นคน merge

### ข้อความส่งให้ agent

> อ่าน CLAUDE.md และ docs/unity-training-ui-agent-plan.md แล้วตรวจ/ต่อยอด implementation ที่มีอยู่ใน worktree ใหม่จาก master ล่าสุด: Training อยู่แถวล่างเดียวกับ Blacksmith เปิดแผงรองขวา ใช้ร่วม Battle/Companion โดยเกมเดินต่อ รองรับเลข u64 แบบไม่เสียความแม่นยำและแสดงราคาเต็ม ใช้ query/event ที่มีอยู่ ไม่แก้ balance หรือ deploy รัน `-roitrainingsmoke` ใน standalone, เก็บหลักฐานและข้อจำกัด แล้วส่ง commit ให้ master เป็นคน merge
