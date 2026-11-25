.PHONY: help build test goldens fmt fmt-check clippy deny structure standards-check docs-check feature-board ci-fast ci-lint ci install clean

CARGO ?= cargo
# Every build uses the committed Cargo.lock as is; a stale lock fails instead
# of being rewritten (STD-05 R23).
LOCKED := --locked
BINARY := tmpl-cli
INSTALL_BIN_DIR ?= $(HOME)/.cargo/bin
# `make goldens UPDATE=1` rewrites the golden files instead of comparing.
UPDATE ?=
# `STANDARDS_STRICT=1` makes a missing docs/standards/ a failure (CI sets it).
STANDARDS_STRICT ?=
# GitHub Actions sets CI=true. There a missing gate tool fails the gate;
# locally it warns and skips or falls back (STD-04 R10).
CI ?=
# The crates of this workspace. With none yet (a new workspace), every Cargo
# target says so and passes, so the gates hold from the first commit.
CRATES := $(wildcard crates/*/Cargo.toml)

# $(call missing_tool,<tool>,<what it provides>): fail under CI, else warn.
define missing_tool
	if [ "$(CI)" = "true" ]; then \
		echo "error: $(1) is not installed; CI requires it for $(2)." >&2; exit 1; \
	fi; \
	echo "warning: $(1) not installed; continuing without $(2) (CI requires it)." >&2; \
	echo "         install: cargo install $(1) --locked" >&2
endef

help:
	@echo "tmpl-cli make targets"
	@echo ""
	@echo "  make ci-fast          Pre-handoff gate, no compile: fmt, structure, docs and standards"
	@echo "  make ci-lint          clippy -D warnings + cargo-deny (skipped locally if not installed)"
	@echo "  make test             All tests (nextest when installed, else cargo test) + doctests"
	@echo "  make goldens          Compare --help and output goldens with the built binary"
	@echo "  make goldens UPDATE=1 Regenerate goldens after an intended surface change"
	@echo "  make standards-check  Run the vendored standards check (docs/standards/check.sh)"
	@echo "  make feature-board    Regenerate the design feature board"
	@echo "  make ci               Everything above; what CI runs"
	@echo "  make build | fmt | install | clean"

# ---------------------------------------------------------------------------
# Cargo targets: skipped while the workspace has no crate
# ---------------------------------------------------------------------------

ifeq ($(strip $(CRATES)),)

build fmt fmt-check clippy deny test goldens install clean:
	@echo "$@: no crates/*/Cargo.toml yet; skipping" >&2

else

build:
	$(CARGO) build --workspace $(LOCKED)

fmt:
	$(CARGO) fmt --all

# STD-12 R11: rustfmt on the stable toolchain is the layout gate, and
# rustfmt.toml sets only stable options: stable rustfmt warns about an
# unstable one and ignores it, so that warning fails the gate. Its stderr is
# kept in target/ for the check.
fmt-check:
	@set -e; mkdir -p target; log=target/fmt-check.log; \
	status=0; $(CARGO) fmt --all -- --check 2>"$$log" || status=$$?; \
	cat "$$log" >&2; \
	if grep -qi 'unstable features are only available in nightly' "$$log"; then \
		echo "error: rustfmt.toml sets an option that is unstable on this toolchain (STD-12 R11)" >&2; exit 1; \
	fi; \
	exit $$status

clippy:
	$(CARGO) clippy --workspace --all-targets $(LOCKED) -- -D warnings

# Supply chain (STD-02 R23, STD-05 R23/R24): advisories, licenses, sources.
deny:
	@set -e; if $(CARGO) deny --version >/dev/null 2>&1; then \
		advisory_db="$${CARGO_HOME:-$$HOME/.cargo}/advisory-dbs"; \
		parent="$$advisory_db"; \
		while [ ! -e "$$parent" ] && [ ! -L "$$parent" ]; do parent=$$(dirname "$$parent"); done; \
		if [ -d "$$parent" ] && [ -w "$$parent" ] && [ -x "$$parent" ] \
			&& { [ ! -e "$$advisory_db/db.lock" ] || [ -w "$$advisory_db/db.lock" ]; }; then \
			$(CARGO) deny check; \
		else \
			mkdir -p target/cargo-deny; \
			config=$$(mktemp target/cargo-deny/config.XXXXXXXX); \
			trap 'rm "$$config"' EXIT; \
			awk '{print} /^\[advisories\]$$/ {print "db-path = \"target/cargo-deny/advisory-dbs\""}' deny.toml > "$$config"; \
			$(CARGO) deny check --config "$$config"; \
		fi; \
	else \
		$(call missing_tool,cargo-deny,the supply-chain check); \
	fi

# nextest enforces per-test timeouts (.config/nextest.toml, STD-03 R21/R22)
# and fails a selection that matches no test (STD-04 R10); plain cargo test
# does neither, so the fallback says so. nextest does not run doctests, so
# they are a second leg; the store crate carries at least one (STD-04 R14).
test:
	@if $(CARGO) nextest --version >/dev/null 2>&1; then \
		$(CARGO) nextest run --workspace --all-targets $(LOCKED) --no-tests=fail; \
	else \
		$(call missing_tool,cargo-nextest,per-test timeouts and the zero-test check); \
		$(CARGO) test --workspace --all-targets $(LOCKED); \
	fi
	$(CARGO) test --workspace --doc $(LOCKED)

# Goldens of the shipped surface (STD-01 R24): captured from the built binary.
goldens:
	@if $(CARGO) nextest --version >/dev/null 2>&1; then \
		UPDATE_GOLDENS=$(if $(UPDATE),1,0) $(CARGO) nextest run -p $(BINARY) --test golden $(LOCKED) --no-tests=fail; \
	else \
		$(call missing_tool,cargo-nextest,the zero-test check); \
		UPDATE_GOLDENS=$(if $(UPDATE),1,0) $(CARGO) test -p $(BINARY) --test golden $(LOCKED); \
	fi

install:
	$(CARGO) build --release --bin $(BINARY) $(LOCKED)
	install -d $(INSTALL_BIN_DIR)
	install -m 755 target/release/$(BINARY) $(INSTALL_BIN_DIR)/$(BINARY)

clean:
	$(CARGO) clean

endif

# ---------------------------------------------------------------------------
# Gates that need no build
# ---------------------------------------------------------------------------

# Source-only structure checks: the architecture map (STD-02 R43), dependency
# direction and no import cycle (STD-02 R1-R6, R39), unit-test module reachability (STD-02 R19), terminal/stream ownership (STD-01 R17),
# singular names (STD-12 R1) and the source style rustfmt cannot check
# (STD-12 R12, R13, R16, R19).
structure:
	./scripts/check-architecture-map.sh
	./scripts/check-dependency-direction.sh
	./scripts/check-test-modules.sh
	./scripts/check-terminal-guard.sh
	./scripts/check-singular-names.sh
	./scripts/check-source-style.sh
	sh scripts/check-private-task-ids.sh
# template:begin
# The template's own gate: the example stays removable (scripts/template-manifest.txt).
	python3 -I scripts/check-template.py
# template:end

# The constellation standards this repo adopted are vendored under
# docs/standards/ with the check.sh that verifies them. Without it this warns
# and passes; with STANDARDS_STRICT=1 (set in CI) it fails.
STANDARDS_HINT := Outside the constellation umbrella the standards are plain vendored files that docs/standards/check.sh only verifies; restore docs/standards/ from the copy this repository came from.
standards-check:
	@if [ -f docs/standards/check.sh ]; then \
		sh docs/standards/check.sh; \
	elif [ "$(STANDARDS_STRICT)" = "1" ]; then \
		echo "error: standards not vendored (no docs/standards/check.sh)." >&2; \
		echo "       $(STANDARDS_HINT)" >&2; \
		exit 1; \
	else \
		echo "warning: standards not vendored (no docs/standards/check.sh); skipping." >&2; \
		echo "         $(STANDARDS_HINT)" >&2; \
	fi

# Pre-handoff gate for agents: no compile, and every parity check CI runs
# that needs no build (STD-04 R12). Goldens need the binary, so they are
# their own target.
docs-check:
	python3 scripts/check-readme.py
	python3 scripts/check-doc-frontmatter.py
	python3 scripts/generate-feature-board.py --check

feature-board:
	python3 scripts/generate-feature-board.py

ci-fast: fmt-check structure standards-check docs-check

ci-lint: clippy deny

# Full pass. Keep aligned with .github/workflows/ci.yml.
ci: ci-fast ci-lint test goldens
