using System;
using System.Collections.Generic;
using UnityEngine;

namespace VrcExpressionHotkeys.EditorTools
{
    /// <summary>
    /// Reads VRChat descriptor fields through reflection so this package
    /// compiles in projects that do not have the VRChat SDK.
    /// </summary>
    public static class VrcOptional
    {
        /// <summary>
        /// True only when com.vrchat.avatars is installed. The editor still
        /// reads descriptor fields by reflection, so the package compiles
        /// when this is false.
        /// </summary>
        public static bool SdkCompiled
        {
            get
            {
#if VRC_SDK_VRCSDK3
                return true;
#else
                return false;
#endif
            }
        }

        public static string TryBlueprintId(GameObject avatar)
        {
            if (avatar == null)
                return "";
            foreach (var behaviour in avatar.GetComponents<MonoBehaviour>())
            {
                if (behaviour == null)
                    continue;
                var field = behaviour.GetType().GetField("blueprintId");
                if (field == null || field.FieldType != typeof(string))
                    continue;
                var value = field.GetValue(behaviour) as string;
                if (!string.IsNullOrEmpty(value))
                    return value;
            }
            return "";
        }

        public static UnityEngine.Object TryExpressionParametersObject(GameObject avatar)
        {
            if (avatar == null)
                return null;
            foreach (var behaviour in avatar.GetComponents<MonoBehaviour>())
            {
                if (behaviour == null)
                    continue;
                var field = behaviour.GetType().GetField("expressionParameters");
                if (field == null)
                    continue;
                var asset = field.GetValue(behaviour) as UnityEngine.Object;
                if (asset != null)
                    return asset;
            }
            return null;
        }

        public static string[] TryExpressionParameterNames(GameObject avatar)
        {
            if (avatar == null)
                return new string[0];
            foreach (var behaviour in avatar.GetComponents<MonoBehaviour>())
            {
                if (behaviour == null)
                    continue;
                var field = behaviour.GetType().GetField("expressionParameters");
                if (field == null)
                    continue;
                var asset = field.GetValue(behaviour) as UnityEngine.Object;
                if (asset == null)
                    continue;
                var parameters = asset.GetType().GetField("parameters");
                if (parameters == null || !(parameters.GetValue(asset) is Array array))
                    continue;
                var names = new List<string>();
                foreach (var item in array)
                {
                    if (item == null)
                        continue;
                    var nameField = item.GetType().GetField("name");
                    var name = nameField == null ? null : nameField.GetValue(item) as string;
                    if (!string.IsNullOrEmpty(name))
                        names.Add(name);
                }
                return names.ToArray();
            }
            return new string[0];
        }
    }
}
