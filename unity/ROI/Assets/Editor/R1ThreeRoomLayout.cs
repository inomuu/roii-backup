using System;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using Object = UnityEngine.Object;

// Edit Mode authoring only. Room traversal/gameplay remains owned by ArenaView.
public static class R1ThreeRoomLayout
{
    public const string ScenePath = "Assets/Scenes/Zone_r1_Painterly_3Rooms.unity";
    const string Folder = "Assets/Environment/PainterlyMeadow/ThreeRooms";
    /// TAKEN FROM THE GAME, not declared here. `ArenaView.RoomSpacing` is the distance the party
    /// actually marches between rooms; if this file carried its own 52 the two would drift the day
    /// either side moved, and the failure would be a party stopping in the gap between two rooms —
    /// visible only by playing it. Editor code may read a runtime constant; the reverse is not
    /// possible, which settles which side owns the number.
    public const float Spacing = Roi.Arena.ArenaView.RoomSpacing;

    [MenuItem("ROI/Art/Three Rooms/Create From Painterly Scene")]
    public static void Build()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Exit Play Mode first.");
        var scene = UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if (scene.path != "Assets/Scenes/Zone_r1_Painterly.unity" || scene.isDirty)
            throw new InvalidOperationException("Open and save the painterly source scene first.");
        if (AssetDatabase.LoadAssetAtPath<SceneAsset>(ScenePath) != null)
            throw new InvalidOperationException("Three-room scene already exists; edit it directly.");
        EditorSceneManager.SaveScene(scene, ScenePath, true);
        scene = EditorSceneManager.OpenScene(ScenePath);
        if (!AssetDatabase.IsValidFolder(Folder))
            AssetDatabase.CreateFolder("Assets/Environment/PainterlyMeadow", "ThreeRooms");

        var field = GameObject.Find("Arena/Field").transform;
        var art = field.Find("PainterlyScenery");
        // Discard dormant art experiments in this new scene, keeping the source intact.
        foreach (var t in art.GetComponentsInChildren<Transform>(true))
            if (t != null && t != art && (!t.gameObject.activeSelf || t.name.StartsWith("Canopy_Shade_")))
                Object.DestroyImmediate(t.gameObject);
        var walls = art.Find("R1_Masonry");
        foreach (var name in new[] { "Wall_0", "Wall_12" })
        {
            var wall = walls.Find(name);
            var p = wall.localPosition; p.z = -10.6f; wall.localPosition = p;
            wall.localRotation = Quaternion.identity;
            PrefabUtility.RecordPrefabInstancePropertyModifications(wall);
        }

