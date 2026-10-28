# ทำ VFX ใน HD-2D — กับดักที่เจอมาแล้ว

เขียนจากการต่อ **LeLu Magic Projectiles (Premium)** เข้ากับ `proto/arena3d/` (2026-08-20/21)
ทุกข้อในนี้**เคยกินเวลาไปแล้วจริง** ไม่ใช่ข้อควรระวังเชิงทฤษฎี

โค้ด: `godot/proto/arena3d/fx/pack_vfx.gd` (ตาราง + การแต่งตัว) ต่อสายที่ `arena3d_fx.gd`
เครื่องมือ: `pack_preview.tscn` (เล่นวน ตรวจด้วยตา) · `_pack_probe.gd` (เรนเดอร์เป็น PNG)

---

## 1. resource ของ asset pack แชร์กันหมด — duplicate ก่อนแตะเสมอ

นี่คือกับดักที่**เกิดซ้ำมากที่สุด** ในงานนี้ เจอ 4 ครั้งในรูปแบบต่างกัน

ทุกอย่างต่อไปนี้เป็น resource ที่ **ทุก instance ของฉากใช้ร่วมกัน** เขียนทับ = เปลี่ยนให้ทุกลูก
ที่ยิงหลังจากนั้นด้วย และเปลี่ยนซ้ำอีกทุกครั้งที่สร้างใหม่:

- `ShaderMaterial` / `StandardMaterial3D` (ทั้งที่เป็น ExtResource และ SubResource)
- `Shader` ที่ material ชี้ไป
- `Gradient` และ `GradientTexture1D/2D` ที่อยู่ใน shader parameter
- material ที่ติดมากับ **mesh** (`mesh.surface_get_material`) — แชร์หนักกว่าใบอื่น เพราะ mesh
  เองก็แชร์ ทางแก้คือเขียนกลับเป็น **surface override** บนโหนด ไม่ใช่เขียนลง mesh
- `AnimationLibrary` และ `Animation` ข้างใน

อาการเวลาพลาด: **ลูกไฟธาตุไฟกลายเป็นน้ำเงินตามไปด้วย**

### กรณีที่หลุดบ่อยเป็นพิเศษ

ชุดสีที่**ไม่มีสี** (routing อย่างเดียว เช่น Fireball ที่ต้องการแค่เปลี่ยนฉาก impact)
`recolour()` จะ return ออกตั้งแต่ต้นและ**ไม่เคย duplicate อะไรเลย** — โค้ดที่มาแก้ทีหลัง
จึงเจอ material ต้นฉบับที่แชร์อยู่ ห้ามสมมติว่า "ผ่าน recolour มาแล้วแปลว่า duplicate แล้ว"

### cache key

เคย key ด้วย `resource_path` แล้วพัง — shader พวกนี้เป็น **sub-resource** ที่ path ซ้ำหรือว่าง
shader คนละตัวเลยได้โค้ดของตัวแรกไปหมด **อาการคือเอฟเฟกต์กลายเป็นสี่เหลี่ยมขาว**
ใช้ `get_instance_id()` แทน

---

## 2. สีไม่ได้อยู่ที่เดียว — มีอย่างน้อย 5 ที่

ไล่แก้ทีละที่ตามที่เห็นด้วยตา = แก้ไม่จบสักที ต้อง**ถาม shader ว่ามี uniform อะไรบ้าง**
แล้วจัดการให้หมดในรอบเดียว

| ที่อยู่ของสี | วิธีจัดการ |
|---|---|
| uniform สีตรง ๆ (`Fire_Color`, `Ball_Color`, `Line_Color`) | `set_shader_parameter` |
| `GradientTexture1D/2D` ใน uniform | duplicate gradient แล้วย้ายเฉด **เก็บ alpha เดิม** |
| texture ที่ระบายสีไว้ (`T_FireTrail.jpg`) | สร้าง ImageTexture ที่ย้อมแล้ว cache ไว้ |
| `StandardMaterial3D.albedo_texture` | `_paint()` ที่เช็คแค่ `is ShaderMaterial` จะข้ามทั้งใบ |
| **vertex colour จาก script export** (`Trail3D.startColor/endColor`) | **แตะไม่ได้เลย** — ไม่ใช่ material |

