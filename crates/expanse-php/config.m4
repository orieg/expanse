dnl config.m4 for the expanse Zend extension (Rust / ext-php-rs).
dnl
dnl PIE and phpize drive the standard ./configure && make flow; this extension
dnl is written in Rust, so configure only locates the Rust toolchain and the
dnl actual build is delegated to cargo by Makefile.frag (added below via
dnl PHP_ADD_MAKEFILE_FRAGMENT). Pattern follows the documented ext-php-rs +
dnl PIE integration (see docs/bindings/php.md in the monorepo).

PHP_ARG_ENABLE([expanse],
  [whether to enable the expanse extension],
  [AS_HELP_STRING([--enable-expanse], [Enable the expanse Judy-array extension])],
  [yes])

if test "$PHP_EXPANSE" != "no"; then
  AC_PATH_PROG(CARGO, cargo, no)
  if test "$CARGO" = "no"; then
    AC_MSG_ERROR([cargo not found. The expanse extension is written in Rust; install the Rust toolchain from https://rustup.rs and re-run.])
  fi
  AC_PATH_PROG(RUSTC, rustc, no)
  if test "$RUSTC" = "no"; then
    AC_MSG_ERROR([rustc not found. Install the Rust toolchain from https://rustup.rs and re-run.])
  fi
  AC_MSG_CHECKING([rustc version])
  RUSTC_VERSION=`$RUSTC --version`
  AC_MSG_RESULT([$RUSTC_VERSION])

  dnl Register the extension with no C sources: cargo produces the shared
  dnl object; Makefile.frag copies it into modules/expanse.so where the
  dnl standard install target expects it.
  PHP_NEW_EXTENSION(expanse, , $ext_shared)
  PHP_ADD_MAKEFILE_FRAGMENT
fi
