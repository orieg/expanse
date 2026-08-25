using System;
using System.Collections.Generic;
using System.Diagnostics;
using Xunit;
using Xunit.Abstractions;

namespace Expanse.Tests;

public class ExpanseBenchmark
{
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
        int pop = 20_000;
        string[] dists = { "random", "sequential", "clustered" };

        _output.WriteLine("\n================================================================================");
        _output.WriteLine("  Expanse .NET (C#) Comparative Performance Report");
        _output.WriteLine("================================================================================");

        foreach (var dist in dists)
        {
            var keys = GenerateKeys(pop, dist);
            var probeKeys = (ulong[])keys.Clone();
            Array.Reverse(probeKeys);

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
            });

            double expBytesPerKey = (double)expMap.MemoryUsed / pop;

            // 2. Dictionary<ulong, ulong>
            GC.Collect();
            GC.WaitForPendingFinalizers();
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
            });

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
            });

            double toMops = pop / 1e6;
            _output.WriteLine($"\n[ Distribution: {dist} | Population: {pop:N0} ]");
            _output.WriteLine($"{"Target",-20} | {"Lookup (ns)",11} | {"Lookup (Mops)",13} | {"Insert (Mops)",13} | {"B/key",8}");
            _output.WriteLine($"{new string('-', 20)}-+-{new string('-', 11)}-+-{new string('-', 13)}-+-{new string('-', 13)}-+-{new string('-', 8)}");
            _output.WriteLine($"{"ExpanseMap (.NET)",-20} | {(expLookupS * 1e9 / pop),11:F2} | {(toMops / expLookupS),13:F2} | {(toMops / expInsertS),13:F2} | {expBytesPerKey,8:F2}");
            _output.WriteLine($"{"Dictionary<u64,u64>",-20} | {(dictLookupS * 1e9 / pop),11:F2} | {(toMops / dictLookupS),13:F2} | {(toMops / dictInsertS),13:F2} | {32.0,8:F2}");
            _output.WriteLine($"{"ExpanseSet (.NET)",-20} | {(expSetLookupS * 1e9 / pop),11:F2} | {(toMops / expSetLookupS),13:F2} | {(toMops / expSetInsertS),13:F2} | {"—",8}");
            _output.WriteLine($"{"HashSet<ulong>",-20} | {(setLookupS * 1e9 / pop),11:F2} | {(toMops / setLookupS),13:F2} | {(toMops / setInsertS),13:F2} | {"—",8}");
        }
    }
}
