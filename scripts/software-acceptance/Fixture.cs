// Deliberately limited to the dedicated acceptance computer and marker directory.
// This is an actual EXE uninstaller; production software is never selected.
using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using Microsoft.Win32;

internal static class Fixture {
    internal const string Prefix = "DustAway.Acceptance.20260930.7c7e25e8.";
    private const string BaseKey = @"Software\Microsoft\Windows\CurrentVersion\Uninstall\";
    private static readonly string[] Names = { "Normal", "Delayed", "Early", "ZeroExit", "Reinstall", "Recovery", "SameA", "SameB", "Missing", "Wrapper", "Ambiguous", "NoRemove", "Hidden", "Update", "Uac" };
    private static string Root;
    private static string Exe;

    private static void Guard() {
        Exe = typeof(Fixture).Assembly.Location;
        Root = Path.GetDirectoryName(Exe);
        string expected = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), @"DustAwayAcceptance\20260930-software");
        if (!String.Equals(Environment.MachineName, "DESKTOP-24NJTT6", StringComparison.OrdinalIgnoreCase)
            || !String.Equals(Path.GetFullPath(Root), Path.GetFullPath(expected), StringComparison.OrdinalIgnoreCase)
            || File.ReadAllText(Path.Combine(Root, ".dustaway-isolated-acceptance")) != Prefix.TrimEnd('.'))
            throw new InvalidOperationException("Not the authorized isolated fixture directory");
    }

    private static void Install(string name) {
        if (!Names.Contains(name)) throw new ArgumentException("Unknown fixture");
        using (RegistryKey key = Registry.CurrentUser.CreateSubKey(BaseKey + Prefix + name)) {
            key.SetValue("DisplayName", "DustAway Acceptance " + (name.StartsWith("Same") ? "Same Name" : name));
            key.SetValue("Publisher", "DustAway Acceptance Fixtures");
            key.SetValue("DisplayVersion", "1.0.0");
            key.SetValue("InstallLocation", Path.Combine(Root, "installed"));
            key.SetValue("UninstallString", "\"" + Exe + "\" uninstall " + name);
            if (name == "Uac") key.SetValue("UninstallString", "\"" + Path.Combine(Root, "DustAwayAcceptanceUac.exe") + "\" uninstall Uac");
            key.SetValue("EstimatedSize", 123, RegistryValueKind.DWord);
            key.SetValue("InstallDate", "20260930");
            if (name == "Missing") {
                key.DeleteValue("Publisher"); key.DeleteValue("DisplayVersion"); key.DeleteValue("EstimatedSize");
                key.SetValue("UninstallString", "\"" + Path.Combine(Root, "missing.exe") + "\"");
            }
            if (name == "Wrapper") key.SetValue("UninstallString", @"C:\Windows\System32\cmd.exe /c echo must-not-run");
            if (name == "Ambiguous") key.SetValue("UninstallString", @"C:\Program Files\DustAway Acceptance\uninstall.exe /remove");
            if (name == "NoRemove") key.SetValue("NoRemove", 1, RegistryValueKind.DWord);
            if (name == "Hidden") key.SetValue("SystemComponent", 1, RegistryValueKind.DWord);
            if (name == "Update") key.SetValue("ReleaseType", "Update");
        }
        Directory.CreateDirectory(Path.Combine(Root, "installed"));
        File.WriteAllText(Path.Combine(Root, "installed", "payload-" + name + ".txt"), "dedicated fixture installation");
    }

    private static void Remove(string name) {
        Registry.CurrentUser.DeleteSubKeyTree(BaseKey + Prefix + name, false);
        File.Delete(Path.Combine(Root, "installed", "payload-" + name + ".txt"));
        // User data deliberately survives the fixture's standard uninstaller.
        File.WriteAllText(Path.Combine(Root, "retained-" + name + ".txt"), "fixture user data retained");
    }

    public static int Main(string[] args) {
        try {
            Guard();
            if (args.Length == 1 && args[0] == "install") { foreach (string name in Names) Install(name); return 0; }
            if (args.Length == 1 && args[0] == "cleanup") {
                foreach (string name in Names) Registry.CurrentUser.DeleteSubKeyTree(BaseKey + Prefix + name, false);
                return 0;
            }
            if (args.Length != 2 || !Names.Contains(args[1])) throw new ArgumentException("Invalid fixture invocation");
            string item = args[1];
            if (args[0] == "install-one") { Install(item); return 0; }
            if (args[0] == "late-worker") { Thread.Sleep(4000); Remove(item); return 0; }
            if (args[0] != "uninstall") throw new ArgumentException("Invalid operation");
            File.AppendAllText(Path.Combine(Root, "launches.log"), item + Environment.NewLine);
            if (item == "Early") {
                Process.Start(new ProcessStartInfo(Exe, "late-worker Early") { UseShellExecute = false, CreateNoWindow = true });
                return 0;
            }
            if (item == "ZeroExit") return 0;
            if (item == "Delayed" || item == "Recovery") Thread.Sleep(4000);
            Remove(item);
            return 0;
        } catch (Exception error) { Console.Error.WriteLine(error.Message); return 99; }
    }
}
