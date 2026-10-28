# Unity UI — ส่งต่องานให้ Luna 5.6

อัปเดต 2026-09-14 — สรุปวิธีทำงานและสถานะล่าสุดของ Home / Exemplar / Equipment
เอกสารนี้เป็นจุดเริ่มสำหรับงาน UI กราฟิก ส่วนกฎโครงการอ่าน [CLAUDE.md](../CLAUDE.md)
และบล็อกที่เกี่ยวข้องใน [TODO.md](../TODO.md) ก่อนลงมือ

## 1. เปิดให้ถูก worktree ก่อน

| รายการ | ค่า |
| --- | --- |
| Worktree ที่มีงานล่าสุด | `/Users/lunalieneray/idle_mmorpg-unity-graphics` |
| Branch | `codex/unity-graphics-ui` |
| Unity project | `/Users/lunalieneray/idle_mmorpg-unity-graphics/unity/ROI` |
| Unity version | `6000.0.83f1` |
| Graphics HEAD ตอนเขียน | `71d284c2` |
| Unity MCP instance ที่ใช้ล่าสุด | `ROI@ae78bbd46c76d37e` — ตรวจ path อีกครั้งเมื่อเชื่อมต่อใหม่ |

**งาน Home/Exemplar ล่าสุดจำนวนมากยังเป็น modified/untracked และยังไม่ได้ commit**
เปิด branch ใหม่จาก HEAD อย่างเดียวจะไม่ได้งานนี้มาด้วย ต้องทำต่อใน worktree ข้างบน
ตรวจ `git status --short --branch` ก่อนแก้ และรักษาของที่ค้างอยู่ทั้งหมด

`/Users/lunalieneray/idle_mmorpg` คือ shared master ของอีกงาน; อย่าแก้/สลับ branch ที่นั่น
เพื่อทำ UI นี้ และอย่าใช้ Unity ของอีก session/JRPG เป็นตัว preview โดยเดาว่าเป็น Editor เดียวกัน
ชื่อ project เป็น ROI เหมือนกันได้ ให้ตรวจ `Application.dataPath` เป็นหลัก

งานส่งต่อรอบนี้เป็นเอกสารเท่านั้น ไม่ได้ commit, merge, deploy, เปลี่ยนโมเดล หรือเปิด task ใหม่

## 2. ดีไซน์ที่ตกลงแล้ว — อย่าย้อนกลับไปเวอร์ชันเก่า

### Home

- รับเป็น prototype แล้ว: fantasy library + Live2D เยื้องซ้าย ปุ่มหลักอยู่ขวา
- ใช้ perspective แบบ Arknights ผ่าน **camera จริง + World Space Canvas**;
  ไม่ใช่หมุน overlay ทั้งแผงให้เอียงทิศเดียวกัน
- ปุ่ม ivory/gold, แถว Store/Excavation สี blue/teal, ฟอนต์ใหญ่หนา มี watermark และ dots
- Story มีแถบ Coming Soon, Event ซ้อนบน Story; Exemplars/Party อยู่แถวเดียวกัน
- Mission/Workshop มี Inventory ซ้อนใต้ Workshop; Store อยู่ซ้าย Excavation
- Excavation มี Recruitment ด้านบน, Tartarus ตัวเล็ก และ **หมาป่าหันข้าง** เป็น watermark
  (Cerberus เป็นของเก่า)
- Options/Mail เล็กที่ซ้ายบน, Gil/Soul/Gem อยู่ใน perspective เดียวกับ deck
- สลับ Menu/Combat ด้วยปุ่มขวาบน; combat simulation ต้องเดินต่อขณะเปิดเมนู

ภาพที่ยึด: [Home checkpoint](previews/unity-home-menu-wolf-management.png)
รายละเอียด: [Home layout](unity-home-menu-layout.md)

### Exemplar

