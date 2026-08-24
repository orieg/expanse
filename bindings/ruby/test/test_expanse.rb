require "minitest/autorun"
require_relative "../lib/expanse"

class TestExpanse < Minitest::Test
  def test_version
    refute_nil Expanse.version
    assert_match(/\d+\.\d+\.\d+/, Expanse.version)
  end

  def test_set
    set = Expanse::Set.new
    assert_equal 0, set.size
    assert set.empty?

    assert set.add(42)
    assert set.add(100)
    assert set.add(10)
    refute set.add(42)

    assert_equal 3, set.size
    assert set.include?(42)
    assert set.include?(100)
    assert set.include?(10)
    refute set.include?(999)

    assert_equal 10, set.first
    assert_equal 100, set.last
    assert_equal 42, set.next(10)
    assert_equal 10, set.prev(42)

    assert_equal 1, set.rank(42)
    assert_equal 100, set.select(2)
    assert_equal 2, set.count_range(10, 42)

    items = []
    set.each { |k| items << k }
    assert_equal [10, 42, 100], items

    assert set.delete(42)
    refute set.include?(42)
    assert_equal 2, set.size

    set.clear
    assert_equal 0, set.size
  end

  def test_map
    map = Expanse::Map.new
    assert_equal 0, map.size

    map[10] = 100
    map[20] = 200
    map[30] = 300

    assert_equal 3, map.size
    assert_equal 100, map[10]
    assert_equal 200, map[20]
    assert_equal 300, map[30]
    assert_nil map[99]

    assert map.key?(10)
    refute map.key?(99)

    assert_equal [10, 100], map.first
    assert_equal [20, 200], map.next(10)

    pairs = []
    map.each { |k, v| pairs << [k, v] }
    assert_equal [[10, 100], [20, 200], [30, 300]], pairs

    assert_equal 100, map.delete(10)
    refute map.key?(10)
    assert_equal 2, map.size
  end

  def test_strmap
    strmap = Expanse::StrMap.new
    assert_equal 0, strmap.size

    strmap["alpha"] = 1
    strmap["beta"] = 2
    strmap["gamma"] = 3

    assert_equal 3, strmap.size
    assert_equal 1, strmap["alpha"]
    assert_equal 2, strmap["beta"]
    assert_nil strmap["delta"]

    assert strmap.key?("alpha")
    refute strmap.key?("delta")

    assert_equal 1, strmap.delete("alpha")
    refute strmap.key?("alpha")
    assert_equal 2, strmap.size
  end

  def test_bytesmap
    bytesmap = Expanse::BytesMap.new
    assert_equal 0, bytesmap.size

    k1 = "\x00\x01\xFE\xFF".b
    k2 = "\xFF\xFE\x01\x00".b

    bytesmap[k1] = 42
    bytesmap[k2] = 84

    assert_equal 2, bytesmap.size
    assert_equal 42, bytesmap[k1]
    assert_equal 84, bytesmap[k2]

    assert bytesmap.key?(k1)
    assert bytesmap.delete(k1)
    refute bytesmap.key?(k1)
  end

  def test_blobmap
    blobmap = Expanse::BlobMap.new
    assert_equal 0, blobmap.size

    blobmap.set(100, "hello world", hot_meta: 1234)
    blobmap.set(200, "foo bar baz", hot_meta: 5678)

    assert_equal 2, blobmap.size
    val, meta = blobmap.get(100)
    assert_equal "hello world", val
    assert_equal 1234, meta

    assert blobmap.key?(100)
    assert blobmap.delete(100)
    refute blobmap.key?(100)
    assert_equal 1, blobmap.size
  end
end
