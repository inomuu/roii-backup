# Sword of Convallaria: frame study และ VFX production pipeline

วิเคราะห์เมื่อ 2026-09-23 จากไฟล์ที่ผู้ใช้ให้โดยตรง เป้าหมายคือออกแบบขั้นตอนผลิต VFX สำหรับ ROI บน Unity โดยอิงภาพจริงในคลิป

**ข้อสรุป:** รูปลักษณ์นี้ต้องทำทั้ง animation ของตัวละคร, รูปทรงหลักของเอฟเฟกต์, จังหวะภาพ และการจัดฉากร่วมกัน การเพิ่ม particle หรือเปลี่ยนสีเอฟเฟกต์ตามธาตุอย่างเดียวจะได้เพียงส่วนประกอบบางส่วน

## หลักฐานและขอบเขต

- Source: `/Users/lunalieneray/Downloads/DROP IT - Sword Of Convallaria Character's Ultimate move - Siberia (720p, h264).mp4`
- Container duration 34.631 วินาที; ภาพจริง 1280 x 576, 30 fps แม้ชื่อไฟล์ระบุ 720p
- ถอดภาพได้ 1,036 เฟรม เก็บต้นฉบับเฟรมที่ `/private/tmp/soc-vfx-review/full-0000.jpg` ถึง `full-1035.jpg`
- ตรวจภาพรวมตลอดคลิปประมาณ 4 ภาพ/วินาที แล้วดูช่วงที่เลือกละเอียดขึ้นที่ 15 หรือ 30 ภาพ/วินาที ไม่ได้ตรวจทุกเฟรมทั้ง 1,036 ภาพทีละภาพ
- เวลาใน contact sheets ใช้เฟรมแรกเป็นศูนย์: `t = frame / 30` ส่วน timestamp ของ video stream เริ่มที่ 0.033333 วินาที เวลา player จึงอาจต่างหนึ่งเฟรม
- คลิปเป็น montage มีรอยตัดและอาจปรับเวลา จึงใช้วัดลำดับภาพในไฟล์นี้ได้ แต่ยืนยันระยะเวลาสกิลหรือ hit-stop ในเกมต้นฉบับไม่ได้
- ข้อเสนอเรื่อง mesh, shader, flipbook และการแยก render layer เป็นวิธีสร้างภาพใกล้เคียง ไม่ใช่ข้อยืนยันว่าเจ้าของเกมใช้เทคนิคเหล่านั้น
- งานนี้ตรวจภาพ ไม่ได้ประเมินเสียงในคลิปหรือจังหวะการ sync กับเสียง

![เฟรมตัวอย่างพร้อมเวลา](frames/selected-reference.jpg)

## สิ่งที่เห็นจริง