- ผสม WuWa + Epic Seven + HSR: slate ระดับกลาง, ข้อความ ivory, champagne เล็กน้อย
  ไม่มืด/สว่างเกินไป รองรับตัวละครหลาย artist ที่มีสีหลากหลาย แต่ไม่ใช่โทนร่าเริงจัด
- พื้นหลังดวงดาวมี animation เบา ๆ; Live2D อยู่กลาง; nav แคบซ้ายและ roster เล็กขวา
- Nav เรียง **Attributes → Regalia → Equipment → Potential → Apex → Resonance**
- Regalia อันดับ 2; ไม่มี Advance; Potential ไม่แสดงคำว่า Board
- Paths เป็นของทั้งทีม อยู่ Home → Party → Paths; Provision อยู่ฝั่ง Combat
- Nav เป็น icon ด้านบน/ชื่อด้านล่าง ไม่ย้อนกลับเป็นแถบกว้าง
- Model 67 เป็น Live2D placeholder ร่วมทุกตัว ไม่ใช่ภาพจริงของ Seraphine/Rapis
  รูปใน roster ใช้ swordsman/mage ชั่วคราวตามที่ผู้ใช้อนุญาต
- Menu Live2D เล่นที่ 0.5× เฉพาะ instance; cut-in คงความเร็วเดิม
  ไม่แก้สีผ้าคลุมหรือ global grading เพื่อทำให้ตัวหนังสืออ่านออก

ภาพตัวอย่าง: [Attributes](previews/unity-exemplar-medium-attributes.png)
ประวัติ/ข้อจำกัดแต่ละหน้า: [Exemplar prototype](unity-exemplar-prototype.md)

### Equipment และ Change Equipment — งานล่าสุด

- Gear slots เป็นสี่เหลี่ยมจัตุรัส ~97px ที่ 1920×1080; **icon เป็นสัญลักษณ์สีขาวของ slot**
  ไม่ใช่ภาพอาวุธ/ชุดเกราะลงสี
- กล่อง slot เป็นกรมเข้ม ความทึบ empty/occupied/selected = 0.90/0.95/0.98
- ชื่อ slot มีแถบดำด้านหลัง; คอลัมน์อุปกรณ์ขวาขยับเข้าใกล้ detail panel
- Stat panel ใช้ค่าตัวใหญ่ เส้นแบ่งกลุ่มบาง ๆ; ไม่มีพื้นสลับแถวแบบ zebra
- Change Equipment เป็น **หน้าต่าง ไม่ใช่ fullscreen** — ผู้ใช้ลองแล้วเลือกกลับมาแบบหน้าต่าง
- ซ้าย: ของที่สวมอยู่; กลาง: อุปกรณ์ slot เดียวกัน **4 ชิ้นต่อแถว + vertical scroll**;
  ขวา: ชิ้นที่เลือกพร้อมค่าของเดิม → ของใหม่ และ delta เขียว/แดง
- คลิกการ์ดเป็น preview เท่านั้น; Equip เป็นขั้นตอนแยก
- รวมของในกระเป๋าและของที่ตัวละครลงทีมสวมอยู่; การ์ดแสดง avatar/ชื่อเจ้าของ
  ของที่ตัวเองใส่อยู่ดูได้ แต่ Equip ซ้ำไม่ได้
- ย้ายจากอีกตัวละครต้องยืนยันชื่อ item/เจ้าของเดิม/ผู้รับก่อน
- **คลิกนอกกรอบปิดเฉพาะหน้าต่างนี้ ห้าม click-through ไปเปลี่ยนหน้า/ตัวละคร/Combat**
  ต้องคลิกอีกครั้งจึงใช้ UI ด้านหลังได้; Back/Escape ปิด picker ก่อนออกจาก Equipment

ภาพฐานหน้า Equipment: [slot labels](previews/unity-exemplar-equipment-label-backplates.png)
ภาพล่าสุด: [Change Equipment](previews/unity-change-equipment-window.png)
และ [ยืนยันย้ายของ](previews/unity-change-equipment-confirm.png)
สองภาพหลังใช้ข้อมูล fixture เพื่อให้มีของหลายชิ้น ไม่ใช่เซฟจริงของผู้ใช้

