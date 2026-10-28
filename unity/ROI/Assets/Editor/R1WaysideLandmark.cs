// Editor authoring only: the result is ordinary saved meshes and a prefab instance.
using System;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using Object = UnityEngine.Object;

public static class R1WaysideLandmark
{
    const string Folder = "Assets/Zones/r1/QualityProps";
    const string PrefabPath = Folder + "/WayfarerShrine.prefab";

    [MenuItem("ROI/Art/Build Wayfarer Shrine")]
    public static void Build()
    {
        if (EditorApplication.isPlayingOrWillChangePlaymode)
            throw new InvalidOperationException("Author the shrine outside Play mode.");
        var scene = UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if (scene.path != ProbeScene.Path)
            throw new InvalidOperationException("Open Zone_r1 before authoring its landmark.");
        var parent = GameObject.Find("Arena/Field/R1_AuthoredScenery");
        if (parent == null) throw new InvalidOperationException("The authored scenery root is missing.");
        var limestone = AssetDatabase.LoadAssetAtPath<Material>(Folder + "/WeatheredLimestone.mat");
        if (limestone == null) throw new InvalidOperationException("WeatheredLimestone.mat is required.");
        var shader = Shader.Find("Universal Render Pipeline/Lit");
        if (shader == null) throw new InvalidOperationException("The URP Lit shader is required.");

        var metal = MaterialAsset("WayfarerAgedBronze", shader, new Color(.17f, .145f, .10f), .6f, .26f);
        var cloth = MaterialAsset("WayfarerTealLinen", shader, new Color(.19f, .35f, .36f), 0f, .03f);
        cloth.SetFloat("_Cull", 0f);
        cloth.doubleSidedGI = true;
        var amber = MaterialAsset("WayfarerAmberGlass", shader, new Color(.65f, .27f, .055f), .05f, .56f);
        amber.EnableKeyword("_EMISSION");
        amber.SetColor("_EmissionColor", new Color(1.65f, .62f, .12f));
        amber.globalIlluminationFlags = MaterialGlobalIlluminationFlags.BakedEmissive;
        var timber = AssetDatabase.LoadAssetAtPath<Material>("Assets/Idyllic Fantasy Nature/Materials/Branches/Branch.mat");
        if (timber == null)
            timber = MaterialAsset("WayfarerTimber", shader, new Color(.25f, .20f, .13f), 0f, .09f);

        var stone = new Geometry();
        // Broad, shallow chamfered steps maintain a true ground-level base pivot.
        stone.Block(new Vector3(0f, .065f, -.12f), new Vector3(2.72f, .13f, 1.48f), .045f, 1);
        stone.Block(new Vector3(0f, .215f, -.07f), new Vector3(2.38f, .17f, 1.15f), .042f, 2);
        for (int side = -1; side <= 1; side += 2)
        {
            stone.Block(new Vector3(side * .87f, .38f, 0f), new Vector3(.61f, .16f, .83f), .035f, side + 8);
            for (int row = 0; row < 4; row++)
                stone.Block(new Vector3(side * .87f + .009f * Mathf.Sin(row * 3f + side), .6575f + row * .395f, 0f),
                    new Vector3(.42f + .015f * Mathf.Sin(row + side), .385f, .68f), .03f, row * 7 + side + 20);
            stone.Block(new Vector3(side * .87f, 2.025f, 0f), new Vector3(.51f, .15f, .78f), .033f, side + 40);
        }
        // Separate voussoirs have narrow mortar joints and a subtly proud keystone.
        for (int segment = 0; segment < 9; segment++)
        {
            float start = segment * Mathf.PI / 9f + .006f;
            float end = (segment + 1) * Mathf.PI / 9f - .006f;
            float outer = segment == 4 ? 1.085f : 1.05f;
            var outline = new List<Vector2>();
            outline.Add(Arc(.68f, start));
            outline.Add(Arc(outer, start));
            outline.Add(Arc(outer, (start + end) * .5f));
            outline.Add(Arc(outer, end));
            outline.Add(Arc(.68f, end));
            stone.Extrusion(outline, segment == 4 ? -.015f : 0f, segment == 4 ? .77f : .69f, .016f);
        }
        // A recess behind the arch makes a shelter, with an illuminated physical lantern inside.
        var recess = new List<Vector2> { new Vector2(-.68f, .46f), new Vector2(.68f, .46f) };
        for (int i = 0; i <= 12; i++) recess.Add(Arc(.68f, i * Mathf.PI / 12f));
        stone.Extrusion(recess, .365f, .17f, .018f);
        stone.Block(new Vector3(0f, .55f, -.06f), new Vector3(1.43f, .15f, .88f), .03f, 58);

        var bronze = new Geometry();
        bronze.Radial(new Vector3(0f, 1.90f, -.105f), .28f, .014f, .014f, 8);
        bronze.Radial(new Vector3(0f, 1.69f, -.105f), .115f, .26f, .095f, 8);
        bronze.Radial(new Vector3(0f, 1.135f, -.105f), .10f, .235f, .22f, 8);
        bronze.Radial(new Vector3(0f, 1.20f, -.105f), .028f, .22f, .22f, 8);
        bronze.Radial(new Vector3(0f, 1.62f, -.105f), .035f, .22f, .22f, 8);
        for (int i = 0; i < 8; i++)
        {
            float angle = i * Mathf.PI / 4f;
            bronze.Radial(new Vector3(Mathf.Cos(angle) * .193f, 1.41f, -.105f + Mathf.Sin(angle) * .193f),
                .43f, .013f, .013f, 6);
        }
        var glass = new Geometry();
        glass.Radial(new Vector3(0f, 1.41f, -.105f), .395f, .187f, .187f, 8);
        var wood = new Geometry();
        wood.Radial(new Vector3(1.38f, 1.39f, .11f), 2.67f, .052f, .038f, 9);
        wood.Beam(new Vector3(1.30f, 2.69f, .11f), new Vector3(1.94f, 2.69f, .10f), .035f, 8);
        var linen = new Geometry();
        linen.Banner(1.39f, 1.91f, 2.65f, 1.28f, .075f);

        var root = new GameObject("WayfarerShrine");
        try
        {
            MeshObject(root, "LimestoneArchAndSteps", stone.Save("WayfarerStonework"), limestone);
            MeshObject(root, "LanternBronzeFrame", bronze.Save("WayfarerLanternFrame"), metal);
            MeshObject(root, "LanternAmberGlass", glass.Save("WayfarerLanternGlass"), amber);
            MeshObject(root, "BannerTimberPole", wood.Save("WayfarerBannerPole"), timber);
            var fabric = MeshObject(root, "FoldedTealBanner", linen.Save("WayfarerBanner"), cloth);
            fabric.GetComponent<MeshRenderer>().shadowCastingMode = ShadowCastingMode.TwoSided;
            var glow = new GameObject("LanternGlow");
            glow.transform.SetParent(root.transform, false);
            glow.transform.localPosition = new Vector3(0f, 1.43f, -.19f);
            var light = glow.AddComponent<Light>();
            light.type = LightType.Point;
            light.color = new Color(1f, .63f, .28f);
            light.intensity = .8f;
            light.range = 2f;
            light.shadows = LightShadows.None;
            light.lightmapBakeType = LightmapBakeType.Realtime;
            EditorUtility.SetDirty(cloth);
            EditorUtility.SetDirty(amber);
            AssetDatabase.SaveAssets();
            var prefab = PrefabUtility.SaveAsPrefabAsset(root, PrefabPath);
            if (prefab == null) throw new InvalidOperationException("Could not save WayfarerShrine.prefab.");
            var old = parent.transform.Find("WayfarerShrine");
            if (old != null) Undo.DestroyObjectImmediate(old.gameObject);
            var instance = (GameObject)PrefabUtility.InstantiatePrefab(prefab, parent.transform);
            Undo.RegisterCreatedObjectUndo(instance, "Author wayfarer shrine");
            instance.name = "WayfarerShrine";
            instance.transform.position = new Vector3(-5.8f, 0f, -11.8f);
            instance.transform.rotation = Quaternion.Euler(0f, 180f, 0f);
            PrefabUtility.RecordPrefabInstancePropertyModifications(instance.transform);
            EditorSceneManager.MarkSceneDirty(scene);
            Debug.Log("Authored WayfarerShrine: " + (stone.TriangleCount + bronze.TriangleCount + glass.TriangleCount + wood.TriangleCount + linen.TriangleCount)
                + " triangles; saved mesh/material/prefab assets, no runtime authoring component.");
        }
        finally { Object.DestroyImmediate(root); }
    }