| เวลาในไฟล์โดยประมาณ | สิ่งที่เห็น | สิ่งที่ควรนำมาใช้ |
| --- | --- | --- |
| 1.00-2.07 s | ริ้วสีทอง วงแหวน แสงแฉก ฉากมืด การเปลี่ยนมุม และดาบแสงขนาดใหญ่ | ใช้รูปทรงเด่นนำสายตา แยกท่าเตรียมกับช่วงปล่อยพลัง; ช่วงนี้มีการเปลี่ยนช็อต อย่าถือเป็นสกิลเดียวทั้งหมด |
| 7.50-8.40 s | เส้นโค้งทองล้อมตัว ก่อนเกิดแนวแสงพาดเฉียงและ burst ตามแนวโจมตี | มุมและทรงเส้นทำหน้าที่บอกทิศแรงได้ชัดกว่า particle กระจายทั่ว ๆ ไป |
| 8.47-9.87 s | ริ้วไฟหมุนรอบแกน ก้อนแสงโต หดเป็นจุดเล็ก แล้วแตกเป็นวงและเปลวไฟ | จังหวะขยาย-หด-ระเบิดสร้างความหนัก; ต้องควบคุม envelope ของแต่ละชั้นแยกกัน |
| 12.75-14.50 s | ตัวละครยังอยู่บนสนาม มีภาพตัวละครประกอบด้านข้าง วงพื้น และแสงเขียว | งานบัฟมี choreography และ asset เฉพาะของตัวเอง ไม่จำเป็นต้องใช้ระเบิดที่เปลี่ยนสี |
| 16.33-19.27 s | เตรียมดาบ วงขาวขยาย ฟันเส้นขาว ตามด้วยแนวเฉียงแดงขอบดำ เป้าหมายเปลี่ยนเป็น silhouette สีแดง | ผูกท่าตัวละครกับ impact; ชั้นสีเข้มช่วยให้ทรงฟันยังอ่านออกบนพื้นสว่าง |
| 20.50-21.30 s | ภาพวาดตัวละครเต็มช็อตและเส้นแสง ก่อนกลับสู่สนาม | แยก pipeline งานคัตอินภาพวาดออกจาก VFX ที่ติดตัวละครบนสนาม |
| 21.73-22.23 s | ฉากหายเป็นมืด พื้นแตกน้ำแข็ง แสงปะทุ เศษทรงเหลี่ยมขนาดใหญ่เข้าด้านหน้า | เอฟเฟกต์กินพื้นที่หลายระนาบ: พื้น ตัวละคร และ foreground |
| 23.50-26.60 s | ช็อตโจมตีประชิดสีชมพู/ม่วง ตามด้วยโจมตีบนสนามที่มี flash เป้าหมายและรอยฟันแดง | identity อยู่ที่ pose, เส้น และจังหวะร่วมกับสี ไม่ใช่สีอย่างเดียว |
| 26.77-27.60 s | วงพลังและช่องว่างมืด จุดแสงเล็ก เฟรมขาว แล้วระเบิดแดง ก่อนเหลือควันเป็นก้อน | ตั้งใจเว้นช่วงรายละเอียดน้อยก่อน peak และเปลี่ยนจากเส้นคมไปสู่ควันในช่วงจบ |
| 27.63-28.33 s | ฉากเหลืองลายดาวแทนสนาม แล้วเกิดระเบิดแบบการ์ตูน | ท่าที่มีบุคลิกอาจใช้ฉากกราฟิกเฉพาะ เป็นงานกำกับภาพเพิ่มเติม |
| 28.50-30.30 s | ริ้วฟ้าวนรอบตัว เปลวแขวนด้านบน โดมมืดขยาย แล้วปะทุเป็นเปลว/แผ่นแสงรูปทรงชัด | ผสมทรงวนในอวกาศกับภาพแอนิเมชันทรงอิสระ; สีฟ้าไม่ได้พิสูจน์ว่าเป็นธาตุใด |
| 30.33-34.50 s | ช็อตกว้างสภาพแวดล้อมและไฟ ก่อนตัดเป็นภาพวาดท้ายคลิป | จัดเป็น cinematic/environment reference ไม่ใช้เป็นหลักฐาน runtime combat VFX |

ตัวละครมีขอบพิกเซลชัด ส่วนริ้วแสงและวงหลายชิ้นเรียบกว่าตัวละคร และมีแสงฟุ้งรอบรูปทรง ภาพนี้จึงเป็นงานผสม pixel character กับ stylized effects ไม่จำเป็นต้องบังคับทุกชั้นให้เป็น pixel art ความละเอียดเดียวกัน ทั้งนี้วิดีโอที่บีบอัดไม่สามารถบอก texture filtering ต้นฉบับได้

## แยกเอฟเฟกต์หนึ่งท่าเป็นอะไรบ้าง

