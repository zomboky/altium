# Altium build entry points. `make build` produces a single self-contained
# binary at target/release/altium with the web front embedded.

CARGO ?= cargo
NPM ?= npm

.PHONY: build web test e2e dev clean

build: web
	$(CARGO) build --release -p altium-cli

web: web/node_modules
	$(NPM) --prefix web run build

web/node_modules: web/package.json web/package-lock.json
	$(NPM) --prefix web ci --no-fund --no-audit
	@touch $@

test: web/node_modules
	$(CARGO) test --workspace
	$(NPM) --prefix web run check
	$(NPM) --prefix web run test

# End-to-end: the real binary, a throwaway config and Playwright's Chromium
# (`npx --prefix web playwright install chromium` once).
e2e: web
	$(CARGO) build -p altium-cli
	$(NPM) --prefix web run e2e

# Front with hot reload; API calls are proxied to `altium serve` on :7420.
dev: web/node_modules
	$(NPM) --prefix web run dev

clean:
	$(CARGO) clean
	rm -rf web/dist