> `Trail2_dynamic` ของ pack ถือ `startColor = เหลือง` / `endColor = แดง` อบลง ImmediateMesh
> เป็น vertex colour ไม่มีทางย้อมด้วยระบบทาสี ทางเดียวคือ**อย่าเปิดมัน** หรือเซ็ต export ตรง ๆ

### เก็บ mask/noise ไว้ อย่าย้อม

shader พวกนี้ sample บาง texture เอา**สี** บางอันเอา**ค่าเดี่ยว** (dissolve mask, noise, flare)
ย้อม mask สีเทา = คูณด้วย hue ที่ทุกแชนเนลต่ำกว่า 1 → **dissolve กับ fade ทุกอันอ่อนลงเงียบ ๆ**
ใช้ค่า saturation แยก: texture สีอิ่ม mask ไม่อิ่ม

---

## 3. AnimationPlayer เขียนทับค่าที่ตั้งบนโหนด

9 ใน 12 ฉาก hit/anticipation ของ pack **ขับ `OmniLight3D:light_energy` ด้วย animation**
ค่าที่เก็บในไฟล์คือ `0.0` แล้วอนิเมชันเขียนเส้นจริงทับทุกเฟรม

ผลคือ:
- คูณ `light_energy` ตอนสร้าง = **คูณศูนย์** แล้วโดนทับอยู่ดี
- ซ่อนไฟก็ไม่รอด — ฉากเดียวกันขับ `OmniLight3D:visible` ด้วย มันเปิดกลับเอง

**ที่เดียวที่ค่าถูกตัดสินจริงคือ keyframe** ต้องคูณที่ track บน AnimationLibrary ที่ duplicate แล้ว

ตัวเลขจริงที่วัดได้: muzzle ไต่ถึง **energy 12.0** ขณะที่ลูกที่บินทั้งลูกมี 0.675 — **สว่างกว่า 18 เท่า
และนั่งอยู่บนสไปรต์ตัวละคร** นี่คือสิ่งที่หา "ทำไมสว่างที่ caster" อยู่สามรอบ

**ก่อนสรุปว่าค่าไม่ถูกเขียนทับ ให้ไล่ track ก่อน**:
`grep 'tracks/[0-9]*/path' scene.tscn`

---

## 4. glow threshold คือ 1.0 — ค่าเกินนั้น bloom

`arena3d_scene.GLOW_THRESHOLD = 1.0` ทุกพิกเซลที่เกินจะเรืองแสง

asset pack เขียนสีเป็น HDR (2.0–4.0) ตามนิสัยของมัน ลอกมาใช้ตรง ๆ บน mesh แบบ additive
ที่อยู่ห่างตัวละครหนึ่งเมตร **ไม่ใช่ "สว่าง" แต่เป็นรูขาวที่มีตัวละครอยู่ข้างใน**

ทุกค่าที่ทาเองต้อง **≤ 1.0** และควรมี guard ตรวจไฟล์ ไม่ใช่ดูด้วยตา

**และ preview ต้องเปิด glow ด้วยค่าเดียวกับฉากจริง** ไม่งั้นค่าที่เกินจะมองไม่เห็นเลย
ลูกที่ระเบิดเป็นรูขาวจะดูแค่ "สว่าง" เฉย ๆ

---

## 5. โดนพื้นกิน = **สองกลไก** ไม่ใช่หนึ่ง

ระเบิด AoE เป็นทรงกลมที่จุดกระแทก ครึ่งล่างหายไป เสียเวลาสามรอบเพราะแก้ทีละอย่าง

1. **depth test / depth write** — pack ตั้ง `depth_test_default, depth_draw_opaque`
   อะไรที่ต่ำกว่าพื้นถูกตัดทิ้ง