    static Vector2 Arc(float radius, float angle)
    {
        return new Vector2(Mathf.Cos(angle) * radius, 2.05f + Mathf.Sin(angle) * radius);
    }

    static Material MaterialAsset(string name, Shader shader, Color color, float metallic, float smoothness)
    {
        string path = Folder + "/" + name + ".mat";
        var material = AssetDatabase.LoadAssetAtPath<Material>(path);
        if (material == null) { material = new Material(shader); AssetDatabase.CreateAsset(material, path); }
        material.name = name;
        material.shader = shader;
        material.SetColor("_BaseColor", color);
        material.SetFloat("_Metallic", metallic);
        material.SetFloat("_Smoothness", smoothness);
        EditorUtility.SetDirty(material);
        return material;
    }

    static GameObject MeshObject(GameObject parent, string name, Mesh mesh, Material material)
    {
        var child = new GameObject(name);
        child.transform.SetParent(parent.transform, false);
        child.AddComponent<MeshFilter>().sharedMesh = mesh;
        child.AddComponent<MeshRenderer>().sharedMaterial = material;
        return child;
    }

    sealed class Geometry
    {
        readonly List<Vector3> vertices = new List<Vector3>();
        readonly List<int> triangles = new List<int>();
        readonly List<Vector2> uv = new List<Vector2>();
        public int TriangleCount { get { return triangles.Count / 3; } }

