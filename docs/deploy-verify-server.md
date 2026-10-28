# Deploy verify-server to Fly.io (PoC test)

เอา verify-server (S2/S3 replay-verify) ขึ้น host จริง เพื่อ **ลองยิงจากเน็ตจริง**.
นี่คือ PoC test deploy — **ยังไม่ใช่ production** (in-memory, ไม่มี DB/backup/hardening —
ดู "80% ที่เหลือ" ใน s3-economy-design / server-plan). ราคา: ~ฟรี (scale-to-zero, 256MB).

Files (repo root): `Dockerfile` · `.dockerignore` · `fly.toml`.

## ครั้งแรก (one-time)

```bash
# 1) ติดตั้ง flyctl
curl -L https://fly.io/install.sh | sh          # แล้ว restart shell / add to PATH

# 2) login (สมัคร Fly ถ้ายังไม่มี — ต้องผูกบัตร แต่ tier นี้ ~ฟรี)
fly auth login

# 3) สร้าง app จาก fly.toml ที่ให้ไว้ (เลือกชื่อ unique) — ยังไม่ deploy
fly launch --copy-config --no-deploy --name <ชื่อ-unique-ของคุณ> --region sin
```

`fly launch` จะอ่าน `fly.toml` + `Dockerfile` ที่มีอยู่. ถ้ามันถามจะสร้าง Postgres/Redis →
**ตอบ No** (เรายัง in-memory). ถ้ามันแก้ `app = ` ในไฟล์ให้เป็นชื่อที่เลือก — ok.

## Deploy

```bash
fly deploy                       # build image (บน builder ของ Fly) + rollout
fly status                       # ดู machine ตื่นไหม
```

## ลองยิงจริง (แทน localhost ด้วย URL จริง)

```bash
URL=https://<ชื่อapp>.fly.dev

curl $URL/health                                   # -> ok

# login → token
TOKEN=$(curl -s -X POST $URL/auth/dev -H 'content-type: application/json' \
  -d '{"name":"luna"}' | python3 -c "import sys,json;print(json.load(sys.stdin)['token'])")

# verify a real bundle end-to-end (server สร้าง demo bundle ให้)
curl -s $URL/demo/bundle -o /tmp/b.json
curl -s -X POST $URL/verify -H 'content-type: application/json' --data @/tmp/b.json

# ทั้ง economy flow (register → market → ship → mint) ใช้ endpoint เดียวกับที่ demo local
```

## Ops เบื้องต้น

```bash
fly logs                         # ดู log สด
fly scale count 1                # ปิด scale-to-zero (always-on) ถ้าอยากไม่มี cold start
fly apps destroy <ชื่อapp>        # ลบทิ้งตอนเลิกลอง (กันโดนชาร์จ)
```

## เช็คก่อน (ถ้ามี Docker ในเครื่อง — verify ก่อน deploy)

```bash
docker build -t verify-server .
docker run -p 8080:8080 verify-server
curl http://127.0.0.1:8080/health   # -> ok
```

## ⚠️ อย่าเข้าใจผิด
deploy นี้ = **PoC ขึ้นเน็ต** (in-memory, restart = state หาย, ไม่มี auth จริง/DB/backup).
เอาไว้ "ลองของ + เห็นมันตอบจากเน็ต" เท่านั้น. Production ต้อง: Postgres + Steamworks จริง +
economy-invariants harness + ops (ดู docs/s2-server-build-plan.md, s3-economy-design.md).