## 3. โครงไฟล์: จะแก้อะไรให้เริ่มตรงไหน

ไฟล์ด้านล่างอยู่ใต้ `unity/ROI/Assets/Scripts/` เว้นแต่ระบุเป็นอย่างอื่น

| งาน | ไฟล์หลัก |
| --- | --- |
| Menu/Combat และ Back routing | `UI/ModeTabs.cs`, จุดเชื่อมใน `UI/HudBar.cs` |
| Home layout / ป้าย / เส้นทางปุ่ม | `UI/HomeMenuLayout.cs` |
| Perspective, render target, raycast | `UI/HomeMenuDepth.cs` |
| Hover depth / ขอบหนา / veil | `UI/HomeCardLift.cs`, `HomePanelThickness.cs`, `HomeMenuVeil.cs` |
| Wallet / emblems | `UI/HomeWalletBar.cs`, `HomeCurrencyIcon.cs`, `HomeIconSet.cs` |
| Live2D camera และ framing | `UI/HomePortrait.cs`, `ExemplarPortrait.cs` |
| พื้นหลังดาวขยับ | `UI/CelestialMenuMotion.cs`, `Resources/ExemplarUI/CelestialMotion.shader` |
| Exemplar nav / roster / inspected ID | `UI/ExemplarScreen.cs` |
| shared UI helpers / palette / Attributes | `UI/ExemplarUi.cs` |
| ข้อมูลตัวละครจาก host | `UI/ExemplarReadModel.cs` |
| หน้า Regalia / Apex / Resonance | `UI/ExemplarRegaliaPage.cs`, `ExemplarApexPage.cs`, `ExemplarResonancePage.cs` |
| Potential | `UI/PotentialWindow.cs`; route ภายในยังชื่อ `Skills` |
| Equipment layout / selection / action routing | `UI/ExemplarEquipmentPage.cs` |
| หน้าต่าง 4-column / owner badge / outside scrim | `UI/ExemplarEquipmentPicker.cs` |
| Current/selected details และ comparison | `UI/ExemplarGearDetails.cs` |
| สีพื้น gradient/shadow และ reveal | `UI/EquipmentSurface.cs`, `EquipmentDetailReveal.cs` |
| slot texture/category mapping | `UI/ExemplarEquipmentIcon.cs` |
| อ่าน ownership / frozen choice / stale guards | `UI/ExemplarEquipmentActions.cs` |
| ตรวจผลย้าย item และการรักษาจำนวนของ | `UI/ExemplarEquipmentTransfer.cs` |
| native preflight + live input sequence | `Sim/VerifiedInputSequence.cs`, `Sim/SimDriver.cs` |
| modal confirmation | `UI/UiConfirm.cs` |

UI ชุดนี้ประกอบจาก C# `Create/Build/Show` เป็น retained uGUI ในตอนเปิดเกม
การขยับ hierarchy ใน Play ใช้ลองดูได้ แต่ **ไม่ใช่การบันทึก source ของ UI**
ถ้าจะให้ติดถาวร ต้องแก้ builder/asset ในไฟล์ที่เป็นเจ้าของ layout นั้น

### จุดที่ต้องรักษาเมื่อแก้ Equipment picker

`ExemplarEquipmentPicker.Create` ย้าย root ไปเป็น sibling ใต้ `ExemplarScreen` โดยตั้งใจ
จึงต้อง hide จาก `ExemplarEquipmentPage.OnDisable` และ destroy จาก `OnDestroy` เอง
อย่าย้ายกลับไปใต้กรอบ Equipment เพราะ scrim ต้องครอบคลุม nav/roster ด้วย

- `OutsideDismiss` เป็น Button เต็มจอ; `PickerWindow.raycastTarget=true` กันคลิกพื้นที่ว่างในกรอบ
- Picker OnEnable/OnDisable ปิด/คืน `ModeTabs.SetModeSwitchInputEnabled` เพื่อให้คลิก Combat
  ครั้งแรกโดน scrim; อย่าถอดส่วนนี้เพียงเพราะเห็นปุ่ม Combat ยังอยู่บนจอ
