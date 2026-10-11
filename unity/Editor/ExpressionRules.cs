using System;
using System.Collections.Generic;
using System.Text.RegularExpressions;

namespace VrcExpressionHotkeys.EditorTools
{
    /// <summary>
    /// Decisions shared with tools/expression_rules.py. No Unity types, so the
    /// same inputs classify the same way with or without the editor.
    /// </summary>
    public static class ExpressionRules
    {
        public const int SlotCount = 4;
        public const int SyncLimit = 256;
        public const string LayerTag = "VREmoKey";

        public static readonly Dictionary<string, string> Reasons = new Dictionary<string, string>
        {
            { "empty", "空のクリップ" },
            { "hand", "手のポーズだけ" },
            { "mouth_reset", "口リセット" },
            { "blink", "まばたき" },
            { "viseme", "Viseme" },
            { "default_face", "DefaultFace 相当" },
            { "not_face", "顔の blendShape を動かさない" },
            { "expression", "" }
        };

        static readonly string[] EyeHints =
        {
            "blink", "pupil", "iris", "highlight", "high_light", "eyeclose", "eye_close",
            "eyes_close", "まばたき", "瞳", "ハイライト"
        };

        static readonly string[] BlinkHints =
        {
            "blink", "まばたき", "eyeclose", "eye_close", "eyes_close", "目閉じ", "目を閉じ"
        };

        static readonly string[] MouthHints = { "mouth", "lip", "viseme", "vrc.v_", "口", "jaw" };

        static readonly string[] FacialHints =
        {
            "blink", "pupil", "iris", "highlight", "high_light", "eyeclose", "eye_close",
            "eyes_close", "まばたき", "瞳", "ハイライト", "mouth", "lip", "viseme", "vrc.v_",
            "口", "jaw", "brow", "眉", "笑", "頬", "cheek"
        };

        static readonly HashSet<string> VisemeTokens = new HashSet<string>
        {
            "sil", "pp", "ff", "th", "dd", "kk", "ch", "ss", "nn", "rr", "aa", "e", "ih", "oh", "ou"
        };

        public class CurveFact
        {
            public string attribute = "";
            public string path = "";
            public int classId;
            public float value;
        }

        public class ClipFact
        {
            public string name = "";
            public string path = "";
            public string guid = "";
            public readonly List<CurveFact> curves = new List<CurveFact>();
            public bool otherMotion;
        }

        public class Verdict
        {
            public string name = "";
            public string path = "";
            public string guid = "";
            public string kind = "empty";
            public bool include;
            public string reason = "";
            public string source = "";
        }

        public class ParameterFact
        {
            public string name = "";
            public string type = "bool";
        }

        public class LayerFact
        {
            public string name = "";
            public bool ours;
        }

        public class ControllerFact
        {
            public readonly List<ParameterFact> parameters = new List<ParameterFact>();
            public readonly List<LayerFact> layers = new List<LayerFact>();
            public int writeOn;
            public int writeOff;
        }

        public class ExprParamFact
        {
            public string name = "";
            public int valueType = 2;
            public bool synced = true;
            public string type = "bool";
        }

        public class ParamPlan
        {
            public string name = "";
            public string type = "bool";
            public bool synced = true;
            public bool alreadyPresent;
            public bool expressionPresent;
        }

        public class LayerPlan
        {
            public string name = "";
            public string parameter = "";
            public string clip = "";
            public bool writeDefaults;
            public bool alreadyPresent;
            public bool aboveHand;
            public int index;
            public string placement = "end";
            public string tag = LayerTag;
            public bool hasExitTime;
            public string onCondition = "If";
            public string offCondition = "IfNot";
        }

        public class InstallPlan
        {
            public bool ok;
            public string error = "";
            public bool writeDefaults;
            public int? syncedBits;
            public int addedBits;
            public int limit = SyncLimit;
            public int slotCount = SlotCount;
            public bool expressionParametersMissing;
            public readonly List<ParamPlan> parameters = new List<ParamPlan>();
            public readonly List<LayerPlan> layers = new List<LayerPlan>();
        }