2. **proximity fade** — เป็น `VisualShaderNodeProximityFade` สร้างโค้ด:
   ```glsl
   n_outNN = clamp(1.0 - smoothstep(depth.z + dist, depth.z, VERTEX.z), 0.0, 1.0);
   ALPHA   = ... * n_outNN;
   ```
   **fragment stage ล้วน ๆ ไม่มี depth flag ตัวไหนแตะได้**

แก้อย่างใดอย่างหนึ่ง = เหลืออีกครึ่งค้างไว้ และ emitter ที่**ไม่มี** proximity fade (เช่น `sparks`)
จะทะลุพื้นได้ตั้งแต่แก้ข้อ 1 → เห็นเป็น "บางส่วนทำงาน บางส่วนไม่" ซึ่งเป็นเบาะแสว่ามีสองสาเหตุ

### ตั้ง `Proximity_Fade = 0` ไม่ได้ปิดโหนด

ทำให้ขอบ smoothstep เท่ากัน ซึ่ง **GLSL ไม่นิยาม** ไดรเวอร์ยังคืน 0 ให้ครึ่งหนึ่งของพิกเซล
ต้องบังคับตัวคูณเป็น `1.0` โดยจับ pattern `__depth_world_pos.z +` (Godot ตั้งชื่อตัวแปรใหม่ทุกกราฟ
จะจับด้วยชื่อไม่ได้)

### แก้ render_mode ต้องแทนที่ ไม่ใช่ต่อท้าย

Godot ปฏิเสธ `depth_test` ตัวที่สองเป็น redefinition — ต้องถอด token เดิมออกก่อน
และถอด `depth_draw_*` ไปด้วย (material ที่เขียน depth แต่บอกว่าไม่ทดสอบ depth คือคู่ที่ขัดกัน)

---

## 6. pooling — reuse คือที่ที่บั๊กไปซ่อน

**บั๊กที่แพงที่สุดในงานนี้อยู่ในเส้นทาง reuse ล้วน ๆ และ preview จับไม่ได้เลย**
เพราะ preview สร้าง instance เดียว ไม่เคยลงพื้น ไม่เคยถูกใช้ซ้ำ

- **คืนค่า `visible` เดิมของแต่ละโหนด อย่าเปิดทุกอัน** — `set_body_visible()` ที่เปิด
  MeshInstance3D ทุกตัวไปปลุก `Trail2_dynamic` ที่ pack ตั้งใจปิด (หางสำหรับวิถีโค้ง) ขึ้นมา
  เป็นหางเหลือง-แดงบนลูกน้ำแข็ง จำค่าที่ฉากตั้งไว้ครั้งแรกด้วย meta
- **particle อยู่ใน world coordinates** — โหนดที่ย้ายไปลูกใหม่ทิ้งประกายลูกเก่าค้างกลางอากาศ
  ต้อง `restart()`
- **restart หลังตั้ง transform ไม่ใช่ก่อน** — `restart()` พ่นจากตำแหน่งปัจจุบัน โหนดที่หยิบจาก pool
  ยังอยู่ที่ลูกเก่าตาย
- **key ต้องเป็นตัวตนของ bolt ไม่ใช่ index** — `_projectiles` สร้างใหม่ทุกเฟรม
  และต้องมี **skill id** อยู่ในคีย์ ไม่งั้นสองสกิลชนคีย์กันจะใส่สีของกันและกัน
- **pool แยกตามชุดสี** ไม่งั้น Frost Bolt จะได้ลูกไฟแดงเป็นบางครั้ง
- **hit/muzzle ต้องซ่อนตัวเองตอน `animation_finished`** — เงื่อนไข pool ("อนิเมชันจบยัง")
  ตอบว่า *ช่องนี้ใช้ซ้ำได้เมื่อไหร่* ไม่ได้ตอบว่า *ควรเลิกวาดเมื่อไหร่*
- **อย่าแปะ emitter ที่ไม่มี animation ลงบนฉาก hit/muzzle** — มันจะพ่นตลอดไปทุกจุดที่เคยระเบิด
  (เกล็ดหิมะเคยทำ FPS ตกเหลือ 19)
