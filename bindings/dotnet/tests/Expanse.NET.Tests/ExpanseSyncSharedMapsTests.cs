using System;
using System.Buffers.Binary;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Xunit;

namespace Expanse.Tests;

/// <summary>
/// Tests of the shared string, bytes, ordered-bytes and blob maps (the .NET wrappers of the
/// <c>expanse_sync_*map</c> C handles, issue #1538).
/// </summary>
public class ExpanseSyncSharedMapsTests
{
    private static readonly ParallelOptions Eight = new() { MaxDegreeOfParallelism = 8 };

    private static byte[] Bytes(params byte[] b) => b;

    // A payload whose every byte is a function of its length, so a torn read (bytes of one value, length
    // or metadata of another) is detectable from the payload alone.
    private static byte[] SelfDescribing(int len)
    {
        var b = new byte[len];
        Array.Fill(b, (byte)(len % 251));
        return b;
    }

    // ------------------------------------------------------------------ string map

    [Fact]
    public void StrMapPointOperationsAndReader()
    {
        using var map = new ExpanseSyncStrMap();
        Assert.True(map.Set("apple", 1));
        Assert.False(map.Set("apple", 2));
        Assert.True(map.Insert("banana", 20, out ulong old));
        Assert.Equal(0UL, old);
        Assert.False(map.Insert("banana", 21, out old));
        Assert.Equal(20UL, old);
        Assert.True(map.Set("日本語", 3));
        Assert.True(map.Set("", 4));
        Assert.Equal(4UL, map.Count);

        Assert.True(map.TryGet("apple", out ulong v));
        Assert.Equal(2UL, v);
        Assert.True(map.ContainsKey("日本語"));
        Assert.True(map.TryGet("", out v));
        Assert.Equal(4UL, v);
        Assert.False(map.ContainsKey("cherry"));

        using (var reader = map.CreateReader())
        {
            Assert.True(reader.TryGet("banana", out v));
            Assert.Equal(21UL, v);
            Assert.True(reader.ContainsKey("日本語"));
            Assert.False(reader.ContainsKey("cherry"));
        }

        Assert.True(map.Remove("apple", out old));
        Assert.Equal(2UL, old);
        Assert.False(map.Remove("apple"));
        Assert.True(map.Remove("banana"));
        Assert.Equal(2UL, map.Count);
        Assert.True(map.MemoryUsed > 0);
        Assert.True(map.MemoryHeld > 0);
        Assert.Equal(map.MemoryHeld - map.ShrinkToFit(), map.MemoryHeld);
    }

    [Fact]
    public void StrMapRejectsEmbeddedNul()
    {
        using var map = new ExpanseSyncStrMap();
        Assert.Throws<ArgumentException>(() => map.Set("a\0b", 1));
        Assert.Throws<ArgumentException>(() => map.TryGet("a\0b", out _));
        Assert.Throws<ArgumentException>(() => map.CompareExchange("a\0b", null, 1, out _));
        Assert.Throws<ArgumentException>(() => map.GetOrInsert("a\0b", 1));
        Assert.Throws<ArgumentNullException>(() => map.Set((string)null!, 1));
        Assert.Equal(0UL, map.Count);
        using var reader = map.CreateReader();
        Assert.Throws<ArgumentException>(() => reader.ContainsKey("\0"));
        Assert.Throws<ArgumentException>(() => reader.Next("x\0"));
    }

    [Fact]
    public void StrMapReaderOrderedReads()
    {
        using var map = new ExpanseSyncStrMap();
        using var reader = map.CreateReader();
        Assert.Null(reader.First());
        Assert.Null(reader.Last());
        Assert.Null(reader.Next("a"));

        foreach (string k in new[] { "banana", "apple", "date", "cherry" })
        {
            map.Set(k, (ulong)k.Length);
        }

        Assert.Equal(new KeyValuePair<string, ulong>("apple", 5), reader.First());
        Assert.Equal(new KeyValuePair<string, ulong>("date", 4), reader.Last());
        Assert.Equal("banana", reader.Next("apple")?.Key);
        Assert.Null(reader.Next("date"));
        Assert.Equal("cherry", reader.NextAtOrAfter("cherry")?.Key);
        Assert.Equal("cherry", reader.NextAtOrAfter("c")?.Key);
        Assert.Null(reader.NextAtOrAfter("e"));
        Assert.Equal("apple", reader.Prev("banana")?.Key);
        Assert.Null(reader.Prev("apple"));
        Assert.Null(reader.Prev(""));
        Assert.Equal("banana", reader.PrevAtOrBefore("banana")?.Key);
        Assert.Equal("banana", reader.PrevAtOrBefore("c")?.Key);
        Assert.Null(reader.PrevAtOrBefore("a"));

        var walked = new List<string>();
        for (var e = reader.First(); e is { } cur; e = reader.Next(cur.Key))
        {
            walked.Add(cur.Key);
        }
        Assert.Equal(new[] { "apple", "banana", "cherry", "date" }, walked);
    }

