#!/usr/bin/env python3
"""Convert Unity particle-effect prefabs (Shuriken, Particles/Standard Unlit materials) into Godot 4
scenes: one .tscn per prefab, GPUParticles3D + ParticleProcessMaterial + StandardMaterial3D.

    unity_vfx_to_godot.py <pack_root> <godot_project_dir> [--only <substr>] [--report]

<pack_root> is the exported Unity folder holding Prefabs/ Materials/ Textures/ Models/ (+ .meta).
Assets are expected (already copied) under <godot>/assets/vfx/hovl/{textures,models}; meshes as
extracted .res (see tools/extract_hovl_meshes.gd). Scenes go to
<godot>/proto/arena3d/fx/hovl/<Category>/<Name>.tscn.

Unity YAML is parsed with a small purpose-built parser (no PyYAML on this machine). Everything
that has no Godot equivalent is listed in the per-scene report instead of being silently dropped.
"""
from __future__ import annotations

import math
import os
import re
import sys

# ----------------------------------------------------------------------------- YAML (Unity subset)

_NUM = re.compile(r"^-?(\d+\.?\d*(e[-+]?\d+)?|\.\d+(e[-+]?\d+)?|inf|nan)$", re.I)


def _scalar(s: str):
    s = s.strip()
    if s == "" or s == "~" or s == "null":
        return None
    if (s[0] == "'" and s[-1] == "'") or (s[0] == '"' and s[-1] == '"'):
        return s[1:-1]
    if len(s) == 32 and all(ch in "0123456789abcdef" for ch in s):
        return s                                   # GUID — never a number (0000…e000… parses as a float!)
    if _NUM.match(s):
        try:
            return int(s)
        except ValueError:
            return float(s)
    return s


def _flow(s: str, i: int):
    """Parse a flow value ({...} / [...] / scalar) starting at s[i]; returns (value, next_i)."""
    while i < len(s) and s[i] == " ":
        i += 1
    if i >= len(s):
        return None, i
    c = s[i]
    if c == "{":
        d = {}
        i += 1
        while True:
            while i < len(s) and s[i] in " ,":
                i += 1
            if s[i] == "}":
                return d, i + 1
            j = s.index(":", i)
            key = s[i:j].strip()
            val, i = _flow(s, j + 1)
            d[key] = val
    if c == "[":
        lst = []
        i += 1
        while True:
            while i < len(s) and s[i] in " ,":
                i += 1
            if s[i] == "]":
                return lst, i + 1
            val, i = _flow(s, i)
            lst.append(val)
    if c in "'\"":
        j = s.index(c, i + 1)
        return s[i + 1:j], j + 1
    j = i
    depth = 0
    while j < len(s) and not (depth == 0 and s[j] in ",}]"):
        j += 1
    return _scalar(s[i:j]), j


def _parse_block(lines: list[str], i: int, indent: int):
    """Parse a mapping or list block whose entries sit at column `indent`."""
    if i >= len(lines):
        return None, i
    if lines[i][indent:].startswith("- "):
        return _parse_list(lines, i, indent)
    return _parse_map(lines, i, indent)


