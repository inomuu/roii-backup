using System;
using System.IO;
using Roi.Arena;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using Object = UnityEngine.Object;

// Renders a temporary scene; does not open, save or change the user's arena scene.
public static class SeraphineSlashPreview
{
    const string Output = "/tmp/roi-seraphine-slash";

    [MenuItem("ROI/Art/Capture Seraphine Basic Slash")]
    public static void Capture()
    {
        Directory.CreateDirectory(Output);
        var scene = EditorSceneManager.NewPreviewScene();
        var previousTarget = RenderTexture.active;
        var texture = new Texture2D(2, 2, TextureFormat.RGBA32, false);
        var material = new Material(Shader.Find("Sprites/Default"));
        var target = new RenderTexture(960, 480, 24, RenderTextureFormat.ARGBHalf) { antiAliasing = 4 };
        var readback = new Texture2D(960, 480, TextureFormat.RGB24, false);
        var sprites = new Sprite[7];
        var profile = ScriptableObject.CreateInstance<VolumeProfile>();
        Material baselineMaterial = null;
        try
        {
            texture.LoadImage(File.ReadAllBytes(Path.Combine(Application.streamingAssetsPath, "art/seraphine.png")));
            texture.filterMode = FilterMode.Point;
            for (int f = 0; f < sprites.Length; f++)
                sprites[f] = Sprite.Create(texture, new Rect(f * 120, texture.height - 300, 120, 100),
                    new Vector2(.5f, .5f), 100);

            var camera = InScene("SlashPreviewCamera", scene).AddComponent<Camera>();
            camera.scene = scene;
            camera.transform.position = new Vector3(0f, 0f, -10f);
            camera.orthographic = true;
            camera.orthographicSize = .62f;
            camera.clearFlags = CameraClearFlags.SolidColor;
            camera.backgroundColor = new Color(.095f, .13f, .12f);
            camera.targetTexture = target;
            camera.enabled = false;
            camera.allowHDR = true;
            var cameraData = camera.GetUniversalAdditionalCameraData();
            cameraData.renderPostProcessing = true;
            cameraData.volumeLayerMask = 1 << 31;
            var post = InScene("SlashPreviewPost", scene);
            post.layer = 31;
            var volume = post.AddComponent<Volume>();
            volume.isGlobal = true;
            volume.sharedProfile = profile;
            var bloom = profile.Add<Bloom>(true);
            var arenaProfile = AssetDatabase.LoadAssetAtPath<VolumeProfile>("Assets/Zones/r1/VolumeProfile_VolumeProfile.asset");
            if (arenaProfile == null || !arenaProfile.TryGet<Bloom>(out var arenaBloom))
                throw new InvalidOperationException("Arena bloom reference is missing.");
            bloom.threshold.value = arenaBloom.threshold.value;
            bloom.intensity.value = arenaBloom.intensity.value;
            bloom.scatter.value = arenaBloom.scatter.value;
            profile.Add<Tonemapping>(true).mode.value = TonemappingMode.Neutral;
            var light = InScene("PreviewLight", scene).AddComponent<Light>();
            light.type = LightType.Directional;
            light.transform.rotation = Quaternion.Euler(35, -30, 0);

            var original = InScene("Original", scene).AddComponent<SpriteRenderer>();
            var enhanced = InScene("SwordEdge", scene).AddComponent<SpriteRenderer>();
            original.transform.position = new Vector3(-.62f, 0f, 0f);
            enhanced.transform.position = new Vector3(.62f, 0f, 0f);
            original.sharedMaterial = enhanced.sharedMaterial = material;
            var edge = SeraphineBasicSlash.Create(enhanced);
            if (edge == null) throw new InvalidOperationException("Sword edge material is missing.");
            var renderer = edge.GetComponent<MeshRenderer>();
            var baseline = SeraphineBasicSlash.Create(original);
            baselineMaterial = new Material(renderer.sharedMaterial);
            baselineMaterial.SetFloat("_EmissionGain", 2.5f);
            baseline.GetComponent<MeshRenderer>().sharedMaterial = baselineMaterial;
            if (!renderer.sharedMaterial.shader.isSupported)
                throw new InvalidOperationException("Sword edge shader is unsupported.");
            edge.Show(2, 0f, false);
            if (renderer.enabled) throw new InvalidOperationException("Suppressed edge is still visible.");
            edge.Show(0, 0f, true);
            if (renderer.enabled) throw new InvalidOperationException("Edge appears during preparation.");
            for (int frame = 2; frame <= 4; frame++)
            {
                edge.Show(frame, .5f, true);
                foreach (var p in edge.GetComponent<MeshFilter>().sharedMesh.vertices)
                    if (float.IsNaN(p.sqrMagnitude) || float.IsInfinity(p.sqrMagnitude))
                        throw new InvalidOperationException("Sword edge contains non-finite vertices.");
            }

            for (int sample = 0; sample < 60; sample++)
            {
                float time = sample / 60f;
                float pose = Mathf.Clamp((time - .12f) * 14f, 0f, 6f);
                int frame = Mathf.FloorToInt(pose);
                original.sprite = enhanced.sprite = sprites[frame];
                edge.Show(frame, pose - frame, true);
                Render(camera, target, readback, "motion-" + sample.ToString("D3") + ".png");
                if (sample == 17)
                {
                    var pixels = readback.GetPixels32();
                    int changed = 0;
                    for (int y = 0; y < readback.height; y++)
                        for (int x = 0; x < readback.width / 2; x++)
                        {
                            var left = pixels[y * readback.width + x];
                            var right = pixels[y * readback.width + x + readback.width / 2];
                            if (Math.Abs(left.r - right.r) + Math.Abs(left.g - right.g) + Math.Abs(left.b - right.b) > 36) changed++;
                        }
                    if (changed < 100) throw new InvalidOperationException("Sword edge did not render in the comparison.");
                    Debug.Log("SERAPHINE SLASH: changed comparison pixels=" + changed);
                    File.Copy(Path.Combine(Output, "motion-017.png"), Path.Combine(Output, "comparison.png"), true);
                    bloom.active = false;
                    Render(camera, target, readback, "no-bloom.png");
                    bloom.active = true;
                }
                // Same shape and post-processing; only the local emission differs.
                baseline.Show(frame, pose - frame, true);
                Render(camera, target, readback, "glow-compare-" + sample.ToString("D3") + ".png");
                baseline.Show(frame, pose - frame, false);
            }

            original.sprite = enhanced.sprite = sprites[2];
            edge.Show(2, .5f, true);
            original.flipX = enhanced.flipX = true;
            edge.Show(2, .5f, true);
            Render(camera, target, readback, "flipped.png");
            camera.backgroundColor = new Color(.7f, .73f, .7f);
            Render(camera, target, readback, "light-background.png");
            edge.Show(6, 0f, true);
            if (renderer.enabled) throw new InvalidOperationException("Edge outlives the attack.");
            Debug.Log("SERAPHINE SLASH: captured 60 motion frames; preparation, suppression, recovery and material checks passed. " + Output);
        }
        finally
        {
            RenderTexture.active = previousTarget;
            EditorSceneManager.ClosePreviewScene(scene);
            foreach (var sprite in sprites) if (sprite != null) Object.DestroyImmediate(sprite);
            Object.DestroyImmediate(texture);
            Object.DestroyImmediate(material);
            if (baselineMaterial != null) Object.DestroyImmediate(baselineMaterial);
            Object.DestroyImmediate(readback);
            foreach (var component in profile.components) Object.DestroyImmediate(component);
            Object.DestroyImmediate(profile);
            target.Release();
            Object.DestroyImmediate(target);
        }
    }

    static GameObject InScene(string name, Scene scene)
    {
        var go = new GameObject(name);
        SceneManager.MoveGameObjectToScene(go, scene);
        return go;
    }

    static void Render(Camera camera, RenderTexture target, Texture2D readback, string filename)
    {
        camera.Render();
        RenderTexture.active = target;
        readback.ReadPixels(new Rect(0, 0, target.width, target.height), 0, 0);
        readback.Apply();
        File.WriteAllBytes(Path.Combine(Output, filename), readback.EncodeToPNG());
    }
}
