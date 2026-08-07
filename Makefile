.PHONY: audit check ci coverage demo-e2e demo-fleet demo-fleet-e2e demo-video deny diagnose-example fmt fmt-check lint live-browser-e2e live-contract live-demo live-e2e live-video lsmart-contract lsmart-demo release-check release-lint sbom-check test web-test

RELEASE_SBOM ?= /tmp/vda5050-doctor.cdx.json
DEMO_MEDIA_DIR ?= dist
DEMO_ROBOTS ?= 1
DEMO_FLEET_ARTIFACT_DIR ?=

audit:
	cargo audit --file Cargo.lock

deny:
	cargo deny check

check:
	cargo check --workspace --all-targets --locked

coverage:
	cargo llvm-cov --workspace --all-targets --exclude vda5050-demo --fail-under-lines 80 --fail-under-regions 80

demo-e2e:
	bash tests/tier1_demo_e2e.sh
	bash tests/tier1_demo_verdict_matrix.sh

demo-fleet:
	VDA5050_DEMO_SUITE_ARTIFACT_DIR="$(DEMO_FLEET_ARTIFACT_DIR)" bash scripts/run-tier1-fleet-suite.sh "$(DEMO_ROBOTS)"

demo-fleet-e2e:
	bash tests/tier1_multirobot_contract.sh
	bash tests/tier1_multirobot_e2e.sh

demo-video:
	test -n "$(RELEASE_TAG)"
	RELEASE_TAG="$(RELEASE_TAG)" DEMO_MEDIA_DIR="$(DEMO_MEDIA_DIR)" bash scripts/render-demo-video.sh

live-demo:
	bash scripts/run-live-demo.sh

live-contract:
	bash tests/tier1_live_contract.sh

live-e2e:
	bash tests/tier1_live_e2e.sh

live-browser-e2e:
	VDA5050_LIVE_BROWSER_TEST=1 bash tests/tier1_live_e2e.sh

live-video:
	bash scripts/render-live-video.sh

lsmart-contract:
	bash tests/lsmart_integration_contract.sh

lsmart-demo:
	bash scripts/run-lsmart-demo.sh

web-test:
	docker build --file demo/Dockerfile --target web-builder .

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets --locked -- -D warnings

test:
	cargo test --workspace --all-targets --locked
	bash tests/release_contract.sh
	bash tests/fleet_release_contract.sh

release-lint:
	actionlint .github/workflows/*.yml
	shellcheck scripts/verify-release-tag.sh scripts/normalize-cyclonedx.sh scripts/verify-release-assets.sh scripts/render-demo-video.sh scripts/render-fleet-media.sh scripts/render-live-video.sh scripts/run-tier1-demo.sh scripts/run-tier1-fleet-suite.sh scripts/run-live-demo.sh scripts/run-lsmart-demo.sh demo/lsmart/run-official-lsmart.sh tests/release_contract.sh tests/fleet_media_contract.sh tests/fleet_release_contract.sh tests/lsmart_integration_contract.sh tests/tier1_demo_e2e.sh tests/tier1_demo_verdict_matrix.sh tests/tier1_live_contract.sh tests/tier1_live_e2e.sh tests/tier1_multirobot_contract.sh tests/tier1_multirobot_e2e.sh

ci: fmt-check lint test lsmart-contract audit deny

diagnose-example:
	cargo run --locked --bin vda5050-doctor -- diagnose fixtures/synthetic/repeated-order-changed.jsonl --vda-version 3.0.0 --input-format envelope-jsonl --format terminal

sbom-check:
	cargo cyclonedx --manifest-path apps/vda5050-doctor/Cargo.toml --format json --describe binaries --target all --spec-version 1.5
	mv apps/vda5050-doctor/vda5050-doctor_bin.cdx.json "$(RELEASE_SBOM)"
	version="$$(cargo metadata --locked --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "vda5050-doctor") | .version')"; \
		source_commit="$$(git rev-parse 'HEAD^{commit}')"; \
		bash scripts/normalize-cyclonedx.sh "$(RELEASE_SBOM)" "v$$version" "$$source_commit"

release-check: release-lint ci coverage sbom-check demo-e2e demo-fleet-e2e
	bash tests/fleet_media_contract.sh
	VDA5050_BUILD_COMMIT=$${VDA5050_BUILD_COMMIT:-UNVERIFIED} cargo build --release --locked --bin vda5050-doctor
	target/release/vda5050-doctor --version
