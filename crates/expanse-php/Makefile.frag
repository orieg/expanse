# Makefile fragment for the expanse Zend extension (Rust / ext-php-rs).
#
# The standard phpize Makefile handles configure/install; this fragment makes
# the actual build a cargo invocation and drops the resulting cdylib where the
# standard targets expect the extension module (modules/expanse.so).

EXPANSE_CARGO_FLAGS = --release

# Appending a prerequisite to `all` runs the cargo build as part of the
# standard flow; the copy below overwrites the placeholder module the
# source-less libtool link produced.
all: expanse-cargo-build

.PHONY: expanse-cargo-build
expanse-cargo-build:
	@echo "Building the expanse extension via cargo ($(EXPANSE_CARGO_FLAGS))..."
	cd $(srcdir) && cargo build $(EXPANSE_CARGO_FLAGS)
	@mkdir -p modules
	@if test -f "$(srcdir)/target/release/libexpanse_php.so"; then \
		cp "$(srcdir)/target/release/libexpanse_php.so" modules/expanse.so; \
	elif test -f "$(srcdir)/target/release/libexpanse_php.dylib"; then \
		cp "$(srcdir)/target/release/libexpanse_php.dylib" modules/expanse.so; \
	else \
		echo "error: cargo did not produce libexpanse_php.{so,dylib}" >&2; exit 1; \
	fi
	@echo "expanse extension staged at modules/expanse.so"
