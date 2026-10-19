// Adapted from the MIT-licensed Duck Studio installer; see Installer/LICENSE.
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using Microsoft.Win32;

namespace DuckWorkshop.Setup;

public static class Program
{
    public static string? Argument(string[] args,string key)
    {
        var index=Array.IndexOf(args,key);return index>=0&&index+1<args.Length?args[index+1]:null;
    }
    private static async Task WaitForParent(string[] args)
    {
        if(int.TryParse(Argument(args,"--wait-pid"),out var pid))
        {
            try{using var process=Process.GetProcessById(pid);using var timeout=new CancellationTokenSource(TimeSpan.FromMinutes(5));await process.WaitForExitAsync(timeout.Token);}
            catch(ArgumentException){}
        }
    }
    [STAThread]
    public static void Main(string[] args)
    {
        if(args.Contains("--uninstall-worker"))
        {
            try{WaitForParent(args).GetAwaiter().GetResult();InstallEngine.Uninstall(Argument(args,"--install-dir")??InstallEngine.DefaultFolder,!args.Contains("--no-integrations"));MessageBox.Show("Duck Workshop was uninstalled. Saved projects and source files are kept.","Duck Workshop");}
            catch(Exception ex){MessageBox.Show(ex.Message,"Uninstall failed",MessageBoxButton.OK,MessageBoxImage.Error);}return;
        }
        if(args.Contains("--test-install"))
        {
            var metadata=Metadata();using var payload=Payload();InstallEngine.InstallAsync(Argument(args,"--install-dir")??throw new ArgumentException("Test install folder missing."),payload,metadata,Feed(args),null,integrations:false).GetAwaiter().GetResult();return;
        }
        var app=new Application();var window=new SetupWindow(args);app.Run(window);
    }
    public static Stream Payload()=>Assembly.GetExecutingAssembly().GetManifestResourceStream("DuckWorkshop.Payload.zip")??throw new IOException("The installer is missing its app package.");
    public static PayloadMetadata Metadata()
    {
        using var stream=Assembly.GetExecutingAssembly().GetManifestResourceStream("DuckWorkshop.Payload.json")??throw new IOException("The installer is missing its package metadata.");return JsonSerializer.Deserialize<PayloadMetadata>(stream,InstallEngine.Json)??throw new IOException("Invalid installation package.");
    }
    public static string Feed(string[] args)
    {
        if(Argument(args,"--feed") is { } feed)return feed;
        return "https://github.com/Rubber-Ducks-Development/duckworkshop/releases/latest/download/latest.json";
    }
    public static Task WaitAsync(string[] args)=>WaitForParent(args);
}

