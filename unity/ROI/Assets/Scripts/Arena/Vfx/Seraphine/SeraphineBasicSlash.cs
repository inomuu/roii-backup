using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace Roi.Arena
{
    // Authored over row 2 of seraphine.png, in its 120 x 100 source-pixel coordinates.
    public sealed class SeraphineBasicSlash : MonoBehaviour
    {
        const int Steps = 36;
        static readonly int Opacity = Shader.PropertyToID("_Opacity");
        readonly Mesh[] _poses = new Mesh[3];
        SpriteRenderer _body;
        MeshFilter _filter;
        MeshRenderer _renderer;
        MaterialPropertyBlock _properties;

        public static SeraphineBasicSlash Create(SpriteRenderer body)
        {
            var material = Resources.Load<Material>("Effects/SeraphineBasicSlash");
            if (body == null || material == null) return null;
            var go = new GameObject("BasicSwordEdge");
            go.layer = body.gameObject.layer;
            go.transform.SetParent(body.transform, false);
            go.transform.localPosition = new Vector3(0f, 0f, -.006f);
            var fx = go.AddComponent<SeraphineBasicSlash>();
            fx._body = body;
            fx._filter = go.AddComponent<MeshFilter>();
            fx._renderer = go.AddComponent<MeshRenderer>();
            fx._renderer.sharedMaterial = material;
            fx._renderer.shadowCastingMode = ShadowCastingMode.Off;
            fx._renderer.receiveShadows = false;
            fx._renderer.lightProbeUsage = LightProbeUsage.Off;
            fx._renderer.reflectionProbeUsage = ReflectionProbeUsage.Off;
            fx._renderer.enabled = false;
            fx._properties = new MaterialPropertyBlock();
            for (int i = 0; i < fx._poses.Length; i++) fx._poses[i] = BuildPose(i + 2);
            return fx;
        }

        public void Show(int attackFrame, float frameProgress, bool visible)
        {
            int pose = attackFrame - 2;
            visible &= pose >= 0 && pose < _poses.Length && _body != null && _body.enabled;
            _renderer.enabled = visible;
            if (!visible) return;
            _filter.sharedMesh = _poses[pose];
            transform.localScale = new Vector3(_body.flipX ? -1f : 1f, 1f, 1f);
            _renderer.sortingLayerID = _body.sortingLayerID;
            _renderer.sortingOrder = _body.sortingOrder + 1;
            float progress = Mathf.Clamp01(frameProgress);
            float alpha = pose == 0 ? Mathf.Lerp(.8f, 1f, progress)
                        : pose == 1 ? Mathf.Lerp(1f, .8f, progress)
                        : .48f * (1f - progress);
            _properties.SetFloat(Opacity, alpha * _body.color.a);
            _renderer.SetPropertyBlock(_properties);
        }

        static Mesh BuildPose(int frame)
        {
            var vertices = new List<Vector3>();
            var colors = new List<Color>();
            var indices = new List<int>();
            if (frame == 2)
            {
                Stroke(vertices, colors, indices, new Vector2(89, 30), new Vector2(61, 18),
                    new Vector2(9, 50), new Vector2(19, 73), 3.1f, 1f);
                Stroke(vertices, colors, indices, new Vector2(72, 32), new Vector2(54, 32),
                    new Vector2(27, 54), new Vector2(31, 68), 1.7f, .95f);
            }
            else if (frame == 3)
            {
                Stroke(vertices, colors, indices, new Vector2(55, 34), new Vector2(30, 42),
                    new Vector2(11, 63), new Vector2(20, 74), 3.8f, 1f);
                Stroke(vertices, colors, indices, new Vector2(49, 41), new Vector2(35, 49),
                    new Vector2(23, 65), new Vector2(28, 72), 1.6f, .95f);
            }
            else
            {
                Stroke(vertices, colors, indices, new Vector2(17, 63), new Vector2(11, 77),
                    new Vector2(34, 80), new Vector2(54, 75), 1.5f, .8f);
                Stroke(vertices, colors, indices, new Vector2(23, 65), new Vector2(21, 73),
                    new Vector2(38, 76), new Vector2(49, 73), .85f, .7f);
            }
            var mesh = new Mesh { name = "Seraphine sword edge " + frame };
            mesh.SetVertices(vertices);
            mesh.SetColors(colors);
            mesh.SetTriangles(indices, 0);
            mesh.RecalculateBounds();
            return mesh;
        }

        static void Stroke(List<Vector3> vertices, List<Color> colors, List<int> indices,
            Vector2 a, Vector2 b, Vector2 c, Vector2 d, float width, float opacity)
        {
            int first = vertices.Count;
            // Narrow ivory core with a warm translucent lip, tapered at both ends.
            float[] bands = { -1f, -.48f, -.20f, .20f, .48f, 1f };
            Color[] ramp = {
                new Color(1f, .68f, .26f, 0f), new Color(1f, .78f, .36f, .55f),
                new Color(1.35f, 1.30f, 1.12f, 1f), new Color(1.35f, 1.30f, 1.12f, 1f),
                new Color(1f, .86f, .52f, .7f), new Color(1f, .72f, .3f, 0f)
            };
            for (int i = 0; i <= Steps; i++)
            {
                float t = i / (float)Steps, s = 1f - t;
                var p = s * s * s * a + 3f * s * s * t * b + 3f * s * t * t * c + t * t * t * d;
                var tangent = (3f * s * s * (b - a) + 6f * s * t * (c - b) + 3f * t * t * (d - c)).normalized;
                var normal = new Vector2(-tangent.y, tangent.x);
                float taper = Mathf.Pow(Mathf.Max(0f, Mathf.Sin(Mathf.PI * t)), .65f);
                for (int j = 0; j < bands.Length; j++)
                {
                    var v = p + normal * (bands[j] * width * taper);
                    vertices.Add(new Vector3((v.x - 60f) / 100f, (50f - v.y) / 100f, 0f));
                    var color = ramp[j]; color.a *= opacity;
                    colors.Add(color);
                    if (i == Steps || j == bands.Length - 1) continue;
                    int k = first + i * bands.Length + j;
                    indices.Add(k); indices.Add(k + bands.Length); indices.Add(k + 1);
                    indices.Add(k + 1); indices.Add(k + bands.Length); indices.Add(k + bands.Length + 1);
                }
            }
        }

        void OnDisable()
        {
            if (_renderer != null) _renderer.enabled = false;
        }

        void OnDestroy()
        {
            foreach (var mesh in _poses)
                if (mesh != null)
                {
                    if (Application.isPlaying) Destroy(mesh);
                    else DestroyImmediate(mesh);
                }
        }
    }
}