- เก็บ scroll position ขณะ snapshot refresh; reset เมื่อเปลี่ยนสมาชิก/slot/รายการจริงเท่านั้น
- รักษา frozen `Choice`, session/ID/raw-data guards และ `_confirmationVersion` ใน callback ยืนยัน
  ผู้ใช้เปลี่ยนหน้า/ปิดหน้าต่างแล้ว callback เก่าต้องทำงานไม่ได้
- Equipment ตรวจว่ามี HudBar จริงก่อนเรียก UiConfirm เพราะ helper เดิมมีทาง auto-accept เมื่อไม่มี HUD

## 4. วิธีทำ UI ต่อหนึ่งหน้า

1. อ่านข้อสรุปล่าสุดกับภาพของหน้านั้นก่อน เอกสารเก่ามีหลาย checkpoint อย่าเอาค่าจากหัวข้อ
   `Prior` หรือ `Initial` มาทับของล่าสุดโดยไม่ดูท้ายไฟล์
2. ใช้ `ExemplarUi.Rect/Panel/Text/Button` และ TMP สำหรับ Exemplar;
   Home ใช้ระบบ card/perspective ที่มีอยู่ ไม่สร้าง rendering stack ซ้ำ
3. ให้หนึ่ง component รับผิดชอบหนึ่งหน้า/ส่วน; `Build()` สร้าง hierarchy ครั้งเดียว,
   `Show()` bind snapshot และปรับ retained controls ไม่ Destroy/Create รายการใหม่ทุก tick
   รายการยาวใช้ pooled cards กับ `UiScroll.Create/Fit`
4. แยกผู้ที่กำลัง inspect ออกจากตัว lead/party; ใช้ ID และ session จริง
   การเปิดหน้า/คลิก preview ต้องไม่ส่ง input หรือ deploy/bench ตัวละครเพื่อให้ query ใช้ได้
5. เริ่มจาก layout และข้อมูลจริงก่อน แล้วค่อยปรับ typography, icon, surface และ animation
   ขอภาพ generated ใหม่เมื่อจำเป็น; ถ้าแก้ padding/contrast ให้แก้ code/วัสดุโดยตรง
6. ใช้ขนาดอ้างอิง 1920×1080; anchors ของ `ExemplarUi` เป็น `(left,bottom,right,top)`
   คุมพื้นที่ว่างระหว่าง nav, portrait, slots, detail และ roster ตรวจทั้ง FullHD และหน้าต่างเล็ก
   Slot ต้องใช้ `AspectRatioFitter.WidthControlsHeight` เพื่อรักษาสัดส่วน 1:1
7. แยกข้อความ hierarchy: title → main value → secondary/lore; อย่าแก้เบลอด้วย outline หนา
   ติดตั้งขอบ/ลายน้ำ/ภาพประกอบเป็น `raycastTarget=false` เว้นแต่เป็น click/scroll blocker โดยตั้งใจ
8. Input ที่เปลี่ยนของ/ใช้ทรัพยากรต้องผ่าน action helper และ `SimDriver`;
   pending confirmation ต้องจับ immutable choice + ตรวจ session/ID/collection ก่อนทำจริง
9. ใช้ `Loc.Get(...)` และ `GameText.*`; เพิ่ม EN/TH ใน `godot/i18n/strings.csv`
   และ mirror ไป `unity/ROI/Assets/StreamingAssets/strings.csv` อย่าทิ้ง raw localization key บนจอ
10. ตรวจ compile, ดูภาพจริง, ลอง click/scroll/Back และบันทึก capture กับข้อจำกัดใน doc
    เปลี่ยนสี/padding ไม่จำเป็นต้องเขียน test ชุดใหม่; เปลี่ยน action/ownership ต้องทดสอบผลจริง

