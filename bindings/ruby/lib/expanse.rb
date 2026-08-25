# frozen_string_literal: true

require "fiddle"
require "fiddle/import"

module Expanse
  VERSION = "0.4.0"

  module Native
    extend Fiddle::Importer

    lib_paths = [
      File.expand_path("../../../target/release/libexpanse.dylib", __dir__),
      File.expand_path("../../../target/release/libexpanse.so", __dir__),
      File.expand_path("../../../target/release/expanse.dll", __dir__),
      File.expand_path("../../../target/debug/libexpanse.dylib", __dir__),
      File.expand_path("../../../target/debug/libexpanse.so", __dir__),
      "libexpanse.dylib",
      "libexpanse.so",
      "expanse.dll"
    ]

    loaded = false
    lib_paths.each do |path|
      if File.exist?(path) || !path.include?("/")
        begin
          dlload path
          loaded = true
          break
        rescue StandardError
          # continue searching
        end
      end
    end

    raise "Could not load libexpanse native library" unless loaded

    # Library metadata
    extern "const char* expanse_version(void)"

    # Set
    extern "void* expanse_set_new(void)"
    extern "void expanse_set_free(void*)"
    extern "int expanse_set_insert(void*, unsigned long long)"
    extern "int expanse_set_remove(void*, unsigned long long)"
    extern "int expanse_set_contains(void*, unsigned long long)"
    extern "unsigned long long expanse_set_len(void*)"
    extern "size_t expanse_set_mem_used(void*)"
    extern "void expanse_set_clear(void*)"
    extern "int expanse_set_first(void*, void*)"
    extern "int expanse_set_last(void*, void*)"
    extern "int expanse_set_next_after(void*, unsigned long long, void*)"
    extern "int expanse_set_prev_before(void*, unsigned long long, void*)"
    extern "unsigned long long expanse_set_count_below(void*, unsigned long long)"
    extern "unsigned long long expanse_set_count_range(void*, unsigned long long, unsigned long long)"
    extern "int expanse_set_by_count(void*, unsigned long long, void*)"

    # Map
    extern "void* expanse_map_new(void)"
    extern "void expanse_map_free(void*)"
    extern "int expanse_map_insert(void*, unsigned long long, unsigned long long, void*)"
    extern "int expanse_map_get(void*, unsigned long long, void*)"
    extern "int expanse_map_remove(void*, unsigned long long, void*)"
    extern "unsigned long long expanse_map_len(void*)"
    extern "size_t expanse_map_mem_used(void*)"
    extern "void expanse_map_clear(void*)"
    extern "int expanse_map_first(void*, void*, void*)"
    extern "int expanse_map_last(void*, void*, void*)"
    extern "int expanse_map_next_after(void*, unsigned long long, void*, void*)"
    extern "int expanse_map_prev_before(void*, unsigned long long, void*, void*)"

    # StrMap
    extern "void* expanse_strmap_new(void)"
    extern "void expanse_strmap_free(void*)"
    extern "int expanse_strmap_insert(void*, const char*, unsigned long long, void*)"
    extern "int expanse_strmap_get(void*, const char*, void*)"
    extern "int expanse_strmap_remove(void*, const char*, void*)"
    extern "unsigned long long expanse_strmap_len(void*)"
    extern "void expanse_strmap_clear(void*)"

    # BytesMap
    extern "void* expanse_bytesmap_new(void)"
    extern "void expanse_bytesmap_free(void*)"
    extern "int expanse_bytesmap_insert(void*, const void*, size_t, unsigned long long, void*)"
    extern "int expanse_bytesmap_get(void*, const void*, size_t, void*)"
    extern "int expanse_bytesmap_remove(void*, const void*, size_t, void*)"
    extern "unsigned long long expanse_bytesmap_len(void*)"
    extern "void expanse_bytesmap_clear(void*)"

    # BlobMap
    extern "void* expanse_blob_map_new(size_t)"
    extern "void expanse_blob_map_free(void*)"
    extern "int expanse_blob_map_insert(void*, unsigned long long, const void*, size_t, unsigned int)"
    extern "int expanse_blob_map_get(void*, unsigned long long, void*)"
    extern "int expanse_blob_map_remove(void*, unsigned long long)"
    extern "unsigned long long expanse_blob_map_len(void*)"
    extern "void expanse_blob_map_clear(void*)"
    extern "int expanse_blob_map_contains_key(void*, unsigned long long)"
  end

  def self.version
    Native.expanse_version.to_s
  end

  class Set
    include Enumerable

    def initialize
      @ptr = Native.expanse_set_new
      ObjectSpace.define_finalizer(self, self.class.finalize(@ptr))
    end

    def self.finalize(ptr)
      proc { Native.expanse_set_free(ptr) if ptr && !ptr.null? }
    end

    def add(key)
      Native.expanse_set_insert(@ptr, key) != 0
    end
    alias << add

    def delete(key)
      Native.expanse_set_remove(@ptr, key) != 0
    end
    alias remove delete

    def include?(key)
      Native.expanse_set_contains(@ptr, key) != 0
    end
    alias key? include?
    alias member? include?

    def size
      Native.expanse_set_len(@ptr)
    end
    alias length size
    alias count size

    def empty?
      size.zero?
    end

    def clear
      Native.expanse_set_clear(@ptr)
      self
    end

    def first
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_set_first(@ptr, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end

    def last
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_set_last(@ptr, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end

    def next(key)
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_set_next_after(@ptr, key, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end

    def prev(key)
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_set_prev_before(@ptr, key, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end

    def rank(key)
      Native.expanse_set_count_below(@ptr, key)
    end

    def select(k)
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_set_by_count(@ptr, k, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end

    def count_range(lo, hi)
      Native.expanse_set_count_range(@ptr, lo, hi)
    end

    def each
      return enum_for(:each) unless block_given?
      cur = first
      while cur
        yield cur
        cur = self.next(cur)
      end
    end
  end

  class Map
    include Enumerable

    def initialize
      @ptr = Native.expanse_map_new
      ObjectSpace.define_finalizer(self, self.class.finalize(@ptr))
    end

    def self.finalize(ptr)
      proc { Native.expanse_map_free(ptr) if ptr && !ptr.null? }
    end

    def []=(key, val)
      Native.expanse_map_insert(@ptr, key, val, nil)
      val
    end
    alias set []=

    def [](key)
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_map_get(@ptr, key, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end
    alias get []

    def delete(key)
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_map_remove(@ptr, key, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end

    def key?(key)
      !self[key].nil?
    end
    alias include? key?

    def size
      Native.expanse_map_len(@ptr)
    end
    alias length size

    def clear
      Native.expanse_map_clear(@ptr)
      self
    end

    def first
      k_buf = Fiddle::Pointer.malloc(8)
      v_buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_map_first(@ptr, k_buf, v_buf) != 0
        [k_buf.to_str(8).unpack1("Q<"), v_buf.to_str(8).unpack1("Q<")]
      end
    end

    def next(key)
      k_buf = Fiddle::Pointer.malloc(8)
      v_buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_map_next_after(@ptr, key, k_buf, v_buf) != 0
        [k_buf.to_str(8).unpack1("Q<"), v_buf.to_str(8).unpack1("Q<")]
      end
    end

    def each
      return enum_for(:each) unless block_given?
      pair = first
      while pair
        yield pair[0], pair[1]
        pair = self.next(pair[0])
      end
    end
  end

  class StrMap
    def initialize
      @ptr = Native.expanse_strmap_new
      ObjectSpace.define_finalizer(self, self.class.finalize(@ptr))
    end

    def self.finalize(ptr)
      proc { Native.expanse_strmap_free(ptr) if ptr && !ptr.null? }
    end

    def []=(key, val)
      s = key.to_s
      Native.expanse_strmap_insert(@ptr, s, val, nil)
      val
    end
    alias set []=

    def [](key)
      s = key.to_s
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_strmap_get(@ptr, s, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end
    alias get []

    def delete(key)
      s = key.to_s
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_strmap_remove(@ptr, s, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end

    def key?(key)
      !self[key].nil?
    end

    def size
      Native.expanse_strmap_len(@ptr)
    end

    def clear
      Native.expanse_strmap_clear(@ptr)
      self
    end
  end

  class BytesMap
    def initialize
      @ptr = Native.expanse_bytesmap_new
      ObjectSpace.define_finalizer(self, self.class.finalize(@ptr))
    end

    def self.finalize(ptr)
      proc { Native.expanse_bytesmap_free(ptr) if ptr && !ptr.null? }
    end

    def []=(key, val)
      b = key.b
      Native.expanse_bytesmap_insert(@ptr, b, b.bytesize, val, nil)
      val
    end
    alias set []=

    def [](key)
      b = key.b
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_bytesmap_get(@ptr, b, b.bytesize, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end
    alias get []

    def delete(key)
      b = key.b
      buf = Fiddle::Pointer.malloc(8)
      if Native.expanse_bytesmap_remove(@ptr, b, b.bytesize, buf) != 0
        buf.to_str(8).unpack1("Q<")
      end
    end

    def key?(key)
      !self[key].nil?
    end

    def size
      Native.expanse_bytesmap_len(@ptr)
    end

    def clear
      Native.expanse_bytesmap_clear(@ptr)
      self
    end
  end

  class BlobMap
    def initialize(chunk_size: 0)
      @ptr = Native.expanse_blob_map_new(chunk_size)
      ObjectSpace.define_finalizer(self, self.class.finalize(@ptr))
    end

    def self.finalize(ptr)
      proc { Native.expanse_blob_map_free(ptr) if ptr && !ptr.null? }
    end

    def set(key, payload, hot_meta: 0)
      b = payload.b
      Native.expanse_blob_map_insert(@ptr, key, b, b.bytesize, hot_meta) != 0
    end

    def get(key)
      # ExpanseBlobView: ptr (8B), len (8B), hot_meta (4B), is_inline (1B) + padding (3B) = 24 bytes
      view_buf = Fiddle::Pointer.malloc(24)
      if Native.expanse_blob_map_get(@ptr, key, view_buf) != 0
        raw_ptr = view_buf[0, 8].unpack1("Q<")
        len = view_buf[8, 8].unpack1("Q<")
        meta = view_buf[16, 4].unpack1("L<")
        val = len.zero? ? "" : Fiddle::Pointer.new(raw_ptr).to_str(len)
        [val, meta]
      end
    end

    def delete(key)
      Native.expanse_blob_map_remove(@ptr, key) != 0
    end

    def key?(key)
      Native.expanse_blob_map_contains_key(@ptr, key) != 0
    end

    def size
      Native.expanse_blob_map_len(@ptr)
    end

    def clear
      Native.expanse_blob_map_clear(@ptr)
      self
    end
  end
end
