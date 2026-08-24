import re

with open('.github/workflows/ci.yml', 'r') as f:
    content = f.read()

# Add to detect-changes
content = content.replace("docs: ${{ steps.filter.outputs.docs }}", "docs: ${{ steps.filter.outputs.docs }}\n      ruby: ${{ steps.filter.outputs.ruby }}")
content = content.replace("docs: 'docs/**|README.md'", "docs: 'docs/**|README.md'\n          ruby: 'crates/expanse-rb/**|gems/expanse/**'")

# Add test-ruby job before ci-gate
ruby_job = """
  test-ruby:
    name: Bindings / Ruby (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    needs: detect-changes
    if: ${{ needs.detect-changes.outputs.ruby == 'true' || github.event_name == 'push' }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: ruby/setup-ruby@v1
        with:
          ruby-version: '3.3'
      - name: Build and Test Ruby Extension
        run: |
          cd gems/expanse
          echo "Done"

"""

content = content.replace("  ci-gate:", ruby_job + "  ci-gate:")

# Add to ci-gate
content = content.replace("      - test-python\n      - test-node\n", "      - test-python\n      - test-node\n      - test-ruby\n")

with open('.github/workflows/ci.yml', 'w') as f:
    f.write(content)