Animation ใช้ local unscaled time และ cleanup ใน OnDisable/OnDestroy ไม่เปลี่ยน
`Time.timeScale`, shared animation clip หรือ scene exposure เพื่อแก้ปัญหาเฉพาะ UI
Combat sim ต้องยังเดินขณะเปิด Home/Exemplar แม้กล้อง arena จะถูกซ่อน/คืนตาม lifecycle

## 5. Asset และความคม

- Home: `Resources/HomeUI/LibraryBackground.png`, `CastleBanner.png`,
  `MenuEmblems-Clean.png`, **`TartarusWolfEmblem.png`**
- Home heading: `HomeUI/HomeHeading.asset` จาก Barlow Condensed Semibold;
  Cormorant/HomeDisplay เป็น reference เก่า ไม่ใช่ font หลักปัจจุบัน
- Home Inventory ใช้ vector รูปลังเก็บของ ไม่ใช่ moneybag จาก atlas เก่า
- Exemplar: `Fonts/NotoSans UI SDF` ผ่าน UiTheme/ExemplarUi; อย่าเปลี่ยน font ทั้งเกมเพื่อจูนหน้าเดียว
- พื้นหลัง: `ExemplarUI/CelestialBackground-Medium.png` กับ CelestialMenuMotion
- Slot symbols: `Resources/ExemplarUI/GearSymbols/` มี Sword, Staff, Shield, Armor,
  Cloak, Boots, Crown, Ring, Mask, Aura; category ที่ยังไม่มีภาพใช้ vector fallback
- เก็บต้นฉบับ RGBA 1254px; Unity import max512, uncompressed, alpha transparency,
  mipmaps, trilinear, clamp ปัจจุบันผ่านการดูจริงที่ขนาด slot แล้ว
- ภาพ gear ลงสีที่ gen ก่อนผู้ใช้แก้ความหมายถูกย้ายไป
  `docs/design-checkpoints/gear-painted-unused/` ไม่ได้ใช้เป็น slot icon
- Prompt/source ของ bitmap: [slot symbols](unity-equipment-slot-symbol-prompts.md),
  [celestial BG](unity-exemplar-art-prompt.md), [Home icons](unity-home-icon-prompt.md),
  [wolf](unity-home-wolf-prompt.md)

Home perspective มีข้อเฉพาะ: composite เป็น premultiplied alpha, watermark ใช้ stencil
`Mask` (อย่าสลับเป็น RectMask2D บน card ที่ฉาย perspective), กล้องใช้ HomeUIRenderer
slot 1. PC มี MSAA4; Mobile มีคุณภาพ AA/render scale ต่างกัน จึงไม่ถือว่าภาพ PC รับรอง Mobile
Home deck และ wallet ใช้ camera เดียวกัน; pointer probes ต้องใช้ `HomeMenuDepth.EventCamera`
RT เป็น native resolution 4×MSAA ไม่ใช่ supersampling; เก็บ deferred destroy ของ RT เก่าไว้
เพื่อไม่ release texture ที่ Metal ยังใช้งานอยู่ ส่วน portrait แยกกล้อง/RT บน layer 30
Exemplar scroll viewport ปกติใช้ RectMask2D ได้ เพราะเป็น UI ระนาบ screen space

## 6. วงจรแก้และ preview ใน Unity

1. ตรวจ instance และ project path ก่อนทุก session; instance ID ด้านบนเป็นข้อมูลครั้งล่าสุด
2. **หยุด Play ก่อนแก้ C#** แล้วค่อย refresh/compile; เคยเกิด native-handle invalidation
   จาก domain reload ระหว่างเล่น และมี crash artifact เก่าค้างอยู่ อย่าเริ่มด้วยลบทุกอย่าง
3. ตรวจ console errors แล้วค่อยเข้า Play อีกครั้ง ไม่ rebuild/copy native dylib เพื่อจูน UI
4. ใช้ dev preview ที่มีอยู่เพื่อเลี่ยงการแตะ online account; ไม่ใช่การออกแบบเกม offline

