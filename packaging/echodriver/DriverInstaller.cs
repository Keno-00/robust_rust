using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Security.Principal;
using System.ServiceProcess;
using System.Text;
using System.Web.Script.Serialization;
using Microsoft.Win32;

// MSI owns this executable and contract. No script host or execution-policy changes.
internal static class DriverInstaller {
    private sealed class Contract {
        public Contract() {}
        public string service { get; set; }
        public string destination { get; set; }
        public string driver_sha256 { get; set; }
        public string certificate_sha256 { get; set; }
        public string key_script_sha256 { get; set; }
        public string certificate_thumbprint { get; set; }
    }
    private sealed class Journal {
        public Journal() {}
        public string path, file, sha256;
        public bool running;
        public bool uninstall;
        public string protected_key;
    }
    private static readonly JavaScriptSerializer Json = new JavaScriptSerializer { MaxJsonLength = 65536 };
    private static readonly SecurityIdentifier Admin = new SecurityIdentifier("S-1-5-32-544");
    private static readonly SecurityIdentifier SystemAccount = new SecurityIdentifier("S-1-5-18");
    private static string source, managed, root, journalPath, logPath;
    private static Contract contract;

    private static void Log(string text) {
        Console.WriteLine(text);
        if (logPath != null) File.AppendAllText(logPath, DateTime.UtcNow.ToString("o") + " " + text + Environment.NewLine);
    }
    private static void Ordinary(string path) {
        string current = Path.GetFullPath(path);
        while (current != null) {
            if ((File.Exists(current) || Directory.Exists(current)) && (File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidOperationException("Reparse path rejected: " + current);
            current = Path.GetDirectoryName(current);
        }
    }
    private static void TrustedDirectory(string path) {
        Ordinary(path);
        DirectorySecurity acl = Directory.GetAccessControl(path);
        IdentityReference owner = acl.GetOwner(typeof(SecurityIdentifier));
        if (!owner.Equals(Admin) && !owner.Equals(SystemAccount)) throw new InvalidOperationException("Untrusted directory owner: " + path);
        FileSystemRights writes = FileSystemRights.Write | FileSystemRights.Delete | FileSystemRights.DeleteSubdirectoriesAndFiles | FileSystemRights.ChangePermissions | FileSystemRights.TakeOwnership;
        foreach (FileSystemAccessRule rule in acl.GetAccessRules(true, true, typeof(SecurityIdentifier))) {
            if (rule.AccessControlType == AccessControlType.Allow && (rule.PropagationFlags & PropagationFlags.InheritOnly) == 0 &&
                !rule.IdentityReference.Equals(Admin) && !rule.IdentityReference.Equals(SystemAccount) && (rule.FileSystemRights & writes) != 0)
                throw new InvalidOperationException("Untrusted directory writes: " + path);
        }
    }
    private static void SecureJournalDirectory() {
        string parent = Path.GetDirectoryName(root);
        if (!Directory.Exists(parent)) Directory.CreateDirectory(parent);
        TrustedDirectory(parent);
        if (Directory.Exists(root)) { TrustedDirectory(root); return; }
        DirectorySecurity acl = new DirectorySecurity();
        acl.SetAccessRuleProtection(true, false); acl.SetOwner(Admin);
        foreach (SecurityIdentifier sid in new[] { Admin, SystemAccount })
            acl.AddAccessRule(new FileSystemAccessRule(sid, FileSystemRights.FullControl, InheritanceFlags.ContainerInherit | InheritanceFlags.ObjectInherit, PropagationFlags.None, AccessControlType.Allow));
        Directory.CreateDirectory(root, acl);
        TrustedDirectory(root);
    }
    private static void ApplicationDirectory(string path) {
        Ordinary(path);
        if (Directory.Exists(path)) {
            IdentityReference owner = Directory.GetAccessControl(path).GetOwner(typeof(SecurityIdentifier));
            if (!owner.Equals(Admin) && !owner.Equals(SystemAccount)) throw new InvalidOperationException("Untrusted application directory owner: " + path);
        }
        DirectorySecurity acl = new DirectorySecurity(); acl.SetAccessRuleProtection(true, false); acl.SetOwner(Admin);
        foreach (SecurityIdentifier sid in new[] { Admin, SystemAccount }) acl.AddAccessRule(new FileSystemAccessRule(sid, FileSystemRights.FullControl, InheritanceFlags.ContainerInherit | InheritanceFlags.ObjectInherit, PropagationFlags.None, AccessControlType.Allow));
        acl.AddAccessRule(new FileSystemAccessRule(new SecurityIdentifier("S-1-5-32-545"), FileSystemRights.ReadAndExecute, InheritanceFlags.ContainerInherit | InheritanceFlags.ObjectInherit, PropagationFlags.None, AccessControlType.Allow));
        if (Directory.Exists(path)) Directory.SetAccessControl(path, acl); else Directory.CreateDirectory(path, acl);
        TrustedDirectory(path);
    }
    private static string Hash(string path) {
        Ordinary(path);
        using (FileStream file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read))
        using (SHA256 sha = SHA256.Create()) return BitConverter.ToString(sha.ComputeHash(file)).Replace("-", "").ToLowerInvariant();
    }
    private static string RawServicePath() {
        using (RegistryKey key = Registry.LocalMachine.OpenSubKey(@"SYSTEM\CurrentControlSet\Services\" + contract.service)) {
            if (key == null) return null;
            object value = key.GetValue("ImagePath", null, RegistryValueOptions.DoNotExpandEnvironmentNames);
            if (!Object.Equals(key.GetValue("Type"), 1)) throw new InvalidOperationException("Existing service is not a kernel driver.");
            if (!(value is string)) throw new InvalidOperationException("Driver ImagePath is missing or invalid.");
            return (string)value;
        }
    }
    private static string DriverFile(string path) {
        string value = Environment.ExpandEnvironmentVariables(path).Trim('"');
        if (value.StartsWith(@"\??\", StringComparison.Ordinal)) value = value.Substring(4);
        if (value.StartsWith(@"\SystemRoot\", StringComparison.OrdinalIgnoreCase)) value = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows), value.Substring(12));
        if (value.Length < 3 || !Char.IsLetter(value[0]) || value[1] != ':' || value[2] != '\\' || !String.Equals(Path.GetExtension(value), ".sys", StringComparison.OrdinalIgnoreCase) || value.Contains('"'))
            throw new InvalidOperationException("Unsupported driver path: " + value);
        Ordinary(value);
        return Path.GetFullPath(value);
    }
    private static bool Same(string a, string b) { return String.Equals(a, b, StringComparison.OrdinalIgnoreCase); }
    private static string Quote(string value) {
        StringBuilder result = new StringBuilder("\""); int slashes = 0;
        foreach (char c in value) {
            if (c == '\\') { slashes++; continue; }
            result.Append('\\', c == '"' ? slashes * 2 + 1 : slashes); result.Append(c); slashes = 0;
        }
        result.Append('\\', slashes * 2); return result.Append('"').ToString();
    }
    private static void Sc(params string[] arguments) {
        ProcessStartInfo info = new ProcessStartInfo(Path.Combine(Environment.SystemDirectory, "sc.exe"), String.Join(" ", arguments.Select(Quote)));
        info.UseShellExecute = false; info.CreateNoWindow = true; info.RedirectStandardOutput = true; info.RedirectStandardError = true;
        using (Process process = Process.Start(info)) {
            var error = process.StandardError.ReadToEndAsync(); string output = process.StandardOutput.ReadToEnd(); process.WaitForExit();
            Log(output.Trim()); string diagnostic = error.GetAwaiter().GetResult().Trim(); if (diagnostic.Length != 0) Log(diagnostic);
            if (process.ExitCode != 0) throw new InvalidOperationException("Service command failed (" + process.ExitCode + ").");
        }
    }
    private static ServiceControllerStatus Status() { using (ServiceController s = new ServiceController(contract.service)) return s.Status; }
    private static void Stop() {
        using (ServiceController s = new ServiceController(contract.service)) {
            if (s.Status != ServiceControllerStatus.Stopped) { Sc("stop", contract.service); s.WaitForStatus(ServiceControllerStatus.Stopped, TimeSpan.FromSeconds(20)); }
        }
    }
    private static void Start() {
        Sc("start", contract.service);
        using (ServiceController s = new ServiceController(contract.service)) s.WaitForStatus(ServiceControllerStatus.Running, TimeSpan.FromSeconds(20));
    }
    private static X509Certificate2 ValidatePayload() {
        if (!Same(Hash(Path.Combine(source, "EchoDriver.sys")), contract.driver_sha256) || !Same(Hash(Path.Combine(source, "EchoDriver.cer")), contract.certificate_sha256) ||
            !Same(Hash(Path.Combine(source, "Initialize-TransportKey.ps1")), contract.key_script_sha256)) throw new InvalidOperationException("Driver payload integrity check failed.");
        X509Certificate2 certificate = new X509Certificate2(Path.Combine(source, "EchoDriver.cer"));
        if (certificate.HasPrivateKey || !Same(certificate.Thumbprint, contract.certificate_thumbprint) || DateTime.Now < certificate.NotBefore || DateTime.Now > certificate.NotAfter) {
            certificate.Dispose(); throw new InvalidOperationException("Public driver certificate rejected.");
        }
        return certificate;
    }
    private static void ProvisionKey() {
        if (Status() != ServiceControllerStatus.Stopped) throw new InvalidOperationException("Stop the driver before provisioning its key.");
        using (RegistryKey key = Registry.LocalMachine.CreateSubKey(@"SYSTEM\CurrentControlSet\Services\" + contract.service + @"\Parameters", RegistryKeyPermissionCheck.ReadWriteSubTree)) {
            RegistrySecurity acl = new RegistrySecurity(); acl.SetAccessRuleProtection(true, false);
            foreach (SecurityIdentifier sid in new[] { SystemAccount, Admin }) acl.AddAccessRule(new RegistryAccessRule(sid, RegistryRights.FullControl, InheritanceFlags.ContainerInherit, PropagationFlags.None, AccessControlType.Allow));
            key.SetAccessControl(acl);
            object existing = key.GetValue("TransportKey", null);
            if (existing != null) {
                byte[] bytes = existing as byte[];
                if (bytes == null || bytes.Length != 32) throw new InvalidOperationException("Existing TransportKey is invalid; explicit recovery is required.");
                Log("Existing transport key retained; SYSTEM/Administrators ACL applied."); return;
            }
            byte[] secret = new byte[32];
            try { using (RandomNumberGenerator rng = RandomNumberGenerator.Create()) rng.GetBytes(secret); key.SetValue("TransportKey", secret, RegistryValueKind.Binary); key.Flush(); }
            finally { Array.Clear(secret, 0, secret.Length); }
            Log("Transport key provisioned; key material was not logged.");
        }
    }
    [StructLayout(LayoutKind.Sequential)] private struct TrustFile { public uint size; public IntPtr path, file, subject; }
    [StructLayout(LayoutKind.Sequential)] private struct TrustData {
        public uint size; public IntPtr policy, sip; public uint ui, revocation, choice; public IntPtr file;
        public uint action; public IntPtr state, url; public uint flags, context;
    }
    [DllImport("wintrust.dll", ExactSpelling = true)] private static extern int WinVerifyTrust(IntPtr window, ref Guid action, ref TrustData data);
    private static void VerifySignature(string path) {
        IntPtr name = Marshal.StringToCoTaskMemUni(path), file = IntPtr.Zero;
        TrustData data = new TrustData(); Guid action = new Guid("00AAC56B-CD44-11d0-8CC2-00C04FC295EE");
        try {
            TrustFile info = new TrustFile { size = (uint)Marshal.SizeOf(typeof(TrustFile)), path = name };
            file = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(TrustFile))); Marshal.StructureToPtr(info, file, false);
            data.size = (uint)Marshal.SizeOf(typeof(TrustData)); data.ui = 2; data.choice = 1; data.file = file; data.action = 1;
            int result = WinVerifyTrust(new IntPtr(-1), ref action, ref data);
            if (result != 0) throw new InvalidOperationException("Driver signature verification failed (0x" + result.ToString("X8") + ").");
            using (X509Certificate signer = X509Certificate.CreateFromSignedFile(path))
                if (!Same(signer.GetCertHashString(), contract.certificate_thumbprint)) throw new InvalidOperationException("Driver signer does not match the approved certificate.");
        } finally {
            if (data.state != IntPtr.Zero) { data.action = 2; WinVerifyTrust(new IntPtr(-1), ref action, ref data); }
            if (file != IntPtr.Zero) Marshal.FreeHGlobal(file); Marshal.FreeCoTaskMem(name);
        }
    }
    private static void SaveJournal(string raw, bool running, bool uninstall) {
        SecureJournalDirectory();
        Journal record = new Journal { path = raw, file = DriverFile(raw), running = running, uninstall = uninstall };
        record.sha256 = Hash(record.file);
        if (uninstall) {
            using (RegistryKey key = Registry.LocalMachine.OpenSubKey(@"SYSTEM\CurrentControlSet\Services\" + contract.service + @"\Parameters")) {
                object value = key == null ? null : key.GetValue("TransportKey", null);
                if (value != null) {
                    byte[] secret = value as byte[];
                    if (secret == null || secret.Length != 32) throw new InvalidOperationException("Existing TransportKey is invalid.");
                    try { record.protected_key = Convert.ToBase64String(ProtectedData.Protect(secret, null, DataProtectionScope.LocalMachine)); }
                    finally { Array.Clear(secret, 0, secret.Length); }
                }
            }
            string backup = Path.Combine(root, "driver.json"); Ordinary(backup); File.Copy(Path.Combine(source, "driver.json"), backup, true);
        }
        byte[] bytes = Encoding.UTF8.GetBytes(Json.Serialize(record));
        using (FileStream output = new FileStream(journalPath, FileMode.CreateNew, FileAccess.Write, FileShare.None)) { output.Write(bytes, 0, bytes.Length); output.Flush(true); }
        Log("REPLACING " + contract.service + ": " + raw);
    }
    private static void Install(bool replace, bool wasRunning) {
        if (File.Exists(journalPath)) {
            TrustedDirectory(root); Ordinary(journalPath);
            Journal pending = Json.Deserialize<Journal>(File.ReadAllText(journalPath));
            if (pending == null) throw new InvalidOperationException("Driver transaction journal rejected.");
            Log("REPAIRING interrupted driver transaction before installation.");
            if (pending.uninstall) FinishUninstall(true); else Finish(true);
        }
        using (X509Certificate2 certificate = ValidatePayload()) {
            string raw = RawServicePath();
            bool foreign = raw != null && !Same(DriverFile(raw), managed);
            if (foreign && !replace) throw new InvalidOperationException("Existing driver requires confirmed replacement.");
            if (foreign) SaveJournal(raw, wasRunning || Status() == ServiceControllerStatus.Running, false);
            if (raw != null) Stop();
            foreach (StoreName storeName in new[] { StoreName.Root, StoreName.TrustedPublisher }) {
                using (X509Store store = new X509Store(storeName, StoreLocation.LocalMachine)) { store.Open(OpenFlags.ReadWrite); store.Add(certificate); }
            }
            string driver = Path.Combine(source, "EchoDriver.sys"); VerifySignature(driver);
            Ordinary(managed); Directory.CreateDirectory(Path.GetDirectoryName(managed)); File.Copy(driver, managed, true);
            if (raw == null) Sc("create", contract.service, "type=", "kernel", "start=", "demand", "binPath=", @"\??\" + managed);
            else Sc("config", contract.service, "binPath=", @"\??\" + managed);
            ProvisionKey(); Start();
            if (!Same(DriverFile(RawServicePath()), managed) || !Same(Hash(managed), contract.driver_sha256)) throw new InvalidOperationException("Installed driver verification failed.");
            Log("Driver installed and running.");
        }
    }
    private static void Uninstall(bool preserve, bool wasRunning) {
        string raw = RawServicePath();
        if (raw != null) {
            if (!Same(DriverFile(raw), managed)) throw new InvalidOperationException("Driver belongs to another installation.");
            if (preserve) SaveJournal(raw, wasRunning || Status() == ServiceControllerStatus.Running, true);
            Stop(); Sc("delete", contract.service);
        }
        Ordinary(managed); if (File.Exists(managed)) File.Delete(managed);
        Log("Managed driver removed. Existing certificate trust and Windows security settings retained.");
    }
    private static void Finish(bool rollback) {
        if (!File.Exists(journalPath)) return;
        TrustedDirectory(root); Ordinary(journalPath);
        Journal record = Json.Deserialize<Journal>(File.ReadAllText(journalPath));
        if (record == null || !Same(DriverFile(record.path), record.file) || !Same(Hash(record.file), record.sha256)) throw new InvalidOperationException("Original driver changed; journal retained for manual recovery.");
        string current = RawServicePath();
        if (current == null) throw new InvalidOperationException("Driver service disappeared; journal retained.");
        if (rollback) {
            if (!Same(DriverFile(current), managed) && !Same(DriverFile(current), record.file)) throw new InvalidOperationException("Driver ownership changed; journal retained.");
            Stop(); Sc("config", contract.service, "binPath=", record.path); if (record.running) Start();
            if (!Same(DriverFile(RawServicePath()), record.file)) throw new InvalidOperationException("Restored service verification failed.");
            Log("RESTORED driver path and running state; original files and transport key retained.");
        } else {
            if (!Same(DriverFile(current), managed) || Status() != ServiceControllerStatus.Running) throw new InvalidOperationException("Managed driver is not running; journal retained.");
            Log("REPLACED driver; original files and transport key retained.");
        }
        File.Delete(journalPath);
    }
    private static void FinishUninstall(bool rollback) {
        TrustedDirectory(root); Ordinary(journalPath);
        Journal record = Json.Deserialize<Journal>(File.ReadAllText(journalPath));
        if (record == null || !record.uninstall || !Same(DriverFile(record.path), managed)) throw new InvalidOperationException("Uninstall journal rejected.");
        if (rollback) {
            if (!Same(Hash(record.file), record.sha256)) throw new InvalidOperationException("Restored driver bytes do not match the uninstall snapshot.");
            string raw = RawServicePath();
            if (raw == null) Sc("create", contract.service, "type=", "kernel", "start=", "demand", "binPath=", record.path);
            else if (!Same(DriverFile(raw), managed)) throw new InvalidOperationException("Service ownership changed during uninstall.");
            Stop();
            if (record.protected_key != null) {
                byte[] secret = ProtectedData.Unprotect(Convert.FromBase64String(record.protected_key), null, DataProtectionScope.LocalMachine);
                try {
                    if (secret.Length != 32) throw new InvalidOperationException("Protected transport key is invalid.");
                    using (RegistryKey key = Registry.LocalMachine.CreateSubKey(@"SYSTEM\CurrentControlSet\Services\" + contract.service + @"\Parameters", RegistryKeyPermissionCheck.ReadWriteSubTree)) {
                        RegistrySecurity acl = new RegistrySecurity(); acl.SetAccessRuleProtection(true, false);
                        foreach (SecurityIdentifier sid in new[] { SystemAccount, Admin }) acl.AddAccessRule(new RegistryAccessRule(sid, RegistryRights.FullControl, InheritanceFlags.ContainerInherit, PropagationFlags.None, AccessControlType.Allow));
                        key.SetAccessControl(acl); key.SetValue("TransportKey", secret, RegistryValueKind.Binary); key.Flush();
                    }
                } finally { Array.Clear(secret, 0, secret.Length); }
            }
            if (record.running) Start();
            Log("RESTORED uninstall state and protected transport key.");
        } else {
            if (RawServicePath() != null || File.Exists(managed)) throw new InvalidOperationException("Uninstall is incomplete; journal retained.");
            Log("UNINSTALLED managed driver.");
        }
        File.Delete(journalPath); File.Delete(Path.Combine(root, "driver.json"));
    }
    public static int Main(string[] args) {
        try {
            if (args.Length < 2 || args.Length > 4 || !new[] { "Install", "Uninstall", "RemoveNew", "Rollback", "Commit", "RestoreUninstall", "CommitUninstall", "Inspect" }.Contains(args[0]) || args.Skip(2).Any(a => a != "0" && a != "1")) throw new InvalidOperationException("Invalid driver installer arguments.");
            source = Path.GetFullPath(args[1]); Ordinary(source);
            root = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), @"VeruxAPB\Migration"); journalPath = Path.Combine(root, "EchoDriver.json");
            bool uninstallCompletion = args[0] == "RestoreUninstall" || args[0] == "CommitUninstall";
            if (uninstallCompletion && !File.Exists(journalPath)) return 0;
            if (uninstallCompletion) TrustedDirectory(root);
            string manifest = Path.Combine(uninstallCompletion ? root : source, "driver.json"); Ordinary(manifest);
            if (new FileInfo(manifest).Length > 65536) throw new InvalidOperationException("Driver contract too large.");
            contract = Json.Deserialize<Contract>(File.ReadAllText(manifest));
            if (contract == null || contract.service != "EchoDriver" || contract.destination != @"VeruxAPB\Driver\EchoDriver.sys" || new[] { contract.driver_sha256, contract.certificate_sha256, contract.key_script_sha256 }.Any(s => s == null || s.Length != 64 || s.Any(c => !Uri.IsHexDigit(c))) || contract.certificate_thumbprint == null || contract.certificate_thumbprint.Length != 40) throw new InvalidOperationException("Driver contract rejected.");
            managed = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), contract.destination);
            if (args[0] == "Inspect") { using (X509Certificate2 certificate = ValidatePayload()) { VerifySignature(Path.Combine(source, "EchoDriver.sys")); } Console.WriteLine("Driver payload and signer verified."); return 0; }
            using (WindowsIdentity identity = WindowsIdentity.GetCurrent()) if (!new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator)) throw new InvalidOperationException("Administrator privileges required.");
            string parent = Path.GetDirectoryName(root); ApplicationDirectory(parent);
            string logs = Path.Combine(parent, "Logs"); ApplicationDirectory(logs);
            logPath = Path.Combine(logs, "EchoDriver-migration.log"); Ordinary(logPath); Log(args[0] + " requested");
            if (Directory.Exists(root)) TrustedDirectory(root);
            bool wasRunning = args.Length > 3 && args[3] == "1";
            switch (args[0]) { case "Install": Install(args.Length > 2 && args[2] == "1", wasRunning); break; case "Uninstall": Uninstall(true, wasRunning); break; case "RemoveNew": Uninstall(false, false); break; case "Rollback": Finish(true); break; case "Commit": Finish(false); break; case "RestoreUninstall": FinishUninstall(true); break; case "CommitUninstall": FinishUninstall(false); break; }
            Log(args[0] + " completed"); return 0;
        } catch (Exception error) {
            try { Log("FAILED: " + error.Message); } catch (Exception logging) { Console.Error.WriteLine("Diagnostic write failed: " + logging.Message); }
            Console.Error.WriteLine(error.ToString()); return 1;
        }
    }
}
