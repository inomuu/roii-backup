// Parses a `.dialogue` file into the block graph `DialogueBox` walks. Ported from
// godot/views/dialogue_script.gd, format and rules unchanged — the FILES are shared between the
// two clients, so a parser that disagreed about one of them would be a second dialect.
//
// The format is borrowed from Nathan Hoad's Dialogue Manager (MIT) because writers already read it
// at a glance, but nothing of that addon is used: no autoload, no second CSV, no expression
// evaluator. The subset here is deliberately small enough to hold in your head.
//
//   # a comment
//   ~ arrival                          a block, jumped to by name
//   The gate has not opened in years.  narration — no speaker
//   Aria: You are late. [ID:STORY_ARIA_001]
//   - I did not choose this [ID:STORY_ARIA_C1] => apologise
//   - And yet I am here [ID:STORY_ARIA_C2] => defy {aria:+1}
//   @ battle prologue_bridge           a CUE — see below
//   => resume                          jump; `=> END` finishes
//
// `[ID:KEY]` names the translation key; the inline text is the EN source an extractor lifts into
// strings.csv. A line WITHOUT an ID still plays, showing its inline text verbatim — so a writer's
// document can be dropped in and run the same day, before any localisation pass.
//
// `@ cue` is the one escape hatch, and it is deliberately not `do`: it emits a NAMED cue with
// string arguments and this file never resolves a method, a property or an expression. The host
// decides which cues exist and what they mean, so a script can start a battle or change a backdrop
// without a text file ever being able to reach into the game.
//
// ERRORS ARE COLLECTED, never thrown: a script with one bad jump should still play up to the bad
// jump, and the writer wants the whole list at once rather than the first one.
using System.Collections.Generic;
using System.Text.RegularExpressions;

namespace Roi.Story
{
    public sealed class DialogueScript
    {
        public const string Entry = "start";

        /// Header keys — data ABOUT the mission, as opposed to `@` cues, which are things that
        /// happen DURING it. The list screen has to read a title without playing the whole scene,
        /// and the two kinds are not the same sort of thing.
        ///
        /// `reward_text` is DISPLAY ONLY. What a player receives is granted by the server, keyed
        /// once per (account, mission); a file on the player's disk cannot be the authority on a
        /// reward without walking straight through docs/economy-invariants.md.
        public static readonly string[] KnownMeta =
            { "id", "requires", "title", "brief", "recommend", "reward_text" };

        /// The complete cue vocabulary. Declared HERE, next to the parser, because the alternative
        /// is the writer's file and the host's handler agreeing by memory — and a typo'd
        /// `@ battel` would then parse, play, and do nothing, which reads as "the story is broken"
        /// with nothing to point at.
        public static readonly string[] KnownCues =
            { "roster", "battle", "scene", "cutscene", "remove_member" };

        public readonly Dictionary<string, List<Line>> Blocks = new();
        public readonly Dictionary<string, string> Meta = new();
        public readonly List<string> Errors = new();

        /// One line of a block. Exactly one of `Goto`, `Cue`, `Choices` or a spoken line is set —
        /// a struct-with-a-kind rather than four classes, because that is what the Godot
        /// dictionary is and the walker branches on it either way.
        public sealed class Line
        {
            public string Goto;                 // a jump
            public string Cue;                  // a named cue
            public string[] Args;               // its arguments
            public List<Choice> Choices;        // a decision
            public string Speaker;              // "SPK_ARIA", or empty for narration
            public string SpeakerName;          // "Aria" — the fallback an un-keyed draft shows
            public string Text;                 // the translation KEY, or empty
            public string Raw;                  // the inline source, carried only when unkeyed
            public bool IsSay => Choices == null && Cue == null && Goto == null;
        }

        public sealed class Choice
        {
            public string Text, Raw, Goto, Effect;
        }

