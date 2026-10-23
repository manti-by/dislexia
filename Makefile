# Makefile for dislexia.
#
# Usage:
#   make build   compile the app into target/debug/dislexia
#   make run     run the test suite
#   make check   fmt, clippy and tests
#   make app     run the gate, then bundle target/Dislexia.app
#   make icons   regenerate assets/ from media/icon-typewriter.svg
#   make clean   remove build artifacts

CARGO ?= cargo
BIN := target/debug/dislexia
RELEASE_BIN := target/release/dislexia

.DEFAULT_GOAL := build
.PHONY: build run test check fmt fmt-check lint clean app icons bundle

## build: compile the app.
build:
	$(CARGO) build

## run: run the test suite.
run:
	$(CARGO) test

# `test` is an alias for `run`, matching cargo's own vocabulary.
test: run

## check: formatting, lints and tests, the gate for committing.
check: fmt-check lint run

## fmt: format the source in place.
fmt:
	$(CARGO) fmt

## fmt-check: fail if the source is not formatted.
fmt-check:
	$(CARGO) fmt --check

## lint: run clippy over every target; must stay warning-free.
lint:
	$(CARGO) clippy --all-targets

## app: gate, release build and bundle the .app.
app: check
	$(CARGO) build --release
	./scripts/bundle.sh

# `bundle` is an alias for the packaging step alone (icons are checked inside).
bundle:
	$(CARGO) build --release
	./scripts/bundle.sh

## icons: regenerate assets/ (icns + tray PNGs) from the icon source SVG.
icons:
	./scripts/make-icons.sh

## clean: remove build artifacts.
clean:
	$(CARGO) clean
	rm -rf target/Dislexia.app .iconset
