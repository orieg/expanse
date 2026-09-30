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

  # The C ABI reads a string-map key as a NUL-terminated const char*, so an
  # unchecked "a\0b" reached the engine as "a" and overwrote that entry. Every
  # call that takes a key must reject it, and "a" must be left alone.
  def test_strmap_rejects_embedded_nul
    strmap = Expanse::StrMap.new
    strmap["a"] = 1

    assert_raises(ArgumentError) { strmap["a\0b"] = 2 }
    assert_raises(ArgumentError) { strmap["a\0b"] }
    assert_raises(ArgumentError) { strmap.get("a\0b") }
    assert_raises(ArgumentError) { strmap.key?("a\0b") }
    assert_raises(ArgumentError) { strmap.delete("a\0b") }

    assert_equal 1, strmap["a"]
    assert_equal 1, strmap.size
  end

  # A map, set or string map drained by delete keeps its freed blocks;
  # shrink_to_fit returns exactly mem_held - mem_used, after which the two
  # agree.
  def test_mem_held_and_shrink_to_fit
    n = 20_000
    map = Expanse::Map.new
    set = Expanse::Set.new
    n.times do |k|
      map[k * 7] = k
      set.add(k * 7)
    end
    [map, set].each { |c| assert_operator c.mem_held, :>=, c.mem_used }
    n.times do |k|
      map.delete(k * 7)
      set.delete(k * 7)
    end
    [map, set].each do |c|
      assert_equal 0, c.mem_used
      held = c.mem_held
      assert_operator held, :>, 0
      assert_equal held, c.shrink_to_fit
      assert_equal c.mem_used, c.mem_held
      assert_equal 0, c.shrink_to_fit
    end

    strmap = Expanse::StrMap.new
    n.times { |k| strmap[format("key/%08d", k)] = k }
    n.times { |k| strmap.delete(format("key/%08d", k)) }
    assert_equal 0, strmap.size
    # StrMap exposes no mem_used; an empty string map uses nothing, so a
    # drained one releases everything it holds.
    held = strmap.mem_held
    assert_operator held, :>, 0
    assert_equal held, strmap.shrink_to_fit
    assert_equal 0, strmap.mem_held
    assert_equal 0, strmap.shrink_to_fit
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

  # A refused native insert must return false, not read as success. hot_meta
  # above 24 bits on a payload longer than 7 bytes is refused by the engine
  # (MetaOverflow) before the index is touched.
  def test_blobmap_refused_insert_returns_false
    blobmap = Expanse::BlobMap.new
    payload = "sixteen byte val"

    assert_equal false, blobmap.set(1, payload, hot_meta: 1 << 24)
    refute blobmap.key?(1)
    assert_equal 0, blobmap.size

    assert_equal true, blobmap.set(2, payload, hot_meta: 0xFFFFFF)
    assert_equal false, blobmap.set(2, "replacement value", hot_meta: 1 << 24)
    assert_equal [payload, 0xFFFFFF], blobmap.get(2)
  end

  # Every function the header declares `bool` must be imported with a 1-byte
  # return type: the ABI defines only the low 8 bits of a `bool` return, so an
  # `int` import can read non-zero garbage for `false`.
  def test_bool_returns_imported_as_one_byte
    header = File.read(File.expand_path("../../../include/expanse.h", __dir__))
    bool_fns = header.scan(/^bool\s+(expanse_\w+)\s*\(/).flatten
    refute_empty bool_fns

    func_map = Expanse::Native.instance_variable_get(:@func_map)
    refute_nil func_map, "Fiddle::Importer no longer keeps @func_map; update this test"
    imported = bool_fns.select { |name| func_map.key?(name) }
    assert_operator imported.size, :>=, 25, "expected the 25 bool imports, found #{imported.size}"
    imported.each do |name|
      assert_equal(-Fiddle::TYPE_CHAR, func_map[name].instance_variable_get(:@return_type),
                   "#{name} returns C bool and must be imported as unsigned char")
    end
  end
end
