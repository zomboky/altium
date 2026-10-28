# Altium build entry points. `make build` produces a single self-contained
# binary at target/release/altium with the web front embedded.

CARGO ?= cargo
NPM ?= npm

.PHONY: build web test dev clean

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

# Front with hot reload; API calls are proxied to `altium serve` on :7420.
dev: web/node_modules
	$(NPM) --prefix web run dev

clean:
	$(CARGO) clean
	rm -rf web/dist
