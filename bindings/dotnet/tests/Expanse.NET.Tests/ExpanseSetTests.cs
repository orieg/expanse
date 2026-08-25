using System;
using System.Collections.Generic;
using System.Linq;
using Xunit;

namespace Expanse.Tests;

public class ExpanseSetTests
{
    [Fact]
    public void BasicCrudOperations()
    {
        using var set = new ExpanseSet();
        Assert.Equal(0, set.Count);
        Assert.Equal(0UL, set.LongCount);
        Assert.True(set.IsEmpty);

        Assert.True(set.Add(42));
        Assert.False(set.Add(42)); // Duplicate insert returns false
        Assert.True(set.Contains(42));
        Assert.False(set.Contains(43));
        Assert.Equal(1, set.Count);
        Assert.Equal(1UL, set.LongCount);
        Assert.False(set.IsEmpty);

        Assert.True(set.Add(100));
        Assert.True(set.Add(1));
        Assert.Equal(3, set.Count);

        Assert.True(set.Remove(42));
        Assert.False(set.Remove(42));
        Assert.False(set.Contains(42));
        Assert.Equal(2, set.Count);

        set.Clear();
        Assert.Equal(0, set.Count);
        Assert.True(set.IsEmpty);
    }

    [Fact]
    public void BoundaryKeys()
    {
        using var set = new ExpanseSet();
        ulong[] keys = { 0, 1, (1UL << 53) - 1, (1UL << 53), (ulong)long.MaxValue, ulong.MaxValue };
        foreach (var k in keys)
        {
            Assert.True(set.Add(k));
        }

        foreach (var k in keys)
        {
            Assert.True(set.Contains(k));
        }
        Assert.Equal(keys.Length, set.Count);
        
        Assert.Equal(0UL, set.First());
        Assert.Equal(ulong.MaxValue, set.Last());
    }

    [Fact]
    public void NavigationMethods()
    {
        using var set = new ExpanseSet();
        ulong[] keys = [10, 20, 30, 40, 50];
        foreach (var k in keys)
        {
            set.Add(k);
        }

        Assert.Equal(10UL, set.First());
        Assert.Equal(50UL, set.Last());

        // Next / NextAtOrAfter
        Assert.Equal(20UL, set.Next(10));
        Assert.Equal(20UL, set.NextAtOrAfter(15));
        Assert.Equal(20UL, set.NextAtOrAfter(20));
        Assert.Null(set.Next(50));
        Assert.Null(set.NextAtOrAfter(51));

        // Prev / PrevAtOrBefore
        Assert.Equal(40UL, set.Prev(50));
        Assert.Equal(40UL, set.PrevAtOrBefore(45));
        Assert.Equal(40UL, set.PrevAtOrBefore(40));
        Assert.Null(set.Prev(10));
        Assert.Null(set.PrevAtOrBefore(9));
    }

    [Fact]
    public void RankAndSelect()
    {
        using var set = new ExpanseSet();
        for (ulong i = 0; i < 100; i += 2)
        {
            set.Add(i); // 0, 2, 4, ..., 98 (50 elements)
        }

        Assert.Equal(50, set.Count);

        // Rank
        Assert.Equal(0UL, set.Rank(0));
        Assert.Equal(1UL, set.Rank(1));
        Assert.Equal(1UL, set.Rank(2));
        Assert.Equal(25UL, set.Rank(50));
        Assert.Equal(50UL, set.Rank(100));

        // CountRange
        Assert.Equal(5UL, set.CountRange(10, 18)); // 10, 12, 14, 16, 18 -> 5 elements
        Assert.Equal(50UL, set.CountRange(0, 100));

        // Select
        Assert.Equal(0UL, set.Select(0));
        Assert.Equal(2UL, set.Select(1));
        Assert.Equal(48UL, set.Select(24));
        Assert.Equal(98UL, set.Select(49));
        Assert.Null(set.Select(50)); // Out of bounds
    }

    [Fact]
    public void EnumerationOrder()
    {
        using var set = new ExpanseSet();
        ulong[] inserted = [100, 5, 20, 500, 1, 9999];
        foreach (var k in inserted)
        {
            set.Add(k);
        }

        var enumerated = set.ToList();
        var sorted = inserted.OrderBy(x => x).ToList();

        Assert.Equal(sorted, enumerated);
    }

    [Fact]
    public void BatchMembership()
    {
        using var set = new ExpanseSet();
        for (ulong i = 0; i < 1000; i++)
        {
            set.Add(i * 10);
        }

        ulong[] queries = { 0, 10, 25, 30, 9999 };
        bool[] outPresent = new bool[queries.Length];

        nuint found = set.ContainsBatch(queries, outPresent);
        Assert.Equal((nuint)3, found);
        Assert.True(outPresent[0]);
        Assert.True(outPresent[1]);
        Assert.False(outPresent[2]);
        Assert.True(outPresent[3]);
        Assert.False(outPresent[4]);
    }
}