    [Fact]
    public void StrMapKeysLongerThanTheInitialBuffers()
    {
        // 5000-byte keys exceed both the 256-byte key stack and the 256-byte navigation buffer, so the
        // wrapper takes its pooled and BUFFER_TOO_SMALL retry paths.
        string longA = new string('a', 5000) + "1";
        string longB = new string('a', 5000) + "2";
        using var map = new ExpanseSyncStrMap();
        map.Set(longB, 2);
        map.Set(longA, 1);
        Assert.True(map.TryGet(longA, out ulong v));
        Assert.Equal(1UL, v);

        using var reader = map.CreateReader();
        Assert.Equal(new KeyValuePair<string, ulong>(longA, 1), reader.First());
        Assert.Equal(new KeyValuePair<string, ulong>(longB, 2), reader.Last());
        Assert.Equal(new KeyValuePair<string, ulong>(longB, 2), reader.Next(longA));
        Assert.Equal(new KeyValuePair<string, ulong>(longA, 1), reader.Prev(longB));
        Assert.Equal(new KeyValuePair<string, ulong>(longA, 1), reader.NextAtOrAfter("a"));
    }

    [Fact]
    public void StrMapCompareExchangeGetOrInsertAndUpdate()
    {
        using var m = new ExpanseSyncStrMap();
        ulong? seen;
        Assert.True(m.CompareExchange("k", null, null, out seen));
        Assert.Null(seen);
        Assert.False(m.ContainsKey("k"));
        Assert.True(m.CompareExchange("k", null, 10, out seen));
        Assert.Null(seen);
        Assert.False(m.CompareExchange("k", null, 99, out seen));
        Assert.Equal(10UL, seen);
        Assert.False(m.CompareExchange("k", 11, 12, out seen));
        Assert.Equal(10UL, seen);
        Assert.False(m.CompareExchange("absent", 5, 6, out seen));
        Assert.Null(seen);
        Assert.True(m.CompareExchange("k", 10, 20, out seen));
        Assert.Equal(10UL, seen);
        Assert.False(m.CompareExchange("k", 21, null, out seen));
        Assert.Equal(20UL, seen);
        Assert.True(m.CompareExchange("k", 20, null, out seen));
        Assert.False(m.ContainsKey("k"));
        Assert.True(m.CompareExchange("z", null, 0, out seen));
        Assert.True(m.CompareExchange("z", 0, ulong.MaxValue, out seen));
        Assert.True(m.TryGet("z", out ulong v));
        Assert.Equal(ulong.MaxValue, v);

        Assert.Null(m.GetOrInsert("g", 7));
        Assert.Equal(7UL, m.GetOrInsert("g", 8));
        Assert.True(m.TryGet("g", out v));
        Assert.Equal(7UL, v);
        Assert.Null(m.GetOrInsert("zero", 0));
        Assert.Equal(0UL, m.GetOrInsert("zero", 5));

        Assert.Null(m.Update("u", cur => (cur ?? 0) + 1));
        Assert.Equal(1UL, m.Update("u", cur => (cur ?? 0) + 1));
        Assert.Equal(2UL, m.Update("u", cur => null));
        Assert.False(m.ContainsKey("u"));
    }

    [Fact]
    public void StrMapReaderKeepsMapAliveAcrossGarbageCollection()
    {
        ExpanseSyncStrMapReader reader = CreateOrphanStrReader();
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        Assert.True(reader.TryGet("answer", out ulong value));
        Assert.Equal(42UL, value);
        reader.Dispose();
    }

    private static ExpanseSyncStrMapReader CreateOrphanStrReader()
    {
        var map = new ExpanseSyncStrMap();
        map.Set("answer", 42);
        return map.CreateReader();
    }

    [Fact]
    public void StrMapConcurrentWritersAndReaders()
    {
        using var map = new ExpanseSyncStrMap();
        const int threads = 8;
        const int bumps = 2000;
        const int perThread = 200;
        int winners = 0;
        int done = 0;

        Task[] readers = Enumerable.Range(0, 4).Select(_ => Task.Run(() =>
        {
            using var reader = map.CreateReader();
            ulong last = 0;
            while (Volatile.Read(ref done) == 0)
            {
                if (reader.TryGet("counter", out ulong v))
                {
                    Assert.True(v >= last, "a counter bumped by compare-exchange never goes backwards");
                    last = v;
                }
                if (reader.First() is { } first)
                {
                    Assert.True(first.Key.Length > 0);
                }
            }
        })).ToArray();

        Parallel.For(0, threads, Eight, id =>
        {
            for (int i = 0; i < bumps; i++)
            {
                map.Update("counter", cur => (cur ?? 0) + 1);
            }
            for (int i = 0; i < perThread; i++)
            {
                map.Set($"t{id}-{i}", (ulong)i);
            }
            if (map.GetOrInsert("once", (ulong)id) is null)
            {
                Interlocked.Increment(ref winners);
            }
        });
        Volatile.Write(ref done, 1);
        Task.WaitAll(readers);

        Assert.Equal(1, winners);
        Assert.True(map.TryGet("counter", out ulong total));
        Assert.Equal((ulong)(threads * bumps), total);
        Assert.Equal((ulong)(2 + threads * perThread), map.Count);
        using var check = map.CreateReader();
        for (int id = 0; id < threads; id++)
        {
            for (int i = 0; i < perThread; i++)
            {
                Assert.True(check.TryGet($"t{id}-{i}", out ulong v));
                Assert.Equal((ulong)i, v);
            }
        }
    }

