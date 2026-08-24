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
}