- **ลูกที่ลงพื้นแล้วต้องอุ้มโหนดไว้อีกหนึ่งช่วงอายุ particle** ไม่งั้นเกล็ดที่ลอยอยู่ดับพร้อมกันหมด

---

## 7. ทิศทางและตำแหน่ง

- **หน้าของ pack นี้คือ local −Z** (ยืนยันจาก `projectile.gd` ของ demo: `basis * Vector3(0,0,-SPEED)`)
  `Trail3D.appendPoint` ที่ทำ `atan2(direction.x, direction.z)` **ไม่ใช่**การประกาศแกนหน้า
  มันคือริบบิ้นหมุนตัวเอง
- **mesh เล็งด้วยเวกเตอร์ในโลก ไม่ใช่มุมบนจอ** — มุมบนจอถูกสำหรับ billboard quad แต่จะทำให้
  mesh เอียงตามกล้องแทนที่จะตามทางบิน
- **ของที่สมมาตรรอบแกนห้ามเล็งตามทิศ** — ระเบิดที่ถูกหมุนไปตามวิถีกระสุนจะเอียงวงแหวนตะแคง
- **`sky_fall` มาจากสิม**: `is_magic && cast_opt.is_some() && aoe_radius == 0` = **bolt สกิลเป้าเดียว
  ทุกอัน** ถ้าเขียน `if not curve.is_empty() and not sky:` เท่ากับปิดฟีเจอร์ทั้งเกม
- **ขนาดต้องมาจากสิม** — `hit_scale_for(radius)` กาง `2*radius+1` ช่อง ตัวแปลงคือ "ฉากนี้กินกี่ช่อง
  ที่ scale 1.0" ซึ่งวัดกับลูกบาศก์ 1 ช่อง ไม่ใช่เลขที่จูนให้พอดีที่รัศมีเดียว
- **arc ต้องสเกลตามระยะยิง** — ค่าที่เลือกไว้สำหรับ 6 ช่อง พอมอนเข้ามา 2 ช่องจะกลายเป็นวงใหญ่
  เทียบระยะ = ดูเหมือนตกจากฟ้า

---

## 7.5 เส้นทางการบิน — สองบทเรียนจาก Spirit Strike

**Bézier ก้อนเดียววกกลับไม่ได้** มันโค้งได้แต่ไม่ย้อนทาง ดันจุดควบคุมไปทางไหนก็ได้แค่ธนูที่อ้วนขึ้น
ถ้าทรงที่ต้องการมี**การวกกลับ** ให้หา**สมการปิดที่มีทรงนั้นอยู่แล้ว** แทนการจูนจุดควบคุม

> ครึ่งหัวใจ: `hx = 16sin³t`, `hy = 13cos t − 5cos2t − 2cos3t − cos4t`, `t = kπ`
> map รอยหยัก (t=0) เป็นผู้ร่าย ปลายแหลม (t=π) เป็นเป้า — **การถอยหลังมาฟรี** เพราะ `hy`
> ไต่ขึ้นเหนือค่าเริ่มต้นก่อนจะร่วง ไม่ต้องเติมเงื่อนไขใด ๆ
>
> และวัดด้วยช่วงของสมการเอง (22 หน่วย) หารระยะยิงจริง → ทรงคงที่ทุกระยะ
> ตัวคงที่ที่เคยต้องมีไว้สเกลตามระยะเลยไม่ต้องมี

**คีย์เดียวห้ามตอบสองคำถามที่ต้องการคุณสมบัติตรงข้าม** ตัวนับแฉกใช้คีย์ตัวตนของ bolt เป็นฐาน
ซึ่งมี `arrive_at_tick` และสิมกระจายเวลาถึงของ volley (`base + k·BOLT_STAGGER`) —
**ไม่มีลูกไหนใช้ฐานร่วมกันเลย ตัวนับจึงคืน 0 ตลอด ทุกลูกได้แฉกเดียวกัน**

- **คีย์ตัวตน** ต้อง **ไม่ซ้ำ** ต่อลูก (จำ instance ใน pool)
- **คีย์นับ** ต้อง **ซ้ำทั้ง volley** (ไล่หมายเลขแฉก)