    // ------------------------------------------------------------------ bytes map

    [Fact]
    public void BytesMapPointOperationsAndReader()
    {
        using var map = new ExpanseSyncBytesMap();
        byte[] withNul = Bytes(1, 0, 2);
        byte[] prefix = Bytes(1);
        Assert.True(map.Set(withNul, 10));
        Assert.True(map.Set(prefix, 11));
        Assert.True(map.Set(ReadOnlySpan<byte>.Empty, 12));
        Assert.False(map.Set(withNul, 13));
        Assert.True(map.Insert(Bytes(9, 9), 14, out ulong old));
        Assert.Equal(0UL, old);
        Assert.False(map.Insert(Bytes(9, 9), 15, out old));
        Assert.Equal(14UL, old);
        Assert.Equal(4UL, map.Count);

        Assert.True(map.TryGet(withNul, out ulong v));
        Assert.Equal(13UL, v);
        Assert.True(map.TryGet(prefix, out v));
        Assert.Equal(11UL, v);
        Assert.True(map.TryGet(ReadOnlySpan<byte>.Empty, out v));
        Assert.Equal(12UL, v);
        Assert.False(map.ContainsKey(Bytes(1, 0)));

        using (var reader = map.CreateReader())
        {
            Assert.True(reader.TryGet(withNul, out v));
            Assert.Equal(13UL, v);
            Assert.True(reader.ContainsKey(ReadOnlySpan<byte>.Empty));
            Assert.False(reader.ContainsKey(Bytes(1, 0)));
        }

        Assert.True(map.Remove(withNul, out old));
        Assert.Equal(13UL, old);
        Assert.False(map.Remove(withNul));
        Assert.True(map.Remove(ReadOnlySpan<byte>.Empty));
        Assert.Equal(2UL, map.Count);
        Assert.True(map.MemoryUsed > 0);
        Assert.True(map.MemoryHeld > 0);
        Assert.Equal(map.MemoryHeld - map.ShrinkToFit(), map.MemoryHeld);
    }

    [Fact]
    public void BytesMapCompareExchangeGetOrInsertAndUpdate()
    {
        using var m = new ExpanseSyncBytesMap();
        byte[] key = Bytes(0, 1, 0);
        ulong? seen;
        Assert.True(m.CompareExchange(key, null, null, out seen));
        Assert.Null(seen);
        Assert.True(m.CompareExchange(key, null, 10, out seen));
        Assert.Null(seen);
        Assert.False(m.CompareExchange(key, null, 99, out seen));
        Assert.Equal(10UL, seen);
        Assert.False(m.CompareExchange(key, 11, 12, out seen));
        Assert.Equal(10UL, seen);
        Assert.True(m.CompareExchange(key, 10, 20, out seen));
        Assert.Equal(10UL, seen);
        Assert.False(m.CompareExchange(Bytes(5), 1, 2, out seen));
        Assert.Null(seen);
        Assert.True(m.CompareExchange(key, 20, null, out seen));
        Assert.False(m.ContainsKey(key));
        // The empty key is a key.
        Assert.True(m.CompareExchange(ReadOnlySpan<byte>.Empty, null, 0, out seen));
        Assert.True(m.ContainsKey(ReadOnlySpan<byte>.Empty));

        Assert.Null(m.GetOrInsert(Bytes(7), 70));
        Assert.Equal(70UL, m.GetOrInsert(Bytes(7), 71));
        Assert.Null(m.Update(Bytes(8), cur => (cur ?? 0) + 1));
        Assert.Equal(1UL, m.Update(Bytes(8), cur => (cur ?? 0) + 1));
        Assert.Equal(2UL, m.Update(Bytes(8), cur => null));
        Assert.False(m.ContainsKey(Bytes(8)));
    }

    [Fact]
    public void BytesMapReaderKeepsMapAliveAcrossGarbageCollection()
    {
        ExpanseSyncBytesMapReader reader = CreateOrphanBytesReader();
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        Assert.True(reader.TryGet(Bytes(1, 2, 3), out ulong value));
        Assert.Equal(123UL, value);
        reader.Dispose();
    }

    private static ExpanseSyncBytesMapReader CreateOrphanBytesReader()
    {
        var map = new ExpanseSyncBytesMap();
        map.Set(Bytes(1, 2, 3), 123);
        return map.CreateReader();
    }

