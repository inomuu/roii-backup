using System.Collections;
using UnityEditor;
using UnityEngine;

// Editor-only specimen using the same popup methods as combat, with no sim damage applied.
public static class DamageTypographyReview
{
    public static void Run()
    {
        var hud = Object.FindFirstObjectByType<Roi.Arena.ArenaHud>();
        var view = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
        if (!EditorApplication.isPlaying || hud == null || view == null) return;
        view.StartCoroutine(Capture(hud, view.GetComponentInChildren<Camera>()));
    }
    static IEnumerator Capture(Roi.Arena.ArenaHud hud, Camera cam)
    {
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        bool paused = driver != null && driver.Paused;
        try
        {
            // Do not let a death overlay cover the number samples.
            float readyBy = Time.unscaledTime + 8f;
            while (Object.FindFirstObjectByType<Roi.UI.DeathBanner>() != null)
            {
                if (paused || Time.unscaledTime > readyBy) yield break;
                yield return null;
            }
            if (driver != null) driver.Paused = true;
            // Let this tool invocation's frame finish before starting short-lived popups.
            yield return null;
            yield return new WaitForEndOfFrame();
            float start = Time.unscaledTime;
            hud.SpawnDamage(-7001, cam.ViewportToWorldPoint(new Vector3(.30f, .48f, 10f)), 29, false);
            hud.SpawnDamage(-7002, cam.ViewportToWorldPoint(new Vector3(.45f, .48f, 10f)), 128, true);
            hud.SpawnDamage(-7003, cam.ViewportToWorldPoint(new Vector3(.61f, .48f, 10f)), 9999, false);
            var multi = cam.ViewportToWorldPoint(new Vector3(.77f, .48f, 10f));
            hud.FeedMultiHit("typography-review", -7004, multi, 24, 3, 0f);
            hud.FeedMultiHit("typography-review", -7004, multi, 35, 3, .08f);
            hud.FeedMultiHit("typography-review", -7004, multi, 41, 3, .16f);
            while (Time.unscaledTime - start < .20f) yield return null;
            yield return new WaitForEndOfFrame();
            var image = ScreenCapture.CaptureScreenshotAsTexture();
            try { System.IO.File.WriteAllBytes("/tmp/damage-typography.png", image.EncodeToPNG()); }
            finally { Object.Destroy(image); }
            Debug.Log("DAMAGE TYPOGRAPHY REVIEW: normal hit, received hit, four digits and multi-hit captured");
        }
        finally { if (driver != null) driver.Paused = paused; }
    }
}
