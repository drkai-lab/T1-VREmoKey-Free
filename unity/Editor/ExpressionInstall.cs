using System;
using System.Collections.Generic;
using System.IO;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;

namespace VrcExpressionHotkeys.EditorTools
{
    /// <summary>
    /// Adds the free edition's bool parameters and FX layers, and can put the
    /// controller back from the backup taken before the first edit.
    /// </summary>
    public static class ExpressionInstall
    {
        const string BackupFolderName = "VREmoKeyBackup";

        public class Request
        {
            public AnimatorController controller;
            public UnityEngine.Object expressionParameters;
            public ExpressionRules.ControllerFact controllerFact;
            public List<ExpressionRules.ExprParamFact> expressionFacts;
            public readonly List<string> names = new List<string>();
            public readonly List<AnimationClip> clips = new List<AnimationClip>();
            public readonly List<int> slots = new List<int>();
        }

        public static ExpressionRules.InstallPlan Apply(Request request)
        {
            var plan = ExpressionRules.PlanInstall(
                request.names,
                request.slots.Count > 0 ? request.slots : null,
                request.controllerFact,
                request.expressionFacts);
            if (!plan.ok)
                return plan;
            if (request.controller == null)
            {
                plan.ok = false;
                plan.error = "err:no_fx";
                return plan;
            }
            string controllerPath = AssetDatabase.GetAssetPath(request.controller);
            Backup(controllerPath);
            if (request.expressionParameters != null)
                Backup(AssetDatabase.GetAssetPath(request.expressionParameters));

            Undo.RecordObject(request.controller, "Expression hotkeys");
            for (int i = 0; i < plan.parameters.Count; i++)
            {
                if (!plan.parameters[i].alreadyPresent)
                    request.controller.AddParameter(plan.parameters[i].name, AnimatorControllerParameterType.Bool);
            }
            var wanted = new HashSet<string>();
            for (int i = 0; i < plan.layers.Count; i++)
                wanted.Add(plan.layers[i].name);
            for (int i = 0; i < plan.layers.Count; i++)
            {
                if (!plan.layers[i].alreadyPresent && !HasLayer(request.controller, plan.layers[i].name))
                    request.controller.AddLayer(plan.layers[i].name);
            }
            var layers = request.controller.layers;
            for (int i = 0; i < layers.Length; i++)
            {
                if (!wanted.Contains(layers[i].name))
                    continue;
                int planIndex = IndexOf(plan, layers[i].name);
                var clip = planIndex >= 0 && planIndex < request.clips.Count ? request.clips[planIndex] : null;
                Configure(layers[i].stateMachine, clip, layers[i].name, plan.writeDefaults);
                layers[i].defaultWeight = 1f;
                layers[i].blendingMode = AnimatorLayerBlendingMode.Override;
            }
            request.controller.layers = MoveToEnd(layers, plan);
            EditorUtility.SetDirty(request.controller);

            if (request.expressionParameters != null)
                AddExpressionParameters(request.expressionParameters, plan);
            AssetDatabase.SaveAssets();
            return plan;
        }

        public static int Restore()
        {
            string folder = BackupRoot();
            string manifest = Path.Combine(folder, "manifest.txt");
            if (!File.Exists(manifest))
                return 0;
            int restored = 0;
            foreach (var line in File.ReadAllLines(manifest))
            {
                if (string.IsNullOrWhiteSpace(line))
                    continue;
                int split = line.IndexOf('\t');
                if (split <= 0)
                    continue;
                string backup = line.Substring(0, split);
                string assetPath = line.Substring(split + 1);
                if (!File.Exists(backup) || string.IsNullOrEmpty(assetPath))
                    continue;
                File.Copy(backup, Absolute(assetPath), true);
                AssetDatabase.ImportAsset(assetPath);
                restored++;
            }
            AssetDatabase.SaveAssets();
            return restored;
        }

        public static bool HasBackup()
        {
            return File.Exists(Path.Combine(BackupRoot(), "manifest.txt"));
        }

        static void Configure(AnimatorStateMachine machine, AnimationClip clip, string parameter, bool writeDefaults)
        {
            if (machine == null)
                return;
            var off = FindState(machine, "Off");
            var on = FindState(machine, "On");
            if (off == null)
                off = machine.AddState("Off", new Vector3(280f, 0f, 0f));
            if (on == null)
                on = machine.AddState("On", new Vector3(280f, 70f, 0f));
            machine.defaultState = off;
            var existing = machine.states;
            for (int i = existing.Length - 1; i >= 0; i--)
            {
                var state = existing[i].state;
                if (state != null && state != off && state != on)
                    machine.RemoveState(state);
            }
            off.tag = ExpressionRules.LayerTag;
            on.tag = ExpressionRules.LayerTag;
            off.writeDefaultValues = writeDefaults;
            on.writeDefaultValues = writeDefaults;
            on.motion = clip;
            machine.defaultState = off;
            ClearTransitions(off);
            ClearTransitions(on);
            var toOn = off.AddTransition(on);
            toOn.hasExitTime = false;
            toOn.duration = 0.05f;
            toOn.AddCondition(AnimatorConditionMode.If, 0f, parameter);
            var toOff = on.AddTransition(off);
            toOff.hasExitTime = false;
            toOff.duration = 0.05f;
            toOff.AddCondition(AnimatorConditionMode.IfNot, 0f, parameter);
        }

