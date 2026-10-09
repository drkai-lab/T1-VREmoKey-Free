using NUnit.Framework;
using VrcExpressionHotkeys.EditorTools;

namespace VrcExpressionHotkeys.EditorTools.Tests
{
    public class ProfileDocumentTests
    {
        [Test]
        public void ExportCarriesNameAndAddress()
        {
            var row = new ProfileDocument.Row
            {
                index = 3,
                name = "Smile",
                parameter = "Smile",
                osc = "/avatar/parameters/Smile",
                thumb = "thumbs/3.png"
            };
            string json = ProfileDocument.Build("smile", "Smile", "avtr_smile", new[] { row });
            Assert.IsTrue(json.Contains("\"name\": \"Smile\""));
            Assert.IsTrue(json.Contains("\"osc\": \"/avatar/parameters/Smile\""));
            Assert.IsTrue(json.Contains("\"thumb\": \"thumbs/3.png\""));
            string product = "T" + "1";
            string keyField = "t" + "1" + "Key";
            Assert.IsFalse(json.Contains(product));
            Assert.IsFalse(json.Contains(keyField));
        }
    }
}