        public static string ParameterName(int index)
        {
            return "Expression" + index;
        }

        public static string OscAddress(string parameter)
        {
            return "/avatar/parameters/" + parameter;
        }

        public static float WeightEpsilon(IList<float> values)
        {
            if (values != null)
            {
                for (int i = 0; i < values.Count; i++)
                {
                    if (Math.Abs(values[i]) > 1.5f)
                        return 8f;
                }
            }
            return 0.08f;
        }

        public static int SyncedBits(IList<ExprParamFact> items)
        {
            int total = 0;
            if (items == null)
                return total;
            for (int i = 0; i < items.Count; i++)
            {
                if (!items[i].synced)
                    continue;
                int valueType = items[i].valueType;
                total += valueType == 0 || valueType == 1 ? 8 : 1;
            }
            return total;
        }

        public static List<Verdict> ClassifyAll(IList<ClipFact> clips)
        {
            var verdicts = new List<Verdict>();
            if (clips == null)
                return verdicts;
            var values = new List<float>();
            for (int i = 0; i < clips.Count; i++)
            {
                var curves = clips[i].curves;
                for (int c = 0; c < curves.Count; c++)
                {
                    if (curves[c].attribute != null && curves[c].attribute.StartsWith("blendShape."))
                        values.Add(curves[c].value);
                }
            }
            float epsilon = values.Count > 0 ? WeightEpsilon(values) : 8f;
            var faces = DiscoverFacePaths(clips);
            ClipFact baseline = null;
            int baselineShapes = -1;
            for (int i = 0; i < clips.Count; i++)
            {
                if (!IsDefaultName(clips[i].name))
                    continue;
                int count = FaceWeights(clips[i], faces).Count;
                if (baseline == null || count > baselineShapes)
                {
                    baseline = clips[i];
                    baselineShapes = count;
                }
            }
            string baselineGuid = baseline != null ? baseline.guid ?? "" : "";
            var baselineMap = baseline != null
                ? FaceWeights(baseline, faces)
                : new Dictionary<string, float>();
            for (int i = 0; i < clips.Count; i++)
            {
                string kind = ClassifyClip(clips[i], faces, epsilon, baselineGuid, baselineMap);
                verdicts.Add(new Verdict
                {
                    name = clips[i].name,
                    path = clips[i].path,
                    guid = clips[i].guid,
                    kind = kind,
                    include = kind == "expression",
                    reason = Reasons.ContainsKey(kind) ? Reasons[kind] : kind
                });
            }
            verdicts.Sort((a, b) =>
            {
                int included = (a.include ? 0 : 1).CompareTo(b.include ? 0 : 1);
                if (included != 0)
                    return included;
                return string.Compare(a.path, b.path, StringComparison.Ordinal);
            });
            return verdicts;
        }

        public static string ClassifyClip(
            ClipFact clip,
            HashSet<string> facePaths,
            float epsilon,
            string baselineGuid,
            Dictionary<string, float> baseline)
        {
            var shapes = FaceWeights(clip, facePaths);
            bool hand = HasHand(clip);
            if (clip.curves.Count == 0 && !clip.otherMotion)
                return "empty";
            if (IsDefaultName(clip.name) || (!string.IsNullOrEmpty(baselineGuid) && clip.guid == baselineGuid))
                return "default_face";
            if (shapes.Count > 0 && AllQuiet(shapes, epsilon))
            {
                if (Mouthish(clip.name, shapes))
                    return "mouth_reset";
                if (hand && !clip.otherMotion)
                    return "hand";
                return "default_face";
            }
            var diff = Deltas(shapes, baseline, epsilon);
            if (diff.Count == 0)
            {
                if (shapes.Count > 0)
                    return "default_face";
                if (hand)
                    return "hand";
                if (clip.otherMotion || clip.curves.Count > 0)
                    return "not_face";
                return "empty";
            }
            bool blinkName = IsBlinkName(clip.name);
            bool allEye = true;
            bool anyBlink = blinkName;
            bool allViseme = true;
            for (int i = 0; i < diff.Count; i++)
            {
                string shape = ShapeOf(diff[i]);
                if (!IsEye(shape))
                    allEye = false;
                if (IsBlinkShape(shape))
                    anyBlink = true;
                if (!IsVisemeShape(shape))
                    allViseme = false;
            }
            if (allEye && anyBlink)
                return "blink";
            if (allViseme)
                return "viseme";
            return "expression";
        }

