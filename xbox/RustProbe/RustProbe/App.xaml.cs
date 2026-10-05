using Windows.ApplicationModel.Activation;
using Windows.UI.ViewManagement;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace RustProbe
{
    sealed partial class App : Application
    {
        public App()
        {
            InitializeComponent();
            // Absturz der Hülle selbst (vor oder neben der Probe) nach LocalState\shell-error.txt
            UnhandledException += (s, e) => System.IO.File.WriteAllText(
                System.IO.Path.Combine(Windows.Storage.ApplicationData.Current.LocalFolder.Path, "shell-error.txt"),
                e.Exception.ToString());
            RequiresPointerMode = ApplicationRequiresPointerMode.WhenRequested;
        }

        protected override void OnLaunched(LaunchActivatedEventArgs e)
        {
            // Xbox: ganze Bildfläche in echten Bildpunkten (sonst skaliert UWP auf dem Fernseher um 200 %)
            ApplicationView.GetForCurrentView().SetDesiredBoundsMode(ApplicationViewBoundsMode.UseCoreWindow);
            ApplicationViewScaling.TrySetDisableLayoutScaling(true);
            if (!(Window.Current.Content is Frame rootFrame))
            {
                rootFrame = new Frame();
                Window.Current.Content = rootFrame;
            }
            if (rootFrame.Content == null) rootFrame.Navigate(typeof(MainPage), e.Arguments);
            Window.Current.Activate();
        }
    }
}