| ชั้น | หน้าที่ในภาพ | วิธีสร้างสำหรับ ROI ที่เสนอ |
| --- | --- | --- |
| Character pose | ทำให้รู้ว่าใครออกแรงไปทางใด | ท่าเตรียม ท่ากระแทก และท่าจบที่มี silhouette ต่างกัน; sync กับ sprite animation เดิม |
| Main shape | รูปทรงที่คนจำได้ เช่น คมดาบ ปีกไฟ เสาแสง | วาด flipbook สำหรับทรงอิสระ; ใช้ textured ribbon/arc mesh สำหรับเส้นที่ต้องหมุนและปรับมุม |
| Dark body | ขอบดำ มวลควัน เงารองใต้แสง | alpha-blended layer แยกจากแสง additive; ไม่ใช้ additive กับทุกชิ้น |
| Bright core | จุดกระแทกและเส้นสว่างสุด | mask แสงแยกจาก body ปรับความเข้มและช่วงเวลาของ bloom ได้ |
| Ground contact | บอกตำแหน่งและพื้นที่บนสนาม | quad/ring บนพื้นและภาพรอยแตก; ใช้พื้นที่ที่ sim ระบุถ้าสื่อขอบเขต gameplay |
| Secondary motion | ทำให้เกิดแรงตาม ทิศทาง และความต่อเนื่อง | ParticleSystem สำหรับสะเก็ด เศษ mesh ฝุ่นและ motes; เริ่มหลัง main shape เล็กน้อยตามแบบที่วางไว้ |
| Target response | ยืนยันว่าถูกโจมตี | flash, pose hurt, hit-stop เฉพาะการแสดงผล และตัวเลขจากผลจริง |
| Presentation | ทำให้ผู้เล่นมองจุดที่ต้องการ | หรี่ฉาก โฟกัสกล้อง foreground overlay และ flash แบบจำกัดความถี่ |

จุดสำคัญคือให้แต่ละชั้นมีหน้าที่และเวลาชัดเจน ถ้าเปิดทุกชั้นเต็มกำลังพร้อมกันตั้งแต่ต้น จะไม่มีการไต่ระดับและรูปทรงหลักจะจมหาย

## Timing ที่อ่านได้จากเฟรม

**ไฟบนสนาม:** [ช่วงเตรียมและหด](frames/fire-release-01.jpg), [ช่วงระเบิด](frames/fire-release-02.jpg)

1. 8.47-8.67 s: ริ้ววนกว้างและมีแกนกลางสว่าง
2. 8.73-8.93 s: รวบเป็นมวลแสงแนวตั้งขนาดใหญ่
3. 9.00-9.47 s: หดเหลือแสงเล็ก พื้นที่จอสว่างน้อยลงชัดเจน
4. ประมาณ 9.53 s: ขยายฉับพลันเป็นวงและก้านแสงรัศมี
5. 9.67-9.87 s: เปลวแตกออกด้านข้าง เหลือสะเก็ดตามมา

**ระเบิดแดง:** [ก่อน peak](frames/red-slam-01.jpg), [peak และควัน](frames/red-slam-02.jpg)

- 27.133 s: จุดแสงกับเส้นพุ่งเข้าหาศูนย์กลางบนพื้นหลังมืด
- เฟรม 815-817 ที่ 27.167-27.233 s: ภาพขาวเกือบเต็มเฟรมต่อเนื่อง 3 เฟรม หรือราว 100 ms ในวิดีโอนี้
- 27.267-27.333 s: ก้อนแสงกลายเป็นปะทุทรงแหลม วงพื้น และริ้วเฉียงด้านหน้า
- 27.367-27.567 s: เส้นคมลดบทบาท ควันรูปก้อนขยายและค่อย ๆ เข้มขึ้น
- 27.633 s: ตัดเข้าท่าอื่น จึงไม่มีข้อมูล recovery เต็มของท่าเดิม

**ท่าสีฟ้า:** [วงวนและโดม](frames/cyan-charge-01.jpg), [เริ่มปะทุ](frames/cyan-charge-02.jpg), [สลายตัว](frames/cyan-impact-01.jpg)