        public static InstallPlan PlanInstall(
            IList<string> chosen,
            ControllerFact controller,
            IList<ExprParamFact> expressionParameters)
        {
            return PlanInstall(chosen, null, controller, expressionParameters);
        }

        public static InstallPlan PlanInstall(
            IList<string> chosen,
            IList<int> slots,
            ControllerFact controller,
            IList<ExprParamFact> expressionParameters)
        {
            var refused = new InstallPlan
            {
                ok = false,
                expressionParametersMissing = expressionParameters == null
            };
            if (chosen != null && chosen.Count > SlotCount)
            {
                refused.error = "無料版は 4 スロットまでです";
                return refused;
            }
            int writeOn = controller != null ? controller.writeOn : 0;
            int writeOff = controller != null ? controller.writeOff : 0;
            bool writeDefaults = writeOn > writeOff;
            var layerRows = controller != null ? controller.layers : new List<LayerFact>();
            var parameters = controller != null ? controller.parameters : new List<ParameterFact>();
            int addedBits = 0;
            var plannedParams = new List<ParamPlan>();
            var plannedLayers = new List<LayerPlan>();
            int count = chosen != null ? chosen.Count : 0;
            var usedSlots = new HashSet<int>();
            for (int offset = 1; offset <= count; offset++)
            {
                int slot = slots != null ? slots[offset - 1] : offset;
                if (slot < 1 || slot > SlotCount || !usedSlots.Add(slot))
                {
                    refused.error = "無料版は 4 スロットまでです";
                    return refused;
                }
                string pname = ParameterName(slot);
                string animatorType = NamedType(parameters, pname);
                string exprType = ExprType(expressionParameters, pname);
                if ((animatorType != null && animatorType != "bool") || (exprType != null && exprType != "bool"))
                {
                    refused.error = pname + " は bool ではありません";
                    refused.writeDefaults = writeDefaults;
                    return refused;
                }
                LayerFact existing = null;
                for (int i = 0; i < layerRows.Count; i++)
                {
                    if (layerRows[i].name == pname)
                    {
                        existing = layerRows[i];
                        break;
                    }
                }
                if (existing != null && !existing.ours)
                {
                    refused.error = pname + " というレイヤーが既にあるため追加しません";
                    refused.writeDefaults = writeDefaults;
                    return refused;
                }
                if (expressionParameters != null && exprType == null)
                    addedBits += 1;
                plannedParams.Add(new ParamPlan
                {
                    name = pname,
                    type = "bool",
                    synced = true,
                    alreadyPresent = animatorType == "bool",
                    expressionPresent = exprType == "bool"
                });
                plannedLayers.Add(new LayerPlan
                {
                    name = pname,
                    parameter = pname,
                    clip = chosen[offset - 1] ?? "",
                    writeDefaults = writeDefaults,
                    alreadyPresent = existing != null,
                    tag = LayerTag
                });
            }
            int? bits = expressionParameters == null ? (int?)null : SyncedBits(expressionParameters);
            if (bits.HasValue && bits.Value + addedBits > SyncLimit)
            {
                refused.error = "同期ビットが " + SyncLimit + " を超えます (現在 " + bits.Value + "、追加 " + addedBits + ")";
                refused.syncedBits = bits;
                refused.addedBits = addedBits;
                refused.writeDefaults = writeDefaults;
                return refused;
            }
            var finalNames = new List<string>();
            var plannedNames = new HashSet<string>();
            for (int i = 0; i < plannedLayers.Count; i++)
                plannedNames.Add(plannedLayers[i].name);
            for (int i = 0; i < layerRows.Count; i++)
            {
                if (!plannedNames.Contains(layerRows[i].name))
                    finalNames.Add(layerRows[i].name);
            }
            for (int i = 0; i < plannedLayers.Count; i++)
                finalNames.Add(plannedLayers[i].name);
            var handIndexes = new List<int>();
            for (int i = 0; i < finalNames.Count; i++)
            {
                if (finalNames[i].ToLowerInvariant().Contains("hand"))
                    handIndexes.Add(i);
            }
            for (int i = 0; i < plannedLayers.Count; i++)
            {
                int index = finalNames.IndexOf(plannedLayers[i].name);
                plannedLayers[i].index = index;
                bool above = true;
                for (int h = 0; h < handIndexes.Count; h++)
                {
                    if (index <= handIndexes[h])
                        above = false;
                }
                plannedLayers[i].aboveHand = handIndexes.Count == 0 || above;
            }
            var plan = new InstallPlan
            {
                ok = true,
                writeDefaults = writeDefaults,
                syncedBits = bits,
                addedBits = addedBits,
                expressionParametersMissing = expressionParameters == null
            };
            plan.parameters.AddRange(plannedParams);
            plan.layers.AddRange(plannedLayers);
            return plan;
        }

