using System;
using System.Threading;
using System.Threading.Tasks;
using Xunit;

namespace Expanse.Tests;

public class ExpanseSyncTests
{
    [Fact]
    public void ConcurrentSetWritesAndLockFreeReads()
    {
        using var set = new ExpanseSyncSet();
        const int count = 1000;

        // Writer populates set
        for (ulong i = 0; i < count; i++)
        {
            set.Add(i * 2);
        }

        Assert.Equal((ulong)count, set.Count);

        // Spawn parallel lock-free reader tasks
        Parallel.For(0, 8, _ =>
        {
            using var reader = set.CreateReader();
            for (ulong i = 0; i < count; i++)
            {
                Assert.True(reader.Contains(i * 2));
                Assert.False(reader.Contains(i * 2 + 1));
            }
        });
    }

    [Fact]
    public void ConcurrentMapWritesAndLockFreeReads()
    {
        using var map = new ExpanseSyncMap();
        const int count = 1000;

        for (ulong i = 0; i < count; i++)
        {
            map.Set(i, i * 10);
        }

        Assert.Equal((ulong)count, map.Count);

        Parallel.For(0, 8, _ =>
        {
            using var reader = map.CreateReader();
            for (ulong i = 0; i < count; i++)
            {
                Assert.True(reader.TryGet(i, out ulong value));
                Assert.Equal(i * 10, value);
            }
        });
    }

    [Fact]
    public void MemoryHeldAndShrinkToFitAfterHalfDrain()
    {
        // The epoch collector keeps the freed blocks. ShrinkToFit returns those
        // past their grace period and MemoryHeld falls by exactly what it
        // reports; blocks still in their grace period stay held.
        const ulong n = 50000;
        using var map = new ExpanseSyncMap();
        using var set = new ExpanseSyncSet();
        for (ulong k = 0; k < n; k++)
        {
            map.Set(k * 0x9E3779B1UL, k);
            set.Add(k * 0x9E3779B1UL);
        }
        for (ulong k = 0; k < n; k += 2)
        {
            Assert.True(map.Remove(k * 0x9E3779B1UL));
            Assert.True(set.Remove(k * 0x9E3779B1UL));
        }

        nuint mapHeld = map.MemoryHeld;
        Assert.True(mapHeld > 0);
        nuint mapReleased = map.ShrinkToFit();
        Assert.Equal(mapHeld - mapReleased, map.MemoryHeld);

        nuint setHeld = set.MemoryHeld;
        Assert.True(setHeld > 0);
        nuint setReleased = set.ShrinkToFit();
        Assert.Equal(setHeld - setReleased, set.MemoryHeld);

        for (ulong k = 1; k < n; k += 2)
        {
            Assert.True(map.TryGet(k * 0x9E3779B1UL, out ulong v));
            Assert.Equal(k, v);
            Assert.True(set.Contains(k * 0x9E3779B1UL));
        }
    }

    [Fact]
    public void MapReaderKeepsMapAliveAcrossGarbageCollection()
    {
        // A reader borrows the map's native storage. Before the fix the reader held no
        // reference to the map handle, so once the last managed reference to the map was
        // dropped the GC could finalize (free) the map SafeHandle while the reader lived
        // on — a use-after-free from safe C#. The reader now DangerousAddRef's the map
        // handle for its lifetime; after dropping the map and forcing a full GC the
        // reader must still read correctly.
        ExpanseSyncMapReader reader = CreateOrphanReader();

        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();

        Assert.True(reader.TryGet(42, out ulong value));
        Assert.Equal(4242ul, value);
        reader.Dispose();
    }

    // Creates a reader whose owning map has no remaining managed reference, so the map
    // is eligible for collection except for the reader's ref-count on its handle.
    private static ExpanseSyncMapReader CreateOrphanReader()
    {
        var map = new ExpanseSyncMap();
        map.Set(42, 4242);
        return map.CreateReader();
    }

    [Fact]
    public void SetReaderKeepsSetAliveAcrossGarbageCollection()
    {
        ExpanseSyncSetReader reader = CreateOrphanSetReader();

        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();

        Assert.True(reader.Contains(7));
        reader.Dispose();
    }

    private static ExpanseSyncSetReader CreateOrphanSetReader()
    {
        var set = new ExpanseSyncSet();
        set.Add(7);
        return set.CreateReader();
    }