ช่วง 28.50-29.17 s มีวงวนล้อมตัวและเปลวด้านบน จากนั้น 29.23-29.63 s มวลมืดขยายบังฉากโดยยังเห็นศูนย์กลางสว่าง ประมาณ 29.70 s รูปทรงใหญ่ปะทุออกและยกขึ้น ก่อนสลายถึง 30.30 s รูปทรงเปลวที่มีช่องว่างและปลายแหลมเป็นชิ้นงานหลักที่ต้องออกแบบ ไม่ใช่ปล่อยจุดแสงจำนวนมากให้สุ่มได้ทรงเอง

ใน ROI ควรใช้ลำดับเหล่านี้เป็น reference ของจังหวะ ไม่คัดลอกเวลาทั้งชุดลงการต่อสู้อัตโนมัติ ค่าเริ่มต้นสำหรับทดลองหนึ่งท่าคือเตรียม 150-250 ms, release 50-100 ms, impact เด่น 50-100 ms, tail 200-400 ms ตัวเลขนี้เป็นข้อเสนอ ไม่ใช่ค่าที่วัดจาก SoC และต้องบีบช่วงเตรียมให้ตรงเวลาของ sim

## Pipeline ผลิตงาน

### 1. ทำ skill brief และ animatic

เลือกสกิลจริงหนึ่งท่า ระบุต้นทาง เป้าหมาย รูปทรงพื้นที่ ระยะโจมตี จุด impact และความถี่ที่จะเห็นบนสนาม วาง storyboard 5-7 ภาพตั้งแต่ pose เตรียมจนกลับสู่สนาม แล้วทำ animatic ขาวดำที่ขนาดกล้องเกมจริงให้ผ่านก่อน

สิ่งส่งมอบ: timing sheet, ภาพ silhouette ของ peak, ตำแหน่ง anchor และเวอร์ชัน compact/full ของท่าเดียวกัน ประเมินว่าท่ายังอ่านออกเมื่อไม่ใส่ bloom หรือไม่

### 2. ผลิต asset ตามชนิดการเคลื่อนไหว

| Asset | เครื่องมือ/วิธีที่เสนอ | สิ่งส่งมอบ |
| --- | --- | --- |
| ท่าตัวละครและ afterimage | Aseprite หรือเครื่องมือ animation ที่ทีมใช้อยู่ | sprite strip, pivot คงที่, marker เตรียม/ปล่อย/กระแทก |
| เปลว stylized, ควัน, impact burst | วาด key shapes และ in-between ใน Krita/Photoshop/Aseprite ตามชนิดเส้น | RGBA flipbook แยก body กับ emission mask เมื่อจำเป็น |
| เสี้ยวดาบ วงหมุน ribbon เสาแสง | Blender ทำ mesh และ UV; วาด texture/mask ที่มีปลายแหลม | mesh ขนาดอ้างอิง, texture, animation curve สำหรับ sweep/scale/dissolve |
| พื้นแตก วงพื้น ประกาย | วาดภาพโปร่งใสและ variation ที่จำเป็น | atlas พร้อม pivot และขนาดพื้นที่อ้างอิง |
| น้ำแข็งและเศษแข็ง | mesh ทรงเหลี่ยมที่อ่าน silhouette ได้ | mesh/material ที่ใช้ซ้ำใน ParticleSystem ได้ |
| คัตอินภาพวาด | วาดภาพแยก foreground/character/background เท่าที่มี motion จริง | layered artwork และแผนเลื่อน/ซูม/reveal |

เริ่มทดสอบ flipbook ที่ 256 หรือ 512 px ต่อเฟรม, 8-16 เฟรมตามความจำเป็น ทดสอบที่ระยะกล้องจริงก่อนล็อกขนาด แยกจังหวะ playback ของภาพวาดออกจาก motion ของ mesh ซึ่งเดินเรียบได้ ระบุ padding, alpha convention, pivot และ frame order ให้แน่นอนเพื่อกันขอบดำและการสั่นของภาพใน atlas

