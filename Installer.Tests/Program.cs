using System.IO.Compression;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using DuckWorkshop.Setup;

var root = Path.GetFullPath(args.Single());
Directory.CreateDirectory(root);
int passed = 0;
void Check(bool condition, string message)
{
    if (!condition) throw new Exception(message);
}
async Task Reject(Func<Task> action)
{
    try { await action(); }
    catch (Exception e) when (e is IOException or InvalidDataException or ArgumentException or OperationCanceledException)
    { return; }
    throw new Exception("Expected installation to be rejected.");
}
(MemoryStream Zip, PayloadMetadata Metadata) Package(string version = "0.3.1")
{
    var stream = new MemoryStream();
    var metadata = new PayloadMetadata { Version = version };
    using (var zip = new ZipArchive(stream, ZipArchiveMode.Create, true))
    {
        foreach (var (path, text) in new[] {
            ("Duck-Workshop.exe", "verified executable " + version),
            ("docs/guide.txt", "installation guide") })
        {
            var bytes = Encoding.UTF8.GetBytes(text);
            using var output = zip.CreateEntry(path).Open();
            output.Write(bytes);
            metadata.Files[path] = Convert.ToHexString(SHA256.HashData(bytes));
        }
    }
    stream.Position = 0;
    return (stream, metadata);
}
Task Install(string path, MemoryStream zip, PayloadMetadata metadata,
    CancellationToken cancellation = default) =>
    InstallEngine.InstallAsync(path, zip, metadata, "", null,
        integrations: false, cancellation: cancellation);

// First install, repair/update, and uninstall keep unrelated user files.
var target = Path.Combine(root, "application");
var first = Package();
await Install(target, first.Zip, first.Metadata);
Check(InstallEngine.ReadRecord(target)?.AppId == "DuckWorkshop", "Wrong product marker.");
File.WriteAllText(Path.Combine(target, "my-project.duckworkshop"), "keep project");
var repair = Package("0.3.2");
await Install(target, repair.Zip, repair.Metadata);
Check(File.ReadAllText(Path.Combine(target, "my-project.duckworkshop")) == "keep project", "Repair lost a project.");
Check(InstallEngine.ReadRecord(target)?.Version == "0.3.2", "Repair did not update the version.");
passed++;

var older = Package("0.3.1");
await Reject(() => Install(target, older.Zip, older.Metadata));
Check(InstallEngine.ReadRecord(target)?.Version == "0.3.2", "Downgrade modified the install.");
passed++;

var broken = Package("0.3.3");
broken.Metadata.Files["Duck-Workshop.exe"] = new string('0', 64);
await Reject(() => Install(target, broken.Zip, broken.Metadata));
Check(File.ReadAllText(Path.Combine(target, "Duck-Workshop.exe")).Contains("0.3.2"), "Checksum failure replaced the application.");
passed++;

var unrelated = Path.Combine(root, "unrelated");
Directory.CreateDirectory(unrelated);
File.WriteAllText(Path.Combine(unrelated, "personal.txt"), "keep");
var normal = Package();
await Reject(() => Install(unrelated, normal.Zip, normal.Metadata));
Check(File.Exists(Path.Combine(unrelated, "personal.txt")), "Unknown folder was changed.");
passed++;

foreach (var relative in new[] { "../escape", "nested/../../escape", "C:/escape", "bad:name", "a//b", "a./b" })
    await Reject(() => { InstallEngine.SafePath(root, relative); return Task.CompletedTask; });
passed++;

var duplicate = Package();
duplicate.Zip.Position = 0;
using (var zip = new ZipArchive(duplicate.Zip, ZipArchiveMode.Update, true))
    zip.CreateEntry("Duck-Workshop.exe");
duplicate.Zip.Position = 0;
await Reject(() => Install(Path.Combine(root, "duplicate"), duplicate.Zip, duplicate.Metadata));
Check(!Directory.Exists(Path.Combine(root, "duplicate")), "Duplicate payload created a live install.");
passed++;

var symbolic = Package();
symbolic.Zip.Position = 0;
using (var zip = new ZipArchive(symbolic.Zip, ZipArchiveMode.Update, true))
    zip.Entries[0].ExternalAttributes = unchecked((int)0xA1FF0000);
symbolic.Zip.Position = 0;
await Reject(() => Install(Path.Combine(root, "symbolic"), symbolic.Zip, symbolic.Metadata));
passed++;

var incomplete = Package();
incomplete.Metadata.Files["missing.txt"] = new string('0', 64);
await Reject(() => Install(Path.Combine(root, "incomplete"), incomplete.Zip, incomplete.Metadata));
passed++;

var canceled = Package("0.3.3");
await Reject(() => Install(target, canceled.Zip, canceled.Metadata, new CancellationToken(true)));
Check(InstallEngine.ReadRecord(target)?.Version == "0.3.2", "Cancellation changed the live install.");
passed++;

var marker = Path.Combine(target, InstallEngine.Marker);
var originalRecord = File.ReadAllText(marker);
var installRecord = JsonSerializer.Deserialize<InstallRecord>(originalRecord, InstallEngine.Json)!;
installRecord.Files.Add("../personal.txt");
File.WriteAllText(marker, JsonSerializer.Serialize(installRecord));
await Reject(() => { InstallEngine.Uninstall(target, false); return Task.CompletedTask; });
Check(File.Exists(Path.Combine(target, "Duck-Workshop.exe")), "Uninstall deleted files before validating all paths.");
File.WriteAllText(marker, originalRecord);
passed++;

InstallEngine.Uninstall(target, false);
Check(File.ReadAllText(Path.Combine(target, "my-project.duckworkshop")) == "keep project", "Uninstall removed a project.");
Check(!File.Exists(Path.Combine(target, "Duck-Workshop.exe")), "Uninstall retained an owned executable.");
Check(!File.Exists(marker), "Uninstall retained the product marker.");
passed++;

Console.WriteLine($"{passed} installer checks passed. Test files: {root}");