public sealed class SetupWindow : Window
{
    private readonly string[] args;
    private readonly bool updating,uninstalling;
    private readonly TextBox folder;
    private readonly TextBlock status;
    private readonly ProgressBar progress;
    private readonly Button install,cancel,browse;
    private bool working,complete;
    public SetupWindow(string[] arguments)
    {
        args=arguments;updating=args.Contains("--update");uninstalling=args.Contains("--uninstall");
        Title=uninstalling?"Uninstall Duck Workshop":updating?"Update Duck Workshop":"Install Duck Workshop";Width=640;Height=530;ResizeMode=ResizeMode.NoResize;WindowStartupLocation=WindowStartupLocation.CenterScreen;
        Background=new SolidColorBrush(Color.FromRgb(44,44,47));Foreground=Brushes.White;FontFamily=new FontFamily("Segoe UI");
        Icon=new BitmapImage(new Uri("pack://application:,,,/DuckWorkshopSetup;component/Logo.png"));
        var root=new StackPanel{Margin=new Thickness(26)};Content=root;
        var heading=new DockPanel{Margin=new Thickness(0,0,0,18)};root.Children.Add(heading);
        heading.Children.Add(new Image{Source=Icon,Width=82,Height=82,Margin=new Thickness(0,0,18,0)});
        var titles=new StackPanel{VerticalAlignment=VerticalAlignment.Center};heading.Children.Add(titles);titles.Children.Add(Text("Duck Workshop",27));titles.Children.Add(Text("Early Access",12));titles.Children.Add(Text(uninstalling?"Remove this application copy":updating?"Install the available update":"Vehicle development tools for GTA V / FiveM",12));
        root.Children.Add(Text(uninstalling?"The app will be removed. Your saved projects, settings and vehicle source files are kept.":"Installs for your Windows account without administrator rights. Saved projects and vehicle files stay separate from the application.",13));
        root.Children.Add(new TextBlock{Text="Application folder",Foreground=Brushes.LightGray,Margin=new Thickness(0,20,0,6)});
        var row=new DockPanel();root.Children.Add(row);browse=Button("Browse");browse.Margin=new Thickness(8,0,0,0);DockPanel.SetDock(browse,Dock.Right);row.Children.Add(browse);
        folder=new TextBox{Text=Program.Argument(args,"--install-dir")??InstallEngine.DefaultFolder,Padding=new Thickness(8,6,8,6),Background=new SolidColorBrush(Color.FromRgb(61,61,65)),Foreground=Brushes.White,BorderBrush=Brushes.Gray,IsReadOnly=updating||uninstalling};row.Children.Add(folder);System.Windows.Automation.AutomationProperties.SetName(folder,"Application folder");
        browse.IsEnabled=!updating&&!uninstalling;browse.Click+=(_,_)=>{var dialog=new OpenFolderDialog{Title="Choose an empty Duck Workshop application folder"};if(dialog.ShowDialog(this)==true)folder.Text=dialog.FolderName;};
        status=Text(uninstalling?"Ready to uninstall.":updating?"Waiting for Duck Workshop to close safely...":"Ready to install Duck Workshop "+Assembly.GetExecutingAssembly().GetName().Version!.ToString(3)+".",12);status.Margin=new Thickness(0,22,0,12);root.Children.Add(status);
        progress=new ProgressBar{Height=13,Minimum=0,Maximum=1,Visibility=Visibility.Collapsed};root.Children.Add(progress);
        var buttons=new StackPanel{Orientation=Orientation.Horizontal,HorizontalAlignment=HorizontalAlignment.Right,Margin=new Thickness(0,24,0,0)};root.Children.Add(buttons);
        cancel=Button("Cancel");cancel.IsCancel=true;cancel.Margin=new Thickness(0,0,10,0);install=Button(uninstalling?"Uninstall":updating?"Install update":"Install");install.Background=new SolidColorBrush(Color.FromRgb(215,185,116));install.Foreground=new SolidColorBrush(Color.FromRgb(30,30,32));install.IsDefault=true;buttons.Children.Add(cancel);buttons.Children.Add(install);
        cancel.Click+=(_,_)=>Close();install.Click+=async(_,_)=>await InstallAsync();
        Closing+=(_,e)=>{if(working)e.Cancel=true;};Loaded+=async(_,_)=>{if(updating)await InstallAsync();};
    }
    private static TextBlock Text(string value,int size)=>new(){Text=value,FontSize=size,Foreground=Brushes.White,TextWrapping=TextWrapping.Wrap,LineHeight=size+7};
    private static Button Button(string text)=>new(){Content=text,MinWidth=96,Height=35,Padding=new Thickness(12,5,12,5),Background=new SolidColorBrush(Color.FromRgb(80,81,85)),Foreground=Brushes.White,BorderBrush=Brushes.Gray};
    private async Task InstallAsync()
    {
        if(complete)
        {
            if(!uninstalling)Process.Start(new ProcessStartInfo(Path.Combine(folder.Text,"Duck-Workshop.exe")){UseShellExecute=true});Close();return;
        }
        working=true;install.IsEnabled=cancel.IsEnabled=browse.IsEnabled=folder.IsEnabled=false;progress.Visibility=Visibility.Visible;
        try
        {
            await Program.WaitAsync(args);var target=InstallEngine.ResolveInstallFolder(folder.Text);
            if(uninstalling)
            {
                InstallEngine.CheckRunning(target);
                var temporary=Path.Combine(Path.GetTempPath(),"DuckWorkshop-Uninstall-"+Guid.NewGuid().ToString("N"));Directory.CreateDirectory(temporary);var exe=Path.Combine(temporary,"DuckWorkshopSetup.exe");File.Copy(Environment.ProcessPath!,exe);
                var start=new ProcessStartInfo(exe){UseShellExecute=true};foreach(var value in new[]{"--uninstall-worker","--install-dir",target,"--wait-pid",Environment.ProcessId.ToString()})start.ArgumentList.Add(value);Process.Start(start);working=false;Close();return;
            }
            status.Text="Installing verified application files...";
            var callback=new Progress<double>(value=>progress.Value=value);var metadata=Program.Metadata();using var payload=Program.Payload();
            await InstallEngine.InstallAsync(target,payload,metadata,Program.Feed(args),Environment.ProcessPath,callback);
            status.Text="Duck Workshop "+metadata.Version+" is ready. A Start menu shortcut and uninstall entry have been created.";progress.Value=1;complete=true;install.Content="Open Duck Workshop";cancel.Content="Finish";
        }
        catch(Exception ex){status.Text="Installation could not finish. "+(ex.InnerException??ex).Message;MessageBox.Show(this,(ex.InnerException??ex).Message,"Duck Workshop installer",MessageBoxButton.OK,MessageBoxImage.Error);}
        finally{working=false;install.IsEnabled=cancel.IsEnabled=true;folder.IsEnabled=!complete;browse.IsEnabled=!complete&&!updating&&!uninstalling;}
    }
}