        static readonly Regex RxMeta = new(@"^%\s*(?<key>\S+)\s*(?<val>.*)$");
        static readonly Regex RxTitle = new(@"^~\s*(?<title>\S+)\s*$");
        static readonly Regex RxGoto = new(@"^=>\s*(?<target>\S+)\s*$");
        static readonly Regex RxChoice = new(@"^-\s*(?<body>.+)$");
        static readonly Regex RxCue = new(@"^@\s*(?<name>\S+)\s*(?<args>.*)$");
        static readonly Regex RxId = new(@"\s*\[ID:(?<id>[^\]]+)\]");
        static readonly Regex RxEffect = new(@"\s*\{(?<effect>[^}]*)\}\s*$");
        static readonly Regex RxJump = new(@"\s*=>\s*(?<target>\S+)\s*$");
        static readonly Regex RxSpeaker = new(@"^(?<who>[^:\[\]{}]{1,32}):\s*(?<line>.*)$");

        public static DialogueScript Parse(string src)
        {
            var d = new DialogueScript();
            var title = Entry;
            d.Blocks[title] = new List<Line>();
            int lineno = 0;

            foreach (var rawLine in (src ?? "").Split('\n'))
            {
                lineno++;
                var line = rawLine.Trim();
                if (line.Length == 0 || line.StartsWith("#")) continue;

                var mm = RxMeta.Match(line);
                if (mm.Success)
                {
                    var key = mm.Groups["key"].Value;
                    if (System.Array.IndexOf(KnownMeta, key) < 0)
                        d.Errors.Add($"line {lineno}: unknown header '% {key}' — known: "
                                   + string.Join(", ", KnownMeta));
                    else d.Meta[key] = mm.Groups["val"].Value.Trim();
                    continue;
                }

                var m = RxTitle.Match(line);
                if (m.Success)
                {
                    title = m.Groups["title"].Value;
                    if (d.Blocks.TryGetValue(title, out var existing) && existing.Count > 0)
                        d.Errors.Add($"line {lineno}: block '{title}' declared twice");
                    if (!d.Blocks.ContainsKey(title)) d.Blocks[title] = new List<Line>();
                    continue;
                }

                m = RxGoto.Match(line);
                if (m.Success) { d.Blocks[title].Add(new Line { Goto = m.Groups["target"].Value }); continue; }

                m = RxCue.Match(line);
                if (m.Success)
                {
                    var rest = m.Groups["args"].Value.Trim();
                    d.Blocks[title].Add(new Line
                    {
                        Cue = m.Groups["name"].Value,
                        Args = rest.Length == 0
                             ? new string[0]
                             : rest.Split(new[] { ' ' }, System.StringSplitOptions.RemoveEmptyEntries),
                    });
                    continue;
                }

                m = RxChoice.Match(line);
                if (m.Success)
                {
                    // Consecutive `- ` lines are ONE decision, not a run of separate ones, so they
                    // attach to the choice line already open rather than starting a new one.
                    var block = d.Blocks[title];
                    if (block.Count == 0 || block[block.Count - 1].Choices == null)
                        block.Add(new Line { Choices = new List<Choice>() });
                    block[block.Count - 1].Choices.Add(
                        ParseChoice(m.Groups["body"].Value, lineno, d.Errors));
                    continue;
                }

                d.Blocks[title].Add(ParseSay(line));
            }

            d.Validate();
            return d;
        }

