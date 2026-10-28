using UnityEngine;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

// Explicit editor authoring. No scene-generation component is added to the saved scene.
public static class R1PainterlyGroundV2
{
    const string Dir="Assets/Environment/PainterlyMeadow/V2/";
    public static void Apply()
    {
        var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if(EditorApplication.isPlaying || scene.path!="Assets/Scenes/Zone_r1_Painterly.unity")
            throw new System.InvalidOperationException("Open painterly alternative in Edit Mode.");
        foreach(string name in new[]{"ShortMeadow","PaleEarth"}) {
            var imp=(TextureImporter)AssetImporter.GetAtPath(Dir+name+".png");
            imp.maxTextureSize=1024;imp.mipmapEnabled=true;imp.anisoLevel=8;
            imp.wrapMode=TextureWrapMode.Repeat;imp.SaveAndReimport();
        }
        var terrain=Object.FindFirstObjectByType<Terrain>();
        var td=terrain.terrainData;
        var layers=new TerrainLayer[3];
        for(int i=0;i<3;i++) {
            string path=Dir+"Ground_"+i+".terrainlayer";
            layers[i]=AssetDatabase.LoadAssetAtPath<TerrainLayer>(path);
            if(layers[i]==null){layers[i]=new TerrainLayer();AssetDatabase.CreateAsset(layers[i],path);}
            layers[i].diffuseTexture=AssetDatabase.LoadAssetAtPath<Texture2D>(Dir+(i==2?"PaleEarth":"ShortMeadow")+".png");
            layers[i].tileSize=Vector2.one*(i==0?15f:i==1?21.3f:7f);
            layers[i].tileOffset=i==1?new Vector2(3.3f,4.7f):Vector2.zero;
            layers[i].diffuseRemapMin=Vector4.zero;
            layers[i].diffuseRemapMax=i==1?new Vector4(.87f,.96f,.97f,1):Vector4.one;
            layers[i].normalMapTexture=null;layers[i].smoothness=.04f;layers[i].metallic=0;
            EditorUtility.SetDirty(layers[i]);
        }
        td.terrainLayers=layers;
        int size=td.alphamapResolution;
        var a=new float[size,size,3];
        for(int z=0;z<size;z++)for(int x=0;x<size;x++){
            float wx=terrain.transform.position.x+x/(float)(size-1)*td.size.x;
            float wz=terrain.transform.position.z+z/(float)(size-1)*td.size.z;
            float center=-1.45f+.48f*Mathf.Sin(wx*.19f)+.1f*Mathf.Sin(wx*.63f);
            float noise=(Mathf.PerlinNoise(wx*1.7f+40,wz*1.7f+20)-.5f)*.7f;
            float width=1.2f+.15f*Mathf.Sin(wx*.45f);
            float road=1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(width,width+.65f,Mathf.Abs(wz-center)+noise));
            float patch=Mathf.PerlinNoise(wx*.31f+17,wz*.31f+37);
            float dust=.20f*Mathf.SmoothStep(0,1,Mathf.InverseLerp(.56f,.75f,patch));
            road=Mathf.Max(road,dust);
            float blend=Mathf.SmoothStep(.15f,.7f,Mathf.PerlinNoise(wx*.12f+31,wz*.12f+18));
            a[z,x,0]=(1-road)*(1-blend);a[z,x,1]=(1-road)*blend;a[z,x,2]=road;
        }
        td.SetAlphamaps(0,0,a);EditorUtility.SetDirty(td);
        RenderSettings.ambientMode=AmbientMode.Flat;
        RenderSettings.ambientLight=new Color(.25f,.34f,.39f);
        foreach(var l in Object.FindObjectsByType<Light>(FindObjectsSortMode.InstanceID)){
            if(l.name=="Sun"){l.intensity=1.8f;l.color=new Color(1,.98f,.9f);l.shadowStrength=.73f;}
            if(l.name=="CanopyBounce"){l.intensity=.65f;l.color=new Color(.64f,.81f,.88f);}
            if(l.name=="Reference_SunBreak"){l.intensity=130;l.color=new Color(1,.95f,.79f);l.spotAngle=105;l.innerSpotAngle=65;}
        }
        foreach(var v in Object.FindObjectsByType<Volume>(FindObjectsSortMode.InstanceID)){
            ColorAdjustments ca;if(v.sharedProfile.TryGet(out ca)){ca.postExposure.value=.8f;ca.contrast.value=5;ca.saturation.value=-7;ca.colorFilter.value=Color.white;}
            SplitToning st;if(v.sharedProfile.TryGet(out st)){st.shadows.value=new Color(.43f,.53f,.57f);st.highlights.value=new Color(.55f,.54f,.48f);}
            LiftGammaGain lift;if(v.sharedProfile.TryGet(out lift)){lift.lift.value=new Vector4(1,1,1,0);lift.gamma.value=new Vector4(1,1,1,0);lift.gain.value=new Vector4(1,1,1,0);}
            EditorUtility.SetDirty(v.sharedProfile);v.profile=null;
        }
        var stone=AssetDatabase.LoadAssetAtPath<Material>("Assets/Environment/PainterlyMeadow/Stone.mat");
        stone.SetFloat("_CelBlend",.35f);stone.SetColor("_BaseColor",new Color(.94f,.91f,.84f));
        stone.SetColor("_ShadowTint",new Color(.66f,.76f,.82f));
        EditorUtility.SetDirty(stone);
        AssetDatabase.SaveAssets();EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);
    }
}
