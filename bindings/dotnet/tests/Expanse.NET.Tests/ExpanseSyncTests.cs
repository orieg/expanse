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
}
