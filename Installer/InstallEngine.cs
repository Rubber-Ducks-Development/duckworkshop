// Adapted from the MIT-licensed Duck Studio installer; see Installer/LICENSE.
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Security.Cryptography;
using System.Text.Json;
using Microsoft.Win32;

namespace DuckWorkshop.Setup;

public sealed class PayloadMetadata
{
    public string Version { get; set; } = "0.3.1";
    public Dictionary<string,string> Files { get; set; } = new(StringComparer.OrdinalIgnoreCase);
}
public sealed class InstallRecord
{
    public string AppId { get; set; } = "DuckWorkshop";
    public string Version { get; set; } = "";
    public List<string> Files { get; set; } = [];
    public string FeedUrl { get; set; } = "";
}
public static class InstallEngine
{
    public const string Marker="duck-workshop-install.json";
    public static readonly JsonSerializerOptions Json=new(){WriteIndented=true,PropertyNameCaseInsensitive=true};
    public static string DefaultFolder=>Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"Programs","Duck Workshop");
    public static string ResolveInstallFolder(string path)
    {
        var full=Path.GetFullPath(path).TrimEnd(Path.DirectorySeparatorChar,Path.AltDirectorySeparatorChar);
        var root=Path.GetPathRoot(full)!.TrimEnd(Path.DirectorySeparatorChar,Path.AltDirectorySeparatorChar);
        if(full==root||Directory.GetParent(full)==null)throw new IOException("Choose an application folder, not the root of a drive.");
        for(var info=new DirectoryInfo(full);info!=null;info=info.Parent)if(info.Exists&&(info.Attributes&FileAttributes.ReparsePoint)!=0)throw new IOException("Choose a normal folder rather than a linked or junction folder.");
        if(Directory.Exists(full)&&Directory.EnumerateFileSystemEntries(full).Any()&&ReadRecord(full)==null)throw new IOException("This folder contains unrelated files. Choose an empty folder or an existing Duck Workshop installation.");
        return full;
    }
    public static string SafePath(string root,string relative)
    {
        var parts=relative.Replace('\\','/').Split('/');
        if(string.IsNullOrWhiteSpace(relative)||Path.IsPathRooted(relative)||relative.Contains(':')||parts.Any(p=>p is "." or ".." or "" || p.EndsWith('.') || p.EndsWith(' ') || p.IndexOfAny(Path.GetInvalidFileNameChars())>=0))throw new InvalidDataException("Unsafe file path in installation package.");
        var full=Path.GetFullPath(Path.Combine(root,relative));var prefix=Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar)+Path.DirectorySeparatorChar;
        if(!full.StartsWith(prefix,StringComparison.OrdinalIgnoreCase))throw new InvalidDataException("Installation file escapes its application folder.");
        for(var current=full;current!=null;current=Path.GetDirectoryName(current))
        {
            try{if((File.GetAttributes(current)&FileAttributes.ReparsePoint)!=0)throw new IOException("Installation files must not use symbolic links or junctions.");}
            catch(FileNotFoundException){}
            catch(DirectoryNotFoundException){}
        }
        return full;
    }
    public static InstallRecord? ReadRecord(string path)
    {
        var file=Path.Combine(path,Marker);if(!File.Exists(file))return null;
        try{var record=JsonSerializer.Deserialize<InstallRecord>(File.ReadAllText(file),Json);return record?.AppId=="DuckWorkshop"?record:null;}
        catch(JsonException){return null;}
    }
    public static void CheckRunning(string target)
    {
        foreach(var process in Process.GetProcessesByName("Duck-Workshop"))using(process)
        {
            try{if(process.MainModule?.FileName is string path&&Path.GetDirectoryName(path)!.Equals(target,StringComparison.OrdinalIgnoreCase))throw new IOException("Close Duck Workshop before installing or uninstalling this copy.");}
            catch(System.ComponentModel.Win32Exception){throw new IOException("Duck Workshop is running. Close it before continuing.");}
            catch(InvalidOperationException){}
        }
    }
    public static async Task InstallAsync(string target,Stream payload,PayloadMetadata metadata,string feed,string? installerPath,IProgress<double>? progress=null,bool integrations=true,CancellationToken cancellation=default)
    {
        target=ResolveInstallFolder(target);CheckRunning(target);
        metadata.Files=metadata.Files.ToDictionary(p=>p.Key.Replace('/',Path.DirectorySeparatorChar),p=>p.Value,StringComparer.OrdinalIgnoreCase);
        if(!Version.TryParse(metadata.Version,out _)||metadata.Files.Count==0||metadata.Files.Count>10000)throw new InvalidDataException("Installation package metadata is invalid.");
        var old=ReadRecord(target);if(old!=null){
            var candidate=Version.Parse(metadata.Version);
            var executable=Path.Combine(target,"Duck-Workshop.exe");
            if((Version.TryParse(old.Version,out var installed)&&candidate<installed)||(File.Exists(executable)&&Version.TryParse(FileVersionInfo.GetVersionInfo(executable).ProductVersion,out var actual)&&candidate<actual))throw new IOException("A newer Duck Workshop version is already installed.");
        }var parent=Directory.GetParent(target)!.FullName;Directory.CreateDirectory(parent);
        var stage=Path.Combine(parent,".duck-install-"+Guid.NewGuid().ToString("N"));Directory.CreateDirectory(stage);
        var backup=target+".previous-"+Guid.NewGuid().ToString("N");bool swapped=false;
        try
        {
            using(var archive=new ZipArchive(payload,ZipArchiveMode.Read,leaveOpen:true))
            {
                var seen=new HashSet<string>(StringComparer.OrdinalIgnoreCase);long total=0;int count=0;
                foreach(var entry in archive.Entries)
                {
                    cancellation.ThrowIfCancellationRequested();
                    if(entry.FullName.EndsWith('/'))continue;
                    if(((entry.ExternalAttributes>>16)&0xF000)==0xA000)throw new InvalidDataException("Installation package contains a symbolic link.");
                    var relative=entry.FullName.Replace('/',Path.DirectorySeparatorChar);
                    if(!seen.Add(relative)||!metadata.Files.TryGetValue(relative,out var expected))throw new InvalidDataException("Package file is duplicated or absent from its manifest: "+entry.FullName);
                    total+=entry.Length;if(total>2147483648L)throw new InvalidDataException("Installation package is too large.");
                    var destination=SafePath(stage,relative);Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
                    await using(var input=entry.Open())await using(var output=new FileStream(destination,FileMode.CreateNew,FileAccess.Write,FileShare.None))await input.CopyToAsync(output,cancellation);
                    await using(var input=File.OpenRead(destination))if(!Convert.ToHexString(await SHA256.HashDataAsync(input,cancellation)).Equals(expected,StringComparison.OrdinalIgnoreCase))throw new InvalidDataException("Installation file checksum failed: "+entry.FullName);
                    progress?.Report((double)++count/metadata.Files.Count*.8);
                }
                if(seen.Count!=metadata.Files.Count)throw new InvalidDataException("Installation package is incomplete.");
            }
            if(!File.Exists(Path.Combine(stage,"Duck-Workshop.exe")))throw new InvalidDataException("Installation package is missing Duck Workshop.");
            var installedFiles=metadata.Files.Keys.ToList();
            if(installerPath!=null){File.Copy(installerPath,Path.Combine(stage,"DuckWorkshopSetup.exe"),true);installedFiles.Add("DuckWorkshopSetup.exe");}
            if(old!=null&&Directory.Exists(target))
            {
                var owned=old.Files.ToHashSet(StringComparer.OrdinalIgnoreCase);owned.Add(Marker);owned.Add("update-channel.json");
                void Preserve(string directory)
                {
                    foreach(var file in Directory.GetFiles(directory)){var relative=Path.GetRelativePath(target,file);if(owned.Contains(relative)||metadata.Files.ContainsKey(relative))continue;if((File.GetAttributes(file)&FileAttributes.ReparsePoint)!=0)throw new IOException("Move linked files out of the application folder before updating.");var destination=SafePath(stage,relative);Directory.CreateDirectory(Path.GetDirectoryName(destination)!);File.Copy(file,destination,false);}
                    foreach(var directoryPath in Directory.GetDirectories(directory))if((File.GetAttributes(directoryPath)&FileAttributes.ReparsePoint)==0)Preserve(directoryPath);
                }
                Preserve(target);
            }
            if(string.IsNullOrWhiteSpace(feed))feed=old?.FeedUrl??"";
            File.WriteAllText(Path.Combine(stage,"update-channel.json"),JsonSerializer.Serialize(new{FeedUrl=feed},Json));installedFiles.Add("update-channel.json");
            File.WriteAllText(Path.Combine(stage,Marker),JsonSerializer.Serialize(new InstallRecord{Version=metadata.Version,Files=installedFiles,FeedUrl=feed},Json));
            // Both paths are validated absolute children of the selected application's
            // parent, and an existing target is identified by its Duck Workshop marker.
            CheckRunning(target);
            if(Directory.Exists(target))Directory.Move(target,backup);
            try{Directory.Move(stage,target);swapped=true;}
            catch{if(Directory.Exists(backup))Directory.Move(backup,target);throw;}
            progress?.Report(.95);
            if(integrations)Register(target,metadata.Version);
            if(Directory.Exists(backup))
            {
                // Keep the most recent install intact for recovery. Do not recursively
                // delete backups, user files, settings or projects during installation.
                File.WriteAllText(Path.Combine(backup,"ROLLBACK.txt"),"Previous Duck Workshop installation. Current copy: "+target);
            }
            progress?.Report(1);
        }
        catch
        {
            if(swapped&&Directory.Exists(backup))
            {
                var failed=target+".failed-"+Guid.NewGuid().ToString("N");Directory.Move(target,failed);Directory.Move(backup,target);
            }
            throw;
        }
        finally{if(Directory.Exists(stage))Directory.Delete(stage,true);}
    }
    private static void Register(string target,string version)
    {
        var shortcut=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Programs),"Duck Workshop.lnk");
        var shellType=Type.GetTypeFromProgID("WScript.Shell");if(shellType!=null){dynamic shell=Activator.CreateInstance(shellType)!;dynamic link=shell.CreateShortcut(shortcut);link.TargetPath=Path.Combine(target,"Duck-Workshop.exe");link.WorkingDirectory=target;link.IconLocation=Path.Combine(target,"Duck-Workshop.exe")+",0";link.Save();}
        using(var key=Registry.CurrentUser.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\DuckWorkshop")){key.SetValue("DisplayName","Duck Workshop");key.SetValue("DisplayVersion",version);key.SetValue("Publisher","Rubber Ducks Development");key.SetValue("InstallLocation",target);key.SetValue("DisplayIcon",Path.Combine(target,"Duck-Workshop.exe"));key.SetValue("UninstallString","\""+Path.Combine(target,"DuckWorkshopSetup.exe")+"\" --uninstall --install-dir \""+target+"\"");key.SetValue("NoModify",1);key.SetValue("NoRepair",1);}
    }
    public static void Uninstall(string target,bool integrations=true)
    {
        target=ResolveInstallFolder(target);var record=ReadRecord(target)??throw new IOException("This folder is not an installed Duck Workshop copy.");CheckRunning(target);
        // Validate every owned path before deleting any file, including linked children.
        var files=record.Files.Distinct(StringComparer.OrdinalIgnoreCase).Select(relative=>SafePath(target,relative)).ToArray();
        foreach(var file in files)
        {
            if(File.Exists(file))File.Delete(file);
        }
        File.Delete(Path.Combine(target,Marker));
        void EmptyDirectories(string folder)
        {
            foreach(var child in Directory.GetDirectories(folder))if((File.GetAttributes(child)&FileAttributes.ReparsePoint)==0){EmptyDirectories(child);if(!Directory.EnumerateFileSystemEntries(child).Any())Directory.Delete(child);}
        }
        EmptyDirectories(target);if(!Directory.EnumerateFileSystemEntries(target).Any())Directory.Delete(target);
        if(!integrations)return;
        const string uninstallKey=@"Software\Microsoft\Windows\CurrentVersion\Uninstall\DuckWorkshop";
        using(var registered=Registry.CurrentUser.OpenSubKey(uninstallKey))
            if(registered?.GetValue("InstallLocation") is not string location||!location.Equals(target,StringComparison.OrdinalIgnoreCase))return;
        var shortcut=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Programs),"Duck Workshop.lnk");
        var shellType=Type.GetTypeFromProgID("WScript.Shell");
        if(File.Exists(shortcut)&&shellType!=null){dynamic shell=Activator.CreateInstance(shellType)!;dynamic link=shell.CreateShortcut(shortcut);if(string.Equals((string)link.TargetPath,Path.Combine(target,"Duck-Workshop.exe"),StringComparison.OrdinalIgnoreCase))File.Delete(shortcut);}
        Registry.CurrentUser.DeleteSubKeyTree(uninstallKey,false);
    }
}
