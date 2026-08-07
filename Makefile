.PHONY: audit check ci coverage deny diagnose-example fmt fmt-check lint release-check sbom-check test

RELEASE_SBOM ?= /tmp/vda5050-doctor.cdx.json

audit:
	cargo audit --file Cargo.lock

deny:
	cargo deny check

check:
	cargo check --workspace --all-targets --locked

coverage:
	cargo llvm-cov --workspace --all-targets --fail-under-lines 80 --fail-under-regions 80

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets --locked -- -D warnings

test:
	cargo test --workspace --all-targets --locked
	bash tests/release_contract.sh

ci: fmt-check lint test audit deny

diagnose-example:
	cargo run --locked --bin vda5050-doctor -- diagnose fixtures/synthetic/repeated-order-changed.jsonl --vda-version 3.0.0 --input-format envelope-jsonl --format terminal

sbom-check:
	cargo cyclonedx --manifest-path apps/vda5050-doctor/Cargo.toml --format json --describe binaries --target all --spec-version 1.5
	mv apps/vda5050-doctor/vda5050-doctor_bin.cdx.json $(RELEASE_SBOM)
	jq empty $(RELEASE_SBOM)

release-check: ci coverage sbom-check
	VDA5050_BUILD_COMMIT=$${VDA5050_BUILD_COMMIT:-UNVERIFIED} cargo build --release --locked --bin vda5050-doctor
	target/release/vda5050-doctor --version