        var ground = field.Find("Ground").GetComponent<Terrain>();
        var originalData = ground.terrainData;
        var tiles = new Terrain[3];
        for (int room = 0; room < 3; room++)
        {
            var root = new GameObject("Room_0" + (room + 1)).transform;
            root.SetParent(field, false);
            root.localPosition = new Vector3(-Spacing * room, 0, 0);
            var marker = new GameObject("CameraAnchor").transform;
            marker.SetParent(root, false);
            marker.localPosition = new Vector3(0, 13.5f, 13);
            marker.localRotation = Quaternion.Euler(42, 180, 0);

            Terrain tile;
            if (room == 0) tile = ground; // ArenaView expects Field/Ground for baked adoption.
            else
            {
                var copy = Object.Instantiate(ground.gameObject, root);
                copy.name = "Ground";
                copy.transform.localPosition = new Vector3(-Spacing / 2, -.01f, -20.976f);
                tile = copy.GetComponent<Terrain>();
                var scenery = Object.Instantiate(art.gameObject, root);
                scenery.name = "PainterlyScenery";
                scenery.transform.localPosition = Vector3.zero;
                foreach (var path in new[] { "Arena/MeadowAtmosphere", "Arena/Reference_SunBreak", "Arena/Field/Atmosphere" })
                {
                    var source = GameObject.Find(path);
                    var extra = Object.Instantiate(source, root);
                    extra.name = source.name;
                    extra.transform.position = source.transform.position + root.position;
                }
            }
            var data = Object.Instantiate(originalData);
            data.name = "Room_0" + (room + 1) + "_Terrain";
            var size = data.size; size.x = Spacing; data.size = size;
            MatchEdges(data);
            var layers = data.terrainLayers;
            for (int l = 0; l < layers.Length; l++)
            {
                layers[l] = Object.Instantiate(layers[l]);
                layers[l].tileOffset += new Vector2(-Spacing * room, 0);
                AssetDatabase.CreateAsset(layers[l], Folder + "/Room" + (room + 1) + "_Layer" + l + ".terrainlayer");
            }
            var weights = data.GetAlphamaps(0, 0, data.alphamapResolution, data.alphamapResolution);
            data.terrainLayers = layers;
            data.SetAlphamaps(0, 0, weights);
            AssetDatabase.CreateAsset(data, Folder + "/Room" + (room + 1) + "_Terrain.asset");
            tile.terrainData = data;
            var collider = tile.GetComponent<TerrainCollider>();
            if (collider != null) collider.terrainData = data;
            tile.transform.position = new Vector3(-Spacing / 2 - Spacing * room, -.01f, -20.976f);
            tiles[room] = tile;
        }
        for (int i = 0; i < 3; i++)
            tiles[i].SetNeighbors(i < 2 ? tiles[i + 1] : null, null, i > 0 ? tiles[i - 1] : null, null);
        art.SetParent(field.Find("Room_01"), true);
        // Adjacent edge cards overlap on the same plane when rooms repeat.
        for (int room = 2; room <= 3; room++)
        {
            var cards = field.Find("Room_0" + room + "/PainterlyScenery/PaintedThicketCards");
            var duplicate = cards.Find("BackBrushThicket_9");
            if (duplicate != null) Object.DestroyImmediate(duplicate.gameObject);
            var previous = field.Find("Room_0" + (room - 1) + "/PainterlyScenery/PaintedThicketCards/BackBrushThicket_0");
            previous.localPosition += new Vector3(-.8f, 0, -.25f);
            PrefabUtility.RecordPrefabInstancePropertyModifications(previous);
        }
        var surround = field.Find("Surround");
        if (surround != null) Object.DestroyImmediate(surround.gameObject);
        AssetDatabase.SaveAssets();
        EditorSceneManager.MarkSceneDirty(scene);
        EditorSceneManager.SaveScene(scene);
    }

    static void MatchEdges(TerrainData data)
    {
        int n = data.heightmapResolution;
        var h = data.GetHeights(0, 0, n, n);
        for (int z = 0; z < n; z++)
        {
            float edge = (h[z, 0] + h[z, n - 1]) * .5f;
            for (int x = 0; x < 12; x++)
            {
                float weight = 1 - Mathf.SmoothStep(0, 1, x / 11f);
                h[z, x] = Mathf.Lerp(h[z, x], edge, weight);
                h[z, n - 1 - x] = Mathf.Lerp(h[z, n - 1 - x], edge, weight);
            }
        }
        data.SetHeights(0, 0, h);
        int a = data.alphamapResolution;
        var alpha = data.GetAlphamaps(0, 0, a, a);
        for (int z = 0; z < a; z++)
            for (int l = 0; l < data.alphamapLayers; l++)
            {
                float edge = (alpha[z, 0, l] + alpha[z, a - 1, l]) * .5f;
                for (int x = 0; x < 24; x++)
                {
                    float weight = 1 - Mathf.SmoothStep(0, 1, x / 23f);
                    alpha[z, x, l] = Mathf.Lerp(alpha[z, x, l], edge, weight);
                    alpha[z, a - 1 - x, l] = Mathf.Lerp(alpha[z, a - 1 - x, l], edge, weight);
                }
            }
        data.SetAlphamaps(0, 0, alpha);
    }

    [MenuItem("ROI/Art/Three Rooms/View Room 1")] public static void View1() { View(0); }
    [MenuItem("ROI/Art/Three Rooms/View Room 2")] public static void View2() { View(1); }
    [MenuItem("ROI/Art/Three Rooms/View Room 3")] public static void View3() { View(2); }
    static void View(int room)
    {
        if (EditorApplication.isPlaying || UnityEngine.SceneManagement.SceneManager.GetActiveScene().path != ScenePath) return;
        var camera = GameObject.Find("Arena/ArenaCamera").transform;
        Undo.RecordObject(camera, "Preview room");
        camera.position = new Vector3(-Spacing * room, 13.5f, 13);
    }
}
