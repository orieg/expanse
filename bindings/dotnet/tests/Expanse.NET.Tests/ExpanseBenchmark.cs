using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using Xunit;
using Xunit.Abstractions;

namespace Expanse.Tests;

public class ExpanseBenchmark
{
    // Marker line prefix so scripts/bench_bindings.py can pull the single JSON
    // result line out of `dotnet test` output (which interleaves xunit/VSTest
    // logging on stdout) without depending on log verbosity or formatting.
    private const string JsonMarker = "##EXPANSE_BENCH_JSON##";

    private readonly ITestOutputHelper _output;

    public ExpanseBenchmark(ITestOutputHelper output)
    {
        _output = output;
    }

    private struct XorShift64
    {
        private ulong _state;
        public XorShift64(ulong seed) => _state = seed;
        public ulong Next()
        {
            ulong x = _state;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            _state = x;
            return x;
        }
    }

    private static ulong[] GenerateKeys(int pop, string dist)
    {
        var rng = new XorShift64(0x0DDB_1A5E_5EED_0001UL);
        var keys = new ulong[pop];
        if (dist == "sequential")
        {
            for (int i = 0; i < pop; i++) keys[i] = (ulong)i;
        }
        else if (dist == "clustered")
        {
            ulong b = 0;
            for (int i = 0; i < pop; i++)
            {
                if (i % 256 == 0) b = rng.Next() & ~0xFFUL;
                keys[i] = b + (ulong)(i % 256);
            }
        }
        else
        {
            for (int i = 0; i < pop; i++) keys[i] = rng.Next();
        }
        return keys;
    }

    private static double Measure(Action action, int rounds = 3)
    {
        double best = double.PositiveInfinity;
        for (int r = 0; r < rounds; r++)
        {
            var sw = Stopwatch.StartNew();
            action();
            sw.Stop();
            double dt = sw.Elapsed.TotalSeconds;
            if (dt < best) best = dt;
        }
        return best;
    }

