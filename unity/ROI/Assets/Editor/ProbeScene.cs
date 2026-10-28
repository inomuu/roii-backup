// WHICH SCENE THE PROBES STAND IN, named once.
//
// Twenty-nine probe files carried the literal ProbeScene.Path. That was fine while
// there was one zone scene and quietly wrong the moment there were three: the band's three
// connected rooms live in `Zone_r1_Painterly_3Rooms`, and a march written against 52 m spacing was
// being exercised in a scene that has one room and nowhere to march to. The suite stayed green
// because probes press controls and read the sim — they never look at the ground — so the wrong
// substrate is exactly the kind of mistake this suite cannot notice on its own.
//
// A constant does not stop that happening again, but it makes it ONE edit rather than
// twenty-nine, and it puts the decision somewhere a person can read it.
public static class ProbeScene
{
    public const string Path = "Assets/Scenes/Zone_r1_Painterly_3Rooms.unity";
}
