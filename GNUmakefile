CARGO:=$(shell which cargo)
export CARGO
RUSTC:=$(shell which rustc)
export RUSTC
RUSTUP:=$(shell which rustup)
export RUSTUP

-:
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?##/ {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)
help:## 	help
	@sed -n 's/^##//p' ${MAKEFILE_LIST} | column -t -s ':' |  sed -e 's/^/ /'
rustup-install:rustup-install-stable## 	rustup-install
rustup-install-stable:## 	rustup-install-stable
##rustup-install-stable:
##	install rustup && rustup default stable
	$(shell echo which rustup) || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain stable --profile default && . "$(HOME)/.cargo/env" || true
	$(shell echo which rustup) && rustup default stable
rustup-install-nightly:## 	rustup-install-nightly
##rustup-install-nightly:
##	install rustup && rustup default nightly
	$(shell echo which rustup) || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain nightly --profile default && . "$(HOME)/.cargo/env" || true
	$(shell echo which rustup) && rustup default nightly

cargo-b:## 	cargo-b
##cargo build
	[ -x "$(shell command -v $(RUSTUP))" ] || $(MAKE) rustup-install-stable
	[ -x "$(shell command -v $(CARGO))" ] && $(CARGO) build
cargo-build-release:cargo-b-release
cargo-b-release:## 	cargo-b-release
cargo-br:cargo-b-release## 	cargo-br
##cargo build --releae --path .
	[ -x "$(shell command -v $(RUSTUP))" ] || $(MAKE) rustup-install-stable
	[ -x "$(shell command -v $(CARGO))" ] && $(CARGO) build --release
cargo-c:## 	cargo-c
##cargo check
	[ -x "$(shell command -v $(RUSTC))" ] || $(MAKE) rustup-install-stable
	[ -x "$(shell command -v $(CARGO))" ] && $(CARGO) c
cargo-d:cargo-doc## 	cargo-d
cargo-doc:## 	cargo-doc
##cargo doc --all-features
	[ -x "$(shell command -v $(RUSTC))" ] || $(MAKE) rustup-install-stable
	[ -x "$(shell command -v $(CARGO))" ] && $(CARGO) doc --all-features --no-deps --bins
cargo-i:## 	cargo-i
##cargo install
	[ -x "$(shell command -v $(RUSTC))" ] || $(MAKE) rustup-install-stable
	[ -x "$(shell command -v $(CARGO))" ] && $(CARGO) install $(FORCE) --path .
cargo-publish:## cargo publish
	cargo publish --registry crates-io

test-gnostr-post-duplicate:## 	test-gnostr-post-duplicate
	@[ -x $(shell which cat) ] && \
		cat tests/event.ab0d7c747e0d6651814f8092287f9a58c9cc7a48ce700e2cf743c082577f7850 | gnostr-post-event --relay wss://relay.damus.io
test-gnostr-post-commit:## 	test-gnostr-post-commit
	@[ -x $(shell which cat) ] && \
		cat tests/first-gnostr-commit.json | gnostr-post-event --relay wss://relay.damus.io
test-gnostr-fetch-first-commit:## 	test-gnostr-fetch-first-commit
	@gnostr-fetch-by-id wss://relay.damus.io fbf73a17a4e0fe390aba1808a8d55f1b50717d5dd765b2904bf39eba18c51f7c
test-gnostr-post-event:## 	test-gnostr-post-event
	@cargo install --bin gnostr-post-event --path . && \
	[ -x $(shell which gnostr) ] && [ -x $(shell which gnostr-sha256) ] && \
	[ -x $(shell which gnostr-weeble) ] && [ -x $(shell which gnostr-wobble) ] && \
	[ -x $(shell which gnostr-blockheight) ] && \
		gnostr --sec $(shell gnostr-sha256 $(shell gnostr-weeble)) -t gnostr --tag weeble $(shell gnostr-weeble) --tag wobble $(shell gnostr-wobble) --tag blockheight $(shell gnostr-blockheight) --content 'gnostr/$(shell gnostr-weeble)/$(shell gnostr-blockheight)/$(shell gnostr-wobble))' | cargo run --bin gnostr-post-event -- --relay wss://nos.lol
test-gnostr-post-event-context:## 	test-gnostr-post-event-context
	@cargo install --bin gnostr-post-event --path . && \
	[ -x $(shell which gnostr) ] && [ -x $(shell which gnostr-sha256) ] && \
	[ -x $(shell which gnostr-weeble) ] && [ -x $(shell which gnostr-wobble) ] && \
	[ -x $(shell which gnostr-blockheight) ] && \
		gnostr --sec $(shell gnostr-sha256 $(shell gnostr-weeble)) -t gnostr --tag weeble $(shell gnostr-weeble) --tag wobble $(shell gnostr-wobble) --tag blockheight $(shell gnostr-blockheight) --content 'gnostr/$(shell gnostr-weeble)/$(shell gnostr-blockheight)/$(shell gnostr-wobble))' | cargo run --bin gnostr-post-event -- --relay wss://nos.lol
test-gnostr-bounce-event:## 	test-gnostr-bounce-event
	make test-gnostr-fetch-first-commit | gnostr-post-event

-include Makefile