        public static HashSet<string> DiscoverFacePaths(IList<ClipFact> clips)
        {
            var shapes = new Dictionary<string, HashSet<string>>();
            for (int i = 0; i < clips.Count; i++)
            {
                var curves = clips[i].curves;
                for (int c = 0; c < curves.Count; c++)
                {
                    if (curves[c].attribute == null || !curves[c].attribute.StartsWith("blendShape."))
                        continue;
                    string path = curves[c].path ?? "";
                    if (!shapes.ContainsKey(path))
                        shapes[path] = new HashSet<string>();
                    shapes[path].Add(curves[c].attribute.Substring("blendShape.".Length));
                }
            }
            var faces = new HashSet<string>();
            foreach (var pair in shapes)
            {
                if (PathIsFace(pair.Key) || ShapesLookFacial(pair.Value))
                    faces.Add(pair.Key);
            }
            return faces;
        }

        static Dictionary<string, float> FaceWeights(ClipFact clip, HashSet<string> facePaths)
        {
            var weights = new Dictionary<string, float>();
            if (clip == null)
                return weights;
            for (int i = 0; i < clip.curves.Count; i++)
            {
                var curve = clip.curves[i];
                if (curve.attribute == null || !curve.attribute.StartsWith("blendShape."))
                    continue;
                if (facePaths == null || !facePaths.Contains(curve.path ?? ""))
                    continue;
                string key = (curve.path ?? "") + "\n" + curve.attribute.Substring("blendShape.".Length);
                float previous;
                if (!weights.TryGetValue(key, out previous) || Math.Abs(curve.value) > Math.Abs(previous))
                    weights[key] = curve.value;
            }
            return weights;
        }

        static List<string> Deltas(
            Dictionary<string, float> shapes,
            Dictionary<string, float> baseline,
            float epsilon)
        {
            var found = new List<string>();
            if (shapes == null)
                return found;
            foreach (var pair in shapes)
            {
                float baseValue = 0f;
                if (baseline != null)
                    baseline.TryGetValue(pair.Key, out baseValue);
                if (Math.Abs(pair.Value - baseValue) >= epsilon)
                    found.Add(pair.Key);
            }
            return found;
        }

