using System;
using Windows.ApplicationModel;
using Windows.ApplicationModel.Activation;
using Windows.UI.ViewManagement;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace GtaBerlin
{
    sealed partial class App : Application
    {
        public App()
        {
            InitializeComponent();
            // Xbox: kein Maus-Cursor-Modus – der Controller gehört dem Spiel.
            RequiresPointerMode = ApplicationRequiresPointerMode.WhenRequested;
            // WinUI-2-WebView2 kennt keine CoreWebView2EnvironmentOptions; Browser-Argumente gehen über die
            // Umgebungsvariable (so auch in Microsofts Xbox-Remote-Debugging-Anleitung).
            //  - Autoplay ohne Nutzergeste: Controller-Eingaben aus der Hülle zählen nicht als Geste.
            //  - Debug: Remote-Debugging über das Xbox Device Portal (edge://inspect).
            var args = "--autoplay-policy=no-user-gesture-required";
#if DEBUG
            args += " --enable-features=msEdgeDevToolsWdpRemoteDebugging";
#endif
            Environment.SetEnvironmentVariable("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", args);
            Suspending += OnSuspending;
        }

        protected override void OnLaunched(LaunchActivatedEventArgs e)
        {
            // Auf der Xbox den ganzen Bildschirm nutzen (TV-Overscan fängt das Spiel mit 5 % Rand selbst ab).
            ApplicationView.GetForCurrentView().SetDesiredBoundsMode(ApplicationViewBoundsMode.UseCoreWindow);

            if (!(Window.Current.Content is Frame rootFrame))
            {
                rootFrame = new Frame();
                Window.Current.Content = rootFrame;
            }
            if (rootFrame.Content == null) rootFrame.Navigate(typeof(MainPage), e.Arguments);
            Window.Current.Activate();
        }

        private void OnSuspending(object sender, SuspendingEventArgs e)
        {
            var deferral = e.SuspendingOperation.GetDeferral();
            deferral.Complete();
        }
    }
}
