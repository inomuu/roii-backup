# Steam Integration — SDK + auth (client/server split)

**Status:** เริ่ม 2026-07-12 (App สร้างแล้ว, ยังไม่ wire). App ID = **4958230** ("ROI: Reign of Idle",
partner Nuttapong Ketam).
**Companion:** `steam-market-requirements.md` (inventory/market), `production-server-plan.md` (prod + secrets),
`anti-cheat-and-server-model.md` (client-sim + verify), `online-robustness.md` (session/auth flow).

## 0. หลัก — ไม่ใช่ทุกอย่างผ่าน Godot

client เชื่อไม่ได้ (หลัก anti-cheat) → มี **ขา server บังคับเสมอ**:
- **Client (Godot + GodotSteam)** = Steamworks SDK บนเครื่องผู้เล่น → อ่าน state local / ขอ ticket / overlay
- **Server (verify-server + Steam Web API)** = HTTP + **publisher Web API key (secret)** → **validate + mint (authoritative)**

**กฎ:** mint มูลค่า / ให้สิทธิ์ / ต้องเชื่อถือ → **server**. อ่าน local / ขอ ticket / แสดงผล → **Godot**.

## 1. ของที่ต้องเตรียม

| item | ค่า / ที่มา | secret? |
|---|---|---|
| App ID | **4958230** — `godot/steam_appid.txt` (วางแล้ว) | ไม่ (public) |
| GodotSteam addon | **GDExtension 4.4+** (ครอบ 4.6; อิง GodotSteam 4.20 + SDK 1.64) — AssetLib / codeberg.org/godotsteam | ไม่ |
| Publisher Web API key | Steamworks → Users & Permissions → API key | **ใช่ → `fly secrets set STEAM_PUBLISHER_KEY` เท่านั้น; ห้าม repo/chat** |

GodotSteam = GDExtension ตัวที่ 2 (อยู่ร่วมกับ gdext `godot-ext` ของเราได้ คนละ class). ติดตั้ง = drop zip ลง
project base / `addons/` → **restart Godot เต็ม** (native ext, ไม่ hot-reload — ตรงกับ reloadable=false).
macOS gotcha: `xattr -dr com.apple.quarantine` ที่ addon ถ้า dylib ไม่โหลด.

## 2. Auth flow (แทน PoC login)

```
Client (Godot)                         Server (/auth/steam)                 Steam Web API
 steamInitEx(4958230)
 getAuthSessionTicket()  ──ticket(hex)──►  ISteamUserAuth/                ─►  validate + คืน steamid จริง
                                            AuthenticateUserTicket(key,appid,ticket)
                                          ─► ISteamUser/CheckAppOwnership ─►  ยืนยันเป็นเจ้าของ
                          ◄──session token──  ผูก steamid → account (แทน PoC)
```

**Client เรียก:** `steamInitEx()` · `getAuthSessionTicket()` (+ callback `get_auth_session_ticket_response`)
· `steamRunCallbacks()` ทุก frame. steamid ที่ client รู้ = ห้ามส่งให้ server เชื่อ — server derive จาก ticket.

**Server เรียก (Web API + key, ห้ามผ่าน Godot):**
- `AuthenticateUserTicket` → **จุด trust หลัก** (ticket แท้ / ออกให้ app เรา / ไม่หมดอายุ/ban) → ได้ steamid
- `CheckAppOwnership` → ownership (anti-piracy; ยังสำคัญแม้ F2P สำหรับบาง entitlement)
- ออก session token ของเรา → เข้าต่อ single-session / save-verify pipeline เดิม

**ทำไม server เลี่ยงไม่ได้:** client ปลอม steamid/ownership ได้; validate ต้องใช้ publisher key = secret = server-only.

## 3. Broader integration (ต่อไป — pattern เดียวกัน)

| งาน | ฝั่ง | API |
|---|---|---|
| ขอ ticket / overlay / `restartAppIfNecessary(4958230)` (soft anti-piracy) | Godot | SDK |
| **mint/grant item** (marketable) | **Server** | `IInventoryService/AddItem` + `SetItemProperties` (key) = จุด mint-on-verify |
| read/แสดง inventory | Godot | `inventoryGetAllItems` (read-only) |
| consume item | **Server** | `IInventoryService/ConsumeItem` |
| **IAP (2nd party / Exemplar)** | **Server** | `ISteamMicroTxn/InitTxn` + `FinalizeTxn`; Godot เปิด overlay |

ทั้งหมด: authoritative = server + key; client = trigger/display. ผูกกับ mint-on-verify + entitlement
([[rare-slot-purchase-design]] / [[multi-party-account-session-split]] / [[character-skins-idea]]).

## 4. สถานะ / ลำดับ

