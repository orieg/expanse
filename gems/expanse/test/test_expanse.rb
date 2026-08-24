require "minitest/autorun"
require "expanse"

class TestExpanse < Minitest::Test
  def test_hello
    assert_equal "Hello from Expanse!", Expanse.hello
  end
end