    [Fact]
    public void BytesMapConcurrentWritersAndReaders()
    {
        using var map = new ExpanseSyncBytesMap();
        const int threads = 8;
        const int bumps = 2000;
        const int perThread = 200;
        int winners = 0;
        int done = 0;
        byte[] counterKey = Bytes(0, 0xFF, 0);

        Task[] readers = Enumerable.Range(0, 4).Select(_ => Task.Run(() =>
        {
            using var reader = map.CreateReader();
            ulong last = 0;
            while (Volatile.Read(ref done) == 0)
            {
                if (reader.TryGet(counterKey, out ulong v))
                {
                    Assert.True(v >= last);
                    last = v;
                }
            }
        })).ToArray();

        Parallel.For(0, threads, Eight, id =>
        {
            for (int i = 0; i < bumps; i++)
            {
                map.Update(counterKey, cur => (cur ?? 0) + 1);
            }
            var key = new byte[5];
            for (int i = 0; i < perThread; i++)
            {
                BinaryPrimitives.WriteInt32BigEndian(key.AsSpan(1), id * 1000 + i);
                map.Set(key, (ulong)i);
            }
            if (map.GetOrInsert(Bytes(0xAA), (ulong)id) is null)
            {
                Interlocked.Increment(ref winners);
            }
        });
        Volatile.Write(ref done, 1);
        Task.WaitAll(readers);

        Assert.Equal(1, winners);
        Assert.True(map.TryGet(counterKey, out ulong total));
        Assert.Equal((ulong)(threads * bumps), total);
        Assert.Equal((ulong)(2 + threads * perThread), map.Count);
        using var check = map.CreateReader();
        var probe = new byte[5];
        for (int id = 0; id < threads; id++)
        {
            for (int i = 0; i < perThread; i++)
            {
                BinaryPrimitives.WriteInt32BigEndian(probe.AsSpan(1), id * 1000 + i);
                Assert.True(check.TryGet(probe, out ulong v));
                Assert.Equal((ulong)i, v);
            }
        }
    }

    // ------------------------------------------------------------------ ordered-bytes map

    [Fact]
    public void OrderedBytesMapPointOperationsAndReader()
    {
        using var map = new ExpanseSyncOrderedBytesMap();
        Assert.True(map.Set(Bytes(1, 0, 2), 10));
        Assert.True(map.Set(Bytes(1), 11));
        Assert.True(map.Set(ReadOnlySpan<byte>.Empty, 12));
        Assert.False(map.Set(Bytes(1, 0, 2), 13));
        Assert.True(map.Insert(Bytes(0xFF), 14, out ulong old));
        Assert.Equal(0UL, old);
        Assert.False(map.Insert(Bytes(0xFF), 15, out old));
        Assert.Equal(14UL, old);
        Assert.Equal(4UL, map.Count);

        Assert.True(map.TryGet(Bytes(1, 0, 2), out ulong v));
        Assert.Equal(13UL, v);
        Assert.False(map.ContainsKey(Bytes(1, 0)));
        using (var reader = map.CreateReader())
        {
            Assert.True(reader.TryGet(Bytes(1), out v));
            Assert.Equal(11UL, v);
            Assert.True(reader.ContainsKey(ReadOnlySpan<byte>.Empty));
            Assert.False(reader.ContainsKey(Bytes(2)));
        }

        Assert.True(map.Remove(Bytes(0xFF), out old));
        Assert.Equal(15UL, old);
        Assert.False(map.Remove(Bytes(0xFF)));
        Assert.Equal(3UL, map.Count);
        Assert.True(map.MemoryUsed > 0);
        Assert.True(map.MemoryHeld > 0);
        Assert.Equal(map.MemoryHeld - map.ShrinkToFit(), map.MemoryHeld);
    }

    [Fact]
    public void OrderedBytesMapOrderedReadsOnTheMapHandle()
    {
        using var map = new ExpanseSyncOrderedBytesMap();
        Assert.Null(map.FirstEntry());
        Assert.Null(map.LastEntry());
        Assert.Null(map.HigherEntry(Bytes(0)));

        // Sorted: "", 00, 00 00, 01, 01 00, FF.
        byte[][] keys =
        {
            Bytes(0xFF), Bytes(1, 0), Array.Empty<byte>(), Bytes(0), Bytes(1), Bytes(0, 0),
        };
        for (int i = 0; i < keys.Length; i++)
        {
            map.Set(keys[i], (ulong)i);
        }

        ExpanseSyncOrderedBytesMap.Entry first = map.FirstEntry()!;
        Assert.Empty(first.Key);
        Assert.Equal(2UL, first.Value);
        ExpanseSyncOrderedBytesMap.Entry last = map.LastEntry()!;
        Assert.Equal(Bytes(0xFF), last.Key);
        Assert.Equal(0UL, last.Value);

        Assert.Equal(Bytes(0), map.HigherEntry(ReadOnlySpan<byte>.Empty)!.Key);
        Assert.Equal(Bytes(0, 0), map.HigherEntry(Bytes(0))!.Key);
        Assert.Equal(Bytes(0), map.CeilingEntry(Bytes(0))!.Key);
        Assert.Equal(Bytes(1), map.CeilingEntry(Bytes(0, 1))!.Key);
        Assert.Null(map.HigherEntry(Bytes(0xFF)));
        Assert.Equal(Bytes(0, 0), map.LowerEntry(Bytes(1))!.Key);
        Assert.Null(map.LowerEntry(ReadOnlySpan<byte>.Empty));
        Assert.Equal(Bytes(1, 0), map.FloorEntry(Bytes(1, 0))!.Key);
        Assert.Equal(Bytes(1, 0), map.FloorEntry(Bytes(0xFE))!.Key);
        Assert.Empty(map.FloorEntry(ReadOnlySpan<byte>.Empty)!.Key);

        var walked = new List<byte[]>();
        for (var e = map.FirstEntry(); e is not null; e = map.HigherEntry(e.Key))
        {
            walked.Add(e.Key);
        }
        Assert.Equal(6, walked.Count);
        for (int i = 1; i < walked.Count; i++)
        {
            Assert.True(walked[i - 1].AsSpan().SequenceCompareTo(walked[i]) < 0);
        }
    }

