using System;
using System.Globalization;
using System.Linq;
using System.Text;
using Microsoft.Web.WebView2.Core;
using Windows.Gaming.Input;
using Windows.System;
using Windows.UI.Core;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Input;
using Windows.UI.Xaml.Navigation;

namespace GtaBerlin
{
    // Hülle: WebView2 lädt das Spiel aus dem Paket (Ordner "Web") über einen virtuellen Hostnamen.
    // Weil die Web-Gamepad-API in UWP-WebView2 nicht zuverlässig funktioniert
    // (github.com/MicrosoftEdge/WebView2Feedback/issues/4366), liest die Hülle den Controller nativ
    // über Windows.Gaming.Input und schickt jede Lesung als JSON an die Seite (siehe web/src/main.js).
    public sealed partial class MainPage : Page
    {
        private const string VirtualHost = "gta-berlin.local";
        private readonly DispatcherTimer _padTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(8) };
        private bool _ready;

        public MainPage()
        {
            InitializeComponent();
            Loaded += OnLoaded;
        }

        protected override void OnNavigatedTo(NavigationEventArgs e)
        {
            // B / Zurück darf die App nicht schließen – das Spiel verarbeitet B selbst.
            SystemNavigationManager.GetForCurrentView().BackRequested += (s, a) => a.Handled = true;
            // Controller-Tasten nicht als Tastatur-/Fokus-Navigation an XAML weiterreichen.
            Window.Current.CoreWindow.Dispatcher.AcceleratorKeyActivated += OnAcceleratorKey;
            Window.Current.Activated += (s, a) => Web.Focus(FocusState.Programmatic);
        }

        private async void OnLoaded(object sender, RoutedEventArgs e)
        {
            await Web.EnsureCoreWebView2Async();
            var core = Web.CoreWebView2;
            core.SetVirtualHostNameToFolderMapping(VirtualHost, "Web", CoreWebView2HostResourceAccessKind.Allow);
            core.Settings.AreDefaultContextMenusEnabled = false;
            core.Settings.IsZoomControlEnabled = false;
            core.Settings.IsStatusBarEnabled = false;
#if !DEBUG
            core.Settings.AreDevToolsEnabled = false;
#endif
            core.WebMessageReceived += OnWebMessage;
            core.Navigate($"https://{VirtualHost}/index.html");
            Web.Focus(FocusState.Programmatic);
            _padTimer.Tick += (s, a) => SendGamepads();
            _padTimer.Start();
        }

        private void OnWebMessage(CoreWebView2 sender, CoreWebView2WebMessageReceivedEventArgs args)
        {
            string type = "";
            try
            {
                var json = Windows.Data.Json.JsonObject.Parse(args.WebMessageAsJson);
                type = json.GetNamedString("type", "");
            }
            catch { return; }
            if (type == "ready") _ready = true;
            else if (type == "quit") Application.Current.Exit();
        }

        private void SendGamepads()
        {
            if (!_ready || Web.CoreWebView2 == null) return;
            var pads = Gamepad.Gamepads.ToList();
            var sb = new StringBuilder("{\"type\":\"gamepad\",\"pads\":[");
            for (int i = 0; i < pads.Count; i++)
            {
                var r = pads[i].GetCurrentReading();
                var b = r.Buttons;
                if (i > 0) sb.Append(',');
                sb.Append('{');
                Btn(sb, "a", b, GamepadButtons.A); Btn(sb, "b", b, GamepadButtons.B);
                Btn(sb, "x", b, GamepadButtons.X); Btn(sb, "y", b, GamepadButtons.Y);
                Btn(sb, "lb", b, GamepadButtons.LeftShoulder); Btn(sb, "rb", b, GamepadButtons.RightShoulder);
                Btn(sb, "view", b, GamepadButtons.View); Btn(sb, "menu", b, GamepadButtons.Menu);
                Btn(sb, "ls", b, GamepadButtons.LeftThumbstick); Btn(sb, "rs", b, GamepadButtons.RightThumbstick);
                Btn(sb, "up", b, GamepadButtons.DPadUp); Btn(sb, "down", b, GamepadButtons.DPadDown);
                Btn(sb, "left", b, GamepadButtons.DPadLeft); Btn(sb, "right", b, GamepadButtons.DPadRight);
                Num(sb, "lx", r.LeftThumbstickX); Num(sb, "ly", r.LeftThumbstickY);
                Num(sb, "rx", r.RightThumbstickX); Num(sb, "ry", r.RightThumbstickY);
                Num(sb, "lt", r.LeftTrigger); Num(sb, "rt", r.RightTrigger, last: true);
                sb.Append('}');
            }
            sb.Append("]}");
            Web.CoreWebView2.PostWebMessageAsJson(sb.ToString());
        }

        private static void Btn(StringBuilder sb, string k, GamepadButtons all, GamepadButtons one) =>
            sb.Append('"').Append(k).Append("\":").Append((all & one) != 0 ? "true" : "false").Append(',');

        private static void Num(StringBuilder sb, string k, double v, bool last = false)
        {
            sb.Append('"').Append(k).Append("\":").Append(v.ToString("0.###", CultureInfo.InvariantCulture));
            if (!last) sb.Append(',');
        }

        // Gamepad-Tasten erscheinen in UWP zusätzlich als VirtualKey.Gamepad*; die würden sonst
        // die XY-Fokusnavigation auslösen und den Fokus aus der WebView2 ziehen.
        private void OnAcceleratorKey(CoreDispatcher sender, AcceleratorKeyEventArgs args)
        {
            var k = args.VirtualKey;
            if (k >= VirtualKey.GamepadA && k <= VirtualKey.GamepadRightThumbstickLeft) args.Handled = true;
        }
    }
}
