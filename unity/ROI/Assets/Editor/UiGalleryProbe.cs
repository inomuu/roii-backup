using System;
using System.Collections;
using System.IO;
using System.Reflection;
using Roi.Sim;
using Roi.UI;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

// Disposable presentation fixture: no SessionBoot, login, save-store or server mutation.
[InitializeOnLoad]
public static class UiGalleryProbe
{
    static UiGalleryProbe() { EditorApplication.playModeStateChanged += Entered; }
    public static void ResumeCapture()
    {
        Application.runInBackground=true; EditorApplication.isPaused=false;
        var view=EditorWindow.GetWindow(Type.GetType("UnityEditor.GameView,UnityEditor"));view.Show();view.Focus();
    }
    static string _label;
    static bool _optionsEnabled;
    static EnterPlayModeOptions _options;
    public static void Motion() => Run("motion");
    public static void Album() => Run("album");
    public static void AlbumCompact() => Run("album-compact");
    public static void Before() => Run("before");
    public static void IconsFinal() => Run("icons-final");
    public static void IconsFinalCompact() => Run("icons-final-compact");
    public static void CelestialIcons() => Run("celestial-icons-61");
    public static void CelestialIconsCompact() => Run("celestial-icons-61-compact");
    public static void IconsCurrent() => Run("ui-icons-61");
    public static void IconsCurrentCompact() => Run("ui-icons-61-compact");
    public static void CardsIcons() => Run("cards-ux-60-icons-61");
    public static void CardsIconsCompact() => Run("cards-ux-60-icons-61-compact");
    public static void AlbumIcons() => Run("album-icons-61");
    public static void AlbumIconsCompact() => Run("album-icons-61-compact");
    public static void MonsterIcons() => Run("monster-icons-61");
    public static void MonsterIconsCompact() => Run("monster-icons-61-compact");
    public static void IconsHome() => Run("font-numbers-icons-61");
    public static void IconsHomeCompact() => Run("font-numbers-icons-61-compact");
    public static void CelestialPolish() => Run("celestial-icons-62");
    public static void CelestialPolishCompact() => Run("celestial-icons-62-compact");
    public static void IconsCurrent62() => Run("ui-icons-62");
    public static void IconsCurrent62Compact() => Run("ui-icons-62-compact");
    public static void Spacing63() => Run("ui-spacing-63");
    public static void Spacing63Compact() => Run("ui-spacing-63-compact");
    public static void LayoutSpacing63() => Run("layout-ux-56-spacing-63");
    public static void LayoutSpacing63Compact() => Run("layout-ux-56-spacing-63-compact");
    public static void Spacing64() => Run("ui-spacing-64");
    public static void Spacing64Compact() => Run("ui-spacing-64-compact");
    public static void AlbumSpacing64() => Run("album-spacing-64");
    public static void AlbumSpacing64Compact() => Run("album-spacing-64-compact");
    public static void CardsSpacing64() => Run("cards-ux-60-spacing-64");
    public static void CardsSpacing64Compact() => Run("cards-ux-60-spacing-64-compact");
    public static void MonsterSpacing64() => Run("monster-spacing-64");
    public static void MonsterSpacing64Compact() => Run("monster-spacing-64-compact");
    public static void CardsIcons62() => Run("cards-ux-60-icons-62");
    public static void CardsIcons62Compact() => Run("cards-ux-60-icons-62-compact");
    public static void AlbumIcons62() => Run("album-icons-62");
    public static void AlbumIcons62Compact() => Run("album-icons-62-compact");
    public static void MonsterIcons62() => Run("monster-icons-62");
    public static void MonsterIcons62Compact() => Run("monster-icons-62-compact");
    public static void SetupCelestialAtlas()
    {
        const string path="Assets/Resources/HomeUI/CelestialEmblems-62.png";
        AssetDatabase.ImportAsset(path,ImportAssetOptions.ForceSynchronousImport);
        var importer=(TextureImporter)AssetImporter.GetAtPath(path);
        importer.textureType=TextureImporterType.Default;
        importer.alphaSource=TextureImporterAlphaSource.FromGrayScale;
        importer.alphaIsTransparency=false;importer.sRGBTexture=false;
        importer.mipmapEnabled=true;importer.filterMode=FilterMode.Trilinear;
        importer.wrapMode=TextureWrapMode.Clamp;importer.npotScale=TextureImporterNPOTScale.None;
        importer.textureCompression=TextureImporterCompression.Uncompressed;
        importer.maxTextureSize=2048;importer.isReadable=false;importer.SaveAndReimport();
        Debug.Log("CELESTIAL62 ATLAS READY: luminance mask, mipmapped, uncompressed.");
    }
    public static void Typography() => Run("typography");
    public static void TypographyCompact() => Run("typography-compact");
    public static void Monster() => Run("monster");
    public static void MonsterCompact() => Run("monster-compact");
    public static void After() => Run("after");
    public static void Compact() => Run("compact");
    public static void ApexRoster() => Run("apex-roster-48");
    public static void ApexRosterCompact() => Run("apex-roster-48-compact");
    public static void ApexDock() => Run("apex-dock-51");
    public static void ApexDockCompact() => Run("apex-dock-51-compact");
    public static void UiFontNumbers() => Run("font-numbers-54");
    public static void UiFontNumbersCompact() => Run("font-numbers-54-compact");
    public static void UiFontReview() => Run("ui-font-54");
    public static void UiFontCompact() => Run("ui-font-54-compact");
    public static void UiFontAlbum() => Run("album-font-54");
    public static void UiFontMonster() => Run("monster-font-54");
    public static void MonsterPolish() => Run("monster-polish-55");
    public static void MonsterPolishCompact() => Run("monster-polish-55-compact");
    public static void UiFontApex() => Run("apex-dock-font-54");
    public static void UiFontThai() => Run("font-thai-54");
    public static void UiAuditBefore() => Run("ui-ux-56-before-compact");
    public static void UiAudit() => Run("ui-ux-56");
    public static void UiAuditCompact() => Run("ui-ux-56-compact");
    public static void UiAuditAlbum() => Run("album-ux-56");
    public static void UiAuditMonster() => Run("monster-ux-56");
    public static void UiAuditApex() => Run("apex-dock-ux-56");
    public static void UiAuditMotion() => Run("ui-motion-56");
    public static void UiAuditThai() => Run("font-thai-56");
    public static void UiAuditGacha() => Run("excavation-ux-56");
    public static void ExcavationTheme() => Run("excavation-theme-58");
    public static void ExcavationThemeCompact() => Run("excavation-theme-58-compact");
    public static void Banner65() => Run("excavation-banner-65");
    public static void Banner65Compact() => Run("excavation-banner-65-compact");
    public static void Banner65Current() => Run("gacha-current-58-banner-65");
    public static void Banner65CurrentCompact() => Run("gacha-current-58-banner-65-compact");
    public static void Banner67() => Run("excavation-banner-67");
    public static void Banner67Compact() => Run("excavation-banner-67-compact");
    public static void Banner67Current() => Run("gacha-current-58-banner-67");
    public static void Banner67CurrentCompact() => Run("gacha-current-58-banner-67-compact");
    public static void Banner68() => Run("excavation-banner-68");
    public static void Banner68Compact() => Run("excavation-banner-68-compact");
    public static void Banner68Current() => Run("gacha-current-58-banner-68");
    public static void Banner68CurrentCompact() => Run("gacha-current-58-banner-68-compact");
    public static void Pull69() => Run("excavation-pull-69");
    public static void Pull69Compact() => Run("excavation-pull-69-compact");
    public static void Pull69Current() => Run("gacha-current-58-pull-69");
    public static void Pull69CurrentCompact() => Run("gacha-current-58-pull-69-compact");
    public static void Action70() => Run("excavation-action-70");
    public static void Action70Compact() => Run("excavation-action-70-compact");
    public static void Action70Current() => Run("gacha-current-58-action-70");
    public static void Action70CurrentCompact() => Run("gacha-current-58-action-70-compact");
    public static void Wallet71() => Run("excavation-wallet-71");
    public static void Wallet71Compact() => Run("excavation-wallet-71-compact");
    public static void Wallet71Current() => Run("gacha-current-58-wallet-71");
    public static void Wallet71CurrentCompact() => Run("gacha-current-58-wallet-71-compact");
    public static void Portrait84() => Run("portrait-center-57-clear-84");
    public static void Portrait84Compact() => Run("portrait-center-57-clear-84-compact");
    public static void Portrait84Content() => Run("ui-spacing-64-portrait-84");
    public static void Portrait84ContentCompact() => Run("ui-spacing-64-portrait-84-compact");
    public static void Attributes85() => Run("management-dock-59-attributes-85");
    public static void Attributes85Compact() => Run("management-dock-59-attributes-85-compact");
    public static void All86() => Run("ui-spacing-64-all-86");
    public static void Dialogs94() => Run("dialogs-all-86-94");
    public static void Dialogs94Compact() => Run("dialogs-all-86-94-compact");
    public static void More94() => Run("more-dialogs-all-86-94");
    public static void More94Compact() => Run("more-dialogs-all-86-94-compact");
    public static void Dialogs93() => Run("dialogs-all-86-93");
    public static void Dialogs93Compact() => Run("dialogs-all-86-93-compact");
    public static void More93() => Run("more-dialogs-all-86-93");
    public static void System93() => Run("system-all-86-93");
    public static void Dialog92() => Run("excavation-dialog-92");
    public static void Dialog92Compact() => Run("excavation-dialog-92-compact");
    public static void Dialog91() => Run("excavation-dialog-91");
    public static void Dialog91Compact() => Run("excavation-dialog-91-compact");
    public static void Home101() => Run("home-101");
    public static void Inventory103() => Run("inventory-103");
    public static void Inventory103Mobile() => Run("inventory-103-mobile");
    public static void Inventory103Compact() => Run("inventory-103-compact");
    public static void InventoryRestored104() => Run("inventory-restored-104");
    public static void InventoryRestored104Mobile() => Run("inventory-restored-104-mobile");
    public static void InventoryRestored104Compact() => Run("inventory-restored-104-compact");
    public static void Home101Mobile() => Run("home-101-mobile");
    public static void Home101Compact() => Run("home-101-compact");
    public static void Menu90() => Run("menu-90");
    public static void Menu90Compact() => Run("menu-90-compact");
    public static void Menu89() => Run("menu-89");
    public static void Menu89Compact() => Run("menu-89-compact");
    public static void Menu88() => Run("menu-88");
    public static void Menu88Compact() => Run("menu-88-compact");
    public static void Dialogs86() => Run("dialogs-all-86");
    public static void Dialogs86Compact() => Run("dialogs-all-86-compact");
    public static void Trade105Mobile() => Run("online-all-86-trade105-mobile");
    public static void Shop113() => Run("online-all-86-shop113");
    public static void Shop113Mobile() => Run("online-all-86-shop113-mobile");
    public static void Shop112() => Run("online-all-86-shop112");
    public static void Shop112Mobile() => Run("online-all-86-shop112-mobile");
    public static void Shop111() => Run("online-all-86-shop111");
    public static void Shop111Mobile() => Run("online-all-86-shop111-mobile");
    public static void Shop110() => Run("online-all-86-shop110");
    public static void Shop110Mobile() => Run("online-all-86-shop110-mobile");
    public static void Trade109() => Run("online-all-86-trade109");
    public static void Trade109Mobile() => Run("online-all-86-trade109-mobile");
    public static void Trade108() => Run("online-all-86-trade108");
    public static void Trade108Mobile() => Run("online-all-86-trade108-mobile");
    public static void Trade107() => Run("online-all-86-trade107");
    public static void Trade107Mobile() => Run("online-all-86-trade107-mobile");
    public static void Trade106() => Run("online-all-86-trade106");
    public static void Trade106Mobile() => Run("online-all-86-trade106-mobile");
    public static void Trade105() => Run("online-all-86-trade105");
    public static void Online86() => Run("online-all-86");
    public static void Online86Compact() => Run("online-all-86-compact");
    public static void System86() => Run("system-all-86");
    public static void System86Compact() => Run("system-all-86-compact");
    public static void More86() => Run("more-dialogs-all-86");
    public static void More86Compact() => Run("more-dialogs-all-86-compact");
    public static void Diagnostics86()
    {
        var log=new System.Text.StringBuilder();
        foreach(var name in new[]{"ROI/ActorSprite","ROI/GroundArt","ROI/GrassBlade"})
        {var shader=Shader.Find(name);log.AppendLine(name+" supported="+shader.isSupported);foreach(var message in ShaderUtil.GetShaderMessages(shader))log.AppendLine(message.severity+": "+message.message+" at "+message.file+":"+message.line);}
        var story=Roi.Story.DialogueScript.LoadFile(Path.Combine(Application.streamingAssetsPath,"story/prologue.dialogue"));
        foreach(var line in story.Blocks["break"])log.AppendLine("story: "+line.Cue+" | "+line.Raw+" choices="+line.Choices?.Count);
        File.WriteAllText("D:/roii/codex86-diagnostics.txt",log.ToString());
    }
    public static void All86Compact() => Run("ui-spacing-64-all-86-compact");
    public static void Album86() => Run("album-all-86");
    public static void Album86Compact() => Run("album-all-86-compact");
    public static void Cards86() => Run("cards-ux-60-all-86");
    public static void Cards86Compact() => Run("cards-ux-60-all-86-compact");
    public static void Excavation86() => Run("excavation-all-86");
    public static void Excavation86Compact() => Run("excavation-all-86-compact");
    public static void Monster86() => Run("monster-all-86");
    public static void Monster86Compact() => Run("monster-all-86-compact");
    public static void Header83() => Run("ui-header-83");
    public static void Header83Compact() => Run("ui-header-83-compact");
    public static void Wallet82Gacha() => Run("excavation-wallet-82");
    public static void Wallet82GachaCompact() => Run("excavation-wallet-82-compact");
    public static void Wallet82Current() => Run("gacha-current-58-wallet-82");
    public static void Wallet82CurrentCompact() => Run("gacha-current-58-wallet-82-compact");
    public static void Currency81() => Run("currency-icons-81");
    public static void Currency81Compact() => Run("currency-icons-81-compact");
    public static void Currency81Gacha() => Run("excavation-currency-81");
    public static void Currency81GachaCompact() => Run("excavation-currency-81-compact");
    public static void Currency81Current() => Run("gacha-current-58-currency-81");
    public static void Currency81CurrentCompact() => Run("gacha-current-58-currency-81-compact");
    public static void Currency73() => Run("currency-icons-73");
    public static void Currency73Compact() => Run("currency-icons-73-compact");
    public static void Currency73Gacha() => Run("excavation-currency-73");
    public static void Currency73GachaCompact() => Run("excavation-currency-73-compact");
    public static void Currency73Current() => Run("gacha-current-58-currency-73");
    public static void Currency73CurrentCompact() => Run("gacha-current-58-currency-73-compact");
    public static void Currency72() => Run("currency-icons-72");
    public static void Currency72Compact() => Run("currency-icons-72-compact");
    public static void Currency72Gacha() => Run("excavation-currency-72");
    public static void Currency72GachaCompact() => Run("excavation-currency-72-compact");
    public static void Currency72Current() => Run("gacha-current-58-currency-72");
    public static void Currency72CurrentCompact() => Run("gacha-current-58-currency-72-compact");
    public static void GachaCurrent() => Run("gacha-current-58");
    public static void GachaCurrentCompact() => Run("gacha-current-58-compact");
    public static void UiAuditLayout() => Run("layout-ux-56");
    public static void UiAuditLayoutCompact() => Run("layout-ux-56-compact");
    public static void UiAuditMonsterCompact() => Run("monster-ux-56-compact");
    public static void UiAuditAlbumCompact() => Run("album-ux-56-compact");
    public static void UiAuditHome() => Run("font-numbers-56");
    public static void UiAuditHomeCompact() => Run("font-numbers-56-compact");
    public static void PortraitCenter() => Run("portrait-center-57");
    public static void PortraitCenterCompact() => Run("portrait-center-57-compact");
    public static void CardsUX() => Run("cards-ux-60");
    public static void CardsUXCompact() => Run("cards-ux-60-compact");
    public static void ManagementDock() => Run("management-dock-59");
    public static void ManagementDockCompact() => Run("management-dock-59-compact");
    public static void UiReviewBefore() => Run("ui-review-52-before");
    public static void UiReview() => Run("ui-review-52");
    public static void UiReviewCompact() => Run("ui-review-52-compact");
    public static void UiStyleMotion() => Run("ui-motion-52");
    public static void UiStyleAlbum() => Run("album-style-52");
    public static void UiStyleMonster() => Run("monster-style-52");
    public static void ApexBalance() => Run("apex-roster-49");
    public static void ApexBalanceCompact() => Run("apex-roster-49-compact");
    public static void Excavation() => Run("excavation");
    public static void ExcavationCompact() => Run("excavation-compact");
    static void Run(string label)
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Stop Play mode first.");
        _label = label;
        _optionsEnabled = EditorSettings.enterPlayModeOptionsEnabled;
        _options = EditorSettings.enterPlayModeOptions;
        EditorSettings.enterPlayModeOptionsEnabled = true;
        SessionState.SetString("ROI_GALLERY_LABEL",label);
        SessionState.SetBool("ROI_GALLERY_OPTIONS_ENABLED",_optionsEnabled);
        SessionState.SetInt("ROI_GALLERY_OPTIONS",(int)_options);
        EditorSettings.enterPlayModeOptions = EnterPlayModeOptions.None;
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
        EditorApplication.EnterPlaymode();
    }
    static void Entered(PlayModeStateChange state)
    {
        if (state != PlayModeStateChange.EnteredPlayMode) return;
        _label=SessionState.GetString("ROI_GALLERY_LABEL","");
        if(string.IsNullOrEmpty(_label))return;
        SessionState.EraseString("ROI_GALLERY_LABEL");
        EditorSettings.enterPlayModeOptionsEnabled = SessionState.GetBool("ROI_GALLERY_OPTIONS_ENABLED",false);
        EditorSettings.enterPlayModeOptions = (EnterPlayModeOptions)SessionState.GetInt("ROI_GALLERY_OPTIONS",0);
        var view = EditorWindow.GetWindow(Type.GetType("UnityEditor.GameView,UnityEditor"));
        view.Show(); view.Focus();
        PlayModeWindow.SetCustomRenderingResolution(_label.Contains("compact")?1024u:1440u,_label.Contains("compact")?768u:(_label.StartsWith("excavation")?810u:900u),"UI gallery");
        if(_label.EndsWith("-mobile"))PlayModeWindow.SetCustomRenderingResolution(844,390,"Mobile landscape");
        var host = new GameObject("UI gallery fixture");
        var camera = new GameObject("Gallery background",typeof(Camera)).GetComponent<Camera>();
        camera.clearFlags = CameraClearFlags.SolidColor;
        camera.backgroundColor = UiTheme.Panel;
        camera.cullingMask = 0;
        var driver = host.AddComponent<SimDriver>();
        driver.Attach(SimSession.NewStarter(42, SimSession.LoadCatalog(), SessionBoot.StartZone, "Luna"));
        driver.Session.SetDebugAllowed(true);
        for(int i=0;i<4;i++)driver.Session.Debug("{\"grant_item\":{\"def_id\":\"shield_s1\"}}");
        driver.Paused = true;
        var runner = host.AddComponent<UiGalleryRunner>();
        Application.runInBackground=true; EditorApplication.isPaused=false;
        runner.Begin(_label);
    }
}