    [Fact]
    public void OrderedBytesMapKeysLongerThanTheInitialBuffer()
    {
        byte[] longA = new byte[1000];
        byte[] longB = new byte[1000];
        Array.Fill(longA, (byte)7);
        Array.Fill(longB, (byte)7);
        longB[999] = 8;
        using var map = new ExpanseSyncOrderedBytesMap();
        map.Set(longB, 2);
        map.Set(longA, 1);
        Assert.Equal(longA, map.FirstEntry()!.Key);
        Assert.Equal(longB, map.LastEntry()!.Key);
        Assert.Equal(2UL, map.HigherEntry(longA)!.Value);
        Assert.Equal(1UL, map.LowerEntry(longB)!.Value);
        Assert.Equal(1UL, map.CeilingEntry(Bytes(7))!.Value);
        Assert.Null(map.FloorEntry(new byte[2000]));
    }

    [Fact]
    public void OrderedBytesMapCompareExchangeGetOrInsertAndUpdate()
    {
        using var m = new ExpanseSyncOrderedBytesMap();
        byte[] key = Bytes(0, 1, 0);
        ulong? seen;
        Assert.True(m.CompareExchange(key, null, null, out seen));
        Assert.Null(seen);
        Assert.True(m.CompareExchange(key, null, 10, out seen));
        Assert.False(m.CompareExchange(key, null, 99, out seen));
        Assert.Equal(10UL, seen);
        Assert.False(m.CompareExchange(key, 11, 12, out seen));
        Assert.Equal(10UL, seen);
        Assert.True(m.CompareExchange(key, 10, 20, out seen));
        Assert.True(m.CompareExchange(key, 20, null, out seen));
        Assert.False(m.ContainsKey(key));
        Assert.Null(m.GetOrInsert(Bytes(7), 70));
        Assert.Equal(70UL, m.GetOrInsert(Bytes(7), 71));
        Assert.Null(m.Update(Bytes(8), cur => (cur ?? 0) + 1));
        Assert.Equal(1UL, m.Update(Bytes(8), cur => (cur ?? 0) + 1));
        Assert.Equal(new[] { Bytes(7), Bytes(8) }, new[] { m.FirstEntry()!.Key, m.LastEntry()!.Key });
    }

    [Fact]
    public void OrderedBytesMapReaderKeepsMapAliveAcrossGarbageCollection()
    {
        ExpanseSyncOrderedBytesMapReader reader = CreateOrphanOrderedReader();
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        Assert.True(reader.TryGet(Bytes(4, 5), out ulong value));
        Assert.Equal(45UL, value);
        reader.Dispose();
    }

    private static ExpanseSyncOrderedBytesMapReader CreateOrphanOrderedReader()
    {
        var map = new ExpanseSyncOrderedBytesMap();
        map.Set(Bytes(4, 5), 45);
        return map.CreateReader();
    }

    [Fact]
    public void OrderedBytesMapConcurrentWritersPointReadersAndOrderedReads()
    {
        using var map = new ExpanseSyncOrderedBytesMap();
        const int threads = 8;
        const int bumps = 1000;
        const int perThread = 100;
        int winners = 0;
        int done = 0;
        byte[] counterKey = Bytes(0xFF, 0xFF, 0xFF, 0xFF, 0xFF);

        Task[] readers = Enumerable.Range(0, 3).Select(_ => Task.Run(() =>
        {
            using var reader = map.CreateReader();
            while (Volatile.Read(ref done) == 0)
            {
                reader.TryGet(counterKey, out _);
            }
        })).ToArray();

        // Ordered reads run on the map handle and block writers; run them beside the writers.
        Task ordered = Task.Run(() =>
        {
            while (Volatile.Read(ref done) == 0)
            {
                // Each entry stores its own big-endian key as the value, so a consistent read is checkable.
                if (map.FirstEntry() is { } f && f.Key.Length == 4)
                {
                    Assert.Equal((ulong)BinaryPrimitives.ReadInt32BigEndian(f.Key), f.Value);
                }
                if (map.CeilingEntry(Bytes(0, 0, 3, 0)) is { } c && c.Key.Length == 4)
                {
                    Assert.True(c.Key.AsSpan().SequenceCompareTo(Bytes(0, 0, 3, 0)) >= 0);
                }
                // Ordered reads exclude every writer for their duration: pace them so the writers progress.
                Thread.Sleep(1);
            }
        });

        Parallel.For(0, threads, Eight, id =>
        {
            for (int i = 0; i < bumps; i++)
            {
                map.Update(counterKey, cur => (cur ?? 0) + 1);
            }
            var key = new byte[4];
            for (int i = 0; i < perThread; i++)
            {
                int n = id * 1000 + i;
                BinaryPrimitives.WriteInt32BigEndian(key, n);
                map.Set(key, (ulong)n);
            }
            if (map.GetOrInsert(Bytes(0xAA), (ulong)id) is null)
            {
                Interlocked.Increment(ref winners);
            }
        });
        Volatile.Write(ref done, 1);
        Task.WaitAll(readers);
        ordered.Wait();

        Assert.Equal(1, winners);
        Assert.True(map.TryGet(counterKey, out ulong total));
        Assert.Equal((ulong)(threads * bumps), total);
        Assert.Equal((ulong)(2 + threads * perThread), map.Count);

        var walked = new List<byte[]>();
        for (var e = map.FirstEntry(); e is not null; e = map.HigherEntry(e.Key))
        {
            walked.Add(e.Key);
        }
        Assert.Equal((int)map.Count, walked.Count);
        for (int i = 1; i < walked.Count; i++)
        {
            Assert.True(walked[i - 1].AsSpan().SequenceCompareTo(walked[i]) < 0);
        }
    }

