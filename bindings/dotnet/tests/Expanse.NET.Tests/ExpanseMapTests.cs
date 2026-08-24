using System;
using System.Collections.Generic;
using System.Linq;
using Xunit;

namespace Expanse.Tests;

public class ExpanseMapTests
{
    [Fact]
    public void BasicCrudAndIndexer()
    {
        using var map = new ExpanseMap();
        Assert.Equal(0, map.Count);
        Assert.True(map.IsEmpty);

        map[10] = 100;
        Assert.Equal(1, map.Count);
        Assert.Equal(100UL, map[10]);
        Assert.True(map.ContainsKey(10));
        Assert.False(map.ContainsKey(11));

        // Overwrite
        map[10] = 200;
        Assert.Equal(1, map.Count);
        Assert.Equal(200UL, map[10]);

        // Insert with oldValue tracking
        Assert.False(map.Insert(10, 300, out ulong oldVal));
        Assert.Equal(200UL, oldVal);
        Assert.Equal(300UL, map[10]);

        Assert.True(map.Insert(20, 400, out oldVal));
        Assert.Equal(2, map.Count);

        // TryGet
        Assert.True(map.TryGet(10, out ulong val));
        Assert.Equal(300UL, val);
        Assert.False(map.TryGet(99, out _));

        // Remove
        Assert.True(map.Remove(10, out ulong removedVal));
        Assert.Equal(300UL, removedVal);
        Assert.False(map.ContainsKey(10));
        Assert.Equal(1, map.Count);

        // KeyNotFoundException
        Assert.Throws<KeyNotFoundException>(() => _ = map[10]);

        map.Clear();
        Assert.Equal(0, map.Count);
        Assert.True(map.IsEmpty);
    }

    [Fact]
    public void BoundaryKeys()
    {
        using var map = new ExpanseMap();
        ulong[] keys = { 0, 1, (1UL << 53) - 1, (1UL << 53), (ulong)long.MaxValue, ulong.MaxValue };
        foreach (var k in keys)
        {
            map[k] = k * 2;
        }

        foreach (var k in keys)
        {
            Assert.True(map.ContainsKey(k));
            Assert.Equal(k * 2, map[k]);
        }
    }

    [Fact]
    public void NavigationAndRankSelect()
    {
        using var map = new ExpanseMap();
        for (ulong i = 10; i <= 50; i += 10)
        {
            map[i] = i * 10;
        }

        Assert.Equal(new KeyValuePair<ulong, ulong>(10, 100), map.First());
        Assert.Equal(new KeyValuePair<ulong, ulong>(50, 500), map.Last());

        Assert.Equal(new KeyValuePair<ulong, ulong>(20, 200), map.Next(10));
        Assert.Equal(new KeyValuePair<ulong, ulong>(20, 200), map.NextAtOrAfter(15));
        Assert.Equal(new KeyValuePair<ulong, ulong>(20, 200), map.NextAtOrAfter(20));

        Assert.Equal(new KeyValuePair<ulong, ulong>(40, 400), map.Prev(50));
        Assert.Equal(new KeyValuePair<ulong, ulong>(40, 400), map.PrevAtOrBefore(45));
        Assert.Equal(new KeyValuePair<ulong, ulong>(40, 400), map.PrevAtOrBefore(40));

        // Rank
        Assert.Equal(0UL, map.Rank(10));
        Assert.Equal(1UL, map.Rank(11));
        Assert.Equal(5UL, map.Rank(100));

        // CountRange
        Assert.Equal(3UL, map.CountRange(20, 40));

        // Select
        Assert.Equal(new KeyValuePair<ulong, ulong>(10, 100), map.Select(0));
        Assert.Equal(new KeyValuePair<ulong, ulong>(30, 300), map.Select(2));
        Assert.Null(map.Select(10));
    }

    [Fact]
    public void EnumerationOrder()
    {
        using var map = new ExpanseMap();
        map[50] = 5;
        map[10] = 1;
        map[30] = 3;

        var entries = map.ToList();
        Assert.Equal(3, entries.Count);
        Assert.Equal(10UL, entries[0].Key);
        Assert.Equal(30UL, entries[1].Key);
        Assert.Equal(50UL, entries[2].Key);
    }
}