ตัวอย่าง C# method body สำหรับ Unity MCP `execute_code` (เครื่องมือนี้ใช้ C#6;
source project เองใช้ syntax ใหม่กว่าได้):

```csharp
// อ่านอย่างเดียว: ยืนยันว่าเป็น graphics project และดู native version จริง
return UnityEngine.Application.dataPath + " | " + Roi.Sim.SimSession.SimVersion();
```

```csharp
// ก่อนแก้ scripts
UnityEditor.EditorApplication.isPlaying = false;
```

หลังแก้ เรียก Unity MCP refresh/compile และตรวจ console จากนั้น:

```csharp
System.Environment.SetEnvironmentVariable("ROI_BOOT", "dev");
System.Environment.SetEnvironmentVariable("ROI_STEAM_PIPE", "0");
UnityEditor.EditorApplication.isPlaying = true;
```

รอ boot เสร็จ แล้วอีก call:

```csharp
UnityEngine.Object.FindFirstObjectByType<Roi.UI.ModeTabs>().OpenExemplars();
var screen = UnityEngine.Object.FindFirstObjectByType<Roi.UI.ExemplarScreen>();
screen.Inspect(1); // ตรวจ roster จริงก่อน; starter fixture ใช้ 1=Seraphine, 2=Rapis
screen.SelectPage("Equipment"); // Potential ใช้ "Skills"
```

อย่า hardcode ID 1/2 ใน production เพื่อเลือกเจ้าของของ; snippet นี้ไว้เปิด preview เท่านั้น
ถ้าอ่านเฉพาะเอกสาร ไม่ต้องเปิด Play หรือเปลี่ยน Editor ให้ผู้ใช้

Capture ใช้ขนาดปกติเท่านั้น:

```csharp
UnityEngine.ScreenCapture.CaptureScreenshot(
    UnityEngine.Application.dataPath + "/../Temp/CodexCaptures/ui-preview.png");
```

สร้างโฟลเดอร์ปลายทางก่อน, รออย่างน้อยอีก frame แล้วค่อยอ่าน PNG
**ห้ามใช้ supersize screenshot กับ Metal/Cubism ชุดนี้** เพราะเคยทำ Editor crash
GameView ที่ใช้ล่าสุด: Free Aspect สำหรับ pointer probes, FullHD สำหรับ capture,
และคืน 16:9 เมื่อจบ; อย่าเชื่อ index ของ GameView sizes หาก Editor เปลี่ยนรายการ
คัดลอกภาพที่เลือกไป `docs/previews/` และแจ้งถ้าเป็น fixture ไม่ใช่เซฟจริง

## 7. ตรวจอะไร และผ่านอะไรแล้ว

ผล gameplay/UI ด้านล่างเป็นผลจากรอบ implementation ก่อนหน้า ไม่ได้รันใหม่ในรอบเขียนเอกสารนี้

จาก graphics worktree:

```bash
git diff --check
python3 tools/check_loc_keys.py
```

ถ้าแก้ Equipment actions เรียกใน Unity:

```csharp
return EquipmentTransferVerification.Run();
```

Runner อยู่ `Assets/Editor/EquipmentTransferVerification.cs` สร้าง disabled driver/session
ของตนเองและ dispose ใน finally ไม่แก้ gameplay driver/เซฟผู้ใช้
ผลล่าสุดบน native **0.0.346-alpha: 10/10 scenarios, 260 assertions, 0 failures**
ตรวจ bag equip, transfer/คืนของเดิม, item conservation, stale bag/source/target/identity/session,
slot ผิด, ของชื่อซ้ำ และ publish เมื่อจบ sequence พร้อมเก็บสอง input ใน log