    // ------------------------------------------------------------------ blob map

    [Fact]
    public void BlobMapPointOperationsAndReader()
    {
        using var map = new ExpanseSyncBlobMap();
        byte[] tiny = Bytes(1, 2, 3);
        byte[] mid = SelfDescribing(100);
        byte[] big = SelfDescribing(70_000);

        map.Set(1, tiny);
        map.Set(2, mid, 0xABCDE);
        map.Set(3, big, 9);
        map.Set(4, Array.Empty<byte>());
        Assert.Equal(4UL, map.Count);

        Assert.True(map.TryGetBytes(1, out byte[] got, out uint meta));
        Assert.Equal(tiny, got);
        Assert.Equal(0U, meta); // a payload of 7 bytes or fewer reads back metadata 0
        Assert.True(map.TryGetBytes(2, out got, out meta));
        Assert.Equal(mid, got);
        Assert.Equal(0xABCDEU, meta);
        Assert.Equal(big, map.GetBytes(3));
        Assert.Empty(map.GetBytes(4)!);
        Assert.Null(map.GetBytes(5));
        Assert.False(map.TryGetBytes(5, out got));
        Assert.Empty(got);
        Assert.True(map.TryGetMeta(2, out meta));
        Assert.Equal(0xABCDEU, meta);
        Assert.False(map.TryGetMeta(5, out _));
        Assert.True(map.Contains(3));
        Assert.False(map.ContainsKey(5));

        using (var reader = map.CreateReader())
        {
            Assert.True(reader.TryGetBytes(3, out got, out meta));
            Assert.Equal(big, got);
            Assert.Equal(9U, meta);
            Assert.Equal(mid, reader.GetBytes(2));
            Assert.Null(reader.GetBytes(5));
            Assert.True(reader.TryGetMeta(3, out meta));
            Assert.Equal(9U, meta);
            Assert.True(reader.Contains(1));
            Assert.False(reader.ContainsKey(5));
        }

        // Replacing a value with a differently sized one is visible to the next read.
        map.Set(3, tiny);
        Assert.Equal(tiny, map.GetBytes(3));
        Assert.True(map.Remove(3));
        Assert.False(map.Remove(3));
        Assert.Equal(3UL, map.Count);
        Assert.True(map.MemoryUsed > 0);
        Assert.True(map.MemoryUsedEstimate > 0);
        Assert.True(map.MemoryHeld > 0);
        Assert.Equal(map.MemoryHeld - map.ShrinkToFit(), map.MemoryHeld);
        Assert.Equal(ExpanseBlobStatus.Ok, map.Compact());
        Assert.Equal(mid, map.GetBytes(2));
        map.SetReclaimAtCap(false);
        map.SetReclaimAtCap(true);
    }

    [Fact]
    public void BlobMapReportsRefusalsAsStatuses()
    {
        using var map = new ExpanseSyncBlobMap(4096);
        Assert.Equal(ExpanseBlobStatus.Ok, map.TrySet(1, SelfDescribing(100), 5));
        Assert.Equal(ExpanseBlobStatus.MetaOverflow, map.TrySet(2, SelfDescribing(100), 1u << 24));
        Assert.Equal(ExpanseBlobStatus.AllocationFailed, map.TrySet(3, SelfDescribing(8192), 5));
        Assert.Throws<InvalidOperationException>(() => map.Set(3, SelfDescribing(8192), 5));
        Assert.False(map.ContainsKey(2));
        Assert.False(map.ContainsKey(3));
        Assert.Equal(1UL, map.Count);
    }