**แฉกต้องวนใช้ซ้ำ** — bolt ยิงได้ถึง 10 ลูก ถ้าปล่อยหมายเลขไต่ต่อ ลูกที่ 6 จะเลย 90°
แล้ว `cos` ติดลบ = ทิศพุ่งลงใต้พื้น

**พัดที่หมุนรอบแกนบิน มองจากด้านบนตรง ๆ ไม่เห็น** — probe ที่วางกล้อง top-down จะบอกว่าผ่าน
ทั้งที่ยังผิด ต้องใช้มุมของ arena จริง

---

## 8. วิธีวัด — สิ่งที่ทำให้จบ

> ทุกครั้งที่ติดนาน สาเหตุคือ**เดาแล้วแก้** ทุกครั้งที่หลุด สาเหตุคือ**วัด**

- **`pack_preview.tscn` เล่นวนซ้ำ** ดูภาพนิ่งครั้งเดียวจับอะไรไม่ทัน และเปิด Remote scene tree
  คลิกอ่าน material ได้สด ๆ ขณะบิน
- **แต่งตัวด้วยฟังก์ชันเดียวกับเกม** (`PackVfx.dress_bolt` / `dress_fx`) preview ที่แต่งเองมีสิทธิ์
  ไม่ตรงกับเกม ซึ่งเป็นสิ่งเดียวที่มันห้ามทำ
- **ใส่ตัวเทียบที่ไม่ถูกแก้ไว้ในภาพเสมอ** (`fire_bolt` ที่ไม่ทาสี = ส้มต้นฉบับ) จะได้ไม่ต้องเถียงกัน
  ว่าพิกเซลนี้อุ่นไปไหม
- **ใส่ไม้บรรทัด** ลูกบาศก์ 1 ช่อง — pack วาดมาเป็นเมตร แต่หน่วยของ arena คือช่อง
- **ทาสีฉูดฉาด (magenta) เพื่อ bisect** สิ่งที่ระบบเข้าถึงจะเปลี่ยนสีหมด อะไรที่ไม่เปลี่ยน
  **ประกาศตัวเอง** — วิธีนี้คือตัวที่หา `Trail2_dynamic` เจอ
- **วางวัตถุควบคุม** ลูกกลมจมพื้นครึ่งลูก คู่หนึ่งเปิด `no_depth_test` อีกอันไม่เปิด
  ตอบคำถามว่า "วิธีนี้ใช้ได้ในฉากนี้ไหม" ซึ่งถ้าไม่มี ทุกความพยายามที่ล้มเหลวจะดูเหมือนบั๊กของเอฟเฟกต์
- **ปุ่มสลับซ่อนพื้น** พิสูจน์ว่าพื้นคือตัวบังจริงหรือไม่ ควรทำตั้งแต่รอบแรก
- **อ่านค่ากลับจากโหนดที่สร้างเสร็จแล้ว ไม่ใช่ log ว่าเลือกอะไร** — log ตัวแรกบอกว่า `paint=ice`
  ขณะที่มีลูกส้มอยู่บนจอ **ตัววัดที่พิสูจน์ผิดเรื่องแย่กว่าไม่มีตัววัด**
- **`--check-only` จับ shader ไม่ได้** และ headless ไม่ compile shader เลย ต้องเรนเดอร์จริง
- **probe ต้องเรียกฟังก์ชันจริงที่เกมใช้ ไม่ใช่สำเนา** สำเนาจะค่อย ๆ เพี้ยนจากของจริงแล้วรับรองผิด
- **รูปที่วาดให้ user ดู ต้องเซฟไปที่ที่เขาเปิดได้** (`~/Desktop`) ไม่ใช่ scratchpad ของเรา

---

## 9. บทเรียนเรื่องวิธีทำงาน

