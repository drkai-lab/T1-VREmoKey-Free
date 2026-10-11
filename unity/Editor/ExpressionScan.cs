using System.Collections.Generic;
using System.IO;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;

namespace VrcExpressionHotkeys.EditorTools
{
    /// <summary>
    /// Reads the FX controller and the avatar folder into the plain facts
    /// ExpressionRules classifies. Clips the FX controller references are
    /// included even when they live outside that folder.
    /// </summary>
    public static class ExpressionScan
    {
        public class Entry
        {
            public AnimationClip clip;
            public ExpressionRules.Verdict verdict;
            public bool fromFx;
        }

        public class Result
        {
            public AnimatorController controller;
            public UnityEngine.Object expressionParameters;
            public string folder = "";
            public readonly List<Entry> entries = new List<Entry>();
            public ExpressionRules.ControllerFact controllerFact = new ExpressionRules.ControllerFact();
            public List<ExpressionRules.ExprParamFact> expressionFacts;
            public int? syncedBits;
            public bool writeDefaults;
            public readonly List<string> handLayers = new List<string>();
            public string notice = "";
        }

        public static Result Gather(GameObject avatar)
        {
            var result = new Result();
            if (avatar == null)
            {
                result.notice = "err:need_avatar";
                return result;
            }
            result.controller = FindFx(avatar);
            result.expressionParameters = VrcOptional.TryExpressionParametersObject(avatar);
            result.folder = FolderOf(avatar, result.controller);
            var fromFx = new HashSet<AnimationClip>();
            if (result.controller != null)
            {
                foreach (var clip in result.controller.animationClips)
                {
                    if (clip != null)
                        fromFx.Add(clip);
                }
                result.controllerFact = Describe(result.controller);
                result.writeDefaults = result.controllerFact.writeOn > result.controllerFact.writeOff;
                foreach (var layer in result.controllerFact.layers)
                {
                    if (layer.name.ToLowerInvariant().Contains("hand"))
                        result.handLayers.Add(layer.name);
                }
            }
            else
            {
                result.notice = "err:no_fx_scan";
            }
            result.expressionFacts = DescribeParameters(result.expressionParameters);
            result.syncedBits = result.expressionFacts == null
                ? (int?)null
                : ExpressionRules.SyncedBits(result.expressionFacts);

            var seen = new HashSet<string>();
            var facts = new List<ExpressionRules.ClipFact>();
            var clips = new List<AnimationClip>();
            var origins = new List<bool>();
            AddClips(fromFx, true, seen, facts, clips, origins);
            AddClips(FolderClips(result.folder), false, seen, facts, clips, origins);
            var verdicts = ExpressionRules.ClassifyAll(facts);
            for (int i = 0; i < verdicts.Count; i++)
            {
                int source = IndexOfGuid(facts, verdicts[i].guid, verdicts[i].path);
                if (source < 0)
                    continue;
                verdicts[i].source = origins[source] ? "fx+folder" : "folder";
                result.entries.Add(new Entry
                {
                    clip = clips[source],
                    verdict = verdicts[i],
                    fromFx = origins[source]
                });
            }
            return result;
        }

        public static AnimatorController FindFx(GameObject avatar)
        {
            if (avatar == null)
                return null;
            foreach (var behaviour in avatar.GetComponents<MonoBehaviour>())
            {
                if (behaviour == null)
                    continue;
                var field = behaviour.GetType().GetField("baseAnimationLayers");
                if (field == null)
                    continue;
                var array = field.GetValue(behaviour) as System.Array;
                if (array == null)
                    continue;
                for (int i = 0; i < array.Length; i++)
                {
                    var layer = array.GetValue(i);
                    if (layer == null)
                        continue;
                    var typeField = layer.GetType().GetField("type");
                    var controllerField = layer.GetType().GetField("animatorController");
                    if (typeField == null || controllerField == null)
                        continue;
                    int type = (int)typeField.GetValue(layer);
                    if (type != 4)
                        continue;
                    var controller = controllerField.GetValue(layer) as AnimatorController;
                    if (controller != null)
                        return controller;
                }
            }
            var animator = avatar.GetComponent<Animator>();
            return animator != null ? animator.runtimeAnimatorController as AnimatorController : null;
        }

        public static ExpressionRules.ControllerFact Describe(AnimatorController controller)
        {
            var fact = new ExpressionRules.ControllerFact();
            if (controller == null)
                return fact;
            foreach (var parameter in controller.parameters)
            {
                string type = "float";
                if (parameter.type == AnimatorControllerParameterType.Bool)
                    type = "bool";
                else if (parameter.type == AnimatorControllerParameterType.Int)
                    type = "int";
                else if (parameter.type == AnimatorControllerParameterType.Trigger)
                    type = "trigger";
                fact.parameters.Add(new ExpressionRules.ParameterFact
                {
                    name = parameter.name,
                    type = type
                });
            }
            foreach (var layer in controller.layers)
            {
                fact.layers.Add(new ExpressionRules.LayerFact
                {
                    name = layer.name,
                    ours = LayerIsOurs(layer)
                });
                CountWriteDefaults(layer.stateMachine, fact);
            }
            return fact;
        }