**AUTH ทำงาน end-to-end แล้ว (พิสูจน์ 2026-07-12: steamid 76561198005045179 → account 13 ใน Neon):**
- [x] `steam_appid.txt` = 4958230
- [x] **GodotSteam GDExtension 4.4+** ติดตั้ง (`godot/addons/godotsteam/`) — client SDK + `getAuthTicketForWebApi`
- [x] client: `net/steam_auth.gd` (init + web-ticket, 5s timeout) + ปุ่ม **LOGIN WITH STEAM** แยกใน mode-select
- [x] **server `/auth/steam`** — `AuthenticateUserTicket` (reqwest rustls, 5s timeout) → resolve steamid → `login_as("steam:<id>")`; ticket **hex-validated** (กัน URL-injection); **NO identity param** (empty ทำ Steam error 104); reuse account/single-session logic
- [x] `STEAM_PUBLISHER_KEY` = fly secret (General perm, "Everyone" group; whitelist ว่างไว้ก่อน)
- [x] fail → re-show mode-select (ไม่ค้าง placeholder)

### MicroTxn — ตรวจกับ Steam จริงแล้ว (2026-08-02, sandbox)

ยิง `ISteamMicroTxnSandbox` ตรง ๆ แทนที่จะเดาจากเอกสาร ได้ข้อเท็จจริง 4 ข้อ:

| | |
|---|---|
| `GetUserInfo` | `result:OK` · `country:TH` · `currency:THB` · **`status:Trusted`** ⇒ MTX เปิดให้แอปแล้ว ไม่มีอะไรค้างใน Steamworks |
| `GetUserInfo` เป็น **GET** | ส่วนที่เหลือของ MicroTxn เป็น POST — ยิงผิดวิธีได้ `Method Not Allowed` ซึ่งดูไม่เหมือนปัญหาเรื่องสกุลเงินเลย |
| `InitTxn` ต้องมีผู้ใช้**อยู่ในเกม** | ไม่งั้น `errorcode 7: User not logged in` ⇒ ทดสอบด้วย curl เปล่าไม่ได้ ต้องเปิดเกมผ่าน Steam ค้างไว้ |
| **USD ข้ามสกุลได้** | บัญชี THB รับทั้ง `currency=USD amount=199` และ `currency=THB amount=6900` ได้ transid ทั้งคู่ |

ข้อสุดท้ายเปลี่ยนสถานะของตารางราคาต่อสกุล: **ไม่ใช่ข้อบังคับ** ประเทศที่ไม่ได้ตั้งราคาจะขายได้ปกติด้วย USD
เหตุผลที่ยังตั้งราคาท้องถิ่นคือ Steam จะแปลงสกุลด้วยเรตของตัวเองเมื่อกระเป๋าเป็นคนละสกุล ผู้เล่นอาจจ่ายแพงกว่า
ราคาป้ายท้องถิ่น — เป็นเรื่องประสบการณ์ผู้เล่น ไม่ใช่เรื่องเทคนิค

### ⚠️ ทดสอบ MicroTxn ต้องเปิดเกมผ่าน Steam เท่านั้น (2026-08-02)

`Steam.isOverlayEnabled()` = **false** เมื่อรัน build จาก Godot/เชลล์ ทั้งที่ `steamInitEx` ผ่าน,
`isSteamRunning()` true และ `loggedOn()` true — Steam client ฉีด overlay เข้าเฉพาะเกมที่**มันเปิดเอง**

ผลคือ InitTxn สำเร็จ (มี transid) แต่ไม่มีหน้าต่างให้ผู้เล่นกดอนุมัติ ⇒ `QueryTxn` ค้างที่
`status: "Init"` ตลอดกาล เส้นทางทั้งหมดถูกต้อง ขาดแค่ทางให้ผู้ใช้กด

⇒ **ทดสอบการซื้อ sandbox แบบครบวงต้องมี build บน Steam** (อัปโหลดขึ้น branch แล้วติดตั้งผ่าน
Steam client) รันจาก Godot ไม่พอ ยังทำไม่ได้จนกว่าเกมจะพร้อมขึ้น Steam

`StoreView._can_show_overlay()` เช็คก่อนเรียก checkout — ไม่งั้นจะสร้างออเดอร์ค้างไว้ที่ Steam
สำหรับการซื้อที่ไม่มีทางสำเร็จ และหน้าจอจะบอกให้ผู้เล่นไปกดหน้าต่างที่ไม่มีอยู่

**เหลือ:**
- [ ] **prod: ปิด `/auth/dev`** (env gate) → Steam-only login
- [ ] Inventory Service + itemdef แรก (`tradable/marketable:false`) — เทส grant/consume ก่อน release
- [ ] LOW deferred: persona name แทน `steam:<id>` (display); `ownersteamid` (บล็อก family-sharing? = policy); share `reqwest::Client` (perf); whitelist egress IP; `restartAppIfNecessary` (soft launch-through-Steam)

## 5. Release gates (App Admin 2026-07-12)

- store page 'Coming Soon' ต้อง visible **≥2 สัปดาห์** ก่อน release
- **≥21 วัน** นับจากซื้อ app credit ครั้งแรก
- review: store 2-3 biz days, build 3-5 biz days
- store page + build ทำคู่กันได้ทุกลำดับ
