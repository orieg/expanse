using System;
using System.IO;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Expanse.Native;

internal static class NativeLoader
{
    private static bool _initialized = false;
    private static readonly object _lock = new();

    [ModuleInitializer]
    internal static void Initialize()
    {
        lock (_lock)
        {
            if (_initialized) return;
            NativeLibrary.SetDllImportResolver(typeof(NativeLoader).Assembly, DllImportResolver);
            _initialized = true;
        }
    }

    private static IntPtr DllImportResolver(string libraryName, Assembly assembly, DllImportSearchPath? searchPath)
    {
        if (!string.Equals(libraryName, "expanse", StringComparison.OrdinalIgnoreCase) &&
            !string.Equals(libraryName, "libexpanse", StringComparison.OrdinalIgnoreCase))
        {
            return IntPtr.Zero;
        }

        // 1. Check explicit environment variable EXPANSE_CDYLIB
        string? envCdylib = Environment.GetEnvironmentVariable("EXPANSE_CDYLIB");
        if (!string.IsNullOrEmpty(envCdylib) && File.Exists(envCdylib))
        {
            if (NativeLibrary.TryLoad(envCdylib, out IntPtr handle))
            {
                return handle;
            }
        }

        // 2. Check explicit directory EXPANSE_LIB_DIR
        string? envLibDir = Environment.GetEnvironmentVariable("EXPANSE_LIB_DIR");
        string nativeLibName = GetNativeLibraryFileName();

        if (!string.IsNullOrEmpty(envLibDir))
        {
            string candidate = Path.Combine(envLibDir, nativeLibName);
            if (File.Exists(candidate) && NativeLibrary.TryLoad(candidate, out IntPtr handle))
            {
                return handle;
            }
        }

        // 3. Search directory of current executing assembly & base directory
        string baseDir = AppDomain.CurrentDomain.BaseDirectory;
        string currentAssemblyDir = Path.GetDirectoryName(assembly.Location) ?? baseDir;

        string[] searchDirs =
        {
            currentAssemblyDir,
            baseDir,
            Path.Combine(baseDir, "runtimes", GetRuntimeIdentifier(), "native"),
            Path.Combine(currentAssemblyDir, "runtimes", GetRuntimeIdentifier(), "native"),
            // Cargo target lookup paths for local dev & testing
            Path.Combine(baseDir, "..", "..", "..", "..", "target", "release"),
            Path.Combine(baseDir, "..", "..", "..", "..", "target", "debug"),
            Path.Combine(baseDir, "..", "..", "..", "target", "release"),
            Path.Combine(baseDir, "..", "..", "..", "target", "debug"),
            Path.Combine(baseDir, "..", "..", "target", "release"),
            Path.Combine(baseDir, "..", "..", "target", "debug"),
            Path.Combine(baseDir, "..", "target", "release"),
            Path.Combine(baseDir, "..", "target", "debug"),
            Path.Combine(baseDir, "target", "release"),
            Path.Combine(baseDir, "target", "debug"),
            Path.Combine(currentAssemblyDir, "..", "..", "..", "..", "target", "release"),
            Path.Combine(currentAssemblyDir, "..", "..", "..", "..", "target", "debug"),
            Path.Combine(currentAssemblyDir, "..", "..", "..", "target", "release"),
            Path.Combine(currentAssemblyDir, "..", "..", "..", "target", "debug"),
            Path.Combine(currentAssemblyDir, "..", "..", "target", "release"),
            Path.Combine(currentAssemblyDir, "..", "..", "target", "debug"),
            "/usr/local/lib",
            "/usr/lib",
            "/opt/homebrew/lib"
        };

        foreach (string dir in searchDirs)
        {
            if (!Directory.Exists(dir)) continue;

            string fullPath = Path.GetFullPath(Path.Combine(dir, nativeLibName));
            if (File.Exists(fullPath) && NativeLibrary.TryLoad(fullPath, out IntPtr handle))
            {
                return handle;
            }
        }

        // 4. Fallback to OS default search
        if (NativeLibrary.TryLoad(libraryName, assembly, searchPath, out IntPtr defaultHandle))
        {
            return defaultHandle;
        }

        return IntPtr.Zero;
    }

    private static string GetNativeLibraryFileName()
    {
        if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
        {
            return "expanse.dll";
        }
        if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
        {
            return "libexpanse.dylib";
        }
        return "libexpanse.so";
    }

    private static string GetRuntimeIdentifier()
    {
        string arch = RuntimeInformation.ProcessArchitecture switch
        {
            Architecture.X64 => "x64",
            Architecture.Arm64 => "arm64",
            Architecture.X86 => "x86",
            Architecture.Arm => "arm",
            _ => "x64"
        };

        if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
        {
            return $"win-{arch}";
        }
        if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
        {
            return $"osx-{arch}";
        }
        return $"linux-{arch}";
    }
}