เครื่องมือ AI เหมาะกับลอง key visual และรูปทรง แต่ไม่ควรถือว่าภาพหลายเฟรมที่ generate แยกกันจะรักษา pivot และ motion continuity ได้เอง ต้องมีขั้นตอน cleanup และตรวจ animation เช่นเดียวกับงานวาด

### 3. นำเข้าและทำวัสดุใน Unity

ใช้ URP ที่โปรเจกต์มีอยู่ แยกวัสดุ body แบบ alpha, core แบบ additive, ribbon แบบ UV scroll/dissolve และ ground layer ให้ชัด คุม filter ตามชนิด asset: ตัวละครพิกเซลรักษาความคม ส่วนริ้ว/แสงทดลอง bilinear ที่ขนาดแสดงจริง อย่าเพิ่ม bloom จนรูปทรงและช่องว่างหาย

ParticleSystem เล่น flipbook จาก texture sheet ได้ทั้งตามอายุและตาม FPS ตาม [Unity Texture Sheet Animation](https://docs.unity3d.com/6000.0/Documentation/Manual/PartSysTexSheetAnimModule.html) จึงเริ่มจากระบบที่ติดตั้งอยู่แล้วได้ ไม่จำเป็นต้องเพิ่ม VFX Graph สำหรับ prototype นี้

### 4. ประกอบใน VFX preview scene

ใช้มุมกล้องและสเกลตัวละครจาก arena จริง มีหุ่นเป้า พื้นสว่าง พื้นมืด และผนังด้านหลัง วาง anchor อย่างน้อยที่เท้า มือ/อาวุธ กลางลำตัว และจุดเป้าหมายที่ล็อกไว้ตอนเริ่ม effect

ประกอบตามลำดับ: pose และ main shape -> ground contact -> target response -> secondary particles -> scene focus -> sound pass. บันทึกคลิปและ contact sheets กลับมาเทียบ reference ทุกครั้งที่เปลี่ยน timing สำคัญ การ scrub/replay ต้อง reset particle, วัสดุ, แสงและกล้องจนเล่นซ้ำได้เหมือนเดิม

### 5. ทำ profile และเชื่อมข้อมูลเกม

ให้ profile เลือก main shape, prefab, anchors, release/impact markers, ระยะเวลา tail และระดับ presentation ตาม `skill_id` ธาตุใช้ช่วยเลือกวัสดุหรือ variation รองลงมา ท่าธาตุเดียวกันสามารถมีรูปทรงและการเคลื่อนไหวต่างกันได้มาก

ท่าที่ซับซ้อนใช้ Timeline ซึ่งติดตั้งอยู่แล้วที่ 1.8.13; Control Track จัด particle/prefab และ Signal Track ส่ง cue ได้ตาม [Unity Timeline tracks](https://docs.unity3d.com/Packages/com.unity.timeline@1.8/manual/trk-add.html) ใช้ signal เป็นคำสั่งภาพ/เสียงเท่านั้น ไม่มี signal ใดทำ damage หรือเลือกผล gameplay

เอฟเฟกต์กระแทกสั้นที่เล่นบ่อยใช้ prefab แบบ pooled และ controller เล็ก ๆ ได้ ไม่ต้องมี PlayableDirector หนึ่งตัวต่อสะเก็ด กลไก pooling ต้องคืนค่า material overrides, trails, particle seed, parent และ transform ทุกครั้ง

### 6. ตรวจในสนามจริง

ตรวจ 1 ตัวและทั้งปาร์ตี้โจมตีพร้อมกัน, ASPD สูง, เป้าหมายตายระหว่าง flight, เปลี่ยนห้อง, ปิดหน้าต่าง presentation กลางท่า และ render FPS ต่ำ วัด CPU/GPU และ transparent overdraw บนเครื่องเป้าหมายก่อนตั้งงบจำนวน particle ถ้าต้องลดรายละเอียด ให้ลด motes, smoke และ glow ก่อนลดรูปทรงหลักที่บอกชนิดท่า

## จุดต่อกับ ROI ที่มีอยู่จริง

| ของเดิม | ใช้ต่ออย่างไร | สิ่งที่ต้องระวัง |
| --- | --- | --- |
| `ArenaProjectiles.cs` | ใช้ `spawn_tick`, `arrive_at_tick`, ตำแหน่งต้น/ปลาย และ `skill_id` กำกับ flight | ปัจจุบันเลือก arrow/fireball และ tint เป็นหลัก; การหายจากรายการไม่ได้พิสูจน์ว่าโดน เพราะ target อาจตายหรือเปลี่ยนฉาก |
| `ArenaView.cs`: `recent_hits`, `skill_fx`, `field_hits`, `heal_fx` | ทำ adapter แปลงเป็นคำสั่งภาพตาม skill/target | `recent_hits` ไม่มี skill id; อย่าจับคู่ทุก hit กับ cast โดยเดาหากมีหลายเหตุการณ์ซ้อนกัน |
| `view_state.rs`: `pending_skill`, casting และ cast-field hints | ใช้เริ่มท่าเตรียมก่อน resolve | DTO ปัจจุบันของ ArenaView ยังไม่ประกาศ `pending_skill`; preview อาจเปลี่ยน ต้อง cancel หรือ retarget ได้ |
| `ArenaSparks`, `ArenaShards`, `ArenaBreakFlash` | ใช้แนวทาง particle, flash และ geometry เดิมเป็นฐาน | กำหนดผู้รับผิดชอบ impact ให้ชัดเพื่อไม่ปล่อย generic spark ซ้ำกับ skill impact |
| `ArenaView.FocusOnBreak` / `TickFocus` / `EndFocus` | มีการคุมแสง กล้อง และคืนค่าอยู่แล้ว | อย่าให้ controller ของสกิลเขียนกล้อง/ไฟแข่งกับ Break; ใช้ owner/priority เดียว |
| `SimDriver.StateUpdated` | จุดรับ view snapshot จาก sim | publish หลัง tick batch; ต้องตรวจ event loss/duplicate ตอนหลาย tick ต่อ frame ก่อนพึ่ง transient lists เป็น event stream |

ไฟล์อ้างอิง: [ArenaProjectiles](../../../unity/ROI/Assets/Scripts/Arena/ArenaProjectiles.cs), [ArenaView](../../../unity/ROI/Assets/Scripts/Arena/ArenaView.cs), [SimDriver](../../../unity/ROI/Assets/Scripts/Sim/SimDriver.cs), [view_state](../../../crates/sim-host/src/view_state.rs), [SkillFx/HitEvent](../../../crates/sim-core/src/state.rs)

```text
sim state + authoritative outcomes
    -> view adapter: windup / flight / resolved hit / heal
    -> skill profile + target anchors + presentation priority
    -> pose + mesh/flipbook + particles + target response
    -> bounded tail -> cleanup / return to pool
```

จุด resolve ต้องตรงกับผลจริง: ถ้าเพิ่งรับ `skill_fx` ตอน damage เกิดแล้ว ห้ามเริ่มชาร์จเต็มความยาวย้อนหลังจนภาพโจมตีมาช้ากว่า HP ให้ข้ามช่วงเตรียมหรือเข้า impact ตาม phase ที่ได้รับ Projectile ต้องเดินตามเวลาจาก sim; tail สามารถเล่นต่อหลัง resolve ได้

สำหรับข้อมูลที่ไม่พอ ให้เพิ่ม renderer metadata ที่ขอบเขต view เท่าที่พิสูจน์ว่าจำเป็น ไม่สมมติว่ามี combat event สมบูรณ์อยู่แล้ว ต้องระบุ ID/sequence ของ event เมื่อจำเป็นต้องแยกหลายเหตุการณ์ชนิดเดียวใน tick เดียว และกำหนดวิธีเก็บข้าม tick batch ก่อนเชื่อมท่าที่พลาดไม่ได้

`SimDriver` ใช้ unscaled time และมี `Paused` อยู่แล้วสำหรับ presentation; Break ปัจจุบันเรียกใช้มันจริง ข้อเสนอสำหรับ VFX ปกติคือใช้ local presentation clock และ freeze เฉพาะภาพตามต้องการ ส่วน cinematic ที่ใช้ pause ต้องเข้าทางเจ้าของ pause เดิมและคืนสิทธิ์อย่างถูกต้อง การตั้ง `Time.timeScale = 0` อย่างเดียวไม่หยุด driver ตัวนี้

## ขนาดงานที่ควรเริ่ม

**แนะนำเริ่มจากท่าฟันบนสนาม 1 ท่าให้ครบวงจร** เพราะได้ทดสอบ pose, main shape, impact sync, target flash และ readability พร้อมกัน ใช้ช่วง 16.33-19.27 s เป็น reference รูปทรง ไม่คัดลอกความยาวหรือจำนวน hit จากคลิปลง gameplay

1. Physical slash: pose เตรียม/กระแทก/จบ, คมฟัน body+core, impact burst, สะเก็ด และ target response ให้ดูดีในกล้องปกติก่อน
2. Fire burst: เติมริ้ววง, core ที่หดก่อนแตก, radial burst, flame flipbook และ ground contact เพื่อพิสูจน์ pipeline เอฟเฟกต์หลายชั้น
3. Signature skill หนึ่งท่า: เพิ่มฉากโฟกัสและ Timeline ใช้ชุด asset ที่ทำแล้ว พร้อม compact variant สำหรับเล่นซ้ำ
4. หลังสามตัวอย่างผ่านจึงขยายเป็น library ตามรูปทรง: slash, thrust, projectile, radial burst, column และ support aura แล้วค่อยทำ variation ตามหกธาตุของ ROI: Physical, Fire, Lightning, Ice, Light, Dark

สำหรับ idle combat ควรให้ท่าปกติเล่นในสนามต่อเนื่อง ท่าพิเศษใช้การโฟกัสเท่าที่จำเป็น และให้ full cinematic เป็น presentation tier ที่จำกัดความถี่ มี Reduced FX และลด screen flash ได้ เมื่อหลายสกิลชนกันให้รักษาภาพบนสนามของทุกท่า แต่มีเจ้าของการเปลี่ยนกล้อง/ฉากเพียงตัวเดียว ไม่เก็บคัตอินเก่าจนการแสดงผลตามการต่อสู้ไม่ทัน

เกณฑ์ผ่าน prototype: มองออกว่าใครโจมตีใครและตรงไหนในขนาดหน้าต่างจริง, impact ตรงผล sim, รูปทรงหลักยังชัดเมื่อปิด bloom, ไม่มี effect ซ้ำ, และหลังถูกขัดจังหวะกล้อง/แสง/actor คืนค่าเสมอ งานชุดนี้เป็นเอกสารออกแบบและหลักฐานภาพ ยังไม่มีการแก้ runtime หรือทดสอบประสิทธิภาพของ implementation

## Contact sheets

ภาพรวมทั้งคลิป: [01](frames/labeled-overview-01.jpg), [02](frames/labeled-overview-02.jpg), [03](frames/labeled-overview-03.jpg), [04](frames/labeled-overview-04.jpg), [05](frames/labeled-overview-05.jpg), [06](frames/labeled-overview-06.jpg), [07](frames/labeled-overview-07.jpg), [08](frames/labeled-overview-08.jpg), [09](frames/labeled-overview-09.jpg)

รายละเอียดเพิ่มเติม: [ทอง](frames/gold-charge-01.jpg), [เตรียมดาบ](frames/sword-prepare-01.jpg), [ดาบกระแทก](frames/sword-hits-01.jpg), [ดาบช่วงต่อ](frames/sword-hits-02.jpg), [น้ำแข็ง](frames/ice-impact-01.jpg)
