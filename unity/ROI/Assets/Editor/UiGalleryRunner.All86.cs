using System;
using System.Collections;
using System.IO;
using System.Reflection;
using Roi.Sim;
using Roi.UI;
using Roi.Story;
using UnityEngine;
using UnityEngine.UI;
using UnityEngine.Rendering.Universal;

// Catalog-only presentation fixtures. Call the shipped renderers, never online actions.
// All callbacks on synthetic confirmations are no-ops and the session is disposable.
public sealed partial class UiGalleryRunner
{
    IEnumerator Menu88Drive()
    {
        var bar=HudBar.Create(transform);var modes=ModeTabs.Create(transform);yield return null;
        bar.Toggle("Menu");yield return Shot("Home-more");
        if(modes.IsCombat || bar.transform.Find("MenuPanel")!=null)Debug.LogError("Legacy menu was not replaced by Home.");
        Finish86();
    }
    const BindingFlags All86Flags=BindingFlags.Instance|BindingFlags.Public|BindingFlags.NonPublic;
    static object Get86(object target,string field)=>target.GetType().GetField(field,All86Flags).GetValue(target);
    static void Set86(object target,string field,object value)=>target.GetType().GetField(field,All86Flags).SetValue(target,value);
    static object Call86(object target,string method,params object[] args)=>target.GetType().GetMethod(method,All86Flags).Invoke(target,args);
    static T Find86<T>() where T:UnityEngine.Object => UnityEngine.Object.FindFirstObjectByType<T>();
    static void Close86(){var c=Find86<UiConfirm>();if(c!=null)Call86(c,"Close");}
    static void Raise86(){var c=Find86<UiConfirm>();var canvas=c.gameObject.AddComponent<Canvas>();canvas.overrideSorting=true;canvas.sortingOrder=100;c.gameObject.AddComponent<GraphicRaycaster>();}
    void Finish86()=>File.WriteAllText(Path.Combine(_output,"result.txt"),$"Captured {_shots} screens; errors: {_errors}. Disposable presentation fixtures; no account changes, network requests or purchases.");
    GameObject Canvas86()
    {
        var go=new GameObject("GalleryCanvas",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler),typeof(GraphicRaycaster));
        go.transform.SetParent(transform,false);go.GetComponent<Canvas>().renderMode=RenderMode.ScreenSpaceOverlay;
        var scale=go.GetComponent<CanvasScaler>();scale.uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;scale.referenceResolution=new Vector2(1440,900);
        return go;
    }
    IEnumerator Dialogs86Drive()
    {
        var bar=HudBar.Create(transform);yield return null;
        bar.Toggle("Inventory");yield return null;
        bar.CloseAll();var bag=bar.Floating;bag.Open();yield return null;
        var item=bag.Model.Bag[0];
        Call86(bag,"Inspect",item,0,0L,"",true);
        yield return Shot("Inventory-item-detail");
        bag.Side(FloatingInventoryHost.SideForge);bag.Model.PickGear(0);bag.RefreshNow();
        yield return Shot("Forge-selected");
        bag.Confirm(Loc.Get("UI_FORGE_GEM_WARNING"),()=>{});
        yield return Shot("Forge-gem-warning");Call86(bag,"CancelConfirmation");yield return null;
        bag.Result.Show(new ForgeModel.Receipt{kind="gear",status="success",gil=120,item=item});
        yield return Shot("Forge-success");bag.Result.Hide();
        bag.Result.Show(new ForgeModel.Receipt{kind="gear",status="failed"});
        yield return Shot("Forge-failure");bag.Result.Hide();
        bag.Model.Driver.Session.Debug("{\"grant_item\":{\"def_id\":\"sword_s3\"}}");bag.RefreshNow();bag.Side(FloatingInventoryHost.SideDisenchant);
        bag.Model.Clear();Call86(bag,"AutoSelectDisenchant",1);Call86(bag,"ConfirmDisenchantBatch");
        yield return Shot("Disenchant-confirm");Call86(bag,"CancelConfirmation");bag.Close();
        yield return HeaderMemberReview(bar);bar.CloseAll();
        bar.SetHomeMode(true);var canvas=Canvas86();var screen=ExemplarScreen.Create(canvas.transform,()=>{});screen.Refresh();screen.SelectPage("Equipment");
        yield return null;
        var equipment=Find86<ExemplarEquipmentPage>();Call86(equipment,"SelectSlot",1);Call86(equipment,"Primary");
        yield return Shot("Equipment-picker");
        // Put the common popup canvas above the Exemplar canvas, as the normal modal host is.
        UiConfirm.Ask(Loc.Get("UI_EX_GEAR_TRANSFER_TITLE"),Loc.Get("UI_EX_GEAR_TRANSFER_BODY").Replace("{item}",GameText.ItemName(item)).Replace("{owner}",GameText.UnitName("lapis")).Replace("{target}",GameText.UnitName("seraphine")),Loc.Get("UI_EX_GEAR_TRANSFER_CONFIRM"),()=>{},false);
        Raise86();yield return Shot("Equipment-transfer-confirm");Close86();yield return null;
        screen.SelectPage("Apex");
        UiConfirm.Ask(Loc.Get("UI_APEX_RESET"),Loc.Fmt("UI_EX_APEX_REFUND_BODY",GameText.UnitName("seraphine")),Loc.Get("UI_EX_APEX_REFUND_ACTION"),()=>{});
        Raise86();yield return Shot("Apex-refund-confirm");Close86();yield return null;
        screen.gameObject.SetActive(false);canvas.SetActive(false);
        bar.Toggle("Options");yield return null;
        foreach(var scroll in Find86<OptionsWindow>().GetComponentsInChildren<ScrollRect>())scroll.verticalNormalizedPosition=0;
        yield return Shot("Options-bottom");bar.CloseAll();
        Finish86();
    }
    IEnumerator Online86Drive()
    {
        var bar=HudBar.Create(transform);yield return null;
        if((_output.Contains("shop110")||(_output.Contains("shop111")||(_output.Contains("shop112")||_output.Contains("shop113"))))){
            bar.Toggle("Shop");yield return null;var shop=Find86<ShopWindow>();
            for(int category=0;category<6;category++){Call86(shop,"Select",category);yield return Shot("Shop-"+category);}
            Call86(shop,"Select",1);yield return null;
            var content=(RectTransform)Get86(shop,"_content");content.GetComponentInChildren<Button>().onClick.Invoke();yield return Shot("Shop-preview");Close86();Finish86();yield break;
        }

        if(_output.Contains("notice119")) {
            foreach(var input in UnityEngine.Object.FindObjectsByType<UnityEngine.EventSystems.EventSystem>(FindObjectsSortMode.None))input.enabled=false;
            foreach(var page in new[]{"Cards","Weekly","Trade","Stash","Pass"}) {
                bar.CloseAll();bar.Toggle(page);yield return Shot(page);
            }
            bar.CloseAll();bar.Toggle("Shop");yield return null;
            var shop=Find86<ShopWindow>();
            foreach(int category in new[]{4,5,1}) {
                Call86(shop,"Select",category);yield return Shot("Shop-"+category);
                var purchase=(Button)Get86(shop,"_purchase");
                if(category!=1&&purchase.gameObject.activeSelf)Debug.LogError("Empty category displays Buy.");
                if(category==1&&!purchase.gameObject.activeSelf)Debug.LogError("Populated category lost Buy.");
            }
            Finish86();yield break;
        }
        // These are fabricated renderer inputs, not server responses or actual account balances.
        string period="{\"ok\":true,\"day\":1,\"week\":1,\"now_ms\":0,\"resets_at_ms\":86400000,\"claimable_crystal\":10,\"points\":40,\"board\":[{\"id\":\"kills_2500\",\"target\":2500,\"progress\":2500,\"done\":true},{\"id\":\"rooms_10\",\"target\":10,\"progress\":10,\"done\":true},{\"id\":\"damage_1m\",\"target\":1000000,\"progress\":640000,\"done\":false},{\"id\":\"fuse_3\",\"target\":3,\"progress\":1,\"done\":false},{\"id\":\"train_1\",\"target\":1,\"progress\":0,\"done\":false},{\"id\":\"spend_gil\",\"target\":10000,\"progress\":3000,\"done\":false}],\"ladder\":[{\"points\":20,\"crystal\":10,\"reached\":true,\"claimed\":true},{\"points\":40,\"crystal\":10,\"reached\":true,\"claimed\":false},{\"points\":60,\"crystal\":10,\"reached\":false,\"claimed\":false},{\"points\":80,\"crystal\":10,\"reached\":false,\"claimed\":false},{\"points\":100,\"crystal\":10,\"reached\":false,\"claimed\":false}]}";
        foreach(var page in new[]{"Daily","Weekly"}){bar.CloseAll();bar.Toggle(page);yield return null;var w=Find86<PeriodWindow>();Set86(w,"_json",period);Call86(w,"Rebuild");yield return Shot(page+"-sample");}
        bar.CloseAll();bar.Toggle("Pass");yield return null;
        var pass=Find86<PassWindow>();Set86(pass,"_json","{\"ok\":true,\"season_id\":1,\"ends_at_ms\":2592000000,\"now_ms\":0,\"xp\":250,\"slot_cost\":100,\"level\":2,\"slots\":4,\"free_track\":[{\"slot\":1,\"kind\":\"crystal\",\"qty\":20},{\"slot\":2,\"kind\":\"item\",\"def_id\":\"shield_s1\",\"name\":\"Shield\",\"rarity\":\"Rare\",\"qty\":1},{\"slot\":3,\"kind\":\"crystal\",\"qty\":30},{\"slot\":4,\"kind\":\"crystal\",\"qty\":40}],\"claimed\":[1]}");Call86(pass,"Rebuild");yield return Shot("Pass-sample");
        foreach(var page in new[]{"Commander","Outpost"}){bar.CloseAll();bar.Toggle(page);yield return null;var w=Find86<CommanderWindow>();Set86(w,"_json","{\"ok\":true,\"level\":5,\"max_level\":50,\"reward_claimed_through\":3,\"cxp\":1250,\"into_level\":50,\"to_next\":250,\"outpost_preview\":120,\"outpost_cap\":1000,\"pending_reward_levels\":[4,5]}");Call86(w,"Rebuild");yield return Shot(page+"-sample");}
        bar.CloseAll();bar.Toggle("Events");yield return null;
        var events=Find86<EventsWindow>();Set86(events,"_json","{\"ok\":true,\"day\":1,\"now_ms\":0,\"tracks\":[{\"track\":\"login\",\"position\":2,\"claimable\":1,\"closes_at_ms\":604800000,\"cells\":[{\"id\":1,\"at\":1,\"state\":\"claimed\",\"reward\":[{\"kind\":\"crystal\",\"qty\":10}]},{\"id\":2,\"at\":2,\"state\":\"claimable\",\"reward\":[{\"kind\":\"crystal\",\"qty\":20}]},{\"id\":3,\"at\":3,\"state\":\"locked\",\"reward\":[{\"kind\":\"crystal\",\"qty\":30}]}]}]}");Call86(events,"Rebuild");yield return Shot("Events-sample");
        bar.CloseAll();bar.Toggle("Crystal");yield return null;
        var crystal=Find86<CrystalWindow>();Set86(crystal,"_catalog","{\"ok\":true,\"sandbox\":true,\"skus\":[{\"sku\":\"preview_small\",\"prices\":[{\"currency\":\"USD\",\"minor\":199}],\"crystal\":100,\"first_buy_bonus\":100},{\"sku\":\"preview_large\",\"prices\":[{\"currency\":\"USD\",\"minor\":999}],\"crystal\":600,\"first_buy_bonus\":600}]}");Set86(crystal,"_gacha","{\"tradeable\":\"500\",\"bound\":\"200\"}");Call86(crystal,"Rebuild");yield return Shot("Crystal-store-sample");
        bar.CloseAll();bar.Toggle("Mail");yield return null;
        Call86(Find86<MailWindow>(),"Rebuild","[{\"id\":1,\"kind\":\"gil\",\"reason\":\"Sample delivery\",\"gil\":120},{\"id\":2,\"kind\":\"item\",\"reason\":\"Sample item\",\"qty\":1,\"item\":{\"def_id\":\"shield_s1\",\"name\":\"Shield\",\"rarity\":\"Rare\"}}]");yield return Shot("Mail-sample");
        bar.CloseAll();bar.Toggle("Trade");yield return null;
        var trade=Find86<TradeWindow>();Set86(trade,"_driver",GetComponent<SimDriver>());
        for(int i=0;i<3;i++){var f=trade.GetType().GetField("_mode",All86Flags);f.SetValue(trade,Enum.ToObject(f.FieldType,i));Call86(trade,"Rebuild");yield return Shot("Trade-"+new[]{"buy","sell","orders"}[i]);}
        if(_output.Contains("trade105")||(_output.Contains("trade106")||(_output.Contains("trade107")||(_output.Contains("trade108")||_output.Contains("trade109")))))
        {
            var modeField=trade.GetType().GetField("_mode",All86Flags);
            modeField.SetValue(trade,Enum.ToObject(modeField.FieldType,0));
            Set86(trade,"_selDef","hp_pot_1");Set86(trade,"_selName","Red Phial");
            Set86(trade,"_bookJson","[{\"id\":1,\"qty\":20,\"unit_price\":500}]");
            Call86(trade,"Rebuild");yield return Shot("Trade-buy-selected");
            if((_output.Contains("trade106")||(_output.Contains("trade107")||(_output.Contains("trade108")||_output.Contains("trade109")))))
            {
                for(int n=0;n<5;n++)GetComponent<SimDriver>().Session.Debug("{\"grant_item\":{\"def_id\":\"hp_pot_1\"}}");
                modeField.SetValue(trade,Enum.ToObject(modeField.FieldType,1));
                Call86(trade,"Rebuild");yield return Shot("Trade-sell-selected");
            }
            if((_output.Contains("trade108")||_output.Contains("trade109")))
                for(int item=2;item<=4;item++)for(int n=0;n<item+1;n++)GetComponent<SimDriver>().Session.Debug("{\"grant_item\":{\"def_id\":\"hp_pot_"+item+"\"}}");
            modeField.SetValue(trade,Enum.ToObject(modeField.FieldType,2));
            Set86(trade,"_mineJson","[{\"id\":1,\"def_id\":\"hp_pot_1\",\"qty\":20,\"unit_price\":500},{\"id\":2,\"def_id\":\"hp_pot_2\",\"qty\":8,\"unit_price\":900}]");Set86(trade,"_selectedOrder",1L);
            Call86(trade,"Rebuild");yield return Shot("Trade-listing-selected");
        }
        var tradeRoot=(RectTransform)Get86(trade,"_root");Call86(trade,"ShowGear",(RectTransform)tradeRoot.parent);yield return null;
        var gear=(ItemMarketWindow)Get86(trade,"_gear");gear.Root.gameObject.SetActive(true);Set86(gear,"_driver",GetComponent<SimDriver>());
        Set86(gear,"_listings","[{\"id\":1,\"price\":250,\"item\":{\"def_id\":\"shield_s1\",\"name\":\"Shield\",\"rarity\":\"Rare\"}}]");
        for(int i=0;i<3;i++){var f=gear.GetType().GetField("_mode",All86Flags);f.SetValue(gear,Enum.ToObject(f.FieldType,i));Call86(gear,"Rebuild");yield return Shot("Item-market-"+new[]{"buy","sell","orders"}[i]);}
        Finish86();
    }
    IEnumerator System86Drive()
    {
        var title=TitleScreen.Create(transform,"Luna","");
        yield return Shot("Title");Call86(title,"OpenSignIn");yield return Shot("Sign-in");
        title.SetBusy(true,"Connecting…");yield return Shot("Sign-in-busy");title.SetBusy(false,"Connection unavailable");yield return Shot("Sign-in-error");
        title.ShowCreation(null);yield return Shot("Create-party");title.Dismiss();yield return null;
        File.WriteAllText(Path.Combine(_output,"progress.txt"),"title dismissed");
        // Build the real combat scene renderer around the already-owned disposable session.
        var arena=new GameObject("GalleryArena").AddComponent<Roi.Arena.ArenaView>();arena.AwaitBoot=true;
        yield return null;yield return null;
        File.WriteAllText(Path.Combine(_output,"progress.txt"),"arena started");
        Set86(arena,"_driver",GetComponent<SimDriver>());Call86(arena,"BuildBar");
        File.WriteAllText(Path.Combine(_output,"progress.txt"),"bar built");
        arena.FeedView(GetComponent<SimDriver>().Session.ViewJson());
        File.WriteAllText(Path.Combine(_output,"progress.txt"),"view fed");
        // Exercise the shipping renderer and actor shader; a gallery-only fallback conceals
        // incompatibilities with the PC deferred renderer.
        arena.enabled=false;
        yield return new WaitForSecondsRealtime(1);
        var materialReport=new System.Text.StringBuilder();
        foreach(var r in UnityEngine.Object.FindObjectsByType<Renderer>(FindObjectsSortMode.None))foreach(var m in r.sharedMaterials)if(m!=null){materialReport.AppendLine(r.name+" | "+m.name+" | "+m.shader.name+" supported="+m.shader.isSupported);foreach(var msg in UnityEditor.ShaderUtil.GetShaderMessages(m.shader))materialReport.AppendLine(msg.severity+":"+msg.message);}
        File.WriteAllText(Path.Combine(_output,"materials.txt"),materialReport.ToString());
        yield return Shot("Combat");
        var bar=Find86<HudBar>();bar.Dps.Toggle();yield return Shot("Combat-DPS");bar.Dps.Toggle();
        var clear=UnityEngine.Object.FindFirstObjectByType<ClearSummary>(FindObjectsInactive.Include);
        clear.OnView("{\"last_clear\":{\"instance_id\":\"gallery_clear\",\"ticks\":1230,\"kills\":20,\"xp\":100,\"gil\":50,\"failed\":false,\"loot\":[{\"def_id\":\"shield_s1\",\"quantity\":1}]}}");clear.enabled=false;
        yield return Shot("Dungeon-clear");clear.OnView("{\"last_clear\":{\"instance_id\":\"gallery_fail\",\"ticks\":420,\"kills\":3,\"xp\":10,\"gil\":0,\"failed\":true,\"loot\":[]}}");yield return Shot("Dungeon-failed");clear.gameObject.SetActive(false);
        var death=UnityEngine.Object.FindFirstObjectByType<DeathBanner>(FindObjectsInactive.Include);death.Bind(GetComponent<SimDriver>());death.OnView("{\"grid\":{\"entities\":[{\"team\":\"Player\",\"alive\":false,\"is_pet\":false}]}}");yield return Shot("Party-defeated");death.gameObject.SetActive(false);
        foreach(var uiCanvas in bar.GetComponentsInChildren<Canvas>())uiCanvas.enabled=false;
        var dialogue=DialogueBox.Create(transform);var script=DialogueScript.LoadFile(Path.Combine(Application.streamingAssetsPath,"story/prologue.dialogue"));
        dialogue.Play("gallery",script);dialogue.Skip();yield return Shot("Story-dialogue");dialogue.Clear();
        dialogue.Play("gallery",script,"break");dialogue.Skip();Call86(dialogue,"BuildChoices",script.Blocks["break"][2].Choices);yield return Shot("Story-choice");
        var choices=(RectTransform)Get86(dialogue,"_choicesRoot");
        var choiceButtons=choices.GetComponentsInChildren<Button>();
        if(choiceButtons.Length!=2 || Array.Exists(choiceButtons,b=>string.IsNullOrWhiteSpace(b.GetComponentInChildren<TMPro.TMP_Text>()?.text)))
            throw new InvalidOperationException("STORY CHOICES: both prologue answers must have visible text.");
        choiceButtons[0].onClick.Invoke();dialogue.Skip();yield return Shot("Story-choice-followup");dialogue.Clear();
        foreach(var uiCanvas in bar.GetComponentsInChildren<Canvas>())uiCanvas.enabled=true;
        SessionNotice.Warn("Not saved — retrying");yield return Shot("Session-save-warning");SessionNotice.Clear();
        SessionNotice.Ended("Session ended","This account is now active on another device. Nothing more will be saved from here; sign in again to continue on this one.",()=>{});yield return Shot("Session-ended");
        foreach(var notice in UnityEngine.Object.FindObjectsByType<SessionNotice>(FindObjectsInactive.Include,FindObjectsSortMode.None))Destroy(notice.gameObject);
        yield return null;
        bar.Companion.Toggle();yield return Shot("Companion-preview");bar.Companion.Toggle();
        Finish86();
    }
    IEnumerator More86Drive()
    {
        var bar=HudBar.Create(transform);yield return null;bar.Toggle("Inventory");yield return null;
        bar.CloseAll();var bag=bar.Floating;bag.Open();yield return null;bag.Side(FloatingInventoryHost.SideSocket);
        bag.Confirm(Loc.Get("UI_FLOAT_SOCKET_WARNING"),()=>{});yield return Shot("Socket-insert-confirm");Call86(bag,"CancelConfirmation");yield return null;
        bag.Confirm(Loc.Get("UI_FLOAT_REPLACE_WARNING"),()=>{});yield return Shot("Socket-replace-confirm");Call86(bag,"CancelConfirmation");yield return null;
        bag.Close();bar.CloseAll();bar.Toggle("Party");yield return null;
        UiConfirm.Ask("Fill to max","Spend Engrams to bring this unit to Lv 10.","Fill",()=>{},false);yield return Shot("Party-fill-confirm");Close86();yield return null;
        bar.CloseAll();bar.SetHomeMode(true);var canvas=Canvas86();var screen=ExemplarScreen.Create(canvas.transform,()=>{});screen.Refresh();screen.SelectPage("Regalia");
        UiConfirm.Ask(Loc.Get("UI_REGALIA_EQUIP_TITLE"),Loc.Fmt("UI_EXEMPLAR_REGALIA_TRANSFER","Memory Fragment",GameText.UnitName("lapis"),GameText.UnitName("seraphine")),Loc.Get("UI_COMMON_EQUIP"),()=>{},false);Raise86();yield return Shot("Regalia-transfer-confirm");Close86();yield return null;
        screen.gameObject.SetActive(false);canvas.SetActive(false);bar.SetHomeMode(false);bar.Toggle("Exemplar");yield return null;
        UiConfirm.Ask("Resonance","Spend one Resonance credit on Seraphine: 0 → 1 stars.\nThis cannot be undone.","Feed",()=>{},false);yield return Shot("Resonance-confirm");Close86();yield return null;
        Finish86();
    }
}