        static AnimatorState FindState(AnimatorStateMachine machine, string name)
        {
            var states = machine.states;
            for (int i = 0; i < states.Length; i++)
            {
                if (states[i].state != null && states[i].state.name == name)
                    return states[i].state;
            }
            return null;
        }

        static void ClearTransitions(AnimatorState state)
        {
            var transitions = state.transitions;
            for (int i = transitions.Length - 1; i >= 0; i--)
                state.RemoveTransition(transitions[i]);
        }

        static AnimatorControllerLayer[] MoveToEnd(AnimatorControllerLayer[] layers, ExpressionRules.InstallPlan plan)
        {
            var head = new List<AnimatorControllerLayer>();
            var tail = new List<AnimatorControllerLayer>();
            var wanted = new HashSet<string>();
            for (int i = 0; i < plan.layers.Count; i++)
                wanted.Add(plan.layers[i].name);
            for (int i = 0; i < layers.Length; i++)
            {
                if (wanted.Contains(layers[i].name))
                    tail.Add(layers[i]);
                else
                    head.Add(layers[i]);
            }
            var orderedTail = new List<AnimatorControllerLayer>();
            for (int i = 0; i < plan.layers.Count; i++)
            {
                for (int t = 0; t < tail.Count; t++)
                {
                    if (tail[t].name == plan.layers[i].name)
                    {
                        orderedTail.Add(tail[t]);
                        break;
                    }
                }
            }
            head.AddRange(orderedTail);
            return head.ToArray();
        }

        static void AddExpressionParameters(UnityEngine.Object asset, ExpressionRules.InstallPlan plan)
        {
            var serialized = new SerializedObject(asset);
            var parameters = serialized.FindProperty("parameters");
            if (parameters == null || !parameters.isArray)
                return;
            Undo.RecordObject(asset, "Expression hotkeys");
            for (int i = 0; i < plan.parameters.Count; i++)
            {
                if (plan.parameters[i].expressionPresent)
                    continue;
                int index = parameters.arraySize;
                parameters.InsertArrayElementAtIndex(index);
                var element = parameters.GetArrayElementAtIndex(index);
                var name = element.FindPropertyRelative("name");
                if (name != null)
                    name.stringValue = plan.parameters[i].name;
                var valueType = element.FindPropertyRelative("valueType");
                if (valueType != null)
                {
                    if (valueType.propertyType == SerializedPropertyType.Enum)
                        valueType.enumValueIndex = 2;
                    else
                        valueType.intValue = 2;
                }
                var synced = element.FindPropertyRelative("networkSynced");
                if (synced != null)
                    synced.boolValue = true;
                var saved = element.FindPropertyRelative("saved");
                if (saved != null)
                    saved.boolValue = false;
                var fallback = element.FindPropertyRelative("defaultValue");
                if (fallback != null)
                    fallback.floatValue = 0f;
            }
            serialized.ApplyModifiedProperties();
            EditorUtility.SetDirty(asset);
        }

        static bool HasLayer(AnimatorController controller, string name)
        {
            foreach (var layer in controller.layers)
            {
                if (layer.name == name)
                    return true;
            }
            return false;
        }

        static int IndexOf(ExpressionRules.InstallPlan plan, string name)
        {
            for (int i = 0; i < plan.layers.Count; i++)
            {
                if (plan.layers[i].name == name)
                    return i;
            }
            return -1;
        }

        static void Backup(string assetPath)
        {
            if (string.IsNullOrEmpty(assetPath))
                return;
            string source = Absolute(assetPath);
            if (!File.Exists(source))
                return;
            string folder = BackupRoot();
            Directory.CreateDirectory(folder);
            string guid = AssetDatabase.AssetPathToGUID(assetPath);
            if (string.IsNullOrEmpty(guid))
                guid = Path.GetFileName(assetPath);
            string dest = Path.Combine(folder, guid + "_" + Path.GetFileName(assetPath));
            if (!File.Exists(dest))
                File.Copy(source, dest);
            string manifest = Path.Combine(folder, "manifest.txt");
            string row = dest + "\t" + assetPath.Replace('\\', '/');
            string existing = File.Exists(manifest) ? File.ReadAllText(manifest) : "";
            if (!existing.Contains(row))
            {
                if (existing.Length > 0 && !existing.EndsWith("\n"))
                    existing += "\n";
                File.WriteAllText(manifest, existing + row + "\n");
            }
        }

        static string BackupRoot()
        {
            string project = Directory.GetParent(Application.dataPath).FullName;
            return Path.Combine(project, BackupFolderName, "original");
        }

        static string Absolute(string assetPath)
        {
            string project = Directory.GetParent(Application.dataPath).FullName;
            return Path.Combine(project, assetPath.Replace('/', Path.DirectorySeparatorChar));
        }
    }
}