    [Fact]
    public void CompareExchangeCoversEveryBranch()
    {
        using var m = new ExpanseSyncMap();
        ulong? seen;

        // absent == absent with nothing to store: succeeds, observes absent.
        Assert.True(m.CompareExchange(1, null, null, out seen));
        Assert.Null(seen);
        Assert.False(m.ContainsKey(1));
        // insert-if-absent on an absent key.
        Assert.True(m.CompareExchange(1, null, 10, out seen));
        Assert.Null(seen);
        Assert.True(m.TryGet(1, out ulong v));
        Assert.Equal(10UL, v);
        // insert-if-absent against a present key fails and reports the word seen.
        Assert.False(m.CompareExchange(1, null, 99, out seen));
        Assert.Equal(10UL, seen);
        // absent == absent against a present key fails.
        Assert.False(m.CompareExchange(1, null, null, out seen));
        Assert.Equal(10UL, seen);
        // expected present, wrong value: fails, value unchanged.
        Assert.False(m.CompareExchange(1, 11, 12, out seen));
        Assert.Equal(10UL, seen);
        Assert.True(m.TryGet(1, out v));
        Assert.Equal(10UL, v);
        // expected present against an absent key: fails, observes absent.
        Assert.False(m.CompareExchange(2, 5, 6, out seen));
        Assert.Null(seen);
        Assert.False(m.ContainsKey(2));
        // replace on match.
        Assert.True(m.CompareExchange(1, 10, 20, out seen));
        Assert.Equal(10UL, seen);
        Assert.True(m.TryGet(1, out v));
        Assert.Equal(20UL, v);
        // remove-if-equals: mismatch leaves it, match removes it.
        Assert.False(m.CompareExchange(1, 21, null, out seen));
        Assert.Equal(20UL, seen);
        Assert.True(m.ContainsKey(1));
        Assert.True(m.CompareExchange(1, 20, null, out seen));
        Assert.Equal(20UL, seen);
        Assert.False(m.ContainsKey(1));
        // Every ulong is a legal value, 0 and the maximum included.
        Assert.True(m.CompareExchange(3, null, 0, out seen));
        Assert.True(m.CompareExchange(3, 0, ulong.MaxValue, out seen));
        Assert.Equal(0UL, seen);
        Assert.True(m.TryGet(3, out v));
        Assert.Equal(ulong.MaxValue, v);
    }

    [Fact]
    public void GetOrInsertStoresOnlyIntoAnAbsentKey()
    {
        using var m = new ExpanseSyncMap();
        Assert.Null(m.GetOrInsert(7, 70));
        Assert.True(m.TryGet(7, out ulong v));
        Assert.Equal(70UL, v);
        Assert.Equal(70UL, m.GetOrInsert(7, 71));
        Assert.True(m.TryGet(7, out v));
        Assert.Equal(70UL, v);
        // Zero is a value, not absence.
        Assert.Null(m.GetOrInsert(8, 0));
        Assert.Equal(0UL, m.GetOrInsert(8, 5));
    }

    [Fact]
    public void UpdateStoresRemovesAndPropagatesExceptions()
    {
        using var m = new ExpanseSyncMap();
        var seen = new System.Collections.Generic.List<ulong?>();
        Func<ulong?, ulong?> inc = cur =>
        {
            seen.Add(cur);
            return (cur ?? 0) + 1;
        };
        Assert.Null(m.Update(5, inc));
        Assert.Equal(1UL, m.Update(5, inc));
        Assert.True(m.TryGet(5, out ulong v));
        Assert.Equal(2UL, v);
        Assert.Equal(new ulong?[] { null, 1UL }, seen.ToArray());
        // A null result removes the key and reports the replaced value.
        Assert.Equal(2UL, m.Update(5, cur => null));
        Assert.False(m.ContainsKey(5));
        Assert.Null(m.Update(5, cur => null));
        // An exception from fn propagates and leaves the key unchanged.
        m.Set(6, 60);
        Assert.Throws<InvalidOperationException>(() => m.Update(6, cur => throw new InvalidOperationException("fn failed")));
        Assert.True(m.TryGet(6, out v));
        Assert.Equal(60UL, v);
    }

    [Fact]
    public void UpdateRetriesOnInterference()
    {
        using var m = new ExpanseSyncMap();
        m.Set(1, 100);
        var calls = new System.Collections.Generic.List<ulong?>();
        ulong? prev = m.Update(1, cur =>
        {
            calls.Add(cur);
            if (calls.Count == 1)
            {
                m.Set(1, 200); // a competing write lands between the read and the exchange
            }
            return cur + 1;
        });
        Assert.Equal(200UL, prev);
        Assert.Equal(new ulong?[] { 100UL, 200UL }, calls.ToArray());
        Assert.True(m.TryGet(1, out ulong v));
        Assert.Equal(201UL, v);
    }

    [Fact]
    public void UpdateFromManyThreadsEndsAtTheExactTotal()
    {
        using var m = new ExpanseSyncMap();
        const int threads = 8;
        const int perThread = 5000;
        int winners = 0;

        Parallel.For(0, threads, new ParallelOptions { MaxDegreeOfParallelism = threads }, id =>
        {
            for (int i = 0; i < perThread; i++)
            {
                m.Update(0, cur => (cur ?? 0) + 1);
            }
            // Of any number of threads racing on one absent key, exactly one stores.
            if (m.GetOrInsert(42, (ulong)id) is null)
            {
                Interlocked.Increment(ref winners);
            }
        });

        Assert.True(m.TryGet(0, out ulong total));
        Assert.Equal((ulong)(threads * perThread), total);
        Assert.Equal(1, winners);
    }
}