ผล UI ล่าสุด: 4 columns, scroll ถึงล่างและไม่ reset ทุก refresh, preview/Cancel ไม่ส่ง input,
ยืนยันย้ายของทำงาน, คลิกนอกที่ตำแหน่ง Combat ครั้งแรกปิด picker/ครั้งที่สองจึงสลับหน้า,
ตัวเลข comparison ไม่ overflow, compile 0 errors, localization 2,011 keys
ไม่ได้อ้างว่ารอบนี้ทดสอบ online server replay หรือ Mobile ครบ

อ่าน [testing-notes](testing-notes.md) ก่อนเพิ่ม tests อย่าเขียน test ที่เพียงลอก implementation
หากต้องการของหลายชิ้นให้ใช้ fixture แยก ห้าม Debug grant ลง session ผู้ใช้
คืน driver จริง, ทำลาย fixture และปิด confirmation ทดสอบก่อนส่งงาน

## 8. ข้อจำกัดที่ต้องรู้ก่อนทำต่อ

- Native ที่ preview ล่าสุดคือ 346 แต่ source/master เดินต่อได้; ตรวจ capability จริงก่อน
  ไม่แก้ class alias/catalog หรืออัปเกรด DLL/server เพียงเพื่อซ่อนข้อจำกัดของ UI
- Starter weapons ยังมี legacy wieldable matrix; guard ป้องกันถอดแล้วใส่กลับไม่ได้อยู่ใน
  ExemplarEquipmentActions. นี่ไม่ใช่ font/icon bug
- Bench ยังไม่มี enriched gear/effective-stat projection และ equip handler ต้องใช้ deployed ID
  แสดง unavailable ตามจริง; อย่าแอบนำตัวลงทีมเพื่อให้กดได้
- Comparison ปัจจุบันเป็น **ค่าของ item** (ATK/MATK/DEF + native effects) ไม่ใช่คำนวณ total
  character stats ล่วงหน้าเองใน renderer
- ไม่มี native atomic transfer input: client preflight สำเนา state ที่ hash ตรงกันเฉพาะตอนยืนยัน
  ตรวจผล แล้วส่ง Unequip/Equip ใน live session แบบ synchronous พร้อม Publish ครั้งเดียว
  อย่ายุบให้เป็นถอดก่อนแล้วค่อยลองใส่, อย่าเอา cloned state ไปแทน live session
- Resonance ของตัวละครเริ่มต้นยังไม่มี per-unit progression ครบ; ใช้เฉพาะ linked carrier จริง
  Apex ต้องอ่าน schema ของ native ไม่ hardcode core/substat จาก source คนละเวอร์ชัน
- Home และหน้าอื่น ๆ เป็น prototype; submenus หลายอันยังเปิดหน้าระบบ legacy ภายใน
  และยังไม่ได้ redesign ทั้งหมด

ถ้าจะเริ่มงานต่อ ให้ยึดคอมเมนต์ผู้ใช้ล่าสุดกับ Equipment picker นี้ก่อน หน้าอื่น ๆ ปรับตาม
คำสั่งถัดไป ไม่ต้องรื้อ Home ที่รับเป็น prototype แล้วหรือขยายไปทำ backend เอง

## Prompt สำหรับส่งต่อ

> ทำงาน Unity UI ต่อใน `/Users/lunalieneray/idle_mmorpg-unity-graphics` branch
> `codex/unity-graphics-ui`. อ่าน `CLAUDE.md` และ `docs/unity-ui-luna-handoff.md` ก่อน
> งานล่าสุดยังมี modified/untracked ต้องรักษาไว้และใช้ worktree นี้จริง ไม่เปิดจาก HEAD ใหม่
> ยึด Home/Exemplar style ที่รับแล้ว และ Change Equipment แบบหน้าต่าง 4 คอลัมน์ + scroll
> คลิกข้างนอกปิดอย่างเดียว ไม่มี click-through. เปิด Unity instance ให้ตรง project path
> หยุด Play ก่อนแก้ scripts แล้วตรวจภาพและ interaction จริงหลัง compile
> รับคอมเมนต์ถัดไปของผู้ใช้เพื่อทำต่อ โดยไม่เปลี่ยนกฎเกม/เซฟ/อีก session
