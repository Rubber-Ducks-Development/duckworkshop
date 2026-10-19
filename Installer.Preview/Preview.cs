using System.Reflection;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using DuckWorkshop.Setup;

namespace DuckWorkshop.Preview;
public static class PreviewProgram
{
    [STAThread]
    public static void Main(string[] args)
    {
        var app = new Application();
        foreach (var completed in new[] { false, true })
        {
            var window = new SetupWindow([]);
            if (completed)
            {
                ((TextBlock)Field(window, "status")).Text = "Duck Workshop 0.3.1 is ready. A Start menu shortcut and uninstall entry have been created.";
                ((ProgressBar)Field(window, "progress")).Visibility = Visibility.Visible;
                ((ProgressBar)Field(window, "progress")).Value = 1;
                ((Button)Field(window, "install")).Content = "Open Duck Workshop";
                ((Button)Field(window, "cancel")).Content = "Finish";
            }
            var content = (UIElement)window.Content;
            window.Content = null;
            var frame = new Border { Background = window.Background, Child = content };
            var height = window.Height - 38;
            frame.Measure(new Size(640, height));
            frame.Arrange(new Rect(0, 0, 640, height));
            frame.UpdateLayout();
            var image = new RenderTargetBitmap(640, (int)height, 96, 96, PixelFormats.Pbgra32);
            image.Render(frame);
            var encoder = new PngBitmapEncoder();
            encoder.Frames.Add(BitmapFrame.Create(image));
            var path = System.IO.Path.Combine(args.Single(), completed ? "setup-complete.png" : "setup-ready.png");
            using (var stream = System.IO.File.Create(path)) encoder.Save(stream);
            foreach (var name in new[] { "install", "cancel", "browse", "folder" })
            {
                var control = (FrameworkElement)Field(window, name);
                var point = control.TranslatePoint(new Point(0, 0), frame);
                if (point.X < 0 || point.Y < 0 || point.X + control.ActualWidth > 640 || point.Y + control.ActualHeight > height)
                    throw new Exception($"{name} is clipped in {(completed ? "complete" : "ready")} state: {point}, {control.ActualWidth}x{control.ActualHeight}, viewport height {height}");
            }
            window.Close();
        }
        app.Shutdown();
    }
    static object Field(object target, string name) => target.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(target)!;
}
