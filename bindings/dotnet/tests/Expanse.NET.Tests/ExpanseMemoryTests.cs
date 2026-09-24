using System;
using Xunit;

namespace Expanse.Tests;

/// <summary>
/// A map, set or string map drained by removal keeps its freed blocks;
/// ShrinkToFit returns exactly MemoryHeld - MemoryUsed, after which the two
/// agree and a second call releases nothing.
/// </summary>
public class ExpanseMemoryTests
{
    private const int N = 20_000;

    private static void CheckDrained(string name, Func<nuint> held, Func<nuint> used, Func<nuint> shrink)
    {
        nuint h = held();
        nuint u = used();
        Assert.True(u == 0, $"{name}: drained container uses {u} B");
        Assert.True(h > 0, $"{name}: drained container retained nothing");
        Assert.Equal(h - u, shrink());
        Assert.Equal(used(), held());
        Assert.Equal((nuint)0, shrink());
    }

    [Fact]
    public void DrainedMapKeepsBlocksUntilShrinkToFit()
    {
        using var map = new ExpanseMap();
        for (ulong k = 0; k < N; k++) map[k * 7] = k;
        Assert.True(map.MemoryHeld >= map.MemoryUsed);
        for (ulong k = 0; k < N; k++) Assert.True(map.Remove(k * 7));
        CheckDrained("map", () => map.MemoryHeld, () => map.MemoryUsed, map.ShrinkToFit);
    }

    [Fact]
    public void DrainedSetKeepsBlocksUntilShrinkToFit()
    {
        using var set = new ExpanseSet();
        for (ulong k = 0; k < N; k++) set.Add(k * 7);
        Assert.True(set.MemoryHeld >= set.MemoryUsed);
        for (ulong k = 0; k < N; k++) Assert.True(set.Remove(k * 7));
        CheckDrained("set", () => set.MemoryHeld, () => set.MemoryUsed, set.ShrinkToFit);
    }

    [Fact]
    public void DrainedStrMapKeepsBlocksUntilShrinkToFit()
    {
        using var map = new ExpanseStrMap();
        for (int k = 0; k < N; k++) map[$"key/{k:D8}"] = (ulong)k;
        Assert.True(map.MemoryHeld >= map.MemoryUsed);
        for (int k = 0; k < N; k++) Assert.True(map.Remove($"key/{k:D8}"));
        CheckDrained("strmap", () => map.MemoryHeld, () => map.MemoryUsed, map.ShrinkToFit);
    }
}