        /// Read and parse a file. A missing file is an error in the list, never a crash — a
        /// prologue that has not been written yet must not take the game down.
        public static DialogueScript LoadFile(string path)
        {
            if (!System.IO.File.Exists(path))
            {
                var miss = new DialogueScript();
                miss.Errors.Add("no such file: " + path);
                return miss;
            }
            var d = Parse(System.IO.File.ReadAllText(path));
            // A FILE is a mission, so it must declare its id. `Parse` deliberately does not require
            // one — a tutorial hint or a mid-fight bark is a fragment, not a mission, and has
            // nothing to be paid for. The requirement belongs at the seam where "this is a mission"
            // becomes true.
            //
            // The id is the key the SERVER pays against, one row per (account, mission_id). Tying
            // it to the filename would make `git mv` an economic act: the old row keys a name
            // nothing claims any more, so a player who cleared `prologue` claims it again as
            // `c1_prologue`.
            if (!d.Meta.TryGetValue("id", out var id) || string.IsNullOrWhiteSpace(id))
                d.Errors.Add("missing '% id' — a mission file must declare the id its rewards "
                           + "are keyed to");
            return d;
        }

        static Choice ParseChoice(string body, int lineno, List<string> errors)
        {
            var effect = "";
            var m = RxEffect.Match(body);
            if (m.Success) { effect = m.Groups["effect"].Value.Trim(); body = body.Substring(0, m.Index).Trim(); }
            var goto_ = "END";
            m = RxJump.Match(body);
            if (m.Success) { goto_ = m.Groups["target"].Value; body = body.Substring(0, m.Index).Trim(); }
            else errors.Add($"line {lineno}: a choice needs a `=> target`");
            var said = ParseSay(body);
            return new Choice { Text = said.Text, Raw = said.Raw, Goto = goto_, Effect = effect };
        }

        /// Split a spoken or narrated line into speaker, translation key and inline source text.
        static Line ParseSay(string line)
        {
            var key = "";
            var m = RxId.Match(line);
            if (m.Success)
            {
                key = m.Groups["id"].Value.Trim();
                line = (line.Substring(0, m.Index) + line.Substring(m.Index + m.Length)).Trim();
            }
            var speaker = "";
            m = RxSpeaker.Match(line);
            if (m.Success)
            {
                // `SPK_<NAME>` so a speaker is translated like everything else. The bare name stays
                // as the fallback, which is what makes an un-keyed draft readable on screen.
                speaker = m.Groups["who"].Value.Trim();
                line = m.Groups["line"].Value.Trim();
            }
            return new Line
            {
                SpeakerName = speaker,
                Speaker = speaker.Length > 0 ? "SPK_" + speaker.ToUpperInvariant().Replace(" ", "_") : "",
                Text = key,
                // `Raw` is only carried when there is no key: a keyed line resolves through Loc and
                // the inline text is the extractor's source, not a runtime fallback.
                Raw = key.Length == 0 ? line : "",
            };
        }

        /// Every jump must land somewhere, and every block must end on one. A typo'd target would
        /// otherwise end the scene silently, which in a prologue reads as "the game skipped the
        /// story" rather than as an error.
        void Validate()
        {
            if (!Blocks.TryGetValue(Entry, out var entry) || entry.Count == 0)
                Errors.Add($"no `~ {Entry}` block — nothing would play");
            foreach (var kv in Blocks)
            {
                var block = kv.Value;
                if (block.Count > 0)
                {
                    var last = block[block.Count - 1];
                    if (last.Goto == null && last.Choices == null)
                        Errors.Add($"block '{kv.Key}' has no `=>` at its end — the script would "
                                 + "stop there");
                }
                foreach (var line in block)
                {
                    foreach (var target in Targets(line))
                        if (target != "END" && !Blocks.ContainsKey(target))
                            Errors.Add($"block '{kv.Key}' jumps to '{target}', which does not exist");
                    if (line.Cue != null && System.Array.IndexOf(KnownCues, line.Cue) < 0)
                        Errors.Add($"block '{kv.Key}' raises unknown cue '@ {line.Cue}' — known: "
                                 + string.Join(", ", KnownCues));
                }
            }
        }

        static IEnumerable<string> Targets(Line line)
        {
            if (line.Goto != null) { yield return line.Goto; yield break; }
            if (line.Choices == null) yield break;
            foreach (var c in line.Choices) yield return c.Goto ?? "END";
        }
    }
}