    [Fact]
    public void BlobMapCompareExchangeCoversEveryBranch()
    {
        using var m = new ExpanseSyncBlobMap();
        var a = new ExpanseBlobValue(SelfDescribing(100), 7);
        var b = new ExpanseBlobValue(SelfDescribing(120), 8);
        // Larger than the wrapper's 256-byte working buffer, so a mismatch has to grow it.
        var large = new ExpanseBlobValue(SelfDescribing(5000), 9);
        ExpanseBlobValue? seen;

        // absent == absent: nothing to store, succeeds.
        Assert.Equal(ExpanseBlobStatus.Ok, m.CompareExchange(1, null, null, out seen));
        Assert.Null(seen);
        Assert.False(m.ContainsKey(1));
        // insert-if-absent.
        Assert.Equal(ExpanseBlobStatus.Ok, m.CompareExchange(1, null, a, out seen));
        Assert.Equal(a.Payload, m.GetBytes(1));
        // insert-if-absent against a present key: mismatch, the present value is reported.
        Assert.Equal(ExpanseBlobStatus.Mismatch, m.CompareExchange(1, null, b, out seen));
        Assert.NotNull(seen);
        Assert.Equal(a.Payload, seen!.Payload);
        Assert.Equal(a.HotMeta, seen.HotMeta);
        // expected present against an absent key: mismatch, observed absent.
        Assert.Equal(ExpanseBlobStatus.Mismatch, m.CompareExchange(2, a, b, out seen));
        Assert.Null(seen);
        // same payload, different metadata: mismatch (the metadata is part of the value).
        Assert.Equal(ExpanseBlobStatus.Mismatch, m.CompareExchange(1, a with { HotMeta = 6 }, b, out seen));
        Assert.Equal(7U, seen!.HotMeta);
        // replace on match.
        Assert.Equal(ExpanseBlobStatus.Ok, m.CompareExchange(1, a, b, out seen));
        Assert.Equal(b.Payload, m.GetBytes(1));
        Assert.True(m.TryGetMeta(1, out uint meta));
        Assert.Equal(8U, meta);
        // a mismatch against a value larger than the working buffer reports the whole value.
        m.Set(3, large.Payload, large.HotMeta);
        Assert.Equal(ExpanseBlobStatus.Mismatch, m.CompareExchange(3, a, b, out seen));
        Assert.Equal(large.Payload, seen!.Payload);
        Assert.Equal(9U, seen.HotMeta);
        // an expected value larger than the working buffer matches.
        Assert.Equal(ExpanseBlobStatus.Ok, m.CompareExchange(3, large, a, out seen));
        Assert.Equal(a.Payload, m.GetBytes(3));
        // remove-if-equals: mismatch leaves it, match removes it.
        Assert.Equal(ExpanseBlobStatus.Mismatch, m.CompareExchange(1, a, null, out seen));
        Assert.True(m.ContainsKey(1));
        Assert.Equal(ExpanseBlobStatus.Ok, m.CompareExchange(1, b, null, out seen));
        Assert.False(m.ContainsKey(1));
        // a refusal when the comparison held leaves the map unchanged and reports no value.
        Assert.Equal(ExpanseBlobStatus.MetaOverflow, m.CompareExchange(3, a, new ExpanseBlobValue(b.Payload, 1u << 24), out seen));
        Assert.Null(seen);
        Assert.Equal(a.Payload, m.GetBytes(3));
    }

    [Fact]
    public void BlobMapGetOrInsertStoresOnlyIntoAnAbsentKey()
    {
        using var m = new ExpanseSyncBlobMap();
        ExpanseBlobValue? existing;
        byte[] first = SelfDescribing(100);
        byte[] big = SelfDescribing(5000);
        Assert.Equal(ExpanseBlobStatus.Ok, m.GetOrInsert(1, first, 7, out existing));
        Assert.Null(existing);
        Assert.Equal(ExpanseBlobStatus.Mismatch, m.GetOrInsert(1, SelfDescribing(120), 8, out existing));
        Assert.Equal(first, existing!.Payload);
        Assert.Equal(7U, existing.HotMeta);
        Assert.Equal(first, m.GetBytes(1));
        // the present value is larger than the working buffer.
        m.Set(2, big, 9);
        Assert.Equal(ExpanseBlobStatus.Mismatch, m.GetOrInsert(2, first, 7, out existing));
        Assert.Equal(big, existing!.Payload);
        Assert.Equal(9U, existing.HotMeta);
        // a refusal on an absent key.
        Assert.Equal(ExpanseBlobStatus.MetaOverflow, m.GetOrInsert(3, first, 1u << 24, out existing));
        Assert.Null(existing);
        Assert.False(m.ContainsKey(3));
    }