    [Fact]
    public void RunComparativeBenchmark()
    {
        // EXPANSE_BENCH_QUICK / EXPANSE_BENCH_JSON let scripts/bench_bindings.py drive this
        // xunit Fact the same way the other bindings' bench.{py,php,rb} CLI scripts accept
        // --quick/--json: `dotnet test` has no first-class way to pass argv into a test method,
        // so environment variables are the equivalent knob here.
        bool quick = Environment.GetEnvironmentVariable("EXPANSE_BENCH_QUICK") == "1";
        bool jsonMode = Environment.GetEnvironmentVariable("EXPANSE_BENCH_JSON") == "1";
        int pop = quick ? 10_000 : 20_000;
        string[] dists = { "random", "sequential", "clustered" };
        var jsonResults = new List<string>();

        if (!jsonMode)
        {
            _output.WriteLine("\n================================================================================");
            _output.WriteLine("  Expanse .NET (C#) Comparative Performance Report");
            _output.WriteLine("================================================================================");
        }

        foreach (var dist in dists)
        {
            var keys = GenerateKeys(pop, dist);
            var probeKeys = (ulong[])keys.Clone();
            Array.Reverse(probeKeys);

            // Lookup sinks escape into this accumulator, which is read after
            // timing (emitted as sink_checksum), so the JIT cannot
            // dead-code-eliminate the probed lookups as dead locals (#373).
            ulong sinkGuard = 0;

            // 1. ExpanseMap
            GC.Collect();
            GC.WaitForPendingFinalizers();
            using var expMap = new ExpanseMap();

            double expInsertS = Measure(() =>
            {
                expMap.Clear();
                for (int i = 0; i < pop; i++) expMap[keys[i]] = keys[i] ^ 0x55UL;
            });

            double expLookupS = Measure(() =>
            {
                ulong sink = 0;
                for (int i = 0; i < pop; i++)
                {
                    if (expMap.TryGet(probeKeys[i], out ulong v)) sink ^= v;
                }
                sinkGuard ^= sink;
            });

            double expBytesPerKey = (double)expMap.MemoryUsed / pop;

            // 2. Dictionary<ulong, ulong> — managed-heap delta measured around
            // the build via GC.GetTotalMemory(true). Approximate by nature; a
            // non-positive delta is emitted as null, never a hardcoded
            // constant (pre-#373 this row fabricated 32.0).
            long heapBefore = GC.GetTotalMemory(forceFullCollection: true);
            var dict = new Dictionary<ulong, ulong>(pop);

            double dictInsertS = Measure(() =>
            {
                dict.Clear();
                for (int i = 0; i < pop; i++) dict[keys[i]] = keys[i] ^ 0x55UL;
            });

            double dictLookupS = Measure(() =>
            {
                ulong sink = 0;
                for (int i = 0; i < pop; i++)
                {
                    if (dict.TryGetValue(probeKeys[i], out ulong v)) sink ^= v;
                }
                sinkGuard ^= sink;
            });

            long heapAfter = GC.GetTotalMemory(forceFullCollection: true);
            GC.KeepAlive(dict);
            double? dictBytesPerKey = heapAfter > heapBefore ? (double)(heapAfter - heapBefore) / pop : (double?)null;

            // 3. ExpanseSet
            using var expSet = new ExpanseSet();
            double expSetInsertS = Measure(() =>
            {
                expSet.Clear();
                for (int i = 0; i < pop; i++) expSet.Add(keys[i]);
            });

            double expSetLookupS = Measure(() =>
            {
                int count = 0;
                for (int i = 0; i < pop; i++) if (expSet.Contains(probeKeys[i])) count++;
                sinkGuard ^= (ulong)count;
            });

            // 4. HashSet<ulong>
            var hashSet = new HashSet<ulong>(pop);
            double setInsertS = Measure(() =>
            {
                hashSet.Clear();
                for (int i = 0; i < pop; i++) hashSet.Add(keys[i]);
            });

            double setLookupS = Measure(() =>
            {
                int count = 0;
                for (int i = 0; i < pop; i++) if (hashSet.Contains(probeKeys[i])) count++;
                sinkGuard ^= (ulong)count;
            });

            double toMops = pop / 1e6;

            if (jsonMode)
            {
                double expLookupNs = expLookupS * 1e9 / pop;
                double expInsertMops = toMops / expInsertS;
                double expLookupMops = toMops / expLookupS;
                double dictInsertMops = toMops / dictInsertS;
                double dictLookupMops = toMops / dictLookupS;
                double dictLookupNs = dictLookupS * 1e9 / pop;
                string dictMemJson = dictBytesPerKey.HasValue
                    ? string.Format(CultureInfo.InvariantCulture, "\"bytes_per_key\": {0:F2}, \"bytes_per_key_approximate\": true", dictBytesPerKey.Value)
                    : "\"bytes_per_key\": null";

                jsonResults.Add(string.Format(CultureInfo.InvariantCulture,
                    "{{\"dist\": \"{0}\", \"pop\": {1}, \"sink_checksum\": \"0x{2:x}\", \"expanse_map\": {{\"insert_mops\": {3:F2}, \"lookup_mops\": {4:F2}, \"lookup_ns\": {5:F2}, \"bytes_per_key\": {6:F2}}}, \"dotnet_dictionary\": {{\"insert_mops\": {7:F2}, \"lookup_mops\": {8:F2}, \"lookup_ns\": {9:F2}, {10}}}}}",
                    dist, pop, sinkGuard, expInsertMops, expLookupMops, expLookupNs, expBytesPerKey, dictInsertMops, dictLookupMops, dictLookupNs, dictMemJson));
            }
            else
            {
                string dictMemCell = dictBytesPerKey.HasValue
                    ? string.Format(CultureInfo.InvariantCulture, "{0,8:F2}", dictBytesPerKey.Value)
                    : string.Format("{0,8}", "n/a");
                _output.WriteLine($"\n[ Distribution: {dist} | Population: {pop:N0} ]");
                _output.WriteLine($"{"Target",-20} | {"Lookup (ns)",11} | {"Lookup (Mops)",13} | {"Insert (Mops)",13} | {"B/key",8}");
                _output.WriteLine($"{new string('-', 20)}-+-{new string('-', 11)}-+-{new string('-', 13)}-+-{new string('-', 13)}-+-{new string('-', 8)}");
                _output.WriteLine($"{"ExpanseMap (.NET)",-20} | {(expLookupS * 1e9 / pop),11:F2} | {(toMops / expLookupS),13:F2} | {(toMops / expInsertS),13:F2} | {expBytesPerKey,8:F2}");
                _output.WriteLine($"{"Dictionary<u64,u64>",-20} | {(dictLookupS * 1e9 / pop),11:F2} | {(toMops / dictLookupS),13:F2} | {(toMops / dictInsertS),13:F2} | {dictMemCell}");
                _output.WriteLine($"{"ExpanseSet (.NET)",-20} | {(expSetLookupS * 1e9 / pop),11:F2} | {(toMops / expSetLookupS),13:F2} | {(toMops / expSetInsertS),13:F2} | {"—",8}");
                _output.WriteLine($"{"HashSet<ulong>",-20} | {(setLookupS * 1e9 / pop),11:F2} | {(toMops / setLookupS),13:F2} | {(toMops / setInsertS),13:F2} | {"—",8}");
                _output.WriteLine($"(sink checksum: 0x{sinkGuard:x})");
            }
        }

        if (jsonMode)
        {
            // Emitted via Console (not ITestOutputHelper) + a unique marker prefix so it
            // survives `dotnet test`'s VSTest logging/interleaving and is trivially
            // greppable by scripts/bench_bindings.py regardless of --logger verbosity.
            Console.WriteLine(JsonMarker + "{\"runtime\": \"dotnet\", \"results\": [" + string.Join(",", jsonResults) + "]}");
        }
    }
}