def _indent(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def _parse_map(lines, i, indent):
    d = {}
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
            continue
        ind = _indent(line)
        if ind < indent or line[ind:].startswith("- "):
            break
        if ind > indent:
            raise ValueError(f"unexpected indent at line {i}: {line!r}")
        body = line[indent:]
        j = body.find(":")
        key = body[:j].strip()
        rest = body[j + 1:].strip()
        if rest == "":
            # nested block (or empty)
            if i + 1 < len(lines) and _indent(lines[i + 1]) > indent and lines[i + 1].strip():
                val, i = _parse_block(lines, i + 1, _indent(lines[i + 1]))
            elif i + 1 < len(lines) and _indent(lines[i + 1]) == indent and lines[i + 1][indent:].startswith("- "):
                val, i = _parse_list(lines, i + 1, indent)
            else:
                val, i = None, i + 1
        else:
            val, _ = _flow(rest, 0)
            i += 1
        d[key] = val
    return d, i


def _parse_list(lines, i, indent):
    lst = []
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
            continue
        ind = _indent(line)
        if ind != indent or not line[ind:].startswith("- "):
            break
        rest = line[indent + 2:]
        if rest.lstrip().startswith("{") or rest.lstrip().startswith("["):
            val, _ = _flow(rest, 0)
            lst.append(val)
            i += 1
        elif ":" in rest and not rest.lstrip().startswith("'"):
            # mapping item: first key on this line, continuation keys at indent+2
            lines[i] = " " * (indent + 2) + rest
            val, i = _parse_map(lines, i, indent + 2)
            lst.append(val)
        else:
            lst.append(_scalar(rest))
            i += 1
    return lst, i


def parse_unity_yaml(text: str) -> list[dict]:
    """-> [{'type': int, 'id': int, 'class': str, 'data': dict}, ...]"""
    docs = []
    cur = None
    for raw in text.splitlines():
        if raw.startswith("%"):
            continue
        if raw.startswith("--- !u!"):
            if cur:
                docs.append(cur)
            m = re.match(r"--- !u!(\d+) &(-?\d+)", raw)
            cur = {"type": int(m.group(1)), "id": int(m.group(2)), "lines": []}
            continue
        if cur is not None:
            cur["lines"].append(raw)
    if cur:
        docs.append(cur)
    out = []
    for d in docs:
        lines = d["lines"]
        cls_line = next((k for k, l in enumerate(lines) if l.strip()), None)
        if cls_line is None:
            continue
        cls = lines[cls_line].rstrip(":").strip()
        data, _ = _parse_map(lines, cls_line + 1, 2)
        out.append({"type": d["type"], "id": d["id"], "class": cls, "data": data})
    return out


# ----------------------------------------------------------------------------- helpers

def guid_map(root: str) -> dict[str, str]:
    m = {}
    for dp, _, fns in os.walk(root):
        for fn in fns:
            if fn.endswith(".meta"):
                p = os.path.join(dp, fn)
                with open(p, "r", encoding="utf-8", errors="replace") as fh:
                    for line in fh:
                        if line.startswith("guid:"):
                            m[line.split(":", 1)[1].strip()] = p[:-5]
                            break
    return m


def fnum(v, default=0.0) -> float:
    return float(v) if isinstance(v, (int, float)) else default


def mm_range(mm: dict) -> tuple[float, float]:
    """MinMaxCurve -> (min, max) constants (curves fall back to their scalar)."""
    if not mm:
        return 0.0, 0.0
    st = mm.get("minMaxState", 0)
    s = fnum(mm.get("scalar"))
    ms = fnum(mm.get("minScalar"))
    if st == 3:
        return min(s, ms), max(s, ms)
    if st == 2:
        return s * curve_avg(mm.get("minCurve")), s * curve_avg(mm.get("maxCurve"))
    if st == 1:
        return s, s
    return s, s


def curve_keys(c: dict) -> list[tuple[float, float, float, float]]:
    if not c or not c.get("m_Curve"):
        return [(0.0, 1.0, 0.0, 0.0), (1.0, 1.0, 0.0, 0.0)]
    ks = []
    for k in c["m_Curve"]:
        ks.append((fnum(k.get("time")), fnum(k.get("value")), fnum(k.get("inSlope")), fnum(k.get("outSlope"))))
    return ks


def curve_avg(c: dict) -> float:
    ks = curve_keys(c)
    return sum(v for _, v, _, _ in ks) / len(ks)


def _lin(v: float) -> float:
    """sRGB -> linear for the pack's authored (gamma-space) colours; HDR values pass through."""
    if v <= 0.0:
        return 0.0
    if v >= 1.0:
        return v
    return ((v + 0.055) / 1.055) ** 2.4 if v > 0.04045 else v / 12.92


def col(c: dict, linear: bool = False) -> tuple[float, float, float, float]:
    """Unity colour -> (r, g, b, a). Particle colours stay as authored (gamma-space values): the
    hovl_particle shader multiplies in sRGB space like Unity did. `linear=True` only for lit
    materials / lights that Godot shades itself."""
    r, g_, b, a = fnum(c.get("r"), 1), fnum(c.get("g"), 1), fnum(c.get("b"), 1), fnum(c.get("a"), 1)
    if linear:
        return (_lin(r), _lin(g_), _lin(b), a)
    return (r, g_, b, a)


def gradient_stops(g: dict) -> list[tuple[float, tuple[float, float, float, float]]]:
    nc = int(g.get("m_NumColorKeys", 2))
    na = int(g.get("m_NumAlphaKeys", 2))
    cks = [(fnum(g.get(f"ctime{i}")) / 65535.0, col(g.get(f"key{i}", {}))) for i in range(nc)]
    aks = [(fnum(g.get(f"atime{i}")) / 65535.0, col(g.get(f"key{i}", {}))[3]) for i in range(na)]
    cks.sort(); aks.sort()

    def lerp_keys(keys, t, pick):
        if t <= keys[0][0]:
            return pick(keys[0][1])
        for (t0, v0), (t1, v1) in zip(keys, keys[1:]):
            if t0 <= t <= t1:
                f = 0.0 if t1 == t0 else (t - t0) / (t1 - t0)
                a, b = pick(v0), pick(v1)
                return tuple(x + (y - x) * f for x, y in zip(a, b))
        return pick(keys[-1][1])

    times = sorted({t for t, _ in cks} | {t for t, _ in aks} | {0.0, 1.0})
    stops = []
    for t in times:
        rgb = lerp_keys(cks, t, lambda c: c[:3])
        a = lerp_keys(aks, t, lambda v: (v,))[0]
        stops.append((t, (rgb[0], rgb[1], rgb[2], a)))
    return stops


def q_to_basis(q: dict, s: dict):
    """Unity (left-handed) quaternion + scale -> Godot basis columns (Z mirrored)."""
    x, y, z, w = fnum(q.get("x")), fnum(q.get("y")), fnum(q.get("z")), fnum(q.get("w"), 1)
    x, y = -x, -y                                      # mirror across the XY plane
    xx, yy, zz = x * x, y * y, z * z
    r = [
        [1 - 2 * (yy + zz), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (xx + zz), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (xx + yy)],
    ]
    sx, sy, sz = fnum(s.get("x"), 1), fnum(s.get("y"), 1), fnum(s.get("z"), 1)
    cols = [[r[0][0] * sx, r[1][0] * sx, r[2][0] * sx],
            [r[0][1] * sy, r[1][1] * sy, r[2][1] * sy],
            [r[0][2] * sz, r[1][2] * sz, r[2][2] * sz]]
    return cols


def unity_euler_dir(rot: dict, v=(0.0, 0.0, 1.0)) -> tuple[float, float, float]:
    """Rotate a Unity-local vector by a Unity Euler (Quaternion.Euler: R = Ry * Rx * Rz), then
    mirror Z for Godot. Default vector = Unity's shape axis (+Z)."""
    ex, ey, ez = (math.radians(fnum(rot.get(k))) for k in ("x", "y", "z"))
    x, y, z = v
    # Rz
    x, y = x * math.cos(ez) - y * math.sin(ez), x * math.sin(ez) + y * math.cos(ez)
    # Rx
    y, z = y * math.cos(ex) - z * math.sin(ex), y * math.sin(ex) + z * math.cos(ex)
    # Ry
    x, z = x * math.cos(ey) + z * math.sin(ey), -x * math.sin(ey) + z * math.cos(ey)
    return (x, y, -z)


def gvec(v) -> str:
    return f"Vector3({g(v[0])}, {g(v[1])}, {g(v[2])})"


def g(v: float) -> str:
    """Float literal that Godot will parse as FLOAT (Curve._data rejects ints — always keep a '.')."""
    s = f"{v:.5g}" if abs(v) < 1e5 else f"{v:.6e}"
    if "." not in s and "e" not in s and "n" not in s:
        s += ".0"
    return s


def gcolor(c) -> str:
    return f"Color({g(c[0])}, {g(c[1])}, {g(c[2])}, {g(c[3])})"


def sanitize(name: str) -> str:
    n = re.sub(r"[.:@/\"%]", "_", name).strip()
    return n or "Node"


# ----------------------------------------------------------------------------- scene writer

class Scene:
    def __init__(self):
        self.ext: list[str] = []
        self.ext_ids: dict[str, str] = {}
        self.sub: list[str] = []
        self.sub_n = 0
        self.nodes: list[str] = []
        self.report: list[str] = []

    def ext_res(self, typ: str, path: str) -> str:
        key = typ + "|" + path
        if key not in self.ext_ids:
            i = f"ext{len(self.ext_ids) + 1}"
            self.ext_ids[key] = i
            self.ext.append(f'[ext_resource type="{typ}" path="{path}" id="{i}"]')
        return f'ExtResource("{self.ext_ids[key]}")'

    def sub_res(self, typ: str, props: dict) -> str:
        self.sub_n += 1
        i = f"{typ}_{self.sub_n}"
        body = "\n".join(f"{k} = {v}" for k, v in props.items())
        self.sub.append(f'[sub_resource type="{typ}" id="{i}"]\n{body}')
        return f'SubResource("{i}")'

    def curve_tex(self, keys, vmax: float) -> str:
        data = []
        for t, v, si, so in keys:
            data.append(f"Vector2({g(t)}, {g(v)}), {g(si)}, {g(so)}, 0, 0")
        curve = self.sub_res("Curve", {
            "min_value": "0.0", "max_value": g(max(vmax, 1.0)),
            "_data": "[" + ", ".join(data) + "]", "point_count": str(len(keys))})
        return self.sub_res("CurveTexture", {"curve": curve})

    def gradient_tex(self, stops) -> str:
        offs = ", ".join(g(t) for t, _ in stops)
        cols = ", ".join(", ".join(g(x) for x in c) for _, c in stops)
        grad = self.sub_res("Gradient", {"offsets": f"PackedFloat32Array({offs})",
                                         "colors": f"PackedColorArray({cols})"})
        return self.sub_res("GradientTexture1D", {"gradient": grad})

    def write(self, path: str, root_name: str):
        steps = len(self.ext) + len(self.sub) + 1
        out = [f"[gd_scene load_steps={steps} format=3]", ""]
        out += self.ext + [""]
        out += [s + "\n" for s in self.sub]
        out.append(f'[node name="{root_name}" type="Node3D"]')
        out += self.nodes
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write("\n".join(out) + "\n")


# ----------------------------------------------------------------------------- materials

class Assets:
    def __init__(self, pack: str, godot: str):
        self.pack = pack
        self.godot = godot
        self.guids = guid_map(pack)
        self.mats: dict[str, dict] = {}
        for guid, p in self.guids.items():
            if p.endswith(".mat"):
                with open(p, "r", encoding="utf-8", errors="replace") as fh:
                    docs = parse_unity_yaml(fh.read())
                if docs:
                    self.mats[guid] = docs[0]["data"]

    def tex_path(self, guid: str) -> str | None:
        p = self.guids.get(guid)
        if not p:
            return None
        return "res://assets/vfx/hovl/textures/" + os.path.basename(p)

    def mesh_path(self, guid: str) -> str | None:
        p = self.guids.get(guid)
        if not p:
            return None
        return "res://assets/vfx/hovl/models/" + os.path.splitext(os.path.basename(p))[0] + ".res"

    @staticmethod
    def mat_props(m: dict):
        sp = m.get("m_SavedProperties", {})
        floats = {}
        for e in sp.get("m_Floats", []) or []:
            floats.update(e)
        colors = {}
        for e in sp.get("m_Colors", []) or []:
            colors.update(e)
        texs = {}
        for e in sp.get("m_TexEnvs", []) or []:
            texs.update(e)
        return floats, colors, texs

    def material(self, sc: Scene, guid: str, render_mode: int, tiles=(1, 1)) -> str:
        m = self.mats.get(guid)
        if m is None:
            sc.report.append(f"material {guid} missing")
            return sc.sub_res("StandardMaterial3D", {"shading_mode": "0", "vertex_color_use_as_albedo": "true"})
        floats, colors, texs = self.mat_props(m)
        name = m.get("m_Name", guid)
        shader_id = (m.get("m_Shader") or {}).get("fileID", 211)
        props: dict[str, str] = {"resource_name": f'"{name}"'}
        main = texs.get("_MainTex", {}) or {}
        tex_guid = ((main.get("m_Texture") or {}).get("guid"))
        if shader_id == 46 or shader_id == 210:                    # Standard / Standard Surface (lit)
            props["albedo_color"] = gcolor(col(colors.get("_Color", {})))
            if tex_guid and self.tex_path(tex_guid):
                props["albedo_texture"] = sc.ext_res("Texture2D", self.tex_path(tex_guid))
            bump = ((texs.get("_BumpMap", {}) or {}).get("m_Texture") or {}).get("guid")
            if bump and self.tex_path(bump):
                props["normal_enabled"] = "true"
                props["normal_texture"] = sc.ext_res("Texture2D", self.tex_path(bump))
            ms = ((texs.get("_MetallicGlossMap", {}) or {}).get("m_Texture") or {}).get("guid")
            if ms and self.tex_path(ms):
                props["metallic"] = "1.0"
                props["metallic_texture"] = sc.ext_res("Texture2D", self.tex_path(ms))
                props["metallic_texture_channel"] = "0"
            if float(floats.get("_Mode", 0)) >= 2:
                props["transparency"] = "1"
            return sc.sub_res("StandardMaterial3D", props)
        # Particles/Standard Unlit -> our hovl_particle.gdshader (StandardMaterial3D would convert
        # the HDR _Color sRGB->linear and blow it out; see the shader header)
        mode = int(floats.get("_Mode", 2))
        if mode in (4, 5, 6):
            sc.report.append(f"material {name}: blend mode {mode} (add/sub/mul) not in hovl_particle.gdshader (mix used)")
        c = col(colors.get("_Color", {"r": 1, "g": 1, "b": 1, "a": 1}), linear=False)   # HDR multiplier, keep as authored
        sp = {"shader": sc.ext_res("Shader", "res://proto/arena3d/fx/hovl/hovl_particle.gdshader"),
              "resource_name": f'"{name}"',
              "shader_parameter/tint": f"Vector4({g(c[0])}, {g(c[1])}, {g(c[2])}, {g(c[3])})"}
        if tex_guid and self.tex_path(tex_guid):
            sp["shader_parameter/albedo_tex"] = sc.ext_res("Texture2D", self.tex_path(tex_guid))
            sca = main.get("m_Scale") or {}
            off = main.get("m_Offset") or {}
            if fnum(sca.get("x"), 1) != 1 or fnum(sca.get("y"), 1) != 1:
                sp["shader_parameter/uv_scale"] = f"Vector2({g(fnum(sca.get('x'), 1))}, {g(fnum(sca.get('y'), 1))})"
            if fnum(off.get("x")) or fnum(off.get("y")):
                sp["shader_parameter/uv_offset"] = f"Vector2({g(fnum(off.get('x')))}, {g(fnum(off.get('y')))})"
        if int(floats.get("_SoftParticlesEnabled", 0)):
            sp["shader_parameter/soft_distance"] = g(max(0.05, fnum(floats.get("_SoftParticlesFarFadeDistance"), 1)))
        if render_mode == 0 or tiles != (1, 1):
            sp["shader_parameter/billboard"] = "1"
        if tiles != (1, 1):
            sp["shader_parameter/h_frames"] = str(tiles[0])
            sp["shader_parameter/v_frames"] = str(tiles[1])
        props = sp
        if int(floats.get("_DistortionEnabled", 0)):
            sc.report.append(f"material {name}: distortion -> using fx/hovl/distortion.gdshader")
            bump = ((texs.get("_BumpMap", {}) or {}).get("m_Texture") or {}).get("guid")
            sp = {"shader": sc.ext_res("Shader", "res://proto/arena3d/fx/hovl/distortion.gdshader"),
                  "shader_parameter/strength": g(fnum(floats.get("_DistortionStrengthScaled"), 0.5) * 0.2)}
            if tex_guid and self.tex_path(tex_guid):
                sp["shader_parameter/mask"] = sc.ext_res("Texture2D", self.tex_path(tex_guid))
            if bump and self.tex_path(bump):
                sp["shader_parameter/normal_map"] = sc.ext_res("Texture2D", self.tex_path(bump))
            return sc.sub_res("ShaderMaterial", sp)
        return sc.sub_res("ShaderMaterial", props)


# ----------------------------------------------------------------------------- prefab -> scene

UNITY_QUAD = 10210
UNITY_SPHERE = 10207


def convert_prefab(path: str, assets: Assets, out_dir: str, category: str) -> tuple[str, list[str]]:
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        docs = parse_unity_yaml(fh.read())
    by_id = {d["id"]: d for d in docs}
    gos = {d["id"]: d["data"] for d in docs if d["type"] == 1}
    trs = {d["id"]: d["data"] for d in docs if d["type"] == 4}
    comps_by_go: dict[int, list[dict]] = {}
    for d in docs:
        go = (d["data"].get("m_GameObject") or {}).get("fileID")
        if go is not None:
            comps_by_go.setdefault(go, []).append(d)
    if any(d["type"] == 1001 for d in docs):
        pass
    sc = Scene()
    roots = [tid for tid, t in trs.items() if (t.get("m_Father") or {}).get("fileID", 0) == 0]
    if not roots:
        return "", ["no root transform"]
    root_name = sanitize(gos.get(trs[roots[0]]["m_GameObject"]["fileID"], {}).get("m_Name", "Effect"))
    ps_nodepath: dict[int, str] = {}                    # ParticleSystem doc id -> node path (for sub-emitters)
    pending_sub: list[tuple[str, int, dict]] = []

    def node_transform(t: dict) -> str:
        pos = t.get("m_LocalPosition") or {}
        cols = q_to_basis(t.get("m_LocalRotation") or {}, t.get("m_LocalScale") or {})
        flat = [c for colv in cols for c in colv]
        ox, oy, oz = fnum(pos.get("x")), fnum(pos.get("y")), -fnum(pos.get("z"))
        return "Transform3D(" + ", ".join(g(v) for v in flat) + f", {g(ox)}, {g(oy)}, {g(oz)})"

    def emit_node(tid: int, parent_path: str, used: set[str]):
        t = trs[tid]
        goid = t["m_GameObject"]["fileID"]
        go = gos.get(goid, {})
        name = sanitize(go.get("m_Name", "Node"))
        base = name
        k = 2
        while name in used:
            name = f"{base}{k}"; k += 1
        used.add(name)
        comps = comps_by_go.get(goid, [])
        ps = next((c for c in comps if c["type"] == 198), None)
        psr = next((c for c in comps if c["type"] == 199), None)
        mf = next((c for c in comps if c["type"] == 33), None)
        mr = next((c for c in comps if c["type"] == 23), None)
        light = next((c for c in comps if c["type"] == 108), None)
        my_path = name if parent_path == "." else f"{parent_path}/{name}"
        active = go.get("m_IsActive", 1)
        props = {"transform": node_transform(t)}
        if not active:
            props["visible"] = "false"
        if ps is not None:
            ntype = "GPUParticles3D"
            props.update(particle_props(sc, assets, ps["data"], psr["data"] if psr else {}, name))
            ps_nodepath[ps["id"]] = my_path
            sub = ps["data"].get("SubModule") or {}
            if sub.get("enabled") and sub.get("subEmitters"):
                pending_sub.append((my_path, len(sc.nodes), sub))
        elif mf is not None and mr is not None:
            ntype = "MeshInstance3D"
            mguid = (mf["data"].get("m_Mesh") or {}).get("guid")
            mp = assets.mesh_path(mguid) if mguid else None
            if mp:
                props["mesh"] = sc.ext_res("Mesh", mp)
            mats = mr["data"].get("m_Materials") or []
            if mats and mats[0].get("guid"):
                props["material_override"] = assets.material(sc, mats[0]["guid"], 4)
        elif light is not None:
            ntype = "OmniLight3D"
            ld = light["data"]
            props["light_color"] = gcolor(col(ld.get("m_Color", {})))
            props["light_energy"] = g(fnum(ld.get("m_Intensity"), 1))
            props["omni_range"] = g(fnum(ld.get("m_Range"), 5))
            props["shadow_enabled"] = "false"
        else:
            ntype = "Node3D"
        body = "\n".join(f"{k2} = {v}" for k2, v in props.items())
        sc.nodes.append(f'\n[node name="{name}" type="{ntype}" parent="{parent_path}"]\n{body}')
        for ch in t.get("m_Children") or []:
            cid = ch.get("fileID")
            if cid in trs:
                emit_node(cid, my_path, used)

    # root's own components live on the root Node3D: emit root children only, but if the root GO has
    # a ParticleSystem itself, wrap it as a child node named after it.
    root_t = trs[roots[0]]
    root_go = root_t["m_GameObject"]["fileID"]
    used: set[str] = set()
    if any(c["type"] in (198, 33, 108) for c in comps_by_go.get(root_go, [])):
        # emit the root as a child with identity transform so its components are kept
        saved = dict(root_t)
        root_t_copy = dict(root_t)
        root_t_copy["m_LocalPosition"] = {"x": 0, "y": 0, "z": 0}   # rotation/scale of the root are KEPT
        trs[roots[0]] = root_t_copy
        emit_node(roots[0], ".", used)
        trs[roots[0]] = saved
    else:
        for ch in root_t.get("m_Children") or []:
            cid = ch.get("fileID")
            if cid in trs:
                emit_node(cid, ".", used)
    # sub-emitters: patch the parent node's text with the NodePath
    for my_path, idx, sub in pending_sub:
        se = sub["subEmitters"][0]
        target = (se.get("emitter") or {}).get("fileID")
        tp = ps_nodepath.get(target)
        if tp is None:
            sc.report.append(f"{my_path}: sub-emitter target not found")
            continue
        rel = os.path.relpath(tp, my_path).replace("\\", "/")
        typ = int(se.get("type", 0))
        mode = {0: 1, 2: 2, 1: 3}.get(typ, 0)           # birth->constant, death->at_end, collision
        sc.nodes[idx] += f'\nsub_emitter = NodePath("{rel}")'
        sc.report.append(f"{my_path}: sub-emitter -> {rel} (unity type {typ}, godot mode {mode}; set amount/frequency by hand)")
    if len(sub_ids := [d for d in docs if d["type"] == 1001]) > 0:
        sc.report.append(f"{len(sub_ids)} nested prefab instance(s) skipped")
    out = os.path.join(out_dir, category, root_name + ".tscn")
    sc.write(out, root_name)
    return out, sc.report


def particle_props(sc: Scene, assets: Assets, ps: dict, psr: dict, name: str) -> dict[str, str]:
    props: dict[str, str] = {}
    rep = sc.report
    init = ps.get("InitialModule") or {}
    shape = ps.get("ShapeModule") or {}
    emis = ps.get("EmissionModule") or {}
    lt_min, lt_max = mm_range(init.get("startLifetime"))
    lt_max = max(lt_max, 0.05)
    props["lifetime"] = g(lt_max)
    if lt_min < lt_max:
        props["lifetime_randomness"] = g(1.0 - lt_min / lt_max)
    looping = int(ps.get("looping", 1))
    rate_min, rate_max = mm_range(emis.get("rateOverTime"))
    bursts = emis.get("m_Bursts") or []
    burst_total = 0.0
    burst_time = 0.0
    for i_b, b in enumerate(bursts):
        _, cmax = mm_range(b.get("countCurve"))
        burst_total += cmax * max(1, int(b.get("cycleCount", 1)) or 1)
        if i_b == 0:
            burst_time = fnum(b.get("time"))
        elif fnum(b.get("time")) != burst_time:
            rep.append(f"{name}: extra burst at {g(fnum(b.get('time')))}s folded into the first")
    amount = int(math.ceil(rate_max * lt_max)) + int(math.ceil(burst_total))
    props["amount"] = str(max(1, min(amount, 4000)))
    if burst_total > 0 and rate_max == 0:
        props["explosiveness"] = "1.0"
    elif burst_total > 0:
        props["explosiveness"] = g(min(0.9, burst_total / max(1.0, amount)))
    if not looping:
        props["one_shot"] = "true"
    duration = fnum(ps.get("lengthInSec"), 1)
    if not int(ps.get("playOnAwake", 1)):
        props["emitting"] = "false"
    d_min, d_max = mm_range(ps.get("startDelay"))
    # --- cycle emulation: Godot re-emits every `lifetime` and cannot delay a burst. For burst-only
    # systems, live for the whole Unity cycle (or delay+life when one-shot) and make the particle
    # visible only inside its real window [T, T+L] via the colour ramp alpha (scale curve shifted
    # to match). Motion still starts at t=0 — fine for the short delays this pack uses.
    win = None                                                 # (f0, f1) fraction of the emitter lifetime
    if burst_total > 0 and rate_max == 0:
        T = burst_time + d_max
        if looping and duration > lt_max + 0.02:
            cycle = duration
        elif T > 0.01:
            cycle = T + lt_max
        else:
            cycle = 0.0
        if cycle > 0.0:
            win = (T / cycle, min(1.0, (T + lt_max) / cycle))
            props["lifetime"] = g(cycle)
            props.pop("lifetime_randomness", None)
            rep.append(f"{name}: burst window {g(T)}..{g(T + lt_max)}s of a {g(cycle)}s cycle emulated via alpha")
    elif d_max > 0:
        rep.append(f"{name}: startDelay {g(d_max)}s (not mapped; use a Timer or AnimationPlayer)")
    props["local_coords"] = "true" if int(ps.get("moveWithTransform", 0)) == 0 else "false"
    props["fixed_fps"] = "60"
    props["draw_order"] = "1"                                   # lifetime
    # --- process material
    pm: dict[str, str] = {}
    stype = int(shape.get("type", 0))
    radius = fnum((shape.get("radius") or {}).get("value"), 1)
    thick = fnum(shape.get("radiusThickness"), 1)
    angle = fnum(shape.get("angle"), 25)
    sp_min, sp_max = mm_range(init.get("startSpeed"))
    srot = shape.get("m_Rotation") or {}
    axis = unity_euler_dir(srot)                               # shape's +Z (cone/circle axis) in Godot space
    sscale = shape.get("m_Scale") or {}
    if stype in (0, 1):                                        # sphere
        pm["emission_shape"] = "1"
        pm["emission_sphere_radius"] = g(max(radius, 0.001))
        pm["direction"] = gvec(axis)
        pm["spread"] = "180.0"
        pm["radial_velocity_min"] = g(sp_min); pm["radial_velocity_max"] = g(sp_max)
    elif stype in (2, 3):                                      # hemisphere (+Z half in unity)
        pm["emission_shape"] = "1"
        pm["emission_sphere_radius"] = g(max(radius, 0.001))
        pm["direction"] = gvec(axis)
        pm["spread"] = "90.0"
        pm["initial_velocity_min"] = g(sp_min); pm["initial_velocity_max"] = g(sp_max)
    elif stype in (4, 7, 8, 9):                                # cone (+Z in unity -> -Z in godot)
        pm["emission_shape"] = "6"                             # RING (5 is directed points!)                             # ring = the cone base disc
        pm["emission_ring_axis"] = gvec(axis)
        pm["emission_ring_radius"] = g(max(radius, 0.001))
        pm["emission_ring_inner_radius"] = g(max(radius * (1.0 - thick), 0.0))
        pm["emission_ring_height"] = "0.0"
        pm["direction"] = gvec(axis)
        pm["spread"] = g(angle)
        pm["initial_velocity_min"] = g(sp_min); pm["initial_velocity_max"] = g(sp_max)
    elif stype in (10, 11):                                    # circle (radial in the plane)
        pm["emission_shape"] = "6"                             # RING (5 is directed points!)
        pm["emission_ring_axis"] = gvec(axis)
        pm["emission_ring_radius"] = g(max(radius, 0.001))
        pm["emission_ring_inner_radius"] = g(max(radius * (1.0 - thick), 0.0))
        pm["emission_ring_height"] = "0.0"
        pm["radial_velocity_min"] = g(sp_min); pm["radial_velocity_max"] = g(sp_max)
        pm["spread"] = "0.0"
    elif stype in (5, 15, 16):                                 # box
        pm["emission_shape"] = "3"
        pm["emission_box_extents"] = f"Vector3({g(fnum(sscale.get('x'),1)/2)}, {g(fnum(sscale.get('y'),1)/2)}, {g(fnum(sscale.get('z'),1)/2)})"
        pm["direction"] = gvec(axis)
        pm["spread"] = "0.0"
        pm["initial_velocity_min"] = g(sp_min); pm["initial_velocity_max"] = g(sp_max)
    else:
        rep.append(f"{name}: shape type {stype} (mesh/edge) -> point emission")
        pm["direction"] = gvec(axis)
        pm["spread"] = "180.0"
        pm["initial_velocity_min"] = g(sp_min); pm["initial_velocity_max"] = g(sp_max)
    spos = shape.get("m_Position") or {}
    if any(fnum(spos.get(k)) for k in "xyz"):
        pm["emission_shape_offset"] = f"Vector3({g(fnum(spos.get('x')))}, {g(fnum(spos.get('y')))}, {g(-fnum(spos.get('z')))})"
    if stype not in (5, 15, 16) and any(fnum(sscale.get(k), 1) != 1 for k in "xyz"):
        pm["emission_shape_scale"] = f"Vector3({g(fnum(sscale.get('x'),1))}, {g(fnum(sscale.get('y'),1))}, {g(fnum(sscale.get('z'),1))})"
    if fnum(shape.get("randomDirectionAmount")) > 0:
        pm["spread"] = g(min(180.0, fnum(pm.get("spread", "0")) + 180.0 * fnum(shape.get("randomDirectionAmount"))))
    grav = fnum((init.get("gravityModifier") or {}).get("scalar"))
    gvx, gvy, gvz = 0.0, -9.81 * grav, 0.0
    force = ps.get("ForceModule") or {}
    if force.get("enabled"):                                   # constant force -> folded into gravity
        fx = mm_range(force.get("x"))[1]; fy = mm_range(force.get("y"))[1]; fz = mm_range(force.get("z"))[1]
        gvx += fx; gvy += fy; gvz -= fz
        if fnum(force.get("randomizePerFrame")):
            rep.append(f"{name}: force 'randomize per frame' not mapped")
    pm["gravity"] = gvec((gvx, gvy, gvz))
    # size
    sz_min, sz_max = mm_range(init.get("startSize"))
    pm["scale_min"] = g(max(sz_min, 0.001)); pm["scale_max"] = g(max(sz_max, 0.001))
    size3d = int(init.get("size3D", 0))
    aspect = 1.0
    if size3d:
        _, sy = mm_range(init.get("startSizeY"))
        aspect = sy / sz_max if sz_max else 1.0
    sizem = ps.get("SizeModule") or {}
    scale_keys = None
    if sizem.get("enabled"):
        cv = sizem.get("curve") or {}
        keys = curve_keys(cv.get("maxCurve"))
        scal = fnum(cv.get("scalar"), 1)
        scale_keys = [(t, v * scal, si * scal, so * scal) for t, v, si, so in keys]
        if cv.get("minMaxState") == 2:
            rep.append(f"{name}: size over lifetime uses two curves; max curve used")
    # colour
    scol = init.get("startColor") or {}
    cst = scol.get("minMaxState", 0)
    if cst == 0:
        pm["color"] = gcolor(col(scol.get("maxColor", {})))
    elif cst == 3:
        pm["color_initial_ramp"] = sc.gradient_tex([(0.0, col(scol.get("minColor", {}))), (1.0, col(scol.get("maxColor", {})))])
    else:
        gr = scol.get("maxGradient") or {}
        pm["color_initial_ramp"] = sc.gradient_tex(gradient_stops(gr)) if gr else "null"
    colm = ps.get("ColorModule") or {}
    ramp_stops = None
    if colm.get("enabled"):
        gv = colm.get("gradient") or {}
        st = gv.get("minMaxState", 1)
        if st in (1, 2):
            ramp_stops = gradient_stops(gv.get("maxGradient") or {})
            if st == 2:
                rep.append(f"{name}: colour over lifetime uses two gradients; max used")
        elif st == 3:
            ramp_stops = [(0.0, col(gv.get("minColor", {}))), (1.0, col(gv.get("maxColor", {})))]
    if win is not None:
        f0, f1 = win
        eps = 0.002
        base = ramp_stops or [(0.0, (1.0, 1.0, 1.0, 1.0)), (1.0, (1.0, 1.0, 1.0, 1.0))]
        inner = [(f0 + t * (f1 - f0), c) for t, c in base]
        hidden = lambda c: (c[0], c[1], c[2], 0.0)
        stops = []
        if f0 > eps:
            stops += [(0.0, hidden(inner[0][1])), (max(0.0, f0 - eps), hidden(inner[0][1]))]
        stops += inner
        if f1 < 1.0 - eps:
            stops += [(min(1.0, f1 + eps), hidden(inner[-1][1])), (1.0, hidden(inner[-1][1]))]
        ramp_stops = stops
        if scale_keys is not None:
            ks = [(f0 + t * (f1 - f0), v, si, so) for t, v, si, so in scale_keys]
            if f0 > eps:
                ks = [(0.0, ks[0][1], 0.0, 0.0)] + ks
            if f1 < 1.0 - eps:
                ks = ks + [(1.0, ks[-1][1], 0.0, 0.0)]
            scale_keys = ks
    if ramp_stops is not None:
        pm["color_ramp"] = sc.gradient_tex(ramp_stops)
    if scale_keys is not None:
        pm["scale_curve"] = sc.curve_tex(scale_keys, max(v for _, v, _, _ in scale_keys))
    # rotation
    r_min, r_max = mm_range(init.get("startRotation"))
    if r_min or r_max:
        pm["angle_min"] = g(math.degrees(r_min)); pm["angle_max"] = g(math.degrees(r_max))
    rotm = ps.get("RotationModule") or {}
    if rotm.get("enabled"):
        av_min, av_max = mm_range(rotm.get("curve"))
        if rotm.get("curve", {}).get("minMaxState") in (1, 2):
            rep.append(f"{name}: angular velocity is a curve; averaged")
        pm["angular_velocity_min"] = g(math.degrees(av_min)); pm["angular_velocity_max"] = g(math.degrees(av_max))
    flat_quad = False
    if int(init.get("rotation3D", 0)):
        rx = mm_range(init.get("startRotationX"))
        if abs(abs(rx[1]) - math.pi / 2) < 0.2:                 # laid flat (ground circles, craters)
            flat_quad = True
        else:
            rep.append(f"{name}: 3D start rotation x={g(math.degrees(rx[1]))} not mapped")
    # velocity over lifetime
    vel = ps.get("VelocityModule") or {}
    if vel.get("enabled"):
        vx = mm_range(vel.get("x")); vy = mm_range(vel.get("y")); vz = mm_range(vel.get("z"))
        rad = mm_range(vel.get("radial"))
        orb = [mm_range(vel.get(k)) for k in ("orbitalX", "orbitalY", "orbitalZ")]
        if any(abs(v) > 1e-6 for pair in (vx, vy, vz) for v in pair):
            pm["linear_accel_min"] = "0.0"
            rep.append(f"{name}: velocity over lifetime xyz=({g(vx[1])},{g(vy[1])},{g(vz[1])}) not mapped (godot has no constant velocity add; tune direction/initial_velocity)")
        if any(abs(v) > 1e-6 for v in rad):
            pm["radial_velocity_min"] = g(rad[0]); pm["radial_velocity_max"] = g(rad[1])
        if any(abs(v) > 1e-6 for pair in orb for v in pair):
            rep.append(f"{name}: orbital velocity {[g(o[1]) for o in orb]} not mapped")
        smod = mm_range(vel.get("speedModifier"))
        if smod[1] not in (0.0, 1.0):
            rep.append(f"{name}: speed modifier {g(smod[1])} not mapped")
    noise = ps.get("NoiseModule") or {}
    if noise.get("enabled"):
        s_min, s_max = mm_range(noise.get("strength"))
        freq = fnum(noise.get("frequency"), 0.5)
        pm["turbulence_enabled"] = "true"
        pm["turbulence_noise_strength"] = g(max(s_max, 0.01))
        pm["turbulence_noise_scale"] = g(4.5 / max(freq, 0.01))
        pm["turbulence_influence_min"] = "0.1"; pm["turbulence_influence_max"] = "0.3"
        pm["turbulence_noise_speed"] = "Vector3(0.5, 0.5, 0.5)"
        rep.append(f"{name}: noise -> turbulence (approximation)")
    uvm = ps.get("UVModule") or {}
    tiles = (1, 1)
    if uvm.get("enabled"):
        tiles = (int(uvm.get("tilesX", 1)), int(uvm.get("tilesY", 1)))
        cycles = fnum(uvm.get("cycles"), 1)
        if win is not None:                                    # play the flipbook inside the window
            cycles = cycles / max(win[1] - win[0], 0.01)
            pm["anim_offset_min"] = g((-win[0] * cycles) % 1.0); pm["anim_offset_max"] = pm["anim_offset_min"]
        pm["anim_speed_min"] = g(cycles); pm["anim_speed_max"] = g(cycles)
        sf = mm_range(uvm.get("startFrame"))
        if sf[1] > 0:
            pm["anim_offset_min"] = g(sf[0] / (tiles[0] * tiles[1])); pm["anim_offset_max"] = g(sf[1] / (tiles[0] * tiles[1]))
    if (ps.get("LightsModule") or {}).get("enabled"):
        rep.append(f"{name}: per-particle lights not mapped (add an OmniLight3D if needed)")
    if (ps.get("InheritVelocityModule") or {}).get("enabled"):
        rep.append(f"{name}: inherit velocity not mapped")
    if (ps.get("ClampVelocityModule") or {}).get("enabled"):
        # Unity clamps speed to `magnitude` within a few frames (dampen = fraction removed per
        # frame). Godot only has linear damping (u/s^2), so decelerate from the burst speed to the
        # limit in ~0.15 s; the particle then keeps slowing to a stop instead of cruising at the
        # limit — close enough for hit sparks, which is what uses this.
        cm = ps.get("ClampVelocityModule") or {}
        lim = mm_range(cm.get("magnitude"))[1]
        damp = fnum(cm.get("dampen"))
        v_top = max(sp_max, mm_range(vel.get("radial"))[1] if vel.get("enabled") else 0.0)
        if damp > 0 and v_top > lim:
            d = 30.0 * v_top * damp
            pm["damping_min"] = g(d * 0.85); pm["damping_max"] = g(d)
        rep.append(f"{name}: limit velocity (mag {g(lim)}, dampen {g(damp)}) -> damping {pm.get('damping_max', '0')}")
    if (ps.get("SizeBySpeedModule") or {}).get("enabled"):
        rep.append(f"{name}: size by speed not mapped")
    if (ps.get("CustomDataModule") or {}).get("enabled"):
        pass                                                   # standard-unlit ignores custom streams
    # --- renderer
    rmode = int(psr.get("m_RenderMode", 0)) if psr else 5
    align = int(psr.get("m_RenderAlignment", 0)) if psr else 0
    mats = psr.get("m_Materials") or [] if psr else []
    mat_guid = mats[0].get("guid") if mats and isinstance(mats[0], dict) else None
    if rmode == 0 and align in (1, 2):
        # World/Local-aligned "billboards" are flat quads locked to the transform (magic circles,
        # ground rings): render them as mesh quads in the emitter's plane, no camera facing
        rmode = 4
        psr = dict(psr); psr["m_Mesh"] = {"fileID": UNITY_QUAD, "guid": "0000000000000000e000000000000000"}
    if rmode == 1:
        pm["particle_flag_align_y"] = "true"
    props["process_material"] = sc.sub_res("ParticleProcessMaterial", pm)
    if rmode == 5 or psr is None:
        rep.append(f"{name}: renderer None (emitter only)")
        return props
    mat = assets.material(sc, mat_guid, rmode, tiles) if mat_guid else sc.sub_res(
        "StandardMaterial3D", {"shading_mode": "0", "vertex_color_use_as_albedo": "true", "transparency": "1"})
    if rmode == 0:                                             # billboard
        mesh = sc.sub_res("QuadMesh", {"size": f"Vector2(1, {g(aspect)})", "material": mat})
    elif rmode == 1:                                           # stretched: long axis = Y (aligned to velocity)
        ls = fnum(psr.get("m_LengthScale"), 2)
        mesh = sc.sub_res("QuadMesh", {"size": f"Vector2(1, {g(max(ls, 0.1))})", "material": mat,
                                       "orientation": "2"})   # FACE_Z stays; align_y turns the quad
        rep.append(f"{name}: stretched billboard -> velocity-aligned quad (lengthScale {g(ls)}, no camera facing)")
    elif rmode == 4:                                           # mesh
        m = psr.get("m_Mesh") or {}
        fid, mg = m.get("fileID"), m.get("guid")
        if mg == "0000000000000000e000000000000000" and fid == UNITY_QUAD:
            qp = {"size": f"Vector2(1, {g(aspect)})", "material": mat}
            if flat_quad:
                qp["orientation"] = "1"                        # FACE_Y: lies on the ground
            mesh = sc.sub_res("QuadMesh", qp)
        elif mg == "0000000000000000e000000000000000" and fid == UNITY_SPHERE:
            mesh = sc.sub_res("SphereMesh", {"radius": "0.5", "height": "1.0", "material": mat})
        else:
            mp = assets.mesh_path(mg) if mg else None
            if mp:
                mesh = sc.ext_res("Mesh", mp)
                props["material_override"] = mat
            else:
                rep.append(f"{name}: mesh {fid}/{mg} not found -> quad")
                mesh = sc.sub_res("QuadMesh", {"size": "Vector2(1, 1)", "material": mat})
    else:
        rep.append(f"{name}: render mode {rmode} -> billboard")
        mesh = sc.sub_res("QuadMesh", {"size": "Vector2(1, 1)", "material": mat})
    props["draw_pass_1"] = mesh
    trail = ps.get("TrailModule") or {}
    if trail.get("enabled"):
        rep.append(f"{name}: trails not mapped (GPUParticles3D.trail_enabled + RibbonTrailMesh by hand)")
    return props


# ----------------------------------------------------------------------------- main

def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    only = None
    if "--only" in sys.argv:
        only = sys.argv[sys.argv.index("--only") + 1]
        args = [a for a in args if a != only]
    if len(args) != 2:
        print(__doc__); return 2
    pack, godot = args
    assets = Assets(pack, godot)
    out_dir = os.path.join(godot, "proto", "arena3d", "fx", "hovl")
    prefabs = []
    for dp, _, fns in os.walk(os.path.join(pack, "Prefabs")):
        for fn in sorted(fns):
            if fn.endswith(".prefab") and (only is None or only.lower() in fn.lower()):
                prefabs.append(os.path.join(dp, fn))
    n = 0
    for p in prefabs:
        cat = sanitize(os.path.basename(os.path.dirname(p))).replace(" ", "_")
        try:
            out, rep = convert_prefab(p, assets, out_dir, cat)
        except Exception as e:                                # keep going; report the failure
            print(f"FAILED {p}: {type(e).__name__}: {e}")
            continue
        n += 1
        print(f"{os.path.relpath(out, godot)}")
        for r in rep:
            print(f"    ! {r}")
    print(f"converted {n}/{len(prefabs)} prefabs -> {out_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