        void Face(Vector3 a, Vector3 b, Vector3 c)
        {
            int first = vertices.Count;
            vertices.Add(a); vertices.Add(b); vertices.Add(c);
            triangles.Add(first); triangles.Add(first + 1); triangles.Add(first + 2);
            var normal = Vector3.Cross(b - a, c - a).normalized;
            uv.Add(Project(a, normal)); uv.Add(Project(b, normal)); uv.Add(Project(c, normal));
        }

        static Vector2 Project(Vector3 v, Vector3 normal)
        {
            if (Mathf.Abs(normal.y) > .65f) return new Vector2(v.x, v.z) * .8f;
            return (Mathf.Abs(normal.x) > .65f ? new Vector2(v.z, v.y) : new Vector2(v.x, v.y)) * .8f;
        }

        void Quad(Vector3 a, Vector3 b, Vector3 c, Vector3 d) { Face(a, b, c); Face(a, c, d); }

        public void Block(Vector3 center, Vector3 size, float chip, int seed)
        {
            var random = new System.Random(seed);
            float x0 = center.x - size.x * .5f, x1 = center.x + size.x * .5f;
            float y0 = center.y - size.y * .5f, y1 = center.y + size.y * .5f;
            float c = Mathf.Min(chip, size.y * .2f);
            var outline = new List<Vector2> {
                new Vector2(x0+c, y0), new Vector2(x1-c, y0), new Vector2(x1, y0+c),
                new Vector2(x1, y1-c), new Vector2(x1-c, y1), new Vector2(x0+c, y1),
                new Vector2(x0, y1-c), new Vector2(x0, y0+c)
            };
            // Tiny chips vary the upper corner sizes without leaning or lifting the base.
            outline[3] += new Vector2(-.012f * (float)random.NextDouble(), 0f);
            outline[6] += new Vector2(.012f * (float)random.NextDouble(), 0f);
            Extrusion(outline, center.z, size.z, Mathf.Min(c * .5f, .018f));
        }

