// Run only in an isolated two-character preview (no SessionBoot/save writer).
// Exercises real UI buttons; captures no production save and uses no online client.
using System;
using System.Collections;
using System.Linq;
using Roi.Sim;
using Roi.UI;
using TMPro;
using UnityEditor;
using UnityEngine;
using UnityEngine.UI;

public static class PotentialBoardProbe
{
    [MenuItem("ROI/Probes/Potential Board (isolated preview)")]
    public static void Run()
    {
        var driver = UnityEngine.Object.FindFirstObjectByType<SimDriver>();
        if (!EditorApplication.isPlaying || driver?.Session == null ||
            UnityEngine.Object.FindFirstObjectByType<SessionBoot>() != null)
            throw new InvalidOperationException("Use an isolated Play Mode preview without SessionBoot.");
        var one = Read(driver, 1); var two = Read(driver, 2);
        Require(one.unit_def == "seraphine" && two.unit_def == "rapis", "Expected Seraphine + Rapis preview.");
        Require(one.skill_points_owned >= 5 && two.skill_points_owned >= 3, "Preview needs points for both routes.");
        driver.Paused = true;
        driver.StartCoroutine(Check(driver));
    }

    static IEnumerator Check(SimDriver driver)
    {
        var window = UnityEngine.Object.FindFirstObjectByType<PotentialWindow>();
        Require(window != null, "Open Potential Board first.");
        foreach (long member in new long[] { 1, 2 })
        {
            PartySelection.Select(member); window.Refresh(); yield return null;
            var baseline = Read(driver, member);
            var other = driver.Session.QueryJson("potential:" + (3 - member));
            string savedBeforePreview = driver.Session.SaveJson();
            foreach (var node in baseline.nodes)
            {
                Press(window, "Node_" + node.id); yield return null;
                var desc = window.GetComponentsInChildren<TextMeshProUGUI>(true).Single(t => t.name == "Description");
                Require(!string.IsNullOrWhiteSpace(desc.text), "Empty preview " + node.id);
                var selected = typeof(PotentialWindow).GetField("_selected", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic);
                Require((string)selected.GetValue(window) == node.id, "Selection mismatch " + node.id);
                Require(Button(window, "Upgrade").interactable == node.can_upgrade, "Wrong upgrade gate " + node.id);
            }
            Require(driver.Session.SaveJson() == savedBeforePreview, "Preview mutated save.");
            string target = member == 1 ? "A" : "B";
            Press(window, "Node_" + target); yield return null;
            var quote = Read(driver, member).nodes.Single(n => n.id == target);
            Require(quote.max_skill_point_cost > 0, "Missing route quote.");
            Press(window, "Max"); yield return null;
            var bought = Read(driver, member);
            Require(bought.skill_points_owned == baseline.skill_points_owned - quote.max_skill_point_cost, "Wrong route debit.");
            Require(bought.nodes.Single(n => n.id == target).rank == 1, "Route did not reach major.");
            Require(driver.Session.QueryJson("potential:" + (3 - member)) == other, "Other member changed.");
            string boughtView = driver.Session.QueryJson("potential:" + member);
            var restored = SimSession.Load(driver.Session.SaveJson(), SimSession.LoadCatalog());
            Require(restored != null, "Reload failed.");
            driver.Attach(restored); window.Refresh(); yield return null;
            Require(driver.Session.QueryJson("potential:" + member) == boughtView, "Reload lost board state.");
            Press(window, "Reset"); yield return null;
            Require(driver.Session.QueryJson("potential:" + member) == boughtView, "First reset click spent/refunded.");
            Press(window, "Reset"); yield return null;
            var reset = Read(driver, member);
            Require(reset.skill_points_owned == baseline.skill_points_owned && reset.reset_refund_points == baseline.reset_refund_points, "Wrong refund.");
            Require(driver.Session.QueryJson("potential:" + (3 - member)) == other, "Refund changed other member.");
            Debug.Log("POTENTIAL QA PASS " + baseline.unit_def + ": 18 previews, route, debit, load, two-click refund, member isolation.");
        }
        PartySelection.Select(1); window.Refresh();
        yield return null;
        Press(window, "Node_k1");
        Debug.Log("POTENTIAL QA COMPLETE: both characters passed.");
    }

    static Board Read(SimDriver d, long id) => JsonUtility.FromJson<Board>(d.Session.QueryJson("potential:" + id));
    static Button Button(PotentialWindow w, string name) => w.GetComponentsInChildren<Button>(true).Last(b => b.name == name);
    static void Press(PotentialWindow w, string name)
    {
        var button = Button(w, name);
        Require(button.interactable, "Disabled button " + name);
        button.onClick.Invoke();
    }
    static void Require(bool ok, string why) { if (!ok) throw new InvalidOperationException("POTENTIAL QA: " + why); }
    [Serializable] class Board { public string unit_def; public long skill_points_owned, reset_refund_points; public Node[] nodes; }
    [Serializable] class Node { public string id; public bool can_upgrade; public int rank, max_skill_point_cost; }
}
