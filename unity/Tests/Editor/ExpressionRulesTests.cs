using System.Collections.Generic;
using NUnit.Framework;
using VrcExpressionHotkeys.EditorTools;

namespace VrcExpressionHotkeys.EditorTools.Tests
{
    public class ExpressionRulesTests
    {
        static ExpressionRules.CurveFact Blend(string path, string shape, float value)
        {
            return new ExpressionRules.CurveFact
            {
                attribute = "blendShape." + shape,
                path = path,
                classId = 137,
                value = value
            };
        }

        static ExpressionRules.ClipFact Clip(string name, params ExpressionRules.CurveFact[] curves)
        {
            var clip = new ExpressionRules.ClipFact
            {
                name = name,
                path = name + ".anim",
                guid = name
            };
            for (int i = 0; i < curves.Length; i++)
                clip.curves.Add(curves[i]);
            return clip;
        }

        static string Kind(IList<ExpressionRules.Verdict> verdicts, string name)
        {
            for (int i = 0; i < verdicts.Count; i++)
            {
                if (verdicts[i].name == name)
                    return verdicts[i].kind;
            }
            return "";
        }

        [Test]
        public void ClassifiesFaceClipsAndKeepsTheReason()
        {
            var smile = Clip("Smile", Blend("Body", "joy", 100f));
            var blink = Clip("Blink", Blend("Body", "まばたき", 100f));
            var viseme = Clip("vrc.v_aa", Blend("Body", "vrc.v_aa", 100f));
            var mouth = Clip("mouse0", Blend("Body", "mouth_a", 0f));
            var hand = new ExpressionRules.ClipFact { name = "ok R", path = "ok.anim", guid = "hand" };
            hand.curves.Add(new ExpressionRules.CurveFact
            {
                attribute = "RightHand.Index.1 Stretched",
                path = "",
                classId = 95,
                value = 1f
            });
            var empty = new ExpressionRules.ClipFact { name = "Empty", path = "empty.anim", guid = "empty" };
            var face = Clip("DefaultFace", Blend("Body", "tooth", 5f));
            var flat = Clip("fistanimation0", Blend("Body", "tooth", 0f));
            var hat = new ExpressionRules.ClipFact
            {
                name = "Hat",
                path = "hat.anim",
                guid = "hat",
                otherMotion = true
            };
            var clips = new List<ExpressionRules.ClipFact>
            {
                smile, blink, viseme, mouth, hand, empty, face, flat, hat
            };
            var verdicts = ExpressionRules.ClassifyAll(clips);
            Assert.AreEqual("expression", Kind(verdicts, "Smile"));
            Assert.AreEqual("blink", Kind(verdicts, "Blink"));
            Assert.AreEqual("まばたき", ExpressionRules.Reasons["blink"]);
            Assert.AreEqual("viseme", Kind(verdicts, "vrc.v_aa"));
            Assert.AreEqual("mouth_reset", Kind(verdicts, "mouse0"));
            Assert.AreEqual("hand", Kind(verdicts, "ok R"));
            Assert.AreEqual("empty", Kind(verdicts, "Empty"));
            Assert.AreEqual("default_face", Kind(verdicts, "DefaultFace"));
            Assert.AreEqual("default_face", Kind(verdicts, "fistanimation0"));
            Assert.AreEqual("not_face", Kind(verdicts, "Hat"));
        }