- **user เห็น = ข้อเท็จจริง** เคยเอา log ที่พิสูจน์แค่ "เลือกสีอะไร" ไปบอก user ว่าเข้าใจผิด
  ทั้งที่เขาถูก log นั้นไม่ได้พิสูจน์อะไรเลยเกี่ยวกับสิ่งที่วาดออกมา
- **บอกว่าจะให้เลือก ต้องให้เลือกจริง** ไม่ใช่เรนเดอร์แล้วตัดสินเอง
- **`git checkout --` ในทรีที่แชร์กับอีก session ลบงานที่เขายังไม่ commit** และ
  `git add <ไฟล์ที่เขาแก้ค้างไว้>` จะดูดงานเขาเข้า commit เรา — stage ทีละ path เสมอ
- **assert ทุก string replace** replace ที่ไม่ match คือการแก้ที่ไม่เคยเกิด แต่รายงานว่าสำเร็จ

## The effects switch is a promise, and it has to be kept everywhere

`ArenaRenderData.FX_ENABLED` gates every decorative effect in both renderers. Two things to know
before adding anything that draws:

- **Off means off, including the projectile itself.** `set_heavy` used to keep the bolt's mesh
  visible and strip only its emitters and light, on the argument that a flying bolt is
  information. The argument is real and it is not this switch's to make (user, 2026-08-21) —
  damage numbers, HP bars and the skill banner each have their OWN row on the options screen, so
  a player who wants those keeps them there. Effects off leaves an empty arena.
- **`FX_ENABLED` is the EFFECTIVE value, `FX_USER_ON` is the player's setting.** Companion mode
  overrides the first while its widget is up and restores from the second on exit. Write both
  from a settings screen; read `FX_ENABLED` to decide whether to draw.

Three traps, all of which bit:

1. **Gate at spawn AND on change.** Gating only at spawn leaves everything already in the air
   running for the rest of its flight. `_sync_projectiles` applies the switch on CHANGE for that
   reason.
2. **A bolt that has landed must not be restored.** Its ball was hidden by `begin_fade_out` and
   only its motes are still going; turning effects back on mid-fade stands the fireball back up
   at the impact point.
3. **A flag nothing writes is not a feature.** `FX_ENABLED` gated the whole renderer for months
   and no screen set it — only companion mode did, so no player could ever ask for it. Whenever
   you add a gate, add the thing that flips it in the same change.

## Adding a paint

`PAINTS` in `fx/pack_vfx.gd`. An entry is one of three shapes and the guard
(`tools/check_vfx_tables.py`) rejects anything in between, because `recolour` decides which branch
to take by testing for `Fire_Color` and a half-written entry takes the painted branch and dies on
a missing key mid-fight:

1. **Shorthand** — `"tint": [edge, core]`. Derives all four colour uniforms plus the trail. Start
   here, look at it in `fx/pack_preview.tscn`, then hand-write whichever values need moving:
   explicit keys win, so `{"tint": [...], "light": Color(...)}` is a legal and expected shape.
2. **Hand-tuned** — all four of `Fire_Color` / `Ball_Color` / `Line_Color` / `light`, plus
   `"trail": [four colours]`. The three paints in the table today are this, and they are NOT
   convertible: fed ice's own edge and core, the shorthand returns `Fire_Color` 0.765/0.900/1.000
   where the tuned value is 0.720/0.920/1.000. Close, and not the same — the difference is rounds
   of looking at it.
3. **Routing only** — no colours at all, just `triplet` / `name` / slot overrides. `blast` is
   this: Fireball is a Fire skill already wearing fire's palette and what it needed was the AoE
   impact instead of the single-target one.

The trail's OFFSETS and ALPHAS are not per-entry: they are `TRAIL_STOPS` / `TRAIL_ALPHAS`, the
pack's own, and they are the effect's timing rather than a colour decision. An entry writes four
colours and the fifth stop wraps back to the first.

**Verify by reading values BACK off a built node, not off the table.** A refactor of this table
was checked by instantiating each pack scene headless, running `recolour`, and comparing the
resulting `Color_Texture` gradient stop by stop against the pre-refactor numbers. That is a
different claim from "the table still says the same thing", and it is the one that matters.
