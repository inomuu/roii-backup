using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

// Explicit Edit Mode authoring only. The saved scene does not run this builder.
public static class R1PainterlyStudy
{
    const string Dir = "Assets/Environment/PainterlyMeadow/";
    public static void Build()
    {
        var scene = UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if (EditorApplication.isPlaying || scene.path != "Assets/Scenes/Zone_r1_Painterly.unity")
            throw new System.InvalidOperationException("Open the alternative in Edit Mode.");
        if (GameObject.Find("Arena/Field/PainterlyScenery") != null)
            throw new System.InvalidOperationException("Study already authored.");
        foreach (string file in new[] { "Meadow", "Trail", "Hazel" })
        {
            var importer = (TextureImporter)AssetImporter.GetAtPath(Dir + file + ".png");
            importer.maxTextureSize = 1024;
            importer.mipmapEnabled = true;
            importer.anisoLevel = 8;
            importer.wrapMode = file == "Hazel" ? TextureWrapMode.Clamp : TextureWrapMode.Repeat;
            importer.alphaIsTransparency = file == "Hazel";
            importer.SaveAndReimport();
        }
        var terrain = Object.FindFirstObjectByType<Terrain>();
        var previous = terrain.terrainData;
        var td = new TerrainData { heightmapResolution = 257, alphamapResolution = 512, size = previous.size };
        AssetDatabase.CreateAsset(td, Dir + "Terrain.asset");
        float[,] heights = new float[257,257];
        for(int z=0;z<257;z++) for(int x=0;x<257;x++) {
            float wz=terrain.transform.position.z+z/256f*td.size.z;
            float bank=Mathf.SmoothStep(0,1,Mathf.InverseLerp(-11,-20,wz));
            heights[z,x]=bank*(.65f+.55f*Mathf.PerlinNoise(x*.02f, z*.015f))/td.size.y;
        }
        td.SetHeights(0,0,heights);
        var layers = new TerrainLayer[3];
        for(int i=0;i<3;i++) {
            layers[i]=new TerrainLayer { diffuseTexture=AssetDatabase.LoadAssetAtPath<Texture2D>(Dir+(i==2?"Trail":"Meadow")+".png"), tileSize=Vector2.one*(i==0?5.2f:i==1?8.3f:4.6f), smoothness=.06f, metallic=0 };
            layers[i].diffuseRemapMin = Vector4.zero;
            layers[i].diffuseRemapMax = i==1?new Vector4(1.12f,1.1f,.95f,1):Vector4.one;
            AssetDatabase.CreateAsset(layers[i],Dir+"Ground_"+i+".terrainlayer");
        }
        td.terrainLayers=layers;
        var alpha=new float[512,512,3];
        for(int z=0;z<512;z++) for(int x=0;x<512;x++) {
            float wx=terrain.transform.position.x+x/511f*td.size.x;
            float wz=terrain.transform.position.z+z/511f*td.size.z;
            float center=-1.6f+.45f*Mathf.Sin(wx*.17f);
            float edge=(Mathf.PerlinNoise(wx*1.1f+40,wz*1.1f+20)-.5f)*.6f;
            float path=1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(.72f,1.48f,Mathf.Abs(wz-center)+edge));
            float patch=Mathf.PerlinNoise(wx*.22f+10,wz*.22f+20);
            float dirt=Mathf.SmoothStep(0,.16f,Mathf.InverseLerp(.58f,.8f,patch));
            path=Mathf.Max(path,dirt);
            float blend=Mathf.PerlinNoise(wx*.1f+31,wz*.1f+17)*.7f;
            alpha[z,x,0]=(1-path)*(1-blend); alpha[z,x,1]=(1-path)*blend; alpha[z,x,2]=path;
        }
        td.SetAlphamaps(0,0,alpha);
        terrain.terrainData=td; terrain.GetComponent<TerrainCollider>().terrainData=td;
        EditorUtility.SetDirty(terrain); EditorUtility.SetDirty(terrain.GetComponent<TerrainCollider>());
        var old=GameObject.Find("Arena/Field/R1_AuthoredScenery");
        var root=new GameObject("PainterlyScenery"); root.transform.SetParent(old.transform.parent,false);
        // Reuse authored structural meshes; the new dressing, terrain and materials are independent.
        foreach(string name in new[]{"R1_Masonry","Reference_Boundary","WayfarerShrine","Fence_East","Fence_West"}) {
            var source=old.transform.Find(name);
            if(source==null) continue;
            var copy=Object.Instantiate(source.gameObject,root.transform); copy.name=name; copy.SetActive(true);
        }
        old.SetActive(false);
        var stone = new Material(Shader.Find("ROI/Painterly/SoftCel"));
        var oldStone=AssetDatabase.LoadAssetAtPath<Material>("Assets/Zones/r1/QualityProps/EnvRef/WeatheredLimestone_Relief.mat");
        stone.SetTexture("_BaseMap",oldStone.GetTexture("_BaseMap")); stone.SetFloat("_CelBlend",.65f);
        AssetDatabase.CreateAsset(stone,Dir+"Stone.mat");
        foreach(var renderer in root.GetComponentsInChildren<MeshRenderer>()) {
            var mats=renderer.sharedMaterials;
            for(int i=0;i<mats.Length;i++) if(mats[i]!=null && mats[i].name.Contains("Limestone")) mats[i]=stone;
            renderer.sharedMaterials=mats;
        }
        var foliage=new Material(AssetDatabase.LoadAssetAtPath<Material>("Assets/Environment/MeadowAtmosphere/HazelThicket_Breeze.mat"));
        foliage.SetTexture("_BaseMap",AssetDatabase.LoadAssetAtPath<Texture2D>(Dir+"Hazel.png"));
        foliage.SetColor("_BaseColor",Color.white); foliage.SetFloat("_Cutoff",.55f); foliage.SetFloat("_RockProtection",0);
        AssetDatabase.CreateAsset(foliage,Dir+"Hazel.mat");
        var mesh=AssetDatabase.LoadAssetAtPath<Mesh>("Assets/Environment/MeadowAtmosphere/BreezeFoliageGrid.asset");
        var shrub=new GameObject("PaintedHazel"); shrub.AddComponent<MeshFilter>().sharedMesh=mesh;
        var mr=shrub.AddComponent<MeshRenderer>(); mr.sharedMaterial=foliage; mr.shadowCastingMode=ShadowCastingMode.Off;
        shrub.transform.rotation=Quaternion.Euler(42,180,0);
        var prefab=PrefabUtility.SaveAsPrefabAsset(shrub,Dir+"PaintedHazel.prefab"); Object.DestroyImmediate(shrub);
        for(int i=0;i<13;i++) {
            float x=-24+i*4f;
            if(Mathf.Abs(x+5.8f)<2.7f) continue;
            Place(prefab,root.transform,"BackHazel_"+i,new Vector3(x,.05f,-12.0f-.35f*Mathf.Sin(i*2)),2.5f+.5f*Mathf.Sin(i*1.7f));
        }
        for(int i=0;i<9;i++) {
            float x=-12+i*3f;
            Place(prefab,root.transform,"FrontHazel_"+i,new Vector3(x,-.05f,7.4f+.4f*Mathf.Sin(i)),2.8f+.5f*Mathf.Sin(i*1.9f));
        }
        for(int i=0;i<22;i++) {
            float x=-24+i*2.25f;
            Place(prefab,root.transform,"WallHerb_"+i,new Vector3(x,-.04f,-9.4f),.55f+.17f*Mathf.Sin(i*2.2f));
        }
        // Preserve the original true 3D trees from the structural boundary; flowers are sparsely regrouped.
        var flowers=old.transform.Find("EnvRef_EdgeGrowth");
        if(flowers!=null) foreach(Transform source in flowers) if(source.name.StartsWith("Ivory_") && source.GetSiblingIndex()%15==0) {
            var copy=Object.Instantiate(source.gameObject,root.transform); copy.name="MeadowFlowers";
        }
        foreach(var volume in Object.FindObjectsByType<Volume>(FindObjectsSortMode.InstanceID)) {
            string dest=Dir+"Grade.asset"; AssetDatabase.CopyAsset(AssetDatabase.GetAssetPath(volume.sharedProfile),dest);
            volume.sharedProfile=AssetDatabase.LoadAssetAtPath<VolumeProfile>(dest);
            ColorAdjustments ca; if(volume.sharedProfile.TryGet(out ca)) { ca.postExposure.value=.7f; ca.contrast.value=10; ca.saturation.value=5; ca.colorFilter.value=Color.white; }
            SplitToning st; if(volume.sharedProfile.TryGet(out st)) { st.shadows.value=new Color(.43f,.53f,.58f); st.highlights.value=new Color(.59f,.56f,.46f); }
            DepthOfField dof; if(volume.sharedProfile.TryGet(out dof)) { dof.focalLength.value=145; dof.aperture.value=4; }
            EditorUtility.SetDirty(volume.sharedProfile);
        }
        foreach(var light in Object.FindObjectsByType<Light>(FindObjectsSortMode.InstanceID)) {
            if(light.type==LightType.Directional && light.name.Contains("Sun")) { light.intensity=1.65f; light.color=new Color(1,.94f,.8f); light.shadowStrength=.68f; }
            if(light.name=="CanopyBounce") { light.intensity=.85f; light.color=new Color(.58f,.76f,.86f); }
            if(light.name=="Reference_SunBreak") light.intensity=100;
        }
        AssetDatabase.SaveAssets(); EditorSceneManager.MarkSceneDirty(scene); EditorSceneManager.SaveScene(scene);
    }
    static void Place(GameObject prefab,Transform parent,string name,Vector3 position,float scale) {
        var go=(GameObject)PrefabUtility.InstantiatePrefab(prefab); go.name=name; go.transform.SetParent(parent,false);
        go.transform.position=position; go.transform.localScale=Vector3.one*scale;
        PrefabUtility.RecordPrefabInstancePropertyModifications(go.transform);
    }
}