        public static List<ExpressionRules.ExprParamFact> DescribeParameters(UnityEngine.Object asset)
        {
            if (asset == null)
                return null;
            var serialized = new SerializedObject(asset);
            var parameters = serialized.FindProperty("parameters");
            if (parameters == null || !parameters.isArray)
                return null;
            var items = new List<ExpressionRules.ExprParamFact>();
            for (int i = 0; i < parameters.arraySize; i++)
            {
                var element = parameters.GetArrayElementAtIndex(i);
                var name = element.FindPropertyRelative("name");
                var valueType = element.FindPropertyRelative("valueType");
                var synced = element.FindPropertyRelative("networkSynced");
                int typeIndex = 2;
                if (valueType != null)
                {
                    typeIndex = valueType.propertyType == SerializedPropertyType.Enum
                        ? valueType.enumValueIndex
                        : valueType.intValue;
                }
                string type = typeIndex == 0 ? "int" : typeIndex == 1 ? "float" : "bool";
                items.Add(new ExpressionRules.ExprParamFact
                {
                    name = name != null ? name.stringValue : "",
                    valueType = typeIndex,
                    synced = synced == null || synced.boolValue,
                    type = type
                });
            }
            return items;
        }

        static void AddClips(
            IEnumerable<AnimationClip> source,
            bool fromFx,
            HashSet<string> seen,
            List<ExpressionRules.ClipFact> facts,
            List<AnimationClip> clips,
            List<bool> origins)
        {
            if (source == null)
                return;
            foreach (var clip in source)
            {
                if (clip == null)
                    continue;
                string path = AssetDatabase.GetAssetPath(clip);
                string guid = string.IsNullOrEmpty(path) ? "" : AssetDatabase.AssetPathToGUID(path);
                string key = string.IsNullOrEmpty(guid) ? "id:" + clip.GetInstanceID() : guid;
                if (!seen.Add(key))
                    continue;
                facts.Add(FactOf(clip, path, guid));
                clips.Add(clip);
                origins.Add(fromFx);
            }
        }

        static ExpressionRules.ClipFact FactOf(AnimationClip clip, string path, string guid)
        {
            var fact = new ExpressionRules.ClipFact
            {
                name = clip.name,
                path = string.IsNullOrEmpty(path) ? clip.name : path,
                guid = guid
            };
            var bindings = AnimationUtility.GetCurveBindings(clip);
            for (int i = 0; i < bindings.Length; i++)
            {
                var binding = bindings[i];
                if (binding.type == typeof(Transform))
                    fact.otherMotion = true;
                var curve = AnimationUtility.GetEditorCurve(clip, binding);
                float value = 0f;
                if (curve != null)
                {
                    var keys = curve.keys;
                    for (int k = 0; k < keys.Length; k++)
                    {
                        if (Mathf.Abs(keys[k].value) >= Mathf.Abs(value))
                            value = keys[k].value;
                    }
                }
                int classId = 0;
                if (binding.type == typeof(SkinnedMeshRenderer))
                    classId = 137;
                else if (binding.type == typeof(Animator))
                    classId = 95;
                fact.curves.Add(new ExpressionRules.CurveFact
                {
                    attribute = binding.propertyName ?? "",
                    path = binding.path ?? "",
                    classId = classId,
                    value = value
                });
            }
            if (AnimationUtility.GetObjectReferenceCurveBindings(clip).Length > 0)
                fact.otherMotion = true;
            return fact;
        }

        static IEnumerable<AnimationClip> FolderClips(string folder)
        {
            if (string.IsNullOrEmpty(folder))
                yield break;
            string[] ids = AssetDatabase.FindAssets("t:AnimationClip", new[] { folder });
            for (int i = 0; i < ids.Length; i++)
            {
                string path = AssetDatabase.GUIDToAssetPath(ids[i]);
                var clip = AssetDatabase.LoadAssetAtPath<AnimationClip>(path);
                if (clip != null)
                    yield return clip;
            }
        }

        static string FolderOf(GameObject avatar, AnimatorController controller)
        {
            string path = PrefabUtility.GetPrefabAssetPathOfNearestInstanceRoot(avatar);
            if (string.IsNullOrEmpty(path))
                path = AssetDatabase.GetAssetPath(avatar);
            if (string.IsNullOrEmpty(path) && controller != null)
                path = AssetDatabase.GetAssetPath(controller);
            if (string.IsNullOrEmpty(path))
                return "";
            string folder = Path.GetDirectoryName(path);
            return string.IsNullOrEmpty(folder) ? "" : folder.Replace('\\', '/');
        }

        static int IndexOfGuid(List<ExpressionRules.ClipFact> facts, string guid, string path)
        {
            for (int i = 0; i < facts.Count; i++)
            {
                if (!string.IsNullOrEmpty(guid) && facts[i].guid == guid)
                    return i;
                if (facts[i].path == path)
                    return i;
            }
            return -1;
        }

        static bool LayerIsOurs(AnimatorControllerLayer layer)
        {
            if (layer.stateMachine == null || string.IsNullOrEmpty(layer.name))
                return false;
            if (!layer.name.StartsWith("Expression"))
                return false;
            return MachineIsOurs(layer.stateMachine);
        }

        static bool MachineIsOurs(AnimatorStateMachine machine)
        {
            if (machine == null)
                return false;
            foreach (var child in machine.states)
            {
                if (child.state != null && child.state.tag == ExpressionRules.LayerTag)
                    return true;
            }
            foreach (var child in machine.stateMachines)
            {
                if (MachineIsOurs(child.stateMachine))
                    return true;
            }
            return false;
        }

        static void CountWriteDefaults(AnimatorStateMachine machine, ExpressionRules.ControllerFact fact)
        {
            if (machine == null)
                return;
            foreach (var child in machine.states)
            {
                if (child.state == null)
                    continue;
                if (child.state.writeDefaultValues)
                    fact.writeOn++;
                else
                    fact.writeOff++;
            }
            foreach (var child in machine.stateMachines)
                CountWriteDefaults(child.stateMachine, fact);
        }
    }
}