        public void Extrusion(List<Vector2> outline, float z, float depth, float bevel)
        {
            int count = outline.Count;
            var center = Vector2.zero;
            foreach (var point in outline) center += point;
            center /= count;
            var rings = new Vector3[4][];
            for (int ring = 0; ring < 4; ring++)
            {
                rings[ring] = new Vector3[count];
                float ringZ = ring == 0 ? z - depth * .5f : ring == 1 ? z - depth * .5f + bevel : ring == 2 ? z + depth * .5f - bevel : z + depth * .5f;
                for (int i = 0; i < count; i++)
                {
                    var point = outline[i];
                    if (ring == 0 || ring == 3) point += (center - point).normalized * bevel;
                    rings[ring][i] = new Vector3(point.x, point.y, ringZ);
                }
            }
            for (int ring = 0; ring < 3; ring++)
                for (int i = 0; i < count; i++)
                {
                    int j = (i + 1) % count;
                    Quad(rings[ring][i], rings[ring][j], rings[ring + 1][j], rings[ring + 1][i]);
                }
            for (int i = 1; i < count - 1; i++)
            {
                Face(rings[0][0], rings[0][i + 1], rings[0][i]);
                Face(rings[3][0], rings[3][i], rings[3][i + 1]);
            }
        }

        public void Radial(Vector3 center, float height, float bottomRadius, float topRadius, int sides)
        {
            Cylinder(center - Vector3.up * height * .5f, center + Vector3.up * height * .5f, bottomRadius, topRadius, sides);
        }

        public void Beam(Vector3 a, Vector3 b, float radius, int sides) { Cylinder(a, b, radius, radius * .93f, sides); }

        void Cylinder(Vector3 a, Vector3 b, float ra, float rb, int sides)
        {
            var axis = (b - a).normalized;
            var tangent = Vector3.Cross(axis, Mathf.Abs(axis.z) < .9f ? Vector3.forward : Vector3.right).normalized;
            var bitangent = Vector3.Cross(axis, tangent);
            for (int i = 0; i < sides; i++)
            {
                float t0 = i * Mathf.PI * 2f / sides, t1 = (i + 1) * Mathf.PI * 2f / sides;
                var d0 = tangent * Mathf.Cos(t0) + bitangent * Mathf.Sin(t0);
                var d1 = tangent * Mathf.Cos(t1) + bitangent * Mathf.Sin(t1);
                Quad(a + d0 * ra, a + d1 * ra, b + d1 * rb, b + d0 * rb);
                Face(a, a + d1 * ra, a + d0 * ra);
                Face(b, b + d0 * rb, b + d1 * rb);
            }
        }

        public void Banner(float left, float right, float top, float bottom, float z)
        {
            const int columns = 10, rows = 20;
            var grid = new Vector3[columns + 1, rows + 1];
            for (int x = 0; x <= columns; x++)
                for (int y = 0; y <= rows; y++)
                {
                    float u = (float)x / columns, v = (float)y / rows;
                    float fork = Mathf.Pow(v, 12f) * .12f * (1f - Mathf.Abs(u * 2f - 1f));
                    grid[x, y] = new Vector3(Mathf.Lerp(left, right, u) + .055f * v * v,
                        Mathf.Lerp(top, bottom, v) + fork,
                        z + Mathf.Sin(u * Mathf.PI * 4f + v * .9f) * .038f + .035f * Mathf.Sin(v * 3f));
                }
            for (int x = 0; x < columns; x++)
                for (int y = 0; y < rows; y++)
                    Quad(grid[x, y], grid[x + 1, y], grid[x + 1, y + 1], grid[x, y + 1]);
        }

        public Mesh Save(string name)
        {
            var mesh = new Mesh { name = name };
            mesh.SetVertices(vertices);
            mesh.SetTriangles(triangles, 0);
            mesh.SetUVs(0, uv);
            mesh.RecalculateNormals();
            mesh.RecalculateBounds();
            mesh.RecalculateTangents();
            string path = Folder + "/" + name + ".asset";
            var existing = AssetDatabase.LoadAssetAtPath<Mesh>(path);
            if (existing == null) { AssetDatabase.CreateAsset(mesh, path); return mesh; }
            EditorUtility.CopySerialized(mesh, existing);
            Object.DestroyImmediate(mesh);
            EditorUtility.SetDirty(existing);
            return existing;
        }
    }
}