    [Fact]
    public void BlobMapCompareExchangeMeta()
    {
        using var m = new ExpanseSyncBlobMap();
        m.Set(1, SelfDescribing(100), 7);
        uint? seen;
        Assert.Equal(ExpanseBlobStatus.Ok, m.CompareExchangeMeta(1, 7, 8, out seen));
        Assert.Null(seen);
        Assert.True(m.TryGetMeta(1, out uint meta));
        Assert.Equal(8U, meta);
        Assert.Equal(ExpanseBlobStatus.Mismatch, m.CompareExchangeMeta(1, 7, 9, out seen));
        Assert.Equal(8U, seen);
        Assert.Equal(SelfDescribing(100), m.GetBytes(1));
        Assert.Equal(ExpanseBlobStatus.Mismatch, m.CompareExchangeMeta(2, 7, 9, out seen));
        Assert.Null(seen);
        // metadata alone cannot create a key.
        Assert.Equal(ExpanseBlobStatus.NoMetaField, m.CompareExchangeMeta(2, null, 5, out seen));
        Assert.False(m.ContainsKey(2));
        Assert.Equal(ExpanseBlobStatus.MetaOverflow, m.CompareExchangeMeta(1, 8, 1u << 24, out seen));
        Assert.Equal(ExpanseBlobStatus.Ok, m.CompareExchangeMeta(1, 8, null, out seen));
        Assert.False(m.ContainsKey(1));
    }

    [Fact]
    public void BlobMapReaderKeepsMapAliveAcrossGarbageCollection()
    {
        ExpanseSyncBlobMapReader reader = CreateOrphanBlobReader();
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        Assert.Equal(SelfDescribing(300), reader.GetBytes(42));
        reader.Dispose();
    }

    private static ExpanseSyncBlobMapReader CreateOrphanBlobReader()
    {
        var map = new ExpanseSyncBlobMap();
        map.Set(42, SelfDescribing(300), 3);
        return map.CreateReader();
    }

    [Fact]
    public void BlobMapConcurrentResizingWritersAndReaders()
    {
        // Writers keep replacing sixteen keys with payloads whose size runs from empty to far beyond the
        // wrapper's working buffer; readers must only ever see a payload and metadata that belong together.
        using var map = new ExpanseSyncBlobMap();
        const int keys = 16;
        const int writers = 4;
        const int rounds = 200;
        int done = 0;

        void Check(byte[] payload, uint meta)
        {
            byte expected = (byte)(payload.Length % 251);
            foreach (byte x in payload)
            {
                Assert.Equal(expected, x);
            }
            if (payload.Length > 7)
            {
                Assert.Equal((uint)payload.Length, meta);
            }
        }

        Task[] readers = Enumerable.Range(0, 4).Select(r => Task.Run(() =>
        {
            using var reader = map.CreateReader();
            ulong k = (ulong)r;
            while (Volatile.Read(ref done) == 0)
            {
                k = (k + 1) % keys;
                if (reader.TryGetBytes(k, out byte[] payload, out uint meta))
                {
                    Check(payload, meta);
                }
                if (map.TryGetBytes(k, out payload, out meta))
                {
                    Check(payload, meta);
                }
            }
        })).ToArray();

        Parallel.For(0, writers, Eight, w =>
        {
            for (int i = 0; i < rounds; i++)
            {
                for (ulong k = 0; k < keys; k++)
                {
                    int len = ((i * 37 + (int)k * 101 + w * 13) % 3000) + (i % 2 == 0 ? 0 : 400);
                    byte[] payload = SelfDescribing(len);
                    map.Set(k, payload, len > 7 ? (uint)len : 0);
                }
            }
        });
        Volatile.Write(ref done, 1);
        Task.WaitAll(readers);

        Assert.Equal((ulong)keys, map.Count);
        for (ulong k = 0; k < keys; k++)
        {
            Assert.True(map.TryGetBytes(k, out byte[] payload, out uint meta));
            Check(payload, meta);
        }
    }

    [Fact]
    public void BlobMapConcurrentCompareExchangeAndGetOrInsert()
    {
        using var map = new ExpanseSyncBlobMap();
        const int threads = 4;
        const int bumps = 500;
        int winners = 0;
        const ulong counterKey = 1;
        map.Set(counterKey, new byte[16], 1);

        Parallel.For(0, threads, Eight, id =>
        {
            for (int i = 0; i < bumps; i++)
            {
                map.TryGetBytes(counterKey, out byte[] payload, out uint meta);
                var expected = new ExpanseBlobValue(payload, meta);
                while (true)
                {
                    byte[] next = (byte[])expected.Payload.Clone();
                    BinaryPrimitives.WriteUInt64LittleEndian(next, BinaryPrimitives.ReadUInt64LittleEndian(next) + 1);
                    ExpanseBlobStatus status = map.CompareExchange(
                        counterKey, expected, new ExpanseBlobValue(next, 1), out ExpanseBlobValue? observed);
                    if (status == ExpanseBlobStatus.Ok)
                    {
                        break;
                    }
                    Assert.Equal(ExpanseBlobStatus.Mismatch, status);
                    expected = observed!;
                }
            }

            // Of any number of threads racing on one absent key, exactly one stores.
            if (map.GetOrInsert(2, SelfDescribing(50 + id), 5, out _) == ExpanseBlobStatus.Ok)
            {
                Interlocked.Increment(ref winners);
            }
        });

        Assert.Equal(1, winners);
        byte[] final = map.GetBytes(counterKey)!;
        Assert.Equal((ulong)(threads * bumps), BinaryPrimitives.ReadUInt64LittleEndian(final));
        Assert.Equal(2UL, map.Count);
    }
}
