using System;
using System.Runtime.InteropServices;
using Windows.Storage;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Media;

namespace RustProbe
{
    /// <summary>
    /// Holt den ISwapChainPanelNative-Zeiger des SwapChainPanel, startet die Rust-Probe damit und zeichnet je Bild
    /// (CompositionTarget.Rendering, UI-Thread). Der Bericht der Probe steht oben links und in
    /// LocalState\probe-status.txt (Xbox Device Portal → File Explorer).
    /// </summary>
    public sealed partial class MainPage : Page
    {
        [DllImport("berlin_probe.dll")] static extern int probe_start(IntPtr panel, uint width, uint height, [MarshalAs(UnmanagedType.LPWStr)] string logDir);
        [DllImport("berlin_probe.dll")] static extern int probe_frame();
        [DllImport("berlin_probe.dll")] static extern void probe_resize(uint width, uint height);
        [DllImport("berlin_probe.dll")] static extern uint probe_status([Out] char[] buffer, uint capacity);

        // ISwapChainPanelNative von Windows.UI.Xaml (UWP, windows.ui.xaml.media.dxinterop.h). 63aad0b8-… ist die
        // WinUI-3-Fassung (Microsoft.UI.Xaml), die ein UWP-SwapChainPanel mit E_NOINTERFACE ablehnt. wgpu nimmt den
        // Zeiger ungeprüft und ruft nur SetSwapChain (gleiche vtable in beiden Fassungen).
        static readonly Guid SwapChainPanelNative = new Guid("F92F19D2-3ADE-45A6-A20C-F6F1EA90554B");
        bool started;
        int frames;

        public MainPage()
        {
            InitializeComponent();
            Panel.Loaded += OnLoaded;
            Panel.SizeChanged += (s, e) => { if (started) { PixelSize(out uint w, out uint h); probe_resize(w, h); } };
        }

        void PixelSize(out uint w, out uint h)
        {
            w = (uint)Math.Max(1, Math.Round(Panel.ActualWidth * Panel.CompositionScaleX));
            h = (uint)Math.Max(1, Math.Round(Panel.ActualHeight * Panel.CompositionScaleY));
        }

        void OnLoaded(object sender, RoutedEventArgs e)
        {
            try
            {
                IntPtr unknown = Marshal.GetIUnknownForObject(Panel);
                Guid iid = SwapChainPanelNative;
                int hr = Marshal.QueryInterface(unknown, ref iid, out IntPtr native);
                Marshal.Release(unknown);
                if (hr != 0)
                {
                    Shell("QueryInterface(ISwapChainPanelNative) fehlgeschlagen: 0x" + hr.ToString("X8"));
                    return;
                }
                Shell("ISwapChainPanelNative geholt, rufe probe_start");
                PixelSize(out uint w, out uint h);
                started = probe_start(native, w, h, ApplicationData.Current.LocalFolder.Path) == 0;
                ShowStatus();
                CompositionTarget.Rendering += OnRendering;
            }
            catch (Exception ex)
            {
                // z. B. DllNotFoundException (DLL fehlt oder eine ihrer Abhängigkeiten lädt nicht)
                Shell("Ausnahme beim Start: " + ex);
            }
        }

        /// Meldung der Hülle selbst: ins Fenster und nach LocalState\shell-status.txt (falls die Probe nichts schreibt).
        void Shell(string text)
        {
            Report.Text = text;
            try { System.IO.File.WriteAllText(System.IO.Path.Combine(ApplicationData.Current.LocalFolder.Path, "shell-status.txt"), text); }
            catch { }
        }

        void OnRendering(object sender, object e)
        {
            if (started && probe_frame() == -2) ShowStatus();
            if (++frames % 30 == 0) ShowStatus();
        }

        void ShowStatus()
        {
            var buffer = new char[16384];
            uint n = probe_status(buffer, (uint)buffer.Length);
            Report.Text = new string(buffer, 0, (int)n);
        }
    }
}