        static string ShapeOf(string key)
        {
            int split = key.IndexOf('\n');
            return split >= 0 ? key.Substring(split + 1) : key;
        }

        static bool AllQuiet(Dictionary<string, float> shapes, float epsilon)
        {
            foreach (var pair in shapes)
            {
                if (Math.Abs(pair.Value) >= epsilon)
                    return false;
            }
            return true;
        }

        static bool HasHand(ClipFact clip)
        {
            for (int i = 0; i < clip.curves.Count; i++)
            {
                string text = (clip.curves[i].attribute ?? "").ToLowerInvariant();
                if (text.Contains("lefthand") || text.Contains("righthand") || text.Contains("finger"))
                    return true;
            }
            return false;
        }

        static bool PathIsFace(string path)
        {
            string last = path ?? "";
            int slash = last.LastIndexOf('/');
            if (slash >= 0)
                last = last.Substring(slash + 1);
            last = last.ToLowerInvariant();
            if (last == "body" || last == "face" || last == "head" || last == "顔")
                return true;
            return last.Contains("face") || last.Contains("body") || last.Contains("顔");
        }

        static bool ShapesLookFacial(HashSet<string> names)
        {
            if (names == null || names.Count == 0)
                return false;
            int hits = 0;
            foreach (var name in names)
            {
                if (ContainsAny((name ?? "").ToLowerInvariant(), FacialHints))
                    hits++;
            }
            return hits >= 3 || (float)hits / names.Count >= 0.3f;
        }

        static bool IsDefaultName(string name)
        {
            string folded = Regex.Replace((name ?? "").ToLowerInvariant(), "[^a-z0-9]", "");
            return folded.Contains("defaultface") || folded.Contains("defaltface");
        }

        static bool IsBlinkName(string name)
        {
            return ContainsAny((name ?? "").ToLowerInvariant(), BlinkHints);
        }

        static bool IsEye(string name)
        {
            return ContainsAny((name ?? "").ToLowerInvariant(), EyeHints);
        }

        static bool IsBlinkShape(string name)
        {
            return ContainsAny((name ?? "").ToLowerInvariant(), BlinkHints);
        }

        static bool IsVisemeShape(string name)
        {
            string folded = (name ?? "").ToLowerInvariant();
            if (folded.StartsWith("vrc.v_") || folded.Contains("viseme"))
                return true;
            int cut = folded.IndexOfAny(new[] { '_', '-', ' ' });
            string token = cut >= 0 ? folded.Substring(0, cut) : folded;
            return VisemeTokens.Contains(token);
        }

        static bool Mouthish(string name, Dictionary<string, float> shapes)
        {
            string folded = Regex.Replace((name ?? "").ToLowerInvariant(), "[\\s_\\-]+", "");
            if (folded == "mouse0" || folded == "mouth0" || folded == "mouthreset" || (name ?? "").Contains("口リセット"))
                return true;
            foreach (var key in shapes.Keys)
            {
                if (ContainsAny(ShapeOf(key).ToLowerInvariant(), MouthHints))
                    return true;
            }
            return false;
        }

        static bool ContainsAny(string text, string[] hints)
        {
            for (int i = 0; i < hints.Length; i++)
            {
                if (text.Contains(hints[i]))
                    return true;
            }
            return false;
        }

        static string NamedType(IList<ParameterFact> parameters, string name)
        {
            if (parameters == null)
                return null;
            for (int i = 0; i < parameters.Count; i++)
            {
                if (parameters[i].name == name)
                    return parameters[i].type;
            }
            return null;
        }

        static string ExprType(IList<ExprParamFact> items, string name)
        {
            if (items == null)
                return null;
            for (int i = 0; i < items.Count; i++)
            {
                if (items[i].name == name)
                    return string.IsNullOrEmpty(items[i].type) ? "bool" : items[i].type;
            }
            return null;
        }
    }
}