public sealed partial class UiGalleryRunner : MonoBehaviour
{
    string _output;
    int _shots, _errors;
    UnityEngine.InputSystem.InputSettings _originalInputSettings, _fixtureInputSettings;
    public void Begin(string label)
    {
        // Synthetic keyboard events must reach the fixture even when review tools take focus.
        // Clone the settings asset so the player's focus behavior and saved settings stay intact.
        var currentInputSettings=UnityEngine.InputSystem.InputSystem.settings;
        // InputManager destroys temporary settings on replacement. Preserve a copy in that case.
        _originalInputSettings=currentInputSettings.hideFlags==HideFlags.HideAndDontSave
            ? Instantiate(currentInputSettings) : currentInputSettings;
        if(_originalInputSettings!=currentInputSettings)_originalInputSettings.hideFlags=HideFlags.HideAndDontSave;
        _fixtureInputSettings=Instantiate(currentInputSettings);
        _fixtureInputSettings.hideFlags=HideFlags.HideAndDontSave;
        _fixtureInputSettings.backgroundBehavior=UnityEngine.InputSystem.InputSettings.BackgroundBehavior.IgnoreFocus;
        _fixtureInputSettings.editorInputBehaviorInPlayMode=UnityEngine.InputSystem.InputSettings.EditorInputBehaviorInPlayMode.AllDeviceInputAlwaysGoesToGameView;
        UnityEngine.InputSystem.InputSystem.settings=_fixtureInputSettings;
        _output = Path.GetFullPath(Path.Combine(Application.dataPath,"../../../../ui-gallery",label));
        Directory.CreateDirectory(_output);
        File.WriteAllText(Path.Combine(_output,"errors.txt"),"");
        File.WriteAllText(Path.Combine(_output,"result.txt"),"Running; not yet completed.");
        Application.logMessageReceived += Log;
        if(label.StartsWith("inventory-restored-104")){StartCoroutine(InventoryRestored104Drive());return;}
        if(label.StartsWith("inventory-103")){StartCoroutine(InventoryRestored104Drive());return;}
        if(label.StartsWith("home-101")){StartCoroutine(Home101Drive());return;}
        if(label.StartsWith("menu-88") || label.StartsWith("menu-89") || label.StartsWith("menu-90")){StartCoroutine(Menu88Drive());return;}
        if(label.StartsWith("more-dialogs-all-86")){StartCoroutine(More86Drive());return;}
        if(label.StartsWith("dialogs-all-86")){StartCoroutine(Dialogs86Drive());return;}
        if(label.StartsWith("online-all-86")){StartCoroutine(Online86Drive());return;}
        if(label.StartsWith("system-all-86")){StartCoroutine(System86Drive());return;}
        StartCoroutine(label.StartsWith("currency-icons-")?CurrencyIconsDrive():label.StartsWith("cards-ux-60")?CardsUXDrive():label.StartsWith("management-dock-59")?ManagementDockDrive():label.StartsWith("gacha-current-58")?GachaCurrentDrive():label.StartsWith("portrait-center-57")?PortraitCenterDrive():label.StartsWith("layout-ux-56")?LayoutAuditDrive():(label.StartsWith("apex-roster")||label.StartsWith("apex-dock"))?ApexRosterDrive():label.StartsWith("ui-motion-")?StyleMotionDrive():label=="motion"?MotionDrive():label.StartsWith("font-thai-")?FontThaiDrive():label.StartsWith("font-numbers-")?FontNumbersDrive():label.StartsWith("album")?AlbumDrive():(label.StartsWith("icons-final")||label.StartsWith("celestial-icons-"))?IconsFinalDrive():label.StartsWith("typography")?TypographyDrive():label.StartsWith("monster")?MonsterDrive():label.StartsWith("excavation")?ExcavationDrive():Drive());
    }
    void Log(string message,string stack,LogType type)
    { if(type==LogType.Error||type==LogType.Exception||type==LogType.Assert) { _errors++; File.AppendAllText(Path.Combine(_output,"errors.txt"),message+"\n"+stack+"\n"); } }
    void OnDestroy()
    {
        Application.logMessageReceived -= Log;
        if(_originalInputSettings!=null)UnityEngine.InputSystem.InputSystem.settings=_originalInputSettings;
        // Restoring through InputSystem.settings also disposes its temporary fixture settings.
        _fixtureInputSettings=null;
    }
    IEnumerator Shot(string name)
    {
        if(name=="Inventory"){
            var inventory=UnityEngine.Object.FindFirstObjectByType<ItemGridWindow>();
            if(inventory!=null)typeof(ItemGridWindow).GetMethod("Select",BindingFlags.NonPublic|BindingFlags.Instance).Invoke(inventory,new object[]{0});
        }
        yield return new WaitForSecondsRealtime(.4f);
        yield return null; yield return null;
        Canvas.ForceUpdateCanvases();
        yield return new WaitForEndOfFrame();
        if(_output.Contains("56")||_output.Contains("58")||_output.Contains("59")||_output.Contains("60")||_output.Contains("61")||_output.Contains("62")||_output.Contains("63")||_output.Contains("64")||_output.Contains("65")||_output.Contains("67")||_output.Contains("68")||_output.Contains("69")||_output.Contains("70")||_output.Contains("71")||_output.Contains("72")||_output.Contains("73")||_output.Contains("81")||_output.Contains("82")||_output.Contains("83")||_output.Contains("84")||_output.Contains("85")||_output.Contains("86"))DumpTypography(name);
        ScreenCapture.CaptureScreenshot(Path.Combine(_output,name+".png"));
        yield return null; yield return null;
        _shots++;
    }
    void DumpTypography(string shot)
    {
        var lines=new System.Text.StringBuilder("path\tfont\tpx\twidth\theight\toverflow\tcolor\ttext\n");
        foreach(var t in UnityEngine.Object.FindObjectsByType<TMPro.TextMeshProUGUI>(FindObjectsSortMode.None))
        {
            if(!t.isActiveAndEnabled||t.canvasRenderer.cull||string.IsNullOrWhiteSpace(t.text))continue;
            string path=t.name;for(var p=t.transform.parent;p!=null;p=p.parent)path=p.name+"/"+path;
            var rt=t.rectTransform;var corners=new Vector3[4];rt.GetWorldCorners(corners);
            if(corners[2].x<0||corners[0].x>Screen.width||corners[2].y<0||corners[0].y>Screen.height)continue;
            float scale=rt.rect.height>0?Vector3.Distance(corners[0],corners[1])/rt.rect.height:1;
            float size=t.fontSize;foreach(var c in t.textInfo.characterInfo)if(c.isVisible){size=c.pointSize;break;}
            lines.Append(path).Append('\t').Append(t.font?.name).Append('\t').Append((size*scale).ToString("0.0",System.Globalization.CultureInfo.InvariantCulture))
                .Append('\t').Append(rt.rect.width.ToString("0.0",System.Globalization.CultureInfo.InvariantCulture)).Append('\t').Append(rt.rect.height.ToString("0.0",System.Globalization.CultureInfo.InvariantCulture))
                .Append('\t').Append(t.isTextOverflowing).Append('\t').Append(ColorUtility.ToHtmlStringRGBA(t.color)).Append('\t').Append(t.text.Replace('\t',' ').Replace('\n',' ')).Append('\n');
        }
        File.WriteAllText(Path.Combine(_output,shot.Replace('/','-')+"-type.tsv"),lines.ToString());
    }
    IEnumerator ApexRosterDrive()
    {
        var driver=GetComponent<SimDriver>();var sim=driver.Session;
        var bar=HudBar.Create(transform);yield return null;bar.gameObject.SetActive(false);
        var go=new GameObject("Apex dock review",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler),typeof(GraphicRaycaster));
        go.transform.SetParent(transform,false);go.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;
        go.GetComponent<Canvas>().sortingOrder=100;
        var scaler=go.GetComponent<CanvasScaler>();scaler.uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;scaler.referenceResolution=new Vector2(1440,900);
        var screen=ExemplarScreen.Create(go.transform,()=>{});screen.Refresh();screen.SelectPage("Apex");
        yield return Shot("Apex-1");
        var allocations=screen.transform.Find("ManagementPage/Widgets/Apex/ApexPage/Summary/Allocations").GetComponent<ScrollRect>();
        allocations.verticalNormalizedPosition=0;yield return Shot("Apex-1-lower");
        if(allocations.content.anchoredPosition.y<=0)Debug.LogError("Apex allocation list cannot reach lower attributes.");
        allocations.verticalNormalizedPosition=1;
        var scroll=screen.transform.Find("Roster").GetComponent<ScrollRect>();var viewport=scroll.viewport;var content=scroll.content;
        if(!DockGeometryValid(viewport,1))Debug.LogError("One-character dock geometry failed.");
        foreach(var unit in new[]{"lapis","lucielle","aveline","raplus","lumi","charon","zolfia","nemu","airi"})
        {
            if(!sim.ApplyInput("{\"DebugGrantExemplarUnit\":{\"exemplar_id\":\""+unit+"\"}}"))Debug.LogError("Cannot grant native fixture character: "+unit);
            screen.Refresh();yield return null;
            int count=ExemplarReadModel.Fetch(driver).Members.Count;
            if(count==2||count==3||count==6){yield return Shot("Apex-"+count);if(!DockGeometryValid(viewport,count))Debug.LogError("Dock clips or overlaps its visible characters: "+count);}
        }
        var owned=ExemplarReadModel.Fetch(driver);
        File.WriteAllText(Path.Combine(_output,"roster.txt"),string.Join("\n",owned.Members.ConvertAll(m=>m.Id+" "+m.UnitDef)));
        if(owned.Members.Count!=10)Debug.LogError("Overflow fixture must contain ten native characters.");
        yield return Shot("Apex-10-first");
        var previous=screen.transform.Find("RosterPrevious").GetComponent<Button>();var next=screen.transform.Find("RosterNext").GetComponent<Button>();
        var first=(RectTransform)content.GetChild(0);var savedPosition=first.anchoredPosition;
        first.anchoredPosition+=Vector2.up*2000;
        if(DockGeometryValid(viewport,6))Debug.LogError("Geometry guard accepted deliberately displaced portrait.");
        first.anchoredPosition=savedPosition;
        // A disconnected callback must fail the same movement predicate as the live button.
        var savedNext=next.onClick;float before=content.anchoredPosition.x;
        next.onClick=new Button.ButtonClickedEvent();next.onClick.Invoke();yield return null;
        if(DockMoved(before,content))Debug.LogError("Movement guard accepted a disconnected button.");
        next.onClick=savedNext;
        next.onClick.Invoke();yield return null;
        if(!DockMoved(before,content))Debug.LogError("Dock next button did not scroll.");
        for(int step=0;step<10&&next.interactable;step++){next.onClick.Invoke();yield return null;}
        var last=owned.Members[owned.Members.Count-1];var lastRow=(RectTransform)content.Find("Character_"+last.Id);
        if(!DockFullyVisible(viewport,lastRow))Debug.LogError("Last character cannot be reached by the next button.");
        yield return Shot("Apex-10-end");
        var select=lastRow.GetComponent<Button>();var savedSelect=select.onClick;
        select.onClick=new Button.ButtonClickedEvent();select.onClick.Invoke();
        if(screen.InspectedId==last.Id)Debug.LogError("Selection guard accepted a disconnected callback.");
        select.onClick=savedSelect;select.onClick.Invoke();
        if(screen.InspectedId!=last.Id||screen.CurrentPage!="Apex")Debug.LogError("Dock selection lost its member or Apex context.");
        yield return Shot("Apex-selected-reserve");
        var blank=screen.transform.Find("CharacterPresentation/ExemplarPortrait/PortraitUnavailable");
        if(ExemplarPortrait.Illustration(last.UnitDef)==null&&ExemplarPortrait.Avatar(last.UnitDef)==null&&
            (!blank.gameObject.activeInHierarchy||blank.Find("Name").GetComponent<TMPro.TextMeshProUGUI>().text!=GameText.UnitName(last.UnitDef)))
            Debug.LogError("Missing artwork did not preserve selected character identity.");
        before=content.anchoredPosition.x;
        UnityEngine.EventSystems.ExecuteEvents.ExecuteHierarchy(viewport.gameObject,new UnityEngine.EventSystems.PointerEventData(UnityEngine.EventSystems.EventSystem.current){scrollDelta=Vector2.up},UnityEngine.EventSystems.ExecuteEvents.scrollHandler);yield return null;
        if(!DockMoved(before,content))Debug.LogError("Mouse wheel failed to scroll the horizontal dock.");
        while(previous.interactable){previous.onClick.Invoke();yield return null;}
        before=content.anchoredPosition.x;var corners=new Vector3[4];viewport.GetWorldCorners(corners);
        var drag=new UnityEngine.EventSystems.PointerEventData(UnityEngine.EventSystems.EventSystem.current){position=(corners[0]+corners[2])*.5f,button=UnityEngine.EventSystems.PointerEventData.InputButton.Left};
        scroll.OnInitializePotentialDrag(drag);scroll.OnBeginDrag(drag);drag.position+=Vector2.left*220;scroll.OnDrag(drag);scroll.OnEndDrag(drag);yield return null;
        if(!DockMoved(before,content))Debug.LogError("Horizontal pointer drag failed to scroll the dock.");
        yield return Shot("Apex-drag");
        screen.Inspect(owned.Members[0].Id);screen.SelectPage("Attributes");yield return Shot("Attributes-restored");
        if(!scroll.horizontal||scroll.vertical)Debug.LogError("Attributes did not retain its horizontal roster.");
        screen.SelectPage("Apex");yield return Shot("Apex-restored");
        if(!DockFullyVisible(viewport,(RectTransform)content.Find("Character_"+owned.Members[0].Id)))Debug.LogError("Returning to Apex hid the selected character.");
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} screens; runtime errors: {_errors}. Native 1/2/3/6/10-character fixture; next/previous, wheel, horizontal drag, selection and page restoration verified. Geometry, movement and selection sabotage rejected.");
    }
    static bool DockMoved(float before,RectTransform content)=>Mathf.Abs(before-content.anchoredPosition.x)>1;
    static Rect DockBounds(RectTransform viewport,RectTransform row)
    {
        var corners=new Vector3[4];row.GetWorldCorners(corners);
        var a=viewport.InverseTransformPoint(corners[0]);var b=viewport.InverseTransformPoint(corners[2]);
        return Rect.MinMaxRect(a.x,a.y,b.x,b.y);
    }
    static bool DockFullyVisible(RectTransform viewport,RectTransform row)
    {
        var b=DockBounds(viewport,row);var v=viewport.rect;
        return b.xMin>=v.xMin-1&&b.xMax<=v.xMax+1&&b.yMin>=v.yMin-1&&b.yMax<=v.yMax+1;
    }
    static bool DockGeometryValid(RectTransform viewport,int expected)
    {
        var seen=new System.Collections.Generic.List<Rect>();int full=0;
        foreach(Transform row in viewport.Find("Content"))
        {
            if(!row.gameObject.activeSelf)continue;
            var b=DockBounds(viewport,(RectTransform)row);
            if(b.yMin<viewport.rect.yMin-1||b.yMax>viewport.rect.yMax+1||b.width<100||b.height<60)return false;
            foreach(var other in seen)if(b.Overlaps(other))return false;
            seen.Add(b);if(DockFullyVisible(viewport,(RectTransform)row))full++;
        }
        return full==expected;
    }
    IEnumerator ManagementDockDrive()
    {
        var driver=GetComponent<SimDriver>();var sim=driver.Session;
        var bar=HudBar.Create(transform);yield return null;bar.gameObject.SetActive(false);bar.Floating.gameObject.SetActive(false);
        var canvas=new GameObject("Management dock review",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler),typeof(GraphicRaycaster));
        canvas.transform.SetParent(transform,false);canvas.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;
        var scaler=canvas.GetComponent<CanvasScaler>();scaler.uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;scaler.referenceResolution=new Vector2(1440,900);
        var screen=ExemplarScreen.Create(canvas.transform,()=>{});screen.Refresh();
        var scroll=screen.transform.Find("Roster").GetComponent<ScrollRect>();var viewport=scroll.viewport;var content=scroll.content;
        var management=(RectTransform)screen.transform.Find("ManagementPage");
        bool ClearsDock()=>DockBounds(viewport,screen.CurrentPage=="Attributes"?(RectTransform)screen.transform.Find("StatsShade"):management).yMin>viewport.rect.yMax+2;
        if(_output.Contains("85"))yield return Shot("Attributes-first-open");
        foreach(int count in new[]{1,6,10})
        {
            string[] units={"lapis","lucielle","aveline","raplus","lumi","charon","zolfia","nemu","airi"};
            int owned=ExemplarReadModel.Fetch(driver).Members.Count;
            for(int i=owned-1;i<count-1;i++)
                if(!sim.ApplyInput("{\"DebugGrantExemplarUnit\":{\"exemplar_id\":\""+units[i]+"\"}}"))Debug.LogError("DOCK: fixture character grant failed");
            screen.Refresh();yield return null;
            foreach(string page in new[]{"Attributes","Equipment","Regalia","Resonance"})
            {
                screen.SelectPage(page);yield return Shot(page+"-"+count);
                if(!scroll.horizontal||scroll.vertical||!DockGeometryValid(viewport,Mathf.Min(count,6)))Debug.LogError("DOCK: missing horizontal roster or clipped cards on "+page);
                if(!ClearsDock())Debug.LogError("DOCK: management panel overlaps character selection on "+page);
                if(screen.transform.Find("RosterShade").gameObject.activeSelf)Debug.LogError("DOCK: obsolete right-hand roster remains visible");
            }
        }
        var position=management.anchoredPosition;management.anchoredPosition+=Vector2.down*1500;
        if(ClearsDock())Debug.LogError("DOCK: overlap guard accepted displaced management controls (sabotage)");
        management.anchoredPosition=position;
        var next=screen.transform.Find("RosterNext").GetComponent<Button>();var previous=screen.transform.Find("RosterPrevious").GetComponent<Button>();
        float before=content.anchoredPosition.x;var nextCallback=next.onClick;next.onClick=new Button.ButtonClickedEvent();next.onClick.Invoke();yield return null;
        if(DockMoved(before,content))Debug.LogError("DOCK: movement check accepted disconnected next button (sabotage)");
        next.onClick=nextCallback;next.onClick.Invoke();yield return null;
        if(!DockMoved(before,content))Debug.LogError("DOCK: next arrow did not move character selection");
        for(int i=0;i<10&&next.interactable;i++){next.onClick.Invoke();yield return null;}
        var members=ExemplarReadModel.Fetch(driver).Members;var last=members[members.Count-1];
        var lastRow=(RectTransform)content.Find("Character_"+last.Id);
        if(!DockFullyVisible(viewport,lastRow))Debug.LogError("DOCK: final character is unreachable");
        yield return Shot("Resonance-10-end");
        before=content.anchoredPosition.x;
        screen.SelectPage("Equipment");yield return null;
        if(Mathf.Abs(content.anchoredPosition.x-before)>1)Debug.LogError("DOCK: changing management pages lost roster scroll position");
        screen.SelectPage("Regalia");yield return null;
        if(Mathf.Abs(content.anchoredPosition.x-before)>1)Debug.LogError("DOCK: Regalia lost roster scroll position");
        screen.SelectPage("Resonance");lastRow.GetComponent<Button>().onClick.Invoke();yield return null;
        if(screen.InspectedId!=last.Id||screen.CurrentPage!="Resonance")Debug.LogError("DOCK: character selection lost its identity or page");
        yield return Shot("Resonance-selected");
        if(_output.Contains("85")){screen.SelectPage("Attributes");yield return Shot("Attributes-selected");screen.SelectPage("Resonance");}
        before=content.anchoredPosition.x;
        UnityEngine.EventSystems.ExecuteEvents.ExecuteHierarchy(viewport.gameObject,new UnityEngine.EventSystems.PointerEventData(UnityEngine.EventSystems.EventSystem.current){scrollDelta=Vector2.up},UnityEngine.EventSystems.ExecuteEvents.scrollHandler);yield return null;
        if(!DockMoved(before,content))Debug.LogError("DOCK: mouse wheel did not move the roster");
        for(int i=0;i<10&&previous.interactable;i++){previous.onClick.Invoke();yield return null;}
        before=content.anchoredPosition.x;var corners=new Vector3[4];viewport.GetWorldCorners(corners);
        var drag=new UnityEngine.EventSystems.PointerEventData(UnityEngine.EventSystems.EventSystem.current){position=(corners[0]+corners[2])*.5f,button=UnityEngine.EventSystems.PointerEventData.InputButton.Left};
        scroll.OnInitializePotentialDrag(drag);scroll.OnBeginDrag(drag);drag.position+=Vector2.left*220;scroll.OnDrag(drag);scroll.OnEndDrag(drag);yield return null;
        if(!DockMoved(before,content))Debug.LogError("DOCK: pointer drag did not move the roster");
        screen.Inspect(members[0].Id);screen.SelectPage("Attributes");yield return Shot("Attributes-restored");
        if(!scroll.horizontal||scroll.vertical)Debug.LogError("DOCK: Attributes roster was not restored");
        screen.SelectPage("Equipment");yield return Shot("Equipment-restored");
        if(!DockFullyVisible(viewport,(RectTransform)content.Find("Character_"+members[0].Id)))Debug.LogError("DOCK: returning to Equipment hid the selected character");
        screen.SelectPage("Apex");yield return Shot("Apex-restored");
        if(!scroll.horizontal||scroll.vertical)Debug.LogError("DOCK: Apex no longer uses its portrait dock");
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} native screens; errors: {_errors}. 1/6/10-character fixtures; next/previous, wheel, drag, selection, retained position and page return. Overlap and movement sabotage rejected. No saved progress changed.");
    }
    IEnumerator PortraitCenterDrive()
    {
        var bar=HudBar.Create(transform);yield return null;bar.gameObject.SetActive(false);bar.Floating.gameObject.SetActive(false);
        var canvas=new GameObject("Portrait review",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler),typeof(GraphicRaycaster));
        canvas.transform.SetParent(transform,false);canvas.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;
        var scaler=canvas.GetComponent<CanvasScaler>();scaler.uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;scaler.referenceResolution=new Vector2(1440,900);
        var screen=ExemplarScreen.Create(canvas.transform,()=>{});screen.Refresh();
        foreach(var page in new[]{"Equipment","Regalia","Resonance","Attributes","Apex","Equipment"})
        {
            screen.SelectPage(page);yield return Shot("Exemplar-"+page);
        }
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} portrait screens; errors: {_errors}. Three centred pages plus Attributes/Apex and return-state captures. Presentation fixture only.");
    }
    IEnumerator LayoutAuditDrive()
    {
        var bar=HudBar.Create(transform);yield return null;
        bar.Toggle("Paths");yield return Shot("Paths");
        var paths=UnityEngine.Object.FindFirstObjectByType<PathsWindow>();
        var scroll=paths.GetComponentInChildren<ScrollRect>();
        var start=scroll.content.anchoredPosition;
        if(!PathsOriginCentred(scroll))Debug.LogError("Paths does not open at the origin.");
        scroll.content.anchoredPosition=Vector2.zero;
        if(PathsOriginCentred(scroll))Debug.LogError("Paths centre guard accepted displaced content.");
        scroll.content.anchoredPosition=start;
        if(!PathsOriginCentred(scroll))Debug.LogError("Paths origin did not recover after sabotage.");
        if(scroll.horizontalScrollbar==null)Debug.LogError("Paths has no horizontal scrollbar.");
        else
        {
            scroll.horizontalScrollbar.value=1;yield return null;
            if(Mathf.Abs(scroll.content.anchoredPosition.x-start.x)<20)Debug.LogError("Paths scrollbar did not move the graph.");
            yield return Shot("Paths-right");
        }
        var position=scroll.content.anchoredPosition;
        bar.CloseAll();bar.Toggle("Paths");yield return null;yield return null;
        if(Vector2.Distance(position,scroll.content.anchoredPosition)>2)Debug.LogError("Paths lost position on return.");
        bar.CloseAll();bar.Toggle("Skills");yield return Shot("Skills");
        VerifyConversionLabels();
        bar.CloseAll();bar.gameObject.SetActive(false);bar.Floating.gameObject.SetActive(false);
        var canvas=new GameObject("Layout review",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler),typeof(GraphicRaycaster));
        canvas.transform.SetParent(transform,false);canvas.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;
        var scaler=canvas.GetComponent<CanvasScaler>();scaler.uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;scaler.referenceResolution=new Vector2(1440,900);
        var exemplars=ExemplarScreen.Create(canvas.transform,()=>{});exemplars.Refresh();exemplars.SelectPage("Skills");
        yield return Shot("Exemplar-Skills");VerifyConversionLabels();
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} layout screens; errors: {_errors}. Paths origin, scrollbar, return position and complete conversion labels checked; geometry sabotage rejected.");
    }
    static bool PathsOriginCentred(ScrollRect scroll)
    {
        var origin=scroll.content.Find("N_origin") as RectTransform;if(origin==null)return false;
        var bounds=RectTransformUtility.CalculateRelativeRectTransformBounds(scroll.viewport,origin);
        return Vector2.Distance(bounds.center,scroll.viewport.rect.center)<5;
    }
    static bool ConversionLabelFits(TMPro.TextMeshProUGUI label)
    {
        label.ForceMeshUpdate();return !label.isTextTruncated&&!label.isTextOverflowing;
    }
    static void VerifyConversionLabels()
    {
        var panel=UnityEngine.Object.FindFirstObjectByType<PotentialWindow>();
        foreach(var name in new[]{"MergeLow","MergeHigh","SplitLow","SplitHigh"})
        {
            var button=panel.transform.Find("Actions/"+name);
            var label=button.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if(!ConversionLabelFits(label))Debug.LogError("Conversion label is cut off: "+name);
            var rt=label.rectTransform;var old=rt.sizeDelta;
            rt.sizeDelta=new Vector2(old.x-rt.rect.width+5,old.y);
            if(ConversionLabelFits(label))Debug.LogError("Conversion label guard accepted a narrowed label.");
            rt.sizeDelta=old;label.ForceMeshUpdate();
        }
    }
    IEnumerator StyleMotionDrive()
    {
        var bar=HudBar.Create(transform);yield return null;
        bool had=PlayerPrefs.HasKey(UiEntrance.ReducedMotionKey);int saved=PlayerPrefs.GetInt(UiEntrance.ReducedMotionKey,0);
        try
        {
            PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,0);
            foreach(var id in new[]{"Album","Options","Shop","Bestiary","Party","Record","Story"})
            {
                bar.CloseAll();bar.Toggle(id);
                UiWindow page=null;foreach(var w in UnityEngine.Object.FindObjectsByType<UiWindow>(FindObjectsSortMode.None))if(w.IsOpen){page=w;break;}
                if(page==null){Debug.LogError("Motion page not opened: "+id);continue;}
                var body=page.Body;var pos=body.anchoredPosition;var scale=body.localScale;var group=body.GetComponent<CanvasGroup>();
                if(group==null||group.alpha>=1)Debug.LogError("Page has no opening dissolve: "+id);
                for(int i=0;i<6;i++)
                {
                    yield return new WaitForEndOfFrame();ScreenCapture.CaptureScreenshot(Path.Combine(_output,id+"-"+i.ToString("D2")+".png"));_shots++;
                    yield return new WaitForSecondsRealtime(.045f);
                }
                if(group.alpha!=1||body.anchoredPosition!=pos||body.localScale!=scale)Debug.LogError("Motion moved layout or did not settle: "+id);
                page.SetOpen(false);PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,1);page.SetOpen(true);
                if(group.alpha!=1||!body.GetComponent<UiPageTransition>().Complete)Debug.LogError("Reduced motion still animates: "+id);
                PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,0);
            }
            bar.CloseAll();bar.Toggle("Shop");yield return new WaitForSecondsRealtime(.3f);
            var shop=UnityEngine.Object.FindFirstObjectByType<ShopWindow>();
            typeof(ShopWindow).GetMethod("Select",BindingFlags.NonPublic|BindingFlags.Instance).Invoke(shop,new object[]{1});
            var tab=shop.GetComponentInChildren<ScrollRect>().viewport;
            if(tab.GetComponent<CanvasGroup>()?.alpha>=1||tab.GetComponent<UiPageTransition>()==null)Debug.LogError("Shop category has no detail dissolve.");
            yield return new WaitForSecondsRealtime(.3f);
            var button=ExemplarUi.Button(shop.transform,"UI state fixture","",.1f,.1f,.3f,.2f,()=>{},true);
            var caption=button.GetComponentInChildren<TMPro.TextMeshProUGUI>();caption.color=new Color(.95f,.45f,.40f);
            var semantic=caption.color;yield return null;
            UnityEngine.EventSystems.EventSystem.current.SetSelectedGameObject(button.gameObject);
            button.interactable=false;yield return new WaitForSecondsRealtime(.16f);
            if(!CaptionDimmed(caption)||caption.color!=semantic)Debug.LogError("Disabled caption lost its semantic colour or did not dim.");
            // The same predicate must reject a deliberately restored bright caption.
            caption.canvasRenderer.SetColor(Color.white);
            if(CaptionDimmed(caption))Debug.LogError("Caption contrast guard accepted sabotage.");
            caption.canvasRenderer.SetColor(new Color(.3f,.3f,.3f,1));
            if(CaptionDimmed(caption))Debug.LogError("Caption contrast guard accepted an unreadably dark caption.");
            button.interactable=true;button.interactable=false;yield return new WaitForSecondsRealtime(.16f);
            if(!CaptionDimmed(caption))Debug.LogError("Caption did not recover after sabotage.");
            Destroy(button.gameObject);
            // Parent reopens must not replay an old nested fade and multiply its alpha.
            var parent=new GameObject("Nested motion fixture",typeof(RectTransform));parent.transform.SetParent(shop.transform,false);
            var child=new GameObject("Child",typeof(RectTransform));child.transform.SetParent(parent.transform,false);
            UiPageTransition.Play((RectTransform)child.transform);yield return new WaitForSecondsRealtime(.25f);
            parent.SetActive(false);parent.SetActive(true);
            if(!MotionSettled(child))Debug.LogError("Nested page replayed its transition on parent reopen.");
            UiPageTransition.Play((RectTransform)child.transform);
            if(MotionSettled(child))Debug.LogError("Nested motion guard accepted an intentionally running fade.");
            yield return new WaitForSecondsRealtime(.25f);Destroy(parent);
            bar.CloseAll();bar.Floating.Open();bar.Floating.Side(3);
            var training=bar.Floating.GetComponentInChildren<UiPageTransition>();
            if(training==null)Debug.LogError("Inventory transition is missing.");
            yield return Shot("Inventory-training");bar.Floating.Close();
        }
        finally{if(had)PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,saved);else PlayerPrefs.DeleteKey(UiEntrance.ReducedMotionKey);}
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} motion frames; runtime errors: {_errors}. Seven pages, reduced motion, unchanged hit regions, tab transitions, disabled-caption semantics and nested fade guards checked; sabotage rejected.");
    }
    static bool CaptionDimmed(TMPro.TextMeshProUGUI label)
    {
        var tint=label.canvasRenderer.GetColor();
        return tint.r>=.7f&&tint.g>=.7f&&tint.b>=.7f&&tint.r<.95f&&tint.g<.95f&&tint.b<.95f;
    }
    static bool MotionSettled(GameObject go)=>go.GetComponent<CanvasGroup>().alpha==1&&go.GetComponent<UiPageTransition>().Complete;
    IEnumerator MotionDrive()
    {
        var bar=HudBar.Create(transform);yield return null;
        bool had=PlayerPrefs.HasKey(UiEntrance.ReducedMotionKey);int saved=PlayerPrefs.GetInt(UiEntrance.ReducedMotionKey,0);
        try {
            PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,0);
            foreach(var id in new[]{"Album","Options","Shop"}){
                bar.CloseAll();bar.Toggle(id);
                UiWindow page=null;foreach(var w in UnityEngine.Object.FindObjectsByType<UiWindow>(FindObjectsSortMode.None))if(w.IsOpen){page=w;break;}
                if(page==null){Debug.LogError("Motion: missing page "+id);continue;}
                var position=page.Body.anchoredPosition;var scale=page.Body.localScale;
                for(int i=0;i<10;i++){
                    yield return new WaitForEndOfFrame();
                    ScreenCapture.CaptureScreenshot(Path.Combine(_output,id+"-motion-"+i.ToString("00")+".png"));
                    yield return new WaitForSecondsRealtime(.035f);
                }
                if(page.Body.anchoredPosition!=position||page.Body.localScale!=scale)Debug.LogError("Motion moved page layout: "+id);
                if(page.Body.GetComponent<CanvasGroup>().alpha!=1)Debug.LogError("Motion did not finish: "+id);
                page.SetOpen(false);PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,1);page.SetOpen(true);
                if(!page.Body.GetComponent<UiPageTransition>().Complete||page.Body.GetComponent<CanvasGroup>().alpha!=1)Debug.LogError("Reduced motion failed: "+id);
                PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,0);
            }
        } finally {if(had)PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,saved);else PlayerPrefs.DeleteKey(UiEntrance.ReducedMotionKey);}
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured 30 motion frames; errors: {_errors}. Layout stability and reduced motion checked.");
    }

    [Serializable] class CardSlotsFixture { public CardSlotFixture[] rows; }
    [Serializable] class CardSlotFixture { public string @class;public int sockets,cap;public PlacedCardFixture[] cards; }
    [Serializable] class PlacedCardFixture { public int panel_idx;public ItemView item; }
    static CardSlotFixture CardPanel(SimSession sim,string cls) => Array.Find(JsonUtility.FromJson<CardSlotsFixture>("{\"rows\":"+sim.QueryJson("card_slots")+"}").rows,r=>r.@class==cls);
    static ItemView[] CardAlbum(SimSession sim)=>JsonUtility.FromJson<ItemList>("{\"items\":"+sim.QueryJson("card_album")+"}").items;
    static Button CardsButton(CardsWindow screen,string name)=>Array.Find(screen.GetComponentsInChildren<Button>(),b=>b.name==name&&b.gameObject.activeInHierarchy);
    static void CardsPress(CardsWindow screen,string name){var b=CardsButton(screen,name);if(b==null||!b.interactable)Debug.LogError("Cards control unavailable: "+name);else b.onClick.Invoke();}
    static void CardsCall(CardsWindow screen,string method,params object[] args)=>typeof(CardsWindow).GetMethod(method,BindingFlags.Instance|BindingFlags.NonPublic).Invoke(screen,args);
    IEnumerator CardsUXDrive()
    {
        var sim=GetComponent<SimDriver>().Session;var bar=HudBar.Create(transform);yield return null;
        bar.Toggle("Cards");yield return Shot("Cards-empty");
        string[] ids={"card_greenling","card_pinkling","card_yellowling","card_chomb","card_toxic_chom","card_burst_chom","card_rechum","card_zombie","card_twinling","card_gloomtooth","card_banshee","card_dustelle","card_ashelle","card_marionette","card_scarlette","card_crimsonne","card_brutus","card_gnarloak","card_broomhilda"};
        foreach(string id in ids){
            for(int n=0;n<(id=="card_chomb"?6:2);n++)sim.Debug("{\"grant_item\":{\"def_id\":\""+id+"\"}}");
            var inv=JsonUtility.FromJson<ItemList>("{\"items\":"+sim.QueryJson("inventory")+"}").items;
            int idx=Array.FindIndex(inv,c=>c.def_id==id);if(idx<0){Debug.LogError("Card fixture missing: "+id);continue;}
            sim.ApplyInput("{\"DepositCardToAlbum\":{\"inv_idx\":"+idx+"}}");
        }
        var screen=UnityEngine.Object.FindFirstObjectByType<CardsWindow>();screen.Refresh();
        CardsCall(screen,"SelectCard",Array.FindIndex(CardAlbum(sim),c=>c.def_id=="card_greenling"));
        yield return Shot("Cards-collection");
        File.WriteAllText(Path.Combine(_output,"album.json"),sim.QueryJson("card_album"));
        File.WriteAllText(Path.Combine(_output,"slots.json"),sim.QueryJson("card_slots"));
        var idle=screen.GetComponentInChildren<MonsterPortraitIdle>();
        if(idle==null)Debug.LogError("Cards selected portrait missing.");
        else{int changes=idle.FrameChanges;yield return new WaitForSecondsRealtime(.7f);if(UiEntrance.MotionAllowed&&PlayerPrefs.GetInt("roi.opt.postfx",1)!=0&&idle.FrameChanges<=changes)Debug.LogError("Cards portrait idle stalled.");}
        bool hadMotion=PlayerPrefs.HasKey(UiEntrance.ReducedMotionKey);int oldMotion=PlayerPrefs.GetInt(UiEntrance.ReducedMotionKey);
        PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,1);int stopped=idle!=null?idle.FrameChanges:0;yield return new WaitForSecondsRealtime(.6f);
        if(idle!=null&&idle.FrameChanges!=stopped)Debug.LogError("Cards reduced motion changed frames.");
        if(hadMotion)PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,oldMotion);else PlayerPrefs.DeleteKey(UiEntrance.ReducedMotionKey);
        CardsPress(screen,"BonusTab");yield return Shot("Cards-bind-bonus");CardsPress(screen,"EffectsTab");
        var search=screen.GetComponentInChildren<TMPro.TMP_InputField>();search.text="green";yield return Shot("Cards-search");
        if(CardsButton(screen,"Card_card_greenling")==null||CardsButton(screen,"Card_card_pinkling")!=null)Debug.LogError("Cards keyword filter returned incorrect entries.");
        search.text="no-such-card";yield return Shot("Cards-no-results");
        if(CardsButton(screen,"EquipAction").interactable)Debug.LogError("Cards empty search enabled equip.");
        CardsPress(screen,"EmptyAction");yield return null;
        CardsPress(screen,"ClassFilter");yield return Shot("Cards-filter-picker");UiPicker.Close();
        CardsCall(screen,"SetFilter","Ring");yield return null;
        string untouchedAlbum=sim.QueryJson("card_album"),untouchedSlots=sim.QueryJson("card_slots");
        CardsPress(screen,"Card_card_banshee");yield return Shot("Cards-accessory");
        if(untouchedAlbum!=sim.QueryJson("card_album")||untouchedSlots!=sim.QueryJson("card_slots"))Debug.LogError("Cards selection changed equipment.");
        // Exercise a nonzero ORIGINAL album index after filtering, not its visible-grid index.
        var target=typeof(CardsWindow).GetField("_target",BindingFlags.Instance|BindingFlags.NonPublic);
        target.SetValue(screen,"Ring");CardsCall(screen,"BuildDetail");
        int before=CardPanel(sim,"Ring").cards.Length;
        var equip=CardsButton(screen,"EquipAction");var real=equip.onClick;equip.onClick=new Button.ButtonClickedEvent();equip.onClick.Invoke();
        bool Changed()=>CardPanel(sim,"Ring").cards.Length==before+1;
        if(Changed())Debug.LogError("Cards mutation guard accepted disconnected dispatch.");equip.onClick=real;
        CardsPress(screen,"EquipAction");yield return Shot("Cards-equipped-inactive");
        var ring=CardPanel(sim,"Ring");
        if(!Changed()||ring.cards[ring.cards.Length-1].item.def_id!="card_banshee")Debug.LogError("Cards filtered original index equipped the wrong card.");
        CardsPress(screen,"EquipAction");yield return Shot("Cards-removed");
        if(CardPanel(sim,"Ring").cards.Length!=before||sim.QueryJson("card_album")!=untouchedAlbum)Debug.LogError("Cards equip/remove did not restore the album.");
        CardsCall(screen,"SetFilter","Weapon");yield return Shot("Cards-weapon");
        int cap=CardPanel(sim,"Weapon").cap;
        for(int i=0;i<cap;i++){
            CardsCall(screen,"SelectCard",Array.FindIndex(CardAlbum(sim),c=>c.def_id=="card_chomb"));CardsPress(screen,"EquipAction");yield return null;
        }
        CardsCall(screen,"SelectCard",Array.FindIndex(CardAlbum(sim),c=>c.def_id=="card_chomb"));yield return Shot("Cards-full");
        if(CardPanel(sim,"Weapon").cards.Length!=cap||CardsButton(screen,"EquipAction").interactable)Debug.LogError("Cards full panel remains actionable.");
        string full=sim.QueryJson("card_album");CardsButton(screen,"EquipAction").onClick.Invoke();
        if(full!=sim.QueryJson("card_album"))Debug.LogError("Cards full panel dispatched another equip.");
        CardsPress(screen,"Socket_1");CardsPress(screen,"EquipAction");yield return Shot("Cards-slot-removed");
        if(CardPanel(sim,"Weapon").cards.Length!=cap-1)Debug.LogError("Cards socket selection did not remove the intended slot.");
        string lang=Loc.Language;Loc.Language="th";screen.Refresh();yield return Shot("Cards-thai");Loc.Language=lang;screen.Refresh();
        CardsCall(screen,"SetFilter","");yield return null;
        var scroll=screen.transform.Find("CardCollection").GetComponent<ScrollRect>();scroll.verticalNormalizedPosition=0;yield return Shot("Cards-collection-bottom");
        if(scroll.content.rect.height>scroll.viewport.rect.height&&scroll.content.anchoredPosition.y<=0)Debug.LogError("Cards collection cannot reach last group.");
        scroll.verticalNormalizedPosition=1;
        CardsPress(screen,"OpenAlbum");yield return Shot("Cards-open-album");
        if(UnityEngine.Object.FindFirstObjectByType<AlbumWindow>()==null||screen.isActiveAndEnabled)Debug.LogError("Cards Album navigation failed.");
        bar.CloseAll();bar.Toggle("Cards");yield return Shot("Cards-reopened");
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} Cards states; errors: {_errors}. Native selection, original-index equip/remove, inactive sockets, full capacity, search, scroll, localization, navigation, idle and reduced motion checked. Dispatch guard sabotage detected.");
    }

    IEnumerator AlbumDrive()
    {
        var sim=GetComponent<SimDriver>().Session;
        var bar=HudBar.Create(transform);yield return null;
        bar.Toggle("Album");yield return Shot("Album-empty");bar.CloseAll();
        string[] ids={"card_greenling","card_pinkling","card_yellowling","card_chomb"};
        foreach(string id in ids){
            for(int n=0;n<3;n++)sim.Debug("{\"grant_item\":{\"def_id\":\""+id+"\"}}");
            var inv=JsonUtility.FromJson<AlbumFixtureItems>("{\"items\":"+sim.QueryJson("inventory")+"}");
            for(int i=0;i<inv.items.Length;i++)if(inv.items[i].def_id==id){sim.ApplyInput("{\"DepositCardToAlbum\":{\"inv_idx\":"+i+"}}");break;}
        }
        foreach(string id in ids)sim.Debug("{\"grant_item\":{\"def_id\":\""+id+"\"}}");
        sim.Debug("{\"grant_item\":{\"def_id\":\"card_greenling\",\"foil\":true}}");
        bar.Toggle("Album");yield return null;
        var screen=UnityEngine.Object.FindFirstObjectByType<AlbumWindow>();screen.Refresh();
        var flags=BindingFlags.NonPublic|BindingFlags.Instance;
        typeof(AlbumWindow).GetMethod("Select",flags).Invoke(screen,new object[]{0});
        yield return Shot("Album-populated");
        if(!_output.Contains("compact"))for(int frame=0;frame<10;frame++){
            yield return new WaitForSecondsRealtime(.1f);yield return new WaitForEndOfFrame();
            ScreenCapture.CaptureScreenshot(Path.Combine(_output,"Album-idle-"+frame.ToString("00")+".png"));yield return null;
        }
        AlbumPress(screen,"BagTab");yield return Shot("Album-bag");
        var bagScroll=screen.transform.Find("CollectionViewport").GetComponent<ScrollRect>();
        if(bagScroll.content.rect.height>bagScroll.viewport.rect.height+1){
            bagScroll.verticalNormalizedPosition=0;yield return Shot("Album-bag-bottom");
            if(bagScroll.content.anchoredPosition.y<=0)Debug.LogError("Album bag cannot scroll to last card");
            bagScroll.verticalNormalizedPosition=1;
        }
        var album=JsonUtility.FromJson<AlbumFixtureItems>("{\"items\":"+sim.QueryJson("card_album")+"}");
        if(album.items.Length!=4)Debug.LogError("ALBUM fixture: expected four card kinds.");
        // Walk through the actual controls and read quantities/stars/binds back from the sim.
        typeof(AlbumWindow).GetMethod("Select",flags).Invoke(screen,new object[]{4});
        yield return Shot("Album-foil-bag");
        typeof(AlbumWindow).GetMethod("Select",flags).Invoke(screen,new object[]{0});
        int beforeDeposit=AlbumQuantity(sim);
        AlbumPress(screen,"Deposit");yield return null;
        if(AlbumQuantity(sim)!=beforeDeposit+1)Debug.LogError("ALBUM: deposit did not move the selected bag card.");
        AlbumPress(screen,"AlbumTab");yield return null;
        typeof(AlbumWindow).GetMethod("Select",flags).Invoke(screen,new object[]{0});
        int beforeWithdraw=AlbumQuantity(sim);
        // Sabotage the UI dispatch, verify the quantity guard detects the missing mutation,
        // then restore the real listener before the normal withdrawal walkthrough.
        var withdrawAction=System.Array.Find(screen.GetComponentsInChildren<Button>(),b=>b.name=="Withdraw");
        var realWithdraw=withdrawAction.onClick;withdrawAction.onClick=new Button.ButtonClickedEvent();
        withdrawAction.onClick.Invoke();
        if(AlbumQuantity(sim)==beforeWithdraw-4)Debug.LogError("ALBUM: withdrawal guard failed its disabled-dispatch audit");
        withdrawAction.onClick=realWithdraw;
        AlbumPress(screen,"Withdraw");yield return Shot("Album-after-withdraw");
        if(AlbumQuantity(sim)!=beforeWithdraw-4)Debug.LogError("ALBUM: withdraw did not move the selected stack.");
        // Pinkling is now selected. One candidate skips the picker, but still confirms consumption.
        int beforeFeed=AlbumQuantity(sim);string beforeCancel=sim.QueryJson("card_album");
        AlbumPress(screen,"LevelUp");yield return Shot("Album-level-confirm");
        var confirm=UnityEngine.Object.FindFirstObjectByType<UiConfirm>();
        if(confirm==null)Debug.LogError("ALBUM: level up lacks confirmation");
        else typeof(UiConfirm).GetMethod("Close",flags).Invoke(confirm,null);
        yield return null;
        if(sim.QueryJson("card_album")!=beforeCancel)Debug.LogError("ALBUM: cancelling consumed a card");
        AlbumPress(screen,"LevelUp");yield return null;
        confirm=UnityEngine.Object.FindFirstObjectByType<UiConfirm>();
        typeof(UiConfirm).GetMethod("ConfirmOnce",flags).Invoke(confirm,null);yield return Shot("Album-after-level");
        var fed=JsonUtility.FromJson<ItemList>("{\"items\":"+sim.QueryJson("card_album")+"}");
        bool raised=false;foreach(var c in fed.items)if(c.def_id=="card_pinkling"&&c.star==1)raised=true;
        if(AlbumQuantity(sim)!=beforeFeed-1||!raised)Debug.LogError("ALBUM: confirmed feed did not consume one copy and raise its level");
        int beforeBind=AlbumQuantity(sim);int selected=(int)typeof(AlbumWindow).GetField("_sel",flags).GetValue(screen);
        if(fed.items[selected].star!=1||fed.items[selected].def_id!="card_pinkling")Debug.LogError("ALBUM: upgraded card did not remain selected");
        string boundId=fed.items[selected].def_id;
        AlbumPress(screen,"Bind");yield return Shot("Album-bind-confirm");
        confirm=UnityEngine.Object.FindFirstObjectByType<UiConfirm>();
        if(confirm==null)Debug.LogError("ALBUM: bind lacks confirmation");
        else typeof(UiConfirm).GetMethod("ConfirmOnce",flags).Invoke(confirm,null);
        yield return Shot("Album-after-bind");
        var afterBind=JsonUtility.FromJson<ItemList>("{\"items\":"+sim.QueryJson("card_album")+"}");
        int boundIndex=System.Array.FindIndex(afterBind.items,c=>c.def_id==boundId);
        var bound=boundIndex>=0?JsonUtility.FromJson<AlbumBindFixture>(sim.QueryJson("bind_info:"+boundIndex)):null;
        if(AlbumQuantity(sim)!=beforeBind-1||bound==null||bound.done!=1)Debug.LogError("ALBUM: bind did not consume one copy and record one permanent bind");
        var idle=screen.GetComponentInChildren<MonsterPortraitIdle>();
        if(idle!=null){int n=idle.FrameChanges;yield return new WaitForSecondsRealtime(.6f);if(UiEntrance.MotionAllowed&&PlayerPrefs.GetInt("roi.opt.postfx",1)!=0&&idle.FrameChanges<=n)Debug.LogError("ALBUM: selected portrait is not idling");}
        bool hadMotion=PlayerPrefs.HasKey(UiEntrance.ReducedMotionKey);int motion=PlayerPrefs.GetInt(UiEntrance.ReducedMotionKey,0);
        try{
            PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,1);int n=idle!=null?idle.FrameChanges:0;
            yield return new WaitForSecondsRealtime(.6f);
            if(idle!=null&&idle.FrameChanges!=n)Debug.LogError("ALBUM: reduced motion continued swapping portrait frames");
        }finally{if(hadMotion)PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,motion);else PlayerPrefs.DeleteKey(UiEntrance.ReducedMotionKey);}
        bar.CloseAll();bar.Toggle("Album");yield return new WaitForSecondsRealtime(.3f);
        idle=screen.GetComponentInChildren<MonsterPortraitIdle>();
        if(idle!=null){int n=idle.FrameChanges;yield return new WaitForSecondsRealtime(.6f);if(UiEntrance.MotionAllowed&&PlayerPrefs.GetInt("roi.opt.postfx",1)!=0&&idle.FrameChanges<=n)Debug.LogError("ALBUM: portrait did not resume after reopening");}
        AlbumPress(screen,"BagTab");yield return null;
        bagScroll.verticalNormalizedPosition=0;
        AlbumPress(screen,"AlbumTab");yield return null;AlbumPress(screen,"BagTab");yield return null;
        if(bagScroll.content.rect.height>bagScroll.viewport.rect.height+1&&bagScroll.verticalNormalizedPosition>.01f)Debug.LogError("ALBUM: bag scroll position lost on tab switch");
        // Short viewport actions must stay reachable, and long localized lines must not overlap.
        var detailScroll=screen.transform.Find("SelectedCardFrame/DetailViewport").GetComponent<ScrollRect>();
        detailScroll.verticalNormalizedPosition=0;yield return Shot("Album-detail-bottom");
        string originalLanguage=Loc.Language;Loc.Language="th";screen.Refresh();yield return Shot("Album-thai");Loc.Language=originalLanguage;screen.Refresh();
        bar.CloseAll();
        var home=new GameObject("Navigation preview",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler),typeof(GraphicRaycaster));home.transform.SetParent(transform,false);
        home.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;home.GetComponent<Canvas>().sortingOrder=100;
        home.GetComponent<CanvasScaler>().uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;
        home.GetComponent<CanvasScaler>().referenceResolution=new Vector2(1440,900);
        HomeMenuLayout.Build(home.transform,bar.Toggle);
        var layout=home.GetComponentInChildren<HomeMenuLayout>();
        typeof(HomeMenuLayout).GetMethod("Navigate",flags).Invoke(layout,new object[]{"Workshop"});
        Destroy(home);yield return null;
        yield return Shot("Workshop-navigation");
        bar.CloseAll();
        var menu=(Button)typeof(HudBar).GetField("_menuBtn",flags).GetValue(bar);menu.onClick.Invoke();
        yield return Shot("System-menu");menu.onClick.Invoke();
        var entries=(Array)typeof(HudBar).GetField("Entries",BindingFlags.NonPublic|BindingFlags.Static).GetValue(null);
        foreach(var entry in entries)if((string)entry.GetType().GetField("Label").GetValue(entry)=="Kitchen")Debug.LogError("Retired Kitchen still in navigation");
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} screens; errors: {_errors}. Disposable Album fixture; no account changes.");
    }

    static int AlbumQuantity(SimSession sim)
    { var items=JsonUtility.FromJson<ItemList>("{\"items\":"+sim.QueryJson("card_album")+"}");int total=0;foreach(var item in items.items)total+=item.quantity;return total; }
    static void AlbumPress(AlbumWindow screen,string name)
    {
        foreach(var button in screen.GetComponentsInChildren<Button>())if(button.name==name){if(!button.interactable)Debug.LogError("ALBUM: action disabled: "+name);else button.onClick.Invoke();return;}
        Debug.LogError("ALBUM: action missing: "+name);
    }
    [Serializable] sealed class AlbumBindFixture { public int done; }
    [Serializable] sealed class AlbumFixtureItems { public AlbumFixtureItem[] items; }
    [Serializable] sealed class AlbumFixtureItem { public string def_id; }
    IEnumerator GachaCurrentDrive()
    {
        var bar=HudBar.Create(transform);yield return null;bar.Toggle("Summon");
        yield return Shot("Summon");
        Loc.Language="th";
        UnityEngine.Object.FindFirstObjectByType<SummonWindow>().Refresh();
        yield return Shot("Summon-thai");
        Loc.Language="en";
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} native gallery states; errors: {_errors}. No server calls or purchases.");
    }
    IEnumerator ExcavationDrive()
    {
        var bar=HudBar.Create(transform);yield return null;bar.Toggle("Summon");
        yield return Shot("Excavation-disconnected");
        var screen=UnityEngine.Object.FindFirstObjectByType<SummonWindow>();
        var flags=BindingFlags.NonPublic|BindingFlags.Instance;
        if(_output.Contains("action-70")) {
            var events=UnityEngine.EventSystems.EventSystem.current;
            var pointer=new UnityEngine.EventSystems.PointerEventData(events){button=UnityEngine.EventSystems.PointerEventData.InputButton.Left};
            var probability=screen.transform.Find("ExcavationDesign/Probability").gameObject;
            var details=screen.transform.Find("ExcavationDesign/CharacterDetails").gameObject;
            UnityEngine.EventSystems.ExecuteEvents.Execute(probability,pointer,UnityEngine.EventSystems.ExecuteEvents.pointerEnterHandler);
            yield return Shot("Excavation-info-hover");
            UnityEngine.EventSystems.ExecuteEvents.Execute(probability,pointer,UnityEngine.EventSystems.ExecuteEvents.pointerDownHandler);
            yield return Shot("Excavation-info-pressed");
            UnityEngine.EventSystems.ExecuteEvents.Execute(probability,pointer,UnityEngine.EventSystems.ExecuteEvents.pointerUpHandler);
            UnityEngine.EventSystems.ExecuteEvents.Execute(probability,pointer,UnityEngine.EventSystems.ExecuteEvents.pointerExitHandler);
            events.SetSelectedGameObject(details);
            yield return Shot("Excavation-details-focus");
            events.SetSelectedGameObject(null);
        }
        bool HasSingleAvailabilityNotice() {
            var design=screen.transform.Find("ExcavationDesign");
            var terms=design.Find("BannerTerms/Terms").GetComponent<TMPro.TextMeshProUGUI>();
            if(terms.text!=Loc.Get("UI_EXCAVATION_CONNECTION_HELP"))return false;
            foreach(string id in new[]{"Pull1","Pull10"}) {
                var pull=design.Find(id);
                if(pull.GetComponent<Button>().interactable||!string.IsNullOrEmpty(pull.Find("ActionHint").GetComponent<TMPro.TextMeshProUGUI>().text))return false;
            }
            return true;
        }
        if(!HasSingleAvailabilityNotice())Debug.LogError("EXCAVATION: unavailable state needs one actionable notice and disabled pulls");
        var duplicate=screen.transform.Find("ExcavationDesign/Pull1/ActionHint").GetComponent<TMPro.TextMeshProUGUI>();
        duplicate.text=Loc.Get("UI_GACHA_UNAVAILABLE");
        if(HasSingleAvailabilityNotice())Debug.LogError("EXCAVATION: notice check accepted a duplicate (sabotage)");
        duplicate.text="";
        typeof(SummonWindow).GetField("_catalogPreview",flags).SetValue(screen,false);
        typeof(SummonWindow).GetField("_stateJson",flags).SetValue(screen,"{\"coupons\":\"10\",\"tradeable\":\"0\",\"bound\":\"0\",\"pity\":{\"regalia\":{\"lost_5050\":false,\"count\":7}},\"banners\":[{\"id\":\"standard\",\"kind\":\"Standard\",\"ticket_kind\":\"standard\",\"tickets\":10,\"open\":true,\"pool\":[\"exemplar_seraphine\",\"exemplar_lapis\"],\"featured\":\"exemplar_seraphine\",\"base_bp\":60,\"has_pity\":false,\"hard\":0},{\"id\":\"regalia\",\"kind\":\"Regalia\",\"base_bp\":80,\"has_pity\":true,\"hard\":80}]}");
        typeof(SummonWindow).GetMethod("Rebuild",flags).Invoke(screen,null);
        yield return Shot("Excavation-layout-fixture");
        if((int)typeof(SummonWindow).GetMethod("PityCount",flags).Invoke(screen,new object[]{"Regalia"})!=7)Debug.LogError("EXCAVATION: progress must read lowercase server key with unordered fields");
        if(screen.transform.Find("ExcavationDesign/BannerTerms/Pity/Progress")!=null)Debug.LogError("EXCAVATION: no-pity banner shows a progress counter");
        typeof(SummonWindow).GetMethod("RenderActionState",flags).Invoke(screen,new object[]{true});
        if(!((Button)typeof(SummonWindow).GetField("_one",flags).GetValue(screen)).interactable)Debug.LogError("EXCAVATION: ready presentation remains disabled");
        if(screen.transform.Find("ExcavationDesign/BannerTerms/NoticeTitle").gameObject.activeSelf)Debug.LogError("EXCAVATION: connection notice remains after availability is restored");
        yield return Shot("Excavation-ready");
        typeof(SummonWindow).GetMethod("RenderActionState",flags).Invoke(screen,new object[]{false});
        var attempt=typeof(SummonWindow).GetMethod("PrepareAttempt",flags);
        attempt.Invoke(screen,new object[]{1});
        var keyField=typeof(SummonWindow).GetField("_pendingKey",flags);string retryKey=(string)keyField.GetValue(screen);
        if(!(bool)attempt.Invoke(screen,new object[]{1})||(string)keyField.GetValue(screen)!=retryKey)Debug.LogError("EXCAVATION: retry changed idempotency key");
        if((bool)attempt.Invoke(screen,new object[]{10}))Debug.LogError("EXCAVATION: unresolved retry changed pull count");
        keyField.SetValue(screen,"");
        int confirmations=0;
        UiConfirm.AskExcavation(1,()=>confirmations++);
        yield return Shot("Excavation-confirm-one");
        foreach(var b in UnityEngine.Object.FindFirstObjectByType<UiConfirm>().GetComponentsInChildren<Button>())if(b.name=="Cancel")b.onClick.Invoke();
        yield return null;
        if(confirmations!=0)Debug.LogError("EXCAVATION: cancel confirmed a pull");
        UiConfirm.AskExcavation(10,()=>confirmations++);
        yield return Shot("Excavation-confirm-ten");
        foreach(var b in UnityEngine.Object.FindFirstObjectByType<UiConfirm>().GetComponentsInChildren<Button>())if(b.name=="Confirm"){b.onClick.Invoke();b.onClick.Invoke();}
        yield return null;
        if(confirmations!=1)Debug.LogError("EXCAVATION: confirm callback must run exactly once");
        foreach(var b in screen.GetComponentsInChildren<Button>())if(b.name=="CharacterDetails")b.onClick.Invoke();
        yield return Shot("Excavation-character-details");
        foreach(var modal in UnityEngine.Object.FindObjectsByType<UiConfirm>(FindObjectsSortMode.None))Destroy(modal.gameObject);
        yield return null;
        var one=(Button)typeof(SummonWindow).GetField("_one",flags).GetValue(screen);
        if(one.interactable)Debug.LogError("EXCAVATION: disconnected fixture permits pulls");
        foreach(var b in screen.GetComponentsInChildren<Button>())if(b.name=="Probability")b.onClick.Invoke();
        yield return Shot("Excavation-probability");
        foreach(var modal in UnityEngine.Object.FindObjectsByType<UiConfirm>(FindObjectsSortMode.None))Destroy(modal.gameObject);
        foreach(var b in screen.GetComponentsInChildren<Button>())if(b.name=="Banner_regalia")b.onClick.Invoke();
        typeof(SummonWindow).GetMethod("RenderActionState",flags).Invoke(screen,new object[]{true});
        yield return new WaitForSecondsRealtime(.3f);
        yield return Shot("Excavation-regalia");
        foreach(var b in screen.GetComponentsInChildren<Button>())if(b.name=="Banner_standard")b.onClick.Invoke();
        Loc.Language="th";typeof(SummonWindow).GetMethod("Rebuild",flags).Invoke(screen,null);
        typeof(SummonWindow).GetMethod("RenderActionState",flags).Invoke(screen,new object[]{true});
        yield return new WaitForSecondsRealtime(.3f);
        yield return Shot("Excavation-thai");
        UiConfirm.AskExcavation(10,()=>confirmations++);
        yield return Shot("Excavation-confirm-thai");
        foreach(var b in UnityEngine.Object.FindFirstObjectByType<UiConfirm>().GetComponentsInChildren<Button>())if(b.name=="Cancel")b.onClick.Invoke();
        yield return null;Loc.Language="en";
        typeof(SummonWindow).GetField("_quoteCount",flags).SetValue(screen,10);
        typeof(SummonWindow).GetField("_quoteCrystal",flags).SetValue(screen,(ulong)1600);
        typeof(SummonWindow).GetMethod("Rebuild",flags).Invoke(screen,null);
        yield return Shot("Excavation-confirm-crystals");
        bool cancelled=false;
        foreach(var b in UnityEngine.Object.FindFirstObjectByType<UiConfirm>().GetComponentsInChildren<Button>())
          if(b.name=="Cancel"){b.onClick.Invoke();cancelled=true;break;}
        if(!cancelled||(int)typeof(SummonWindow).GetField("_quoteCount",flags).GetValue(screen)!=0)Debug.LogError("EXCAVATION: quote cannot be cancelled");
        var stateField=typeof(SummonWindow).GetField("_stateJson",flags);
        string originalState=(string)stateField.GetValue(screen);
        if(_output.Contains("wallet-71")||_output.Contains("currency-72")||_output.Contains("currency-73")||_output.Contains("currency-81")||_output.Contains("wallet-82")) {
            stateField.SetValue(screen,originalState.Replace("\"tradeable\":\"0\"","\"tradeable\":\"1234567\"").Replace("\"bound\":\"0\"","\"bound\":\"890\""));
            typeof(SummonWindow).GetMethod("Rebuild",flags).Invoke(screen,null);
            typeof(SummonWindow).GetMethod("RenderActionState",flags).Invoke(screen,new object[]{true});
            yield return Shot("Excavation-wallet-funded");
            Loc.Language="th";typeof(SummonWindow).GetMethod("Rebuild",flags).Invoke(screen,null);
            typeof(SummonWindow).GetMethod("RenderActionState",flags).Invoke(screen,new object[]{true});
            yield return Shot("Excavation-wallet-funded-thai");
            UiConfirm.AskExcavation(10,()=>{},1600);
            yield return Shot("Excavation-confirm-gem-thai");
            foreach(var b in UnityEngine.Object.FindFirstObjectByType<UiConfirm>().GetComponentsInChildren<Button>())if(b.name=="Cancel")b.onClick.Invoke();
            yield return null;Loc.Language="en";
        }
        stateField.SetValue(screen,originalState.Replace("\"tickets\":10","\"tickets\":2"));
        typeof(SummonWindow).GetMethod("Rebuild",flags).Invoke(screen,null);
        typeof(SummonWindow).GetMethod("RenderActionState",flags).Invoke(screen,new object[]{true});
        yield return Shot("Excavation-short-tickets");
        typeof(SummonWindow).GetField("_requestInFlight",flags).SetValue(screen,true);
        typeof(SummonWindow).GetMethod("RenderActionState",flags).Invoke(screen,new object[]{true});
        if(((Button)typeof(SummonWindow).GetField("_one",flags).GetValue(screen)).interactable)Debug.LogError("EXCAVATION: in-flight button is enabled");
        yield return Shot("Excavation-pending");
        typeof(SummonWindow).GetField("_requestInFlight",flags).SetValue(screen,false);
        stateField.SetValue(screen,originalState.Replace("\"open\":true","\"open\":false"));
        typeof(SummonWindow).GetMethod("Rebuild",flags).Invoke(screen,null);
        typeof(SummonWindow).GetMethod("RenderActionState",flags).Invoke(screen,new object[]{true});
        if(((Button)typeof(SummonWindow).GetField("_one",flags).GetValue(screen)).interactable)Debug.LogError("EXCAVATION: locked banner permits a pull");
        yield return Shot("Excavation-locked");
        stateField.SetValue(screen,originalState);
        string reward="{\"def_id\":\"exemplar_seraphine\",\"rarity\":\"Ssr\",\"featured\":true}";
        string[] rewards=new string[10];for(int i=0;i<10;i++)rewards[i]=i%2==0?reward:reward.Replace("seraphine","lapis").Replace("Ssr","Sr");
        typeof(SummonWindow).GetField("_lastPulls",flags).SetValue(screen,"{\"pulls\":["+string.Join(",",rewards)+"]}");
        typeof(SummonWindow).GetMethod("Rebuild",flags).Invoke(screen,null);
        var sequence=UnityEngine.Object.FindFirstObjectByType<ExcavationResultSequence>();
        if(sequence==null)Debug.LogError("EXCAVATION: reward sequence missing");
        else {
            sequence.Advance();
            if(!sequence.Complete)Debug.LogError("EXCAVATION: skip did not reveal all rewards");
            if(string.IsNullOrEmpty((string)typeof(SummonWindow).GetField("_lastPulls",flags).GetValue(screen)))Debug.LogError("EXCAVATION: skip discarded results");
        }
        yield return Shot("Excavation-results-ten");
        var resultModal=UnityEngine.Object.FindFirstObjectByType<UiConfirm>();
        if(resultModal==null)Debug.LogError("EXCAVATION: result sheet missing");
        foreach(var b in resultModal.GetComponentsInChildren<Button>())if(b.name=="Close")b.onClick.Invoke();
        yield return null;
        if(!string.IsNullOrEmpty((string)typeof(SummonWindow).GetField("_lastPulls",flags).GetValue(screen)))Debug.LogError("EXCAVATION: dismissed rewards remain open");
        Loc.Language="th";
        typeof(SummonWindow).GetField("_lastPulls",flags).SetValue(screen,"{\"pulls\":["+reward+"]}");
        typeof(SummonWindow).GetMethod("Rebuild",flags).Invoke(screen,null);
        yield return new WaitForSecondsRealtime(.6f);
        yield return Shot("Excavation-results-one-thai");
        foreach(var b in UnityEngine.Object.FindFirstObjectByType<UiConfirm>().GetComponentsInChildren<Button>())if(b.name=="Close")b.onClick.Invoke();
        yield return null;Loc.Language="en";
        bool hadMotion=PlayerPrefs.HasKey(UiEntrance.ReducedMotionKey);
        int oldMotion=PlayerPrefs.GetInt(UiEntrance.ReducedMotionKey,0);
        PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,1);
        var motionCheck=new GameObject("ReducedMotionCheck",typeof(RectTransform));
        motionCheck.transform.SetParent(screen.transform,false);
        var reduced=UiEntrance.Play((RectTransform)motionCheck.transform,5,1,100);
        if(!reduced.Complete||motionCheck.GetComponent<CanvasGroup>().alpha!=1)Debug.LogError("EXCAVATION: reduced motion hides or delays content");
        Destroy(motionCheck);
        if(hadMotion)PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,oldMotion);else PlayerPrefs.DeleteKey(UiEntrance.ReducedMotionKey);
        foreach(var b in screen.GetComponentsInChildren<Button>())if(b.name=="Back")b.onClick.Invoke();
        yield return null;
        if(screen.gameObject.activeInHierarchy)Debug.LogError("EXCAVATION: back did not close screen");
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} states; errors: {_errors}. Isolated data fixture; no online calls or purchases.");
        Debug.Log("EXCAVATION PROBE COMPLETE "+_errors);
    }
    IEnumerator CurrencyIconsDrive()
    {
        var bar=HudBar.Create(transform);yield return null;bar.gameObject.SetActive(false);bar.Floating.gameObject.SetActive(false);
        var canvas=new GameObject("Currency review",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler));
        canvas.transform.SetParent(transform,false);canvas.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;
        var scaler=canvas.GetComponent<CanvasScaler>();scaler.uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;scaler.referenceResolution=new Vector2(1440,900);
        var board=UiTheme.Flat(canvas.transform,"CurrencyBoard",UiTheme.Panel);
        board.rectTransform.anchorMin=Vector2.zero;board.rectTransform.anchorMax=Vector2.one;board.rectTransform.offsetMin=board.rectTransform.offsetMax=Vector2.zero;
        RectTransform Box(Transform parent,string name,float x,float y,float w,float h) {
            var rt=(RectTransform)new GameObject(name,typeof(RectTransform)).transform;rt.SetParent(parent,false);
            rt.anchorMin=rt.anchorMax=rt.pivot=new Vector2(0,1);rt.anchoredPosition=new Vector2(x,-y);rt.sizeDelta=new Vector2(w,h);return rt;
        }
        void Label(string name,float x,float y,float w,float size) {
            var cell=Box(board.transform,name,x,y,w,46);
            var label=UiTheme.Label(cell,"Label",name,size,UiTheme.TextMid,TMPro.TextAlignmentOptions.Center);
            label.rectTransform.anchorMin=Vector2.zero;label.rectTransform.anchorMax=Vector2.one;label.rectTransform.offsetMin=label.rectTransform.offsetMax=Vector2.zero;
        }
        Label("PAINTED CURRENCY ITEMS",80,52,1280,30);
        var kinds=new[]{"Gil","Soul","Gem","Ticket"};
        var colors=new[]{Color.white,Color.white,Color.white,Color.white};
        for(int col=0;col<4;col++) {
            float x=90+col*330;
            var large=Box(board.transform,kinds[col],x+95,157,100,100).gameObject.AddComponent<HomeCurrencyIcon>();
            large.Kind=kinds[col];large.color=colors[col];
            Label(kinds[col],x,280,290,24);
            int row=0;foreach(int size in new[]{16,24,32,48}) {
                var icon=Box(board.transform,"Size",x+133,365+row*58,size,size).gameObject.AddComponent<HomeCurrencyIcon>();
                icon.Kind=kinds[col];icon.color=colors[col];Label(size+" px",x+30,356+row*58,80,16);row++;
            }
        }
        Label("MENU FAMILY",90,640,1260,16);
        var samples=new[]{"Summon","Exemplar","Cards","Resonance","Crystal"};
        for(int i=0;i<samples.Length;i++) {
            var icon=Box(board.transform,"MenuFamily",470+i*100,710,48,48).gameObject.AddComponent<HomeIcon>();icon.Kind=samples[i];icon.color=UiTheme.Accent;
        }
        yield return Shot("Currency-set");board.gameObject.SetActive(false);
        HomeMenuLayout.Build(canvas.transform,id=>{});yield return new WaitForSecondsRealtime(1);
        yield return Shot("Home");
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} currency screens; errors: {_errors}. Painted RGBA items, size review and lobby.");
    }
    IEnumerator IconsFinalDrive()
    {
        var bar=HudBar.Create(transform);yield return null;bar.Toggle("Events");yield return Shot("Events");
        var canvas=new GameObject("IconCanvas",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler));
        canvas.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;canvas.GetComponent<Canvas>().sortingOrder=100;
        var board=UiTheme.Flat(canvas.transform,"IconReview",UiTheme.Panel);
        board.rectTransform.anchorMin=Vector2.zero;board.rectTransform.anchorMax=Vector2.one;board.rectTransform.offsetMin=board.rectTransform.offsetMax=Vector2.zero;
        string[] icons={"Exemplar","Party","Inventory","Stash","Workshop","Equipment","Apothecary","Provisions","Shop","Trade","Crystal","Mail","Daily","Weekly","Pass","Commander","Outpost","Record","Story","Album","Bestiary","Map","Dungeon","Cards","Regalia","Skills","Paths","Apex","Resonance","Options","Events","Summon","Combat"};
        int columns=7,rows=5;
        if(_output.Contains("62")){columns=6;rows=6;icons=new[]{"Exemplar","Party","Inventory","Stash","Workshop","Equipment","Apothecary","Provisions","Shop","Trade","Crystal","Mail","Daily","Weekly","Pass","Commander","Outpost","Record","Story","Album","Bestiary","Map","Dungeon","Cards","Regalia","Skills","Paths","Apex","Resonance","Options","Events","Summon","Combat","Info","Probability","Celestial"};}
        for(int i=0;i<icons.Length;i++){
            var cell=new GameObject(icons[i],typeof(RectTransform));cell.transform.SetParent(board.transform,false);
            var cr=(RectTransform)cell.transform;cr.anchorMin=new Vector2((i%columns)/(float)columns,1-(i/columns+1)/(float)rows);cr.anchorMax=new Vector2((i%columns+1)/(float)columns,1-(i/columns)/(float)rows);cr.offsetMin=new Vector2(14,8);cr.offsetMax=new Vector2(-14,-8);
            var go=new GameObject("Glyph",typeof(RectTransform));go.transform.SetParent(cr,false);var icon=go.AddComponent<HomeIcon>();icon.Kind=icons[i];icon.color=UiTheme.TextHigh;icon.raycastTarget=false;
            icon.rectTransform.anchorMin=new Vector2(.23f,.36f);icon.rectTransform.anchorMax=new Vector2(.77f,.92f);icon.rectTransform.offsetMin=icon.rectTransform.offsetMax=Vector2.zero;
            var small=new GameObject("Small",typeof(RectTransform));small.transform.SetParent(cr,false);var tiny=small.AddComponent<HomeIcon>();tiny.Kind=icons[i];tiny.color=UiTheme.Accent;tiny.raycastTarget=false;tiny.rectTransform.anchorMin=tiny.rectTransform.anchorMax=new Vector2(.84f,.62f);tiny.rectTransform.sizeDelta=new Vector2(24,24);
            var label=UiTheme.Label(cr,"Name",icons[i],16,UiTheme.TextMid,TMPro.TextAlignmentOptions.Center);label.rectTransform.anchorMin=new Vector2(0,.07f);label.rectTransform.anchorMax=new Vector2(1,.30f);label.rectTransform.offsetMin=label.rectTransform.offsetMax=Vector2.zero;
        }
        if(_output.Contains("62")){yield return Shot("Icon-set");foreach(Transform child in board.transform)Destroy(child.gameObject);yield return null;
            string[] sample={"Summon","Cards","Exemplar","Inventory","Resonance"};int[] sizes={16,24,32,48,80};
            for(int row=0;row<sample.Length;row++){
                float y=1-(row+.5f)/sample.Length;
                var caption=UiTheme.Label(board.transform,"Label",sample[row],18,UiTheme.TextMid,TMPro.TextAlignmentOptions.Center);
                caption.rectTransform.anchorMin=caption.rectTransform.anchorMax=new Vector2(.10f,y);caption.rectTransform.sizeDelta=new Vector2(130,30);
                for(int col=0;col<sizes.Length+2;col++){
                    int px=col<sizes.Length?sizes[col]:40;float x=.25f+col*.105f;
                    var go=new GameObject("Size_"+px,typeof(RectTransform));go.transform.SetParent(board.transform,false);var icon=go.AddComponent<HomeIcon>();icon.Kind=sample[row];icon.raycastTarget=false;
                    icon.color=col==sizes.Length?UiTheme.Accent:col==sizes.Length+1?new Color(UiTheme.TextMid.r,UiTheme.TextMid.g,UiTheme.TextMid.b,.38f):UiTheme.TextHigh;
                    icon.rectTransform.anchorMin=icon.rectTransform.anchorMax=new Vector2(x,y+.02f);icon.rectTransform.sizeDelta=new Vector2(px,px);
                    var text=UiTheme.Label(board.transform,"Caption",col<sizes.Length?px+" px":col==sizes.Length?"Selected":"Disabled",13,UiTheme.TextMid,TMPro.TextAlignmentOptions.Center);
                    text.rectTransform.anchorMin=text.rectTransform.anchorMax=new Vector2(x,y-.065f);text.rectTransform.sizeDelta=new Vector2(94,22);
                }
            }
            yield return Shot("Icon-sizes");Destroy(board.gameObject);
        }
        else {
        yield return Shot("Icon-set");Destroy(board.gameObject);
        }
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} icon screens; errors: {_errors}.");
    }
    IEnumerator FontNumbersDrive()
    {
        var bar=HudBar.Create(transform);yield return null;bar.gameObject.SetActive(false);bar.Floating.gameObject.SetActive(false);
        var canvas=new GameObject("Numeric font review",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler),typeof(GraphicRaycaster));
        canvas.transform.SetParent(transform,false);canvas.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;
        var scaler=canvas.GetComponent<CanvasScaler>();scaler.uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;scaler.referenceResolution=new Vector2(1440,900);
        HomeMenuLayout.Build(canvas.transform,id=>{});yield return new WaitForSecondsRealtime(1);
        yield return Shot("Home");yield return Shot("Home-navigation");
        foreach(Transform child in canvas.transform)child.gameObject.SetActive(false);
        var screen=ExemplarScreen.Create(canvas.transform,()=>{});screen.Refresh();screen.SelectPage("Attributes");
        yield return Shot("Exemplar-Attributes");screen.SelectPage("Apex");yield return Shot("Exemplar-Apex");
        var scroll=screen.GetComponentInChildren<ExemplarApexPage>().GetComponentInChildren<ScrollRect>();scroll.verticalNormalizedPosition=0;
        yield return Shot("Exemplar-Apex-lower");
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} numeric typography screens; errors: {_errors}.");
    }
    IEnumerator FontThaiDrive()
    {
        string previousLanguage=Loc.Language;
        var body=UiTheme.Font;var title=UiTheme.DisplayFont;
        string sample="Seraphine 0123456789 +10 / 70 เพิ่มพลังโจมตี ค่าพลังชีวิต อัปเกรด คืนแต้มทั้งหมด";
        bool valid=body.faceInfo.familyName.Contains("Bai Jamjuree") && title.faceInfo.familyName.Contains("Marcellus");
        foreach(char c in sample)if(!char.IsWhiteSpace(c)&&(!body.HasCharacter(c,true)||!title.HasCharacter(c,true)))valid=false;
        if(!valid)Debug.LogError("TYPE: active fonts or required Latin/Thai glyphs are missing.");
        // Sabotage: a wrong family must not pass merely because fallback can draw the string.
        var wrong=Resources.Load<TMPro.TMP_FontAsset>("Fonts/NotoSans UI SDF");
        if(wrong.faceInfo.familyName.Contains("Bai Jamjuree"))Debug.LogError("TYPE: family guard accepted the old font.");
        Loc.Language="th";
        var bar=HudBar.Create(transform);yield return null;
        foreach(var id in new[]{"Shop","Options","Summon"})
        {
            bar.CloseAll();bar.Toggle(id);yield return Shot(id+"-thai");
        }
        // Explicit Thai specimen verifies diacritics and layout even where translation is incomplete.
        var canvas=new GameObject("Type specimen",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler),typeof(GraphicRaycaster));
        canvas.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;canvas.GetComponent<Canvas>().sortingOrder=200;
        var scale=canvas.GetComponent<CanvasScaler>();scale.uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;scale.referenceResolution=new Vector2(1440,900);
        var panel=UiTheme.Box(canvas.transform,"Backdrop",UiTheme.Panel);ExemplarUi.Place(panel.rectTransform,0,0,1,1);
        var head=ExemplarUi.Text(panel.transform,"ThaiTitle","เพิ่มศักยภาพตัวละคร",.08f,.73f,.92f,.87f,48,UiTheme.Accent);
        ExemplarUi.Text(panel.transform,"ThaiBody",sample,.08f,.52f,.92f,.69f,27,UiTheme.TextHigh).textWrappingMode=TMPro.TextWrappingModes.Normal;
        ExemplarUi.Text(panel.transform,"Numbers","0123456789    +10 / 70    1,234,567    0.15%",.08f,.35f,.92f,.46f,26,UiTheme.TextHigh);
        ExemplarUi.Text(panel.transform,"LatinTitle","Exemplars / Apex — Seraphine",.08f,.14f,.92f,.30f,44,UiTheme.Accent);
        yield return Shot("Thai-and-Latin-specimen");Destroy(canvas);Loc.Language=previousLanguage;
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} typography states; errors: {_errors}. Latin/Thai glyph coverage and font family checked; wrong-family sabotage rejected.");
    }
    IEnumerator TypographyDrive()
    {
        var bar=HudBar.Create(transform);yield return null;
        foreach(var id in new[]{"Story","Apothecary","Regalia"}){bar.CloseAll();bar.Toggle(id);yield return Shot(id);}
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} typography screens; errors: {_errors}.");
    }
    IEnumerator MonsterDrive()
    {
        var bar=HudBar.Create(transform);yield return null;bar.Toggle("Bestiary");yield return null;
        var screen=UnityEngine.Object.FindFirstObjectByType<BestiaryWindow>();
        yield return Shot("Monster-undiscovered");
        var flags=BindingFlags.NonPublic|BindingFlags.Instance;
        // Isolated display fixture: only copied query records are discovered; no save or network.
        var driver=UnityEngine.Object.FindFirstObjectByType<SimDriver>();
        string json=driver.Session.QueryJson("bestiary");
        var field=typeof(BestiaryWindow).GetField("_data",flags);
        json=json.Replace("\"kills\":0","\"kills\":125");
        field.SetValue(screen,JsonUtility.FromJson(json,field.FieldType));
        var art=new GameObject("Portrait fixture").AddComponent<Roi.Arena.ArenaView>();art.enabled=false;
        typeof(BestiaryWindow).GetField("_arena",flags).SetValue(screen,art);
        var render=typeof(BestiaryWindow).GetMethod("Render",flags);render.Invoke(screen,null);
        yield return Shot("Monster-traits");
        var idlePortrait=screen.GetComponentInChildren<MonsterPortraitIdle>();
        bool hadReduced=PlayerPrefs.HasKey(UiEntrance.ReducedMotionKey),hadFx=PlayerPrefs.HasKey("roi.opt.postfx");
        int oldReduced=PlayerPrefs.GetInt(UiEntrance.ReducedMotionKey,0),oldFx=PlayerPrefs.GetInt("roi.opt.postfx",1);
        PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,0);PlayerPrefs.SetInt("roi.opt.postfx",1);
        if(idlePortrait==null)Debug.LogError("MONSTER: selected portrait missing idle player");
        else {
            int initial=idlePortrait.FrameChanges;
            Directory.CreateDirectory(Path.Combine(_output,"motion"));
            for(int i=0;i<12;i++)yield return Shot("motion/idle-"+i.ToString("D2"));
            if(idlePortrait.FrameChanges<=initial)Debug.LogError("MONSTER: idle did not advance");
            PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,1);yield return new WaitForSecondsRealtime(.3f);
            int frozen=idlePortrait.FrameChanges;yield return new WaitForSecondsRealtime(.8f);
            if(idlePortrait.FrameChanges!=frozen)Debug.LogError("MONSTER: reduced motion still swaps frames");
            PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,0);PlayerPrefs.SetInt("roi.opt.postfx",0);yield return new WaitForSecondsRealtime(.3f);
            frozen=idlePortrait.FrameChanges;yield return new WaitForSecondsRealtime(.8f);
            if(idlePortrait.FrameChanges!=frozen)Debug.LogError("MONSTER: effects-off still swaps frames");
        }
        if(hadReduced)PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,oldReduced);else PlayerPrefs.DeleteKey(UiEntrance.ReducedMotionKey);
        if(hadFx)PlayerPrefs.SetInt("roi.opt.postfx",oldFx);else PlayerPrefs.DeleteKey("roi.opt.postfx");

        var input=screen.GetComponentInChildren<TMPro.TMP_InputField>();
        input.Select();input.ActivateInputField();yield return null;
        var keyboard=UnityEngine.InputSystem.InputSystem.AddDevice<UnityEngine.InputSystem.Keyboard>();
        UnityEngine.InputSystem.InputSystem.QueueStateEvent(keyboard,new UnityEngine.InputSystem.LowLevel.KeyboardState(UnityEngine.InputSystem.Key.I));yield return null;yield return null;
        if(!screen.gameObject.activeInHierarchy)Debug.LogError("MONSTER: typing triggered a menu shortcut");
        UnityEngine.InputSystem.InputSystem.QueueStateEvent(keyboard,new UnityEngine.InputSystem.LowLevel.KeyboardState());yield return null;
        UnityEngine.InputSystem.InputSystem.RemoveDevice(keyboard);input.DeactivateInputField();input.text="zzzz-no-monster";yield return null;
        if(screen.transform.Find("MonsterGrid/Viewport/Content/NoMatches")==null)Debug.LogError("MONSTER: missing no-results state");
        yield return Shot("Monster-search-empty");input.text="";yield return null;
        var tabs=typeof(BestiaryWindow).GetField("_tab",flags);
        tabs.SetValue(screen,1);render.Invoke(screen,null);yield return Shot("Monster-drops");
        tabs.SetValue(screen,2);render.Invoke(screen,null);yield return Shot("Monster-rewards");
        if(_output.Contains("monster-polish-55"))
        {
            var header=(RectTransform)typeof(BestiaryWindow).GetField("_dossierHeader",flags).GetValue(screen);
            var scroll=(ScrollRect)typeof(BestiaryWindow).GetField("_detailScroll",flags).GetValue(screen);
            var originalHeader=header.position;var originalContent=scroll.content.position;
            scroll.verticalNormalizedPosition=0;yield return null;Canvas.ForceUpdateCanvases();
            bool HeaderStayed()=>Vector3.Distance(header.position,originalHeader)<.5f;
            if(!HeaderStayed()||Vector3.Distance(scroll.content.position,originalContent)<10)
                Debug.LogError("MONSTER: reward scroll did not retain the selected creature and tabs");
            var savedPosition=header.position;header.position+=Vector3.down*20;
            if(HeaderStayed())Debug.LogError("MONSTER: pinned-header guard accepted a shifted header");
            header.position=savedPosition;
            yield return Shot("Monster-rewards-scrolled");scroll.verticalNormalizedPosition=1;
        }
        tabs.SetValue(screen,0);Loc.Language="th";render.Invoke(screen,null);yield return Shot("Monster-thai");Loc.Language="en";
        // Search must match a recorded name; unknown names cannot leak through filtering.
        input.text="zzzz-no-monster";yield return null;input.text="";
        var filterButton=(Button)typeof(BestiaryWindow).GetField("_filterButton",flags).GetValue(screen);filterButton.onClick.Invoke();yield return null;
        yield return Shot("Monster-filter");
        var popup=screen.transform.Find("FilterDismiss/FilterOptions");
        if(popup==null)Debug.LogError("MONSTER: filter menu did not open");
        else {var options=popup.GetComponentsInChildren<Button>();options[2].onClick.Invoke();yield return null;}
        
        if(screen.transform.Find("MonsterGrid/Viewport/Content/NoMatches")==null)Debug.LogError("MONSTER: undiscovered filter includes discovered creatures");
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} monster states; errors: {_errors}. Isolated fixture only.");
    }
    IEnumerator HeaderMemberReview(HudBar bar)
    {
        var driver=UnityEngine.Object.FindFirstObjectByType<SimDriver>();
        string original=driver.Session.SaveJson();
        var save=Newtonsoft.Json.Linq.JObject.Parse(original);save["party_slots"]=2;
        var session=SimSession.Load(save.ToString(),SimSession.LoadCatalog());
        driver.Attach(session);driver.Paused=true;session.SetDebugAllowed(true);
        session.ApplyInput("{\"DebugGrantExemplarUnit\":{\"exemplar_id\":\"lapis\"}}");
        var lapis=ExemplarReadModel.Fetch(driver).Members.Find(m=>m.UnitDef=="lapis");
        if(lapis==null)throw new InvalidOperationException("HEADER: second member fixture missing.");
        session.ApplyInput("{\"DeployCharacter\":{\"char_id\":"+lapis.Id+"}}");
        bar.CloseAll();bar.Toggle("Cards");yield return new WaitForSecondsRealtime(.7f);
        var cards=UnityEngine.Object.FindFirstObjectByType<CardsWindow>();
        var header=cards.GetComponentInParent<UiWindow>().transform.Find("Panel/TitleBar");
        var button=header.Find("MemberPicker").GetComponent<Button>();
        if(!button.gameObject.activeInHierarchy)Debug.LogError("HEADER: multiple members have no picker.");
        if(UnityEngine.Object.FindFirstObjectByType<MemberTabs>()!=null)Debug.LogError("HEADER: removed global strip is still active.");
        // Confirm the interaction check rejects a disconnected production button.
        var listener=button.onClick;button.onClick=new Button.ButtonClickedEvent();button.onClick.Invoke();yield return null;
        if(HeaderPickerOpened())Debug.LogError("HEADER: disconnected-button sabotage escaped the interaction check.");
        button.onClick=listener;button.onClick.Invoke();yield return null;
        if(!HeaderPickerOpened())throw new InvalidOperationException("HEADER: member button did not open the native picker.");
        yield return Shot("Cards-member-picker");
        var picker=UnityEngine.Object.FindFirstObjectByType<UiPicker>();
        var option=picker.transform.Find("Panel/List/Viewport/Content").GetComponentsInChildren<Button>()[1];
        option.onClick.Invoke();yield return null;cards.Refresh();
        long subject=(long)typeof(CardsWindow).GetField("_entityId",BindingFlags.NonPublic|BindingFlags.Instance).GetValue(cards);
        if(PartySelection.SelectedId!=lapis.Id||subject!=lapis.Id)Debug.LogError("HEADER: card screen did not switch to the picked member.");
        yield return Shot("Cards-second-member");
        bar.CloseAll();bar.Toggle("Album");yield return null;bar.Toggle("Cards");yield return new WaitForSecondsRealtime(.7f);
        if(PartySelection.SelectedId!=lapis.Id)Debug.LogError("HEADER: member choice lost on page return.");
        bar.CloseAll();driver.Attach(SimSession.Load(original,SimSession.LoadCatalog()));driver.Paused=true;
    }
    static bool HeaderPickerOpened()
    {
        var picker=UnityEngine.Object.FindFirstObjectByType<UiPicker>();
        return picker!=null&&picker.transform.Find("Panel/List/Viewport/Content").GetComponentsInChildren<Button>().Length==2;
    }
    IEnumerator Drive()
    {
        var bar = HudBar.Create(transform);
        yield return null;
        var menu=(Button)typeof(HudBar).GetField("_menuBtn",BindingFlags.NonPublic|BindingFlags.Instance).GetValue(bar);
        menu.onClick.Invoke();
        yield return Shot("System-menu");
        menu.onClick.Invoke();
        var entries = (Array)typeof(HudBar).GetField("Entries",BindingFlags.NonPublic|BindingFlags.Static).GetValue(null);
        foreach(var entry in entries)
        {
            string id = (string)entry.GetType().GetField("Label").GetValue(entry);
            if(id=="Debug"||id=="Kitchen")Debug.LogError("Retired page remains in player navigation: "+id);
            bar.CloseAll();
            bar.Toggle(id);
            yield return Shot(id);
            // Every regular page: a resize must not refetch data, and return must retain scroll.
            UiWindow current=null;foreach(var w in UnityEngine.Object.FindObjectsByType<UiWindow>(FindObjectsSortMode.None))if(w.Title==GameText.ScreenName(id)&&w.IsOpen){current=w;break;}
            if(current!=null){int reopened=0;System.Action opened=()=>reopened++;current.Opened+=opened;
                current.SetOpen(true);if(reopened!=0)Debug.LogError("UX: active-page layout refetched "+id);
                current.Opened-=opened;
                foreach(var scroll in current.Body.GetComponentsInChildren<ScrollRect>()){
                    if(scroll.content.rect.height<=scroll.viewport.rect.height+20)continue;
                    scroll.verticalNormalizedPosition=.35f;yield return null;
                    current.SetOpen(false);current.SetOpen(true);yield return null;yield return null;
                    if(Mathf.Abs(scroll.verticalNormalizedPosition-.35f)>.08f)Debug.LogError("UX: return lost scroll on "+id);
                    scroll.verticalNormalizedPosition=1;break;
                }
            }
            if(id=="Provisions")
            {
                var provisions=UnityEngine.Object.FindFirstObjectByType<ProvisionsWindow>();
                var slot=provisions.GetComponentInChildren<Button>();slot.onClick.Invoke();yield return null;
                var picker=provisions.transform.Find("Picker");
                if(picker==null||!ProvisionsWindow.BlocksNavigation)Debug.LogError("UX: provision picker does not own navigation");
                else {
                    yield return Shot("Provisions-picker");
                    var cancel=picker.Find("Panel/Ghost_"+Loc.Get("UI_COMMON_CANCEL")).GetComponent<Button>();cancel.onClick.Invoke();
                    if(!ProvisionsWindow.BlocksNavigation)Debug.LogError("UX: picker close leaks navigation on the same frame");
                    yield return null;
                    if(ProvisionsWindow.BlocksNavigation||!provisions.gameObject.activeInHierarchy)Debug.LogError("UX: picker cancel closed its parent screen");
                    slot.onClick.Invoke();yield return null;
                    var keyboard=UnityEngine.InputSystem.InputSystem.AddDevice<UnityEngine.InputSystem.Keyboard>();
                    UnityEngine.InputSystem.InputSystem.QueueStateEvent(keyboard,new UnityEngine.InputSystem.LowLevel.KeyboardState(UnityEngine.InputSystem.Key.Escape));
                    yield return null;yield return null;
                    UnityEngine.InputSystem.InputSystem.QueueStateEvent(keyboard,new UnityEngine.InputSystem.LowLevel.KeyboardState());
                    yield return null;UnityEngine.InputSystem.InputSystem.RemoveDevice(keyboard);
                    if(ProvisionsWindow.BlocksNavigation||!provisions.gameObject.activeInHierarchy)Debug.LogError("UX: Escape did not dismiss only the provision picker");
                    slot.onClick.Invoke();yield return null;
                    provisions.transform.Find("Picker/Panel/BrowseSupplies").GetComponent<Button>().onClick.Invoke();yield return null;
                    var supplyShop=UnityEngine.Object.FindFirstObjectByType<ShopWindow>();
                    if(supplyShop==null||(int)typeof(ShopWindow).GetField("_tab",BindingFlags.NonPublic|BindingFlags.Instance).GetValue(supplyShop)!=1)
                        Debug.LogError("UX: supplies action did not navigate to consumables");
                }
            }
            if(id=="Exemplar")
            {
                var collection=UnityEngine.Object.FindFirstObjectByType<ExemplarUnitsWindow>();
                var action=collection!=null?collection.GetComponentInChildren<Button>():null;
                if(action==null)Debug.LogError("UI GALLERY: empty collection has no next action");
                else {
                    action.onClick.Invoke();yield return null;
                    if(collection.gameObject.activeInHierarchy || UnityEngine.Object.FindFirstObjectByType<SummonWindow>()==null)
                        Debug.LogError("UI GALLERY: collection action did not open Excavation");
                }
            }
            if(id=="Stash")
            {
                var grid=UnityEngine.Object.FindFirstObjectByType<ItemGridWindow>();
                var emptyButton=(Button)typeof(ItemGridWindow).GetField("_emptyAction",BindingFlags.NonPublic|BindingFlags.Instance).GetValue(grid);
                if(emptyButton==null||!emptyButton.gameObject.activeInHierarchy)Debug.LogError("UI GALLERY: empty stash has no next action");
                else {
                    emptyButton.onClick.Invoke();yield return null;
                    if(grid.gameObject.activeInHierarchy)Debug.LogError("UI GALLERY: empty stash action did not navigate");
                    bar.Toggle("Stash");yield return null;
                }
                // Populate only this disposable fixture to verify the scrolling state.
                var session=UnityEngine.Object.FindFirstObjectByType<SimDriver>().Session;
                for(int i=0;i<50;i++) {
                    session.Debug("{\"grant_item\":{\"def_id\":\"shield_s1\"}}");
                    session.ApplyInput("{\"DepositToStash\":{\"inv_idx\":0}}");
                }
                grid.Refresh();yield return null;Canvas.ForceUpdateCanvases();
                var stashScroll=grid.GetComponentInChildren<ScrollRect>();
                if(stashScroll==null)Debug.LogError("UI GALLERY: stash has no scroll viewport");
                else {
                    stashScroll.verticalNormalizedPosition=0;
                    yield return Shot("Stash-bottom");
                    if(stashScroll.content.anchoredPosition.y<=0)Debug.LogError("UI GALLERY: stash cannot reach its last slots");
                }
            }
        }
        bar.CloseAll();
        bar.Floating.Open();
        foreach(var side in new[]{1,2,3,4,5})
        {
            bar.Floating.Side(side);
            yield return Shot("Inventory-side-"+side);
        }
        bar.Floating.Side(0);
        bar.Floating.SetFullStatsVisible(true);
        yield return Shot("Inventory-stats");
        bar.Floating.Close();
        bar.Toggle("Shop");
        yield return null;
        foreach(var window in UnityEngine.Object.FindObjectsByType<UiWindow>(FindObjectsSortMode.None))
        {
            if(!window.IsOpen)continue;
            var back=window.transform.Find("Panel/TitleBar/Ghost_‹").GetComponent<Button>();
            back.onClick.Invoke();
            if(window.IsOpen)Debug.LogError("UI GALLERY: back button did not close its screen");
            break;
        }
        bar.Toggle("Shop");
        yield return null;
        var shop=UnityEngine.Object.FindFirstObjectByType<ShopWindow>();
        var select=typeof(ShopWindow).GetMethod("Select",BindingFlags.NonPublic|BindingFlags.Instance);
        for(int category=1;category<6;category++)
        {
            select.Invoke(shop,new object[]{category});
            yield return Shot("Shop-category-"+category);
            var empty=shop.transform.GetComponentInChildren<ScrollRect>().content.Find("MenuNotice");
            if(empty!=null){
                var next=empty.GetComponentInChildren<Button>();
                if(next==null)Debug.LogError("UX: empty shop category has no next action");
                else {next.onClick.Invoke();yield return null;
                    if(shop.GetComponentInChildren<ScrollRect>().content.Find("MenuNotice")!=null)Debug.LogError("UX: browse available items remained empty");}
            }
        }
        select.Invoke(shop,new object[]{0});
        yield return null;
        var list=shop.GetComponentInChildren<ScrollRect>();
        if(list!=null)
        {
            list.verticalNormalizedPosition=0;
            yield return Shot("Shop-bottom");
            if(list.content.anchoredPosition.y<=0)Debug.LogError("UI GALLERY: shop cannot scroll to its last items");
        }
        int picked=0;
        UiPicker.Open("UX fixture",new[]{new UiPicker.Option{label="Choose",detail="Test choice",onPick=()=>picked++}});
        yield return null;
        var choicePicker=UnityEngine.Object.FindFirstObjectByType<UiPicker>();
        var option=choicePicker.transform.Find("Panel/List/Viewport/Content/Option").GetComponent<Button>();
        option.onClick.Invoke();option.onClick.Invoke();
        if(picked!=1||!UiPicker.BlocksNavigation)Debug.LogError("UX: picker duplicate/close-frame guard failed");
        yield return null;
        UiPicker.Open("UX fixture",new[]{new UiPicker.Option{label="Choose",onPick=()=>picked++}});yield return null;
        var escapeKeyboard=UnityEngine.InputSystem.InputSystem.AddDevice<UnityEngine.InputSystem.Keyboard>();
        UnityEngine.InputSystem.InputSystem.QueueStateEvent(escapeKeyboard,new UnityEngine.InputSystem.LowLevel.KeyboardState(UnityEngine.InputSystem.Key.Escape));yield return null;yield return null;
        UnityEngine.InputSystem.InputSystem.QueueStateEvent(escapeKeyboard,new UnityEngine.InputSystem.LowLevel.KeyboardState());yield return null;
        UnityEngine.InputSystem.InputSystem.RemoveDevice(escapeKeyboard);
        if(UnityEngine.Object.FindFirstObjectByType<UiPicker>()!=null||!shop.gameObject.activeInHierarchy||picked!=1)Debug.LogError("UX: picker Escape: picker="+(UnityEngine.Object.FindFirstObjectByType<UiPicker>()!=null)+" shop="+shop.gameObject.activeInHierarchy+" picked="+picked);
        int confirmed=0;
        UiConfirm.Ask(Loc.Get("UI_ITEM_DISCARD"),Loc.Get("UI_ITEM_DISCARD_HINT"),Loc.Get("UI_ITEM_DISCARD"),()=>confirmed++,true);
        yield return Shot("Confirmation");
        var confirm=UnityEngine.Object.FindFirstObjectByType<UiConfirm>();
        var safe=confirm.transform.Find("Panel/Ghost_"+Loc.Get("UI_COMMON_CANCEL")).gameObject;
        if(UnityEngine.EventSystems.EventSystem.current.currentSelectedGameObject!=safe)Debug.LogError("UX: confirmation did not focus cancel");
        var accept=confirm.transform.Find("Panel/Btn_"+Loc.Get("UI_ITEM_DISCARD")).GetComponent<Button>();
        accept.onClick.Invoke();accept.onClick.Invoke();
        if(confirmed!=1||confirm.gameObject.activeSelf)Debug.LogError("UX: confirmation can execute more than once");
        yield return null;
        UiConfirm.Ask(Loc.Get("UI_ITEM_DISCARD"),Loc.Get("UI_ITEM_DISCARD_HINT"),Loc.Get("UI_ITEM_DISCARD"),()=>confirmed++,true);
        confirm=UnityEngine.Object.FindFirstObjectByType<UiConfirm>();
        confirm.transform.Find("Panel/Ghost_"+Loc.Get("UI_COMMON_CANCEL")).GetComponent<Button>().onClick.Invoke();
        yield return null;
        if(confirmed!=1)Debug.LogError("UX: cancelling executed the action");
        if(_output.Contains("header-83"))yield return HeaderMemberReview(bar);
        bar.CloseAll();
        bar.gameObject.SetActive(false);
        bar.Floating.gameObject.SetActive(false);
        var canvas = new GameObject("GalleryCanvas",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler),typeof(GraphicRaycaster));
        canvas.transform.SetParent(transform,false);
        canvas.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;
        var scaler=canvas.GetComponent<CanvasScaler>();
        scaler.uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;
        scaler.referenceResolution=new Vector2(1440,900);
        string homeRoute=null;
        HomeMenuLayout.Build(canvas.transform,id=>homeRoute=id);
        // Cubism initializes/render-target uploads across several frames on a cold run.
        yield return new WaitForSecondsRealtime(1f);
        yield return Shot("Home");
        var home=canvas.GetComponentInChildren<HomeMenuLayout>();
        home.transform.Find("SafeContent/Expedition/Combat").GetComponent<Button>().onClick.Invoke();
        if(homeRoute!="Combat")Debug.LogError("UI GALLERY: home battle action did not navigate");
        home.transform.Find("SafeContent/BottomDock/Party").GetComponent<Button>().onClick.Invoke();
        if(homeRoute!="Party")Debug.LogError("UI GALLERY: team requires an extra navigation step");
        home.transform.Find("SafeContent/BottomDock/Inventory").GetComponent<Button>().onClick.Invoke();
        if(homeRoute!="Inventory")Debug.LogError("UI GALLERY: inventory requires an extra navigation step");
        typeof(HomeMenuLayout).GetMethod("Navigate",BindingFlags.NonPublic|BindingFlags.Instance).Invoke(home,new object[]{"Workshop"});
        yield return Shot("Home-navigation");
        var overlay=home.transform.Find("GroupOverlay");
        if(overlay==null && homeRoute!="Apothecary")Debug.LogError("UI GALLERY: Workshop did not open Apothecary directly");
        else if(overlay!=null) {
            overlay.GetComponent<Button>().onClick.Invoke();yield return null;
            if(home.transform.Find("GroupOverlay")!=null)Debug.LogError("UI GALLERY: backdrop did not dismiss group");
        }
        foreach(Transform child in canvas.transform) child.gameObject.SetActive(false);
        var exemplars=ExemplarScreen.Create(canvas.transform,()=>{});
        exemplars.Refresh();
        yield return new WaitForSecondsRealtime(1f);
        foreach(var page in new[]{"Attributes","Equipment","Skills","Regalia","Resonance","Apex"})
        {
            exemplars.SelectPage(page);
            if(exemplars.CurrentPage!=page)Debug.LogError("UI GALLERY: page did not open: "+page);
            yield return Shot("Exemplar-"+page);
            if(page=="Apex")
            {
                var apex=exemplars.GetComponentInChildren<ExemplarApexPage>();
                var allocation=apex.GetComponentInChildren<ScrollRect>();
                allocation.verticalNormalizedPosition=0;yield return null;
                yield return Shot("Exemplar-Apex-lower");
            }
        }
        if(_output.Contains("spacing-64"))
        {
            var fixtureDriver=UnityEngine.Object.FindFirstObjectByType<SimDriver>();
            var fixture=fixtureDriver.Session;
            fixture.Debug("{\"grant_item\":{\"def_id\":\"regalia_hearthguard\"}}");
            // Load an isolated presentation fixture; no player save or server is involved.
            var snapshot=Newtonsoft.Json.Linq.JObject.Parse(fixture.SaveJson());
            var bag=(Newtonsoft.Json.Linq.JArray)snapshot["inventory"];
            Newtonsoft.Json.Linq.JToken carrier=null;
            foreach(var item in bag)if((string)item["def_id"]=="regalia_hearthguard"){carrier=item;break;}
            if(carrier!=null){carrier.Remove();snapshot["owned_regalia"]=new Newtonsoft.Json.Linq.JArray(carrier);}
            else Debug.LogError("SPACING: Regalia fixture is missing its carrier.");
            var stocked=SimSession.Load(snapshot.ToString(),SimSession.LoadCatalog());
            if(stocked==null)throw new InvalidOperationException("Spacing fixture did not load.");
            fixtureDriver.Attach(stocked);fixtureDriver.Paused=true;
            exemplars.Refresh();exemplars.SelectPage("Regalia");
            yield return Shot("Exemplar-Regalia-populated");
            var regalia=exemplars.GetComponentInChildren<ExemplarRegaliaPage>();
            var info=regalia.transform.Find("RegaliaDetails/EffectsScroll").GetComponent<ScrollRect>();
            info.verticalNormalizedPosition=0;yield return null;
            yield return Shot("Exemplar-Regalia-effects");
            exemplars.SelectPage("Skills");
            var potential=exemplars.GetComponentInChildren<PotentialWindow>();
            potential.transform.Find("Details/DetailScroll").GetComponent<ScrollRect>().verticalNormalizedPosition=0;
            yield return Shot("Exemplar-Skills-lower");
        }
        exemplars.SelectPage("Attributes");
        yield return VerifyAtmosphere();
        var board=UiTheme.Flat(canvas.transform,"IconReview",UiTheme.Panel);
        board.rectTransform.anchorMin=Vector2.zero;board.rectTransform.anchorMax=Vector2.one;board.rectTransform.offsetMin=board.rectTransform.offsetMax=Vector2.zero;
        string[] icons={"Exemplar","Party","Inventory","Stash","Workshop","Equipment","Apothecary","Provisions","Shop","Trade","Crystal","Mail","Daily","Weekly","Pass","Commander","Outpost","Record","Story","Album","Bestiary","Map","Dungeon","Cards","Regalia","Skills","Paths","Apex","Resonance","Options","Events","Summon","Combat"};
        for(int i=0;i<icons.Length;i++){
            var cell=new GameObject(icons[i],typeof(RectTransform));cell.transform.SetParent(board.transform,false);
            var cr=(RectTransform)cell.transform;cr.anchorMin=new Vector2((i%7)/7f,1-(i/7+1)/5f);cr.anchorMax=new Vector2((i%7+1)/7f,1-(i/7)/5f);cr.offsetMin=new Vector2(14,8);cr.offsetMax=new Vector2(-14,-8);
            var go=new GameObject("Glyph",typeof(RectTransform));go.transform.SetParent(cr,false);var icon=go.AddComponent<HomeIcon>();icon.Kind=icons[i];icon.color=UiTheme.TextHigh;icon.raycastTarget=false;
            icon.rectTransform.anchorMin=new Vector2(.23f,.36f);icon.rectTransform.anchorMax=new Vector2(.77f,.92f);icon.rectTransform.offsetMin=icon.rectTransform.offsetMax=Vector2.zero;
            var small=new GameObject("Small",typeof(RectTransform));small.transform.SetParent(cr,false);var tiny=small.AddComponent<HomeIcon>();tiny.Kind=icons[i];tiny.color=UiTheme.Accent;tiny.raycastTarget=false;tiny.rectTransform.anchorMin=tiny.rectTransform.anchorMax=new Vector2(.84f,.62f);tiny.rectTransform.sizeDelta=new Vector2(24,24);
            var label=UiTheme.Label(cr,"Name",icons[i],16,UiTheme.TextMid,TMPro.TextAlignmentOptions.Center);label.rectTransform.anchorMin=new Vector2(0,.07f);label.rectTransform.anchorMax=new Vector2(1,.30f);label.rectTransform.offsetMin=label.rectTransform.offsetMax=Vector2.zero;
        }
        yield return Shot("Icon-set");Destroy(board.gameObject);
        File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} screens; runtime errors: {_errors}. Fixture only; online data unavailable.");
        Debug.Log($"UI GALLERY COMPLETE: {_shots} screenshots, {_errors} runtime errors. {_output}");
    }
    IEnumerator VerifyAtmosphere()
    {
        var effect=UnityEngine.Object.FindFirstObjectByType<MenuAtmosphere>();
        if(effect==null){Debug.LogError("UI GALLERY: ambient background missing");yield break;}
        int preference=PlayerPrefs.GetInt(UiEntrance.ReducedMotionKey,0);
        Mesh mesh=null;
        try {
            PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,0);
            yield return new WaitForSecondsRealtime(.4f);
            Canvas.ForceUpdateCanvases();mesh=effect.canvasRenderer.GetMesh();
            var before=mesh.vertices;
            yield return new WaitForSecondsRealtime(.3f);
            Canvas.ForceUpdateCanvases();mesh=effect.canvasRenderer.GetMesh();
            if(before.Length==0 || before[0]==mesh.vertices[0])Debug.LogError("UI GALLERY: ambient light did not animate");
            if(effect.raycastTarget)Debug.LogError("UI GALLERY: decoration blocks input");
            if(!_output.Contains("compact")) {
                string motion=Path.Combine(_output,"motion");Directory.CreateDirectory(motion);
                for(int i=0;i<24;i++) {
                    yield return new WaitForSecondsRealtime(.25f);
                    yield return new WaitForEndOfFrame();
                    ScreenCapture.CaptureScreenshot(Path.Combine(motion,$"frame-{i:D2}.png"));
                }
            }
            PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,1);
            yield return new WaitForSecondsRealtime(.4f);
            Canvas.ForceUpdateCanvases();mesh=effect.canvasRenderer.GetMesh();before=mesh.vertices;
            yield return new WaitForSecondsRealtime(.3f);
            Canvas.ForceUpdateCanvases();mesh=effect.canvasRenderer.GetMesh();
            if(before.Length!=mesh.vertexCount || before[0]!=mesh.vertices[0])Debug.LogError("UI GALLERY: reduced motion still animates");
        } finally {PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey,preference);}
    }

}