        [Test]
        public void PlanPutsLayersAfterHands()
        {
            var controller = new ExpressionRules.ControllerFact();
            controller.layers.Add(new ExpressionRules.LayerFact { name = "Base" });
            controller.layers.Add(new ExpressionRules.LayerFact { name = "Left Right Hand" });
            controller.writeOn = 3;
            controller.writeOff = 1;
            var exprs = new List<ExpressionRules.ExprParamFact>
            {
                new ExpressionRules.ExprParamFact
                {
                    name = "FaceLock",
                    valueType = 2,
                    synced = true,
                    type = "bool"
                }
            };
            var plan = ExpressionRules.PlanInstall(new[] { "Smile", "Wink" }, controller, exprs);
            Assert.IsTrue(plan.ok, plan.error);
            Assert.IsTrue(plan.writeDefaults);
            Assert.AreEqual(2, plan.layers.Count);
            Assert.AreEqual("Expression1", plan.layers[0].name);
            Assert.AreEqual("If", plan.layers[0].onCondition);
            Assert.AreEqual("IfNot", plan.layers[0].offCondition);
            Assert.IsFalse(plan.layers[0].hasExitTime);
            Assert.AreEqual("end", plan.layers[0].placement);
            Assert.AreEqual("VREmoKey", plan.layers[0].tag);
            Assert.IsTrue(plan.layers[0].aboveHand);
            Assert.Greater(plan.layers[0].index, 1);
            Assert.AreEqual(2, plan.addedBits);

            controller.layers.Add(new ExpressionRules.LayerFact { name = "Expression1", ours = false });
            var blocked = ExpressionRules.PlanInstall(new[] { "Smile" }, controller, exprs);
            Assert.IsFalse(blocked.ok);
            Assert.AreEqual(0, blocked.layers.Count);
            Assert.AreEqual(0, blocked.parameters.Count);
        }

        [Test]
        public void PlanKeepsSlotNumbersAndUpdatesOurLayer()
        {
            var controller = new ExpressionRules.ControllerFact();
            controller.layers.Add(new ExpressionRules.LayerFact { name = "HandPose" });
            controller.layers.Add(new ExpressionRules.LayerFact { name = "Expression2", ours = true });
            controller.parameters.Add(new ExpressionRules.ParameterFact
            {
                name = "Expression2",
                type = "bool"
            });
            var exprs = new List<ExpressionRules.ExprParamFact>
            {
                new ExpressionRules.ExprParamFact
                {
                    name = "Expression2",
                    valueType = 2,
                    synced = true,
                    type = "bool"
                }
            };
            var plan = ExpressionRules.PlanInstall(
                new[] { "A", "B" },
                new[] { 2, 4 },
                controller,
                exprs);
            Assert.IsTrue(plan.ok, plan.error);
            Assert.AreEqual("Expression2", plan.parameters[0].name);
            Assert.IsTrue(plan.parameters[0].alreadyPresent);
            Assert.IsTrue(plan.parameters[0].expressionPresent);
            Assert.AreEqual("Expression4", plan.parameters[1].name);
            Assert.IsFalse(plan.parameters[1].alreadyPresent);
            Assert.AreEqual(1, plan.addedBits);
            Assert.IsTrue(plan.layers[0].alreadyPresent);
            Assert.IsTrue(plan.layers[1].aboveHand);
            Assert.Greater(plan.layers[1].index, plan.layers[0].index);
        }

        [Test]
        public void SyncLimitRefusesTheWholeInstall()
        {
            var exprs = new List<ExpressionRules.ExprParamFact>();
            for (int i = 0; i < 32; i++)
            {
                exprs.Add(new ExpressionRules.ExprParamFact
                {
                    name = "I" + i,
                    valueType = 0,
                    synced = true,
                    type = "int"
                });
            }
            var plan = ExpressionRules.PlanInstall(
                new[] { "Smile" },
                new ExpressionRules.ControllerFact(),
                exprs);
            Assert.IsFalse(plan.ok);
            Assert.AreEqual(0, plan.parameters.Count);
            Assert.AreEqual(0, plan.layers.Count);
            StringAssert.Contains("256", plan.error);
        }

        [Test]
        public void ProfileKeepsAJapaneseNameOnTheSlotParameter()
        {
            var row = new ProfileDocument.Row
            {
                index = 1,
                name = "笑顔",
                parameter = "Expression1",
                osc = "/avatar/parameters/Expression1",
                thumb = "thumbs/1.png"
            };
            string json = ProfileDocument.Build("mizuki", "瑞希", "avtr_mizuki", new[] { row });
            StringAssert.Contains("\"name\": \"笑顔\"", json);
            StringAssert.Contains("\"name\": \"瑞希\"", json);
            StringAssert.Contains("\"parameter\": \"Expression1\"", json);
            StringAssert.Contains("\"thumb\": \"thumbs/1.png\"", json);
        }
    }
}
