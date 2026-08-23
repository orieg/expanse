using System;
using System.Collections.Generic;
using System.Text;
using Xunit;

namespace Expanse.Tests;

public class ExpanseBlobMapTests
{
    [Fact]
    public void InlineAndArenaBlobs()
    {
        using var map = new ExpanseBlobMap(64 * 1024);

        // Inline payloads (0..=7 bytes)
        byte[] b0 = [];
        byte[] b1 = [0xAA];
        byte[] b5 = Encoding.UTF8.GetBytes("hello");
        byte[] b7 = Encoding.UTF8.GetBytes("1234567");

        // Arena payloads (>7 bytes)
        byte[] b8 = Encoding.UTF8.GetBytes("12345678");
        byte[] bLarge = Encoding.UTF8.GetBytes("This is a much larger blob payload that resides in the off-heap slab arena!");

        map.Set(10, b0, 0);
        map.Set(11, b1, 10);
        map.Set(12, b5, 20);
        map.Set(13, b7, 30);
        map.Set(20, b8, 100);
        map.Set(21, bLarge, 200);

        Assert.Equal(6, map.Count);

        // Verify inline payload retrieval
        Assert.True(map.TryGet(10, out ReadOnlySpan<byte> span0, out uint meta0));
        Assert.Equal(0, span0.Length);
        Assert.Equal(0U, meta0);

        Assert.True(map.TryGet(11, out ReadOnlySpan<byte> span1, out uint meta1));
        Assert.Equal(b1, span1.ToArray());
        Assert.Equal(0U, meta1); // Inline payloads (0..=7 bytes) do not store arena hot_meta

        Assert.True(map.TryGet(12, out ReadOnlySpan<byte> span5, out uint meta5));
        Assert.Equal("hello", Encoding.UTF8.GetString(span5));
        Assert.Equal(0U, meta5);

        Assert.True(map.TryGet(13, out ReadOnlySpan<byte> span7, out uint meta7));
        Assert.Equal("1234567", Encoding.UTF8.GetString(span7));
        Assert.Equal(0U, meta7);

        // Verify arena payload retrieval (>7 bytes)
        Assert.True(map.TryGet(20, out ReadOnlySpan<byte> span8, out uint meta8));
        Assert.Equal("12345678", Encoding.UTF8.GetString(span8));
        Assert.Equal(100U, meta8);

        Assert.True(map.TryGet(21, out ReadOnlySpan<byte> spanLarge, out uint metaLarge));
        Assert.Equal(bLarge, spanLarge.ToArray());
        Assert.Equal(200U, metaLarge);

        // Test managed byte array helper
        byte[]? retrievedBytes = map.GetBytes(21);
        Assert.NotNull(retrievedBytes);
        Assert.Equal(bLarge, retrievedBytes);

        // Removal
        Assert.True(map.Remove(10));
        Assert.False(map.ContainsKey(10));
        Assert.Equal(5, map.Count);
    }

    [Fact]
    public void ScanFilteredWithHotMetadata()
    {
        using var map = new ExpanseBlobMap(64 * 1024);

        for (ulong i = 0; i < 50; i++)
        {
            byte[] data = Encoding.UTF8.GetBytes($"payload-{i}");
            uint meta = (uint)(i * 10);
            map.Set(i, data, meta);
        }

        // Scan keys in [10, 30] where meta is between 150 and 250 (keys 15..25)
        var scanned = new List<(ulong Key, string Data, uint Meta)>();

        ulong count = map.ScanFiltered(
            10,
            30,
            (key, meta) => meta >= 150 && meta <= 250,
            (key, payload, meta) =>
            {
                scanned.Add((key, Encoding.UTF8.GetString(payload), meta));
            });

        Assert.Equal(11UL, count);
        Assert.Equal(11, scanned.Count);

        for (int idx = 0; idx < scanned.Count; idx++)
        {
            ulong expectedKey = (ulong)(15 + idx);
            Assert.Equal(expectedKey, scanned[idx].Key);
            Assert.Equal($"payload-{expectedKey}", scanned[idx].Data);
            Assert.Equal((uint)(expectedKey * 10), scanned[idx].Meta);
        }
    }

    [Fact]
    public void PruningAndCompaction()
    {
        using var map = new ExpanseBlobMap(64 * 1024);

        // Insert 100 items (mix of expired and active)
        for (ulong i = 0; i < 100; i++)
        {
            byte[] data = new byte[128];
            Array.Fill(data, (byte)i);
            // Odd keys have meta=1 (expired), even keys have meta=0 (active)
            uint meta = (uint)(i % 2);
            map.Set(i, data, meta);
        }

        Assert.Equal(100, map.Count);

        // Prune entries where meta == 1 (50 items)
        ulong pruned = map.Prune((key, meta) => meta == 1);
        Assert.Equal(50UL, pruned);
        Assert.Equal(50, map.Count);

        // Verify remaining items
        for (ulong i = 0; i < 100; i++)
        {
            if (i % 2 == 1)
            {
                Assert.False(map.ContainsKey(i));
            }
            else
            {
                Assert.True(map.TryGet(i, out ReadOnlySpan<byte> payload, out uint meta));
                Assert.Equal(0U, meta);
                Assert.Equal(128, payload.Length);
                Assert.Equal((byte)i, payload[0]);
            }
        }

        // Test explicit compact
        Assert.True(map.Compact());
    }
}
