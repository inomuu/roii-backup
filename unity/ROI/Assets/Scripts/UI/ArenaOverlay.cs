// THE ARENA'S OWN READOUT — how far this room is from its boss, the boss's health while it lives,
// and the title card when a band falls. Ported from `overlay_view.gd` (the progression strip and
// the loot badge), `boss_bar.gd` (the plate) and `area_clear_banner.gd` (the card).
//
// THREE PIECES, ONE FILE, because they are one subscriber: all three read the same per-tick view
// and all three are about the ROOM rather than about the party or the menus. Godot splits them
// across three nodes for its own scene-tree reasons; here that would be three components fighting
// over the same JSON.
//
// WHAT IT FIXES. Before this the Unity client had damage numbers and a party strip but nothing
// that said "the boss is 40 kills away" — the middle of an idle session had no reading at all, and
// a boss arrived with no warning and no health bar.
//
// Fed from the per-tick view, like the arena itself. Nothing here queries or writes.
using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class ArenaOverlay : MonoBehaviour
    {
        const float StripW = 420f, StripH = 10f, BossW = 0.6f, BossH = 26f, BandH = 96f;
        const float BossLaneH = 7f, BossLaneGap = 2f, BossTrackPadding = 8f;
        const int MaxBossBars = 10;
        const float FadeIn = 0.35f, Hold = 1.7f, FadeOut = 0.6f;

        // The strip's colours, as `overlay_view.gd` picks them: it lerps from a cool blue to the
        // boss red as the room fills, so the bar's COLOUR is a second reading of the same number.
        static readonly Color Far = new Color(0.30f, 0.70f, 0.95f);
        static readonly Color Near = new Color(0.95f, 0.30f, 0.20f);
        static readonly Color Done = new Color(0.30f, 0.85f, 0.40f);

        Canvas _canvas;
        RectTransform _root;
        Image _zoneTrack, _zoneFill, _bossTrack, _bossFill, _bandPanel;
        TextMeshProUGUI _zoneLabel, _loot, _bossHpTop, _bossName, _bossHp, _bandLabel;
        RectTransform _zoneBossRoot;
        readonly List<Image> _bossLaneTracks = new();
        readonly List<Image> _bossLaneFills = new();
        CanvasGroup _bossGroup, _bandGroup;

        int _zonesPerRegion = 5;
        bool _prevEndClear, _seeded;
        bool _zoneRatioSeeded, _bossMode;
        int _bossBarCount;
        float _zoneRatio, _zoneTarget, _zoneVelocity;
        readonly float[] _bossRatios = new float[MaxBossBars];
        readonly float[] _bossTargets = new float[MaxBossBars];
        readonly float[] _bossVelocities = new float[MaxBossBars];
        float _bandT = -1f;

        public static ArenaOverlay Create(Transform parent, int zonesPerRegion)
        {
            var go = new GameObject("ArenaOverlay");
            go.transform.SetParent(parent, false);
            var o = go.AddComponent<ArenaOverlay>();
            o._zonesPerRegion = Mathf.Max(1, zonesPerRegion);
            o.Build();
            return o;
        }

        void Build()
        {
            var canvasGo = new GameObject("OverlayCanvas");
            canvasGo.transform.SetParent(transform, false);
            _canvas = canvasGo.AddComponent<Canvas>();
            _canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            // BELOW the windows (10) and the bar (40), ABOVE the world: this is part of the fight,
            // and an open screen is meant to cover it.
            _canvas.sortingOrder = 5;
            _canvas.pixelPerfect = true;
            var scaler = canvasGo.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1280f, 720f);
            scaler.screenMatchMode = CanvasScaler.ScreenMatchMode.MatchWidthOrHeight;
            scaler.matchWidthOrHeight = 1f;
            _root = (RectTransform)canvasGo.transform;

            // --- the room's progress, top-left under the roster strip -------------------------
            // A PLATE under it. The ground here is bright grass in sunlight, and a caption at
            // caption weight simply disappears on it — the Godot overlay has a dark HUD strip to
            // sit on and this one has open world.
            var plate = UiTheme.Box(_root, "Plate", new Color(0.02f, 0.02f, 0.03f, 0.72f));
            plate.raycastTarget = false;
            var pl = plate.rectTransform;
            pl.anchorMin = pl.anchorMax = new Vector2(0f, 1f);
            pl.pivot = new Vector2(0f, 1f);
            pl.sizeDelta = new Vector2(StripW + 240f + UiTheme.Pad * 2f, 44f);
            pl.anchoredPosition = new Vector2(UiTheme.PadLarge - UiTheme.PadSmall,
                                              -(UiLayout.Top + UiTheme.Gap) + UiTheme.PadSmall);

            _zoneLabel = UiTheme.Label(_root, "ZoneLabel", "", UiTheme.CaptionSize,
                                       UiTheme.TextMid, TextAlignmentOptions.Left);
            var zl = _zoneLabel.rectTransform;
            zl.anchorMin = zl.anchorMax = new Vector2(0f, 1f);
            zl.pivot = new Vector2(0f, 1f);
            zl.sizeDelta = new Vector2(StripW, 18f);
            zl.anchoredPosition = new Vector2(UiTheme.PadLarge, -(UiLayout.Top + UiTheme.Gap));

            _zoneTrack = UiTheme.Flat(_root, "ZoneTrack", new Color(0f, 0f, 0f, 0.45f));
            _zoneTrack.raycastTarget = false;
            var zt = _zoneTrack.rectTransform;
            zt.anchorMin = zt.anchorMax = new Vector2(0f, 1f);
            zt.pivot = new Vector2(0f, 1f);
            zt.sizeDelta = new Vector2(StripW, StripH);
            zt.anchoredPosition = new Vector2(UiTheme.PadLarge, -(UiLayout.Top + UiTheme.Gap + 20f));
            _zoneFill = UiTheme.Flat(zt, "Fill", Far);
            _zoneFill.raycastTarget = false;
            var zf = _zoneFill.rectTransform;
            zf.anchorMin = new Vector2(0f, 0f); zf.anchorMax = new Vector2(0f, 1f);
            zf.pivot = new Vector2(0f, 0.5f);
            zf.sizeDelta = Vector2.zero;

            var bossRootGo = new GameObject("BossBars", typeof(RectTransform));
            bossRootGo.transform.SetParent(_zoneTrack.transform, false);
            _zoneBossRoot = (RectTransform)bossRootGo.transform;
            _zoneBossRoot.anchorMin = Vector2.zero;
            _zoneBossRoot.anchorMax = Vector2.one;
            _zoneBossRoot.offsetMin = _zoneBossRoot.offsetMax = Vector2.zero;
            _zoneBossRoot.gameObject.SetActive(false);

            // The LOOT badge, beside the bar. It is the one number that silently changes what an
            // idle session is worth — over-levelling a zone cuts the drop rate — so it is stated.
            _loot = UiTheme.Label(_root, "Loot", "", UiTheme.CaptionSize, UiTheme.TextLow,
                                  TextAlignmentOptions.Left);
            var ll = _loot.rectTransform;
            ll.anchorMin = ll.anchorMax = new Vector2(0f, 1f);
            ll.pivot = new Vector2(0f, 1f);
            ll.sizeDelta = new Vector2(104f, 18f);
            ll.anchoredPosition = new Vector2(UiTheme.PadLarge + StripW + UiTheme.Pad,
                                              -(UiLayout.Top + UiTheme.Gap));

            _bossHpTop = UiTheme.Label(_root, "BossHpTop", "", UiTheme.CaptionSize, UiTheme.TextHigh,
                                       TextAlignmentOptions.Right);
            var bhTop = _bossHpTop.rectTransform;
            bhTop.anchorMin = bhTop.anchorMax = new Vector2(0f, 1f);
            bhTop.pivot = new Vector2(0f, 1f);
            bhTop.sizeDelta = new Vector2(116f, 18f);
            bhTop.anchoredPosition = new Vector2(UiTheme.PadLarge + StripW + UiTheme.Pad + 108f,
                                                 -(UiLayout.Top + UiTheme.Gap));
            _bossHpTop.gameObject.SetActive(false);

            // --- the boss plate, across the bottom above the action bar -----------------------
            var bossGo = new GameObject("Boss", typeof(RectTransform));
            bossGo.transform.SetParent(_root, false);
            var brt = (RectTransform)bossGo.transform;
            brt.anchorMin = new Vector2(0.5f, 0f); brt.anchorMax = new Vector2(0.5f, 0f);
            brt.pivot = new Vector2(0.5f, 0f);
            brt.sizeDelta = new Vector2(600f, BossH + 26f);
            brt.anchoredPosition = new Vector2(0f, UiLayout.BottomBar + UiTheme.Pad);
            _bossGroup = bossGo.AddComponent<CanvasGroup>();
            _bossGroup.alpha = 0f;
            _bossGroup.blocksRaycasts = false;

            _bossName = UiTheme.Caps(brt, "BossName", "", UiTheme.HeadSize, UiTheme.TextHigh,
                                     TextAlignmentOptions.Center);
            var bn = _bossName.rectTransform;
            bn.anchorMin = new Vector2(0f, 1f); bn.anchorMax = new Vector2(1f, 1f);
            bn.pivot = new Vector2(0.5f, 1f);
            bn.sizeDelta = new Vector2(0f, 22f);

            _bossTrack = UiTheme.Flat(brt, "BossTrack", new Color(0.10f, 0.03f, 0.03f, 0.85f));
            _bossTrack.raycastTarget = false;
            var bt = _bossTrack.rectTransform;
            bt.anchorMin = new Vector2(0f, 0f); bt.anchorMax = new Vector2(1f, 0f);
            bt.pivot = new Vector2(0.5f, 0f);
            bt.sizeDelta = new Vector2(0f, BossH);
            bt.anchoredPosition = Vector2.zero;
            UiTheme.Border(bt, new Color(1f, 1f, 1f, 0.18f));
            _bossFill = UiTheme.Flat(bt, "Fill", new Color(0.80f, 0.18f, 0.16f));
            _bossFill.raycastTarget = false;
            var bf = _bossFill.rectTransform;
            bf.anchorMin = new Vector2(0f, 0f); bf.anchorMax = new Vector2(0f, 1f);
            bf.pivot = new Vector2(0f, 0.5f);
            bf.sizeDelta = Vector2.zero;
            _bossHp = UiTheme.Label(bt, "BossHp", "", UiTheme.CaptionSize, UiTheme.TextHigh,
                                    TextAlignmentOptions.Center);
            var bh = _bossHp.rectTransform;
            bh.anchorMin = Vector2.zero; bh.anchorMax = Vector2.one;
            bh.offsetMin = Vector2.zero; bh.offsetMax = Vector2.zero;

            // --- the title card ----------------------------------------------------------------
            var band = UiTheme.Flat(_root, "Band", new Color(0.02f, 0.02f, 0.03f, 0.88f));
            _bandPanel = band;
            band.raycastTarget = false;
            var bp = band.rectTransform;
            bp.anchorMin = new Vector2(0f, 0.5f); bp.anchorMax = new Vector2(1f, 0.5f);
            bp.pivot = new Vector2(0.5f, 0.5f);
            bp.sizeDelta = new Vector2(0f, BandH);
            bp.anchoredPosition = new Vector2(0f, 40f);
            _bandGroup = band.gameObject.AddComponent<CanvasGroup>();
            _bandGroup.alpha = 0f;
            _bandGroup.blocksRaycasts = false;
            _bandLabel = UiTheme.Caps(bp, "BandLabel", "", 30f, UiTheme.Accent,
                                      TextAlignmentOptions.Center);
            _bandLabel.overflowMode = TextOverflowModes.Overflow;
            var bl = _bandLabel.rectTransform;
            bl.anchorMin = Vector2.zero; bl.anchorMax = Vector2.one;
            bl.offsetMin = Vector2.zero; bl.offsetMax = Vector2.zero;
        }

        /// Called with the same view JSON the arena renders from.
        public void OnView(string json)
        {
            var v = JsonUtility.FromJson<View>(json);
            if (v == null) return;
            RefreshZone(v);
            RefreshBoss(v);
            RefreshBand(v);
        }

        void RefreshZone(View v)
        {
            var zs = v.zone_spawn;
            bool has = zs != null && zs.boss_threshold > 0;
            _zoneLabel.gameObject.SetActive(true);
            _zoneTrack.gameObject.SetActive(true);

            float ratio = 1f;
            if (has && !zs.boss_spawned)
                ratio = Mathf.Clamp01(zs.kill_counter / (float)zs.boss_threshold);

            Entity boss = FindBoss(v);
            if (boss != null && zs != null && zs.boss_spawned)
            {
                RefreshTopBoss(boss);
                return;
            }

            EnterProgressMode();
            SetZoneTarget(ratio);

            if (zs != null && zs.zone_cleared)
            {
                _zoneFill.color = Done;
                _zoneLabel.text = Loc.Get("UI_ARENA_OVERLAY_ROOM_CLEARED_ADVANCING");
            }
            else if (zs != null && zs.boss_spawned)
            {
                _zoneFill.color = Near;
                _zoneLabel.text = Loc.Get("UI_ARENA_BOSS") + " · 100%";
            }
            else if (has)
            {
                _zoneFill.color = Color.Lerp(Far, Near, ratio);
                _zoneLabel.text = $"{Mathf.RoundToInt(ratio * 100f)}%";
            }
            else
            {
                _zoneFill.color = Far;
                _zoneLabel.text = "";
            }

            _bossHpTop.gameObject.SetActive(false);

            // 10000 bp = the full rate. Anything less is the level-gap cut.
            int bp = v.loot_gap_mult_bp == 0 ? 10000 : v.loot_gap_mult_bp;
            if (bp < 10000)
            {
                _loot.text = $"Loot −{(10000 - bp) / 100}%  (level gap)";
                _loot.color = new Color(0.95f, 0.60f, 0.25f);
            }
            else
            {
                _loot.text = Loc.Get("UI_ARENA_OVERLAY_LOOT_100");
                _loot.color = new Color(0.55f, 0.80f, 0.55f);
            }
        }

        void RefreshBoss(View v)
        {
            // The room strip becomes the boss readout as soon as the boss arrives. Keep the old
            // bottom plate hidden so the same HP is never shown twice in different places.
            _bossGroup.alpha = 0f;
        }

        void RefreshTopBoss(Entity boss)
        {
            long max = boss.eff_max_hp > 0 ? boss.eff_max_hp : boss.max_hp;
            if (max <= 0) { EnterProgressMode(); return; }
            // Keep the clamp integer-only; the visual ratio may be a float, but the number shown
            // to the player must never lose precision when a late-game boss exceeds 2^53.
            long hp = Math.Max(0L, Math.Min(boss.hp, max));
            int bars = BossBarCount(boss);
            EnterBossMode(bars);
            _zoneLabel.text = Loc.Get("UI_ARENA_BOSS") + " · " +
                              (string.IsNullOrEmpty(boss.name) ? "Boss" : boss.name);
            _bossHpTop.gameObject.SetActive(true);
            _bossHpTop.text = $"{hp:N0} / {max:N0}";

            long perBar = max / bars + (max % bars == 0 ? 0 : 1);
            for (int i = 0; i < MaxBossBars; i++)
            {
                if (i >= bars) { _bossTargets[i] = 0f; continue; }
                long offset = perBar > 0 && i <= long.MaxValue / perBar
                    ? i * perBar : long.MaxValue;
                long segmentHp = hp > offset ? Math.Min(perBar, hp - offset) : 0L;
                _bossTargets[i] = perBar <= 0 ? 0f : Mathf.Clamp01(segmentHp / (float)perBar);
            }
        }

        void EnterProgressMode()
        {
            if (!_bossMode) return;
            _bossMode = false;
            _zoneTrack.rectTransform.sizeDelta = new Vector2(StripW, StripH);
            _zoneBossRoot.gameObject.SetActive(false);
            _zoneFill.gameObject.SetActive(true);
            _bossHpTop.gameObject.SetActive(false);
            ResizePlate(StripH);
        }

        void EnterBossMode(int bars)
        {
            bars = Mathf.Clamp(bars, 1, MaxBossBars);
            float contentH = bars * BossLaneH + (bars - 1) * BossLaneGap;
            float trackH = Mathf.Max(BossH, contentH + BossTrackPadding);
            if (!_bossMode || _bossBarCount != bars)
            {
                _bossMode = true;
                _bossBarCount = bars;
                EnsureBossLanes(bars, trackH);
                _zoneTrack.rectTransform.sizeDelta = new Vector2(StripW, trackH);
                _zoneBossRoot.gameObject.SetActive(true);
                _zoneFill.gameObject.SetActive(false);
                for (int i = 0; i < MaxBossBars; i++)
                {
                    _bossTargets[i] = 0f;
                    _bossRatios[i] = i < bars ? 1f : 0f;
                    _bossVelocities[i] = 0f;
                }
                ResizePlate(trackH);
            }
            for (int i = 0; i < _bossLaneTracks.Count; i++)
            {
                bool active = i < bars;
                _bossLaneTracks[i].gameObject.SetActive(active);
                _bossLaneFills[i].gameObject.SetActive(active);
            }
        }

        void EnsureBossLanes(int count, float trackH)
        {
            while (_bossLaneTracks.Count < count)
            {
                int index = _bossLaneTracks.Count;
                var track = UiTheme.Flat(_zoneBossRoot, "LaneTrack" + index,
                                         new Color(0.10f, 0.03f, 0.03f, 0.88f));
                track.raycastTarget = false;
                UiTheme.Border(track.rectTransform, new Color(1f, 1f, 1f, 0.18f));
                var fill = UiTheme.Flat(track.rectTransform, "Fill", new Color(0.80f, 0.18f, 0.16f));
                fill.raycastTarget = false;
                var rt = track.rectTransform;
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
                rt.pivot = new Vector2(0f, 1f);
                rt.sizeDelta = new Vector2(StripW - BossTrackPadding, BossLaneH);
                var fr = fill.rectTransform;
                fr.anchorMin = new Vector2(0f, 0f); fr.anchorMax = new Vector2(0f, 1f);
                fr.pivot = new Vector2(0f, .5f); fr.sizeDelta = Vector2.zero;
                _bossLaneTracks.Add(track);
                _bossLaneFills.Add(fill);
            }

            // A boss can change phase count between encounters. Reflow existing lanes as well as
            // newly-created ones so a 10→2 transition never leaves the old rows offset.
            float contentHeight = count * BossLaneH + (count - 1) * BossLaneGap;
            float firstTop = (trackH - contentHeight) * .5f;
            for (int index = 0; index < count; index++)
            {
                var rt = _bossLaneTracks[index].rectTransform;
                rt.sizeDelta = new Vector2(StripW - BossTrackPadding, BossLaneH);
                rt.anchoredPosition = new Vector2(BossTrackPadding * .5f,
                                                  -(firstTop + index * (BossLaneH + BossLaneGap)));
            }
        }

        void ResizePlate(float trackHeight)
        {
            var plate = _root.Find("Plate") as RectTransform;
            if (plate != null)
                plate.sizeDelta = new Vector2(StripW + 240f + UiTheme.Pad * 2f, trackHeight + 34f);
        }

        void SetZoneTarget(float ratio)
        {
            _zoneTarget = Mathf.Clamp01(ratio);
            if (!_zoneRatioSeeded)
            {
                _zoneRatio = _zoneTarget;
                _zoneRatioSeeded = true;
                _zoneVelocity = 0f;
                ApplyZoneRatio();
            }
        }

        void SmoothBars()
        {
            if (!_bossMode)
            {
                _zoneRatio = Mathf.SmoothDamp(_zoneRatio, _zoneTarget, ref _zoneVelocity,
                                              .18f, Mathf.Infinity, Time.unscaledDeltaTime);
                ApplyZoneRatio();
                return;
            }
            for (int i = 0; i < _bossBarCount && i < _bossLaneFills.Count; i++)
            {
                _bossRatios[i] = Mathf.SmoothDamp(_bossRatios[i], _bossTargets[i],
                                                  ref _bossVelocities[i], .16f, Mathf.Infinity,
                                                  Time.unscaledDeltaTime);
                float width = _bossLaneTracks[i].rectTransform.rect.width * _bossRatios[i];
                _bossLaneFills[i].rectTransform.sizeDelta = new Vector2(width, 0f);
            }
        }

        void ApplyZoneRatio()
        {
            if (_zoneFill == null || _bossMode) return;
            _zoneFill.rectTransform.sizeDelta = new Vector2(StripW * _zoneRatio, 0f);
        }

        static Entity FindBoss(View v)
        {
            foreach (var e in v.grid?.entities ?? Array.Empty<Entity>())
                if (e.is_boss && e.alive) return e;
            return null;
        }

        static int BossBarCount(Entity boss)
        {
            if (boss == null) return 1;
            int count = boss.boss_bars > 0 ? boss.boss_bars
                      : boss.health_bars > 0 ? boss.health_bars
                      : boss.hp_bars > 0 ? boss.hp_bars
                      : boss.bar_count > 0 ? boss.bar_count : 1;
            return Mathf.Clamp(count, 1, MaxBossBars);
        }

        void RefreshBand(View v)
        {
            var zs = v.zone_spawn;
            int band = BandOf(v.current_zone_id);
            bool endClear = zs != null && zs.zone_cleared && zs.is_instance_end && band > 0;
            // SEEDED on the first view: a save resumed while standing on a cleared band-end must
            // not flash the card at login.
            if (!_seeded) { _seeded = true; _prevEndClear = endClear; return; }
            // The RISING EDGE, not the band id: re-running the same band clears it again, and
            // keying off the number would show the card once and never again.
            if (endClear && !_prevEndClear)
            {
                _bandLabel.text = Loc.Fmt("UI_STAGE_CLEARED", band);
                _bandT = 0f;
            }
            _prevEndClear = endClear;
        }

        void Update()
        {
            SmoothBars();
            if (_bandT < 0f) return;
            _bandT += Time.unscaledDeltaTime;
            float a = _bandT < FadeIn ? _bandT / FadeIn
                    : _bandT < FadeIn + Hold ? 1f
                    : 1f - (_bandT - FadeIn - Hold) / FadeOut;
            _bandGroup.alpha = Mathf.Clamp01(a);
            if (_bandT > FadeIn + Hold + FadeOut) { _bandT = -1f; _bandGroup.alpha = 0f; }
        }

        /// The band number out of a zone id (`zone_b7_2` -> 7). The same parse the arena does for
        /// its art region, kept here because this one needs the BAND, not the region.
        static int BandOf(string zoneId)
        {
            if (string.IsNullOrEmpty(zoneId)) return 0;
            int i = zoneId.IndexOf("_b", System.StringComparison.Ordinal);
            if (i < 0) return 0;
            int j = i + 2, band = 0;
            while (j < zoneId.Length && char.IsDigit(zoneId[j])) { band = band * 10 + (zoneId[j] - '0'); j++; }
            return band;
        }

        [System.Serializable]
        class ZoneSpawn
        {
            public bool zone_cleared, boss_spawned, is_instance_end;
            public int kill_counter, boss_threshold;
        }
        [System.Serializable]
        class Entity
        {
            public bool is_boss, alive;
            public long hp, max_hp, eff_max_hp;
            // Optional presentation hint. Current sim payloads omit it and render one bar;
            // future bosses can opt into Blue Archive-style 2–10 sequential bars without a
            // client schema change elsewhere.
            public int boss_bars, health_bars, hp_bars, bar_count;
            public string name;
        }
        [System.Serializable] class Grid { public Entity[] entities; }
        [System.Serializable]
        class View
        {
            public Grid grid;
            public ZoneSpawn zone_spawn;
            public string current_zone_id;
            public int loot_gap_mult_bp;
        }
    }
}
