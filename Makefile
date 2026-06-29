VERSION ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
RELEASE_DIR := release
BINARIES := ethctl smctl hcactl xpuctl
BUILD_IMAGE := onm-builder
CONTAINER_ENGINE ?= $(shell command -v podman 2>/dev/null || echo docker)

.PHONY: all build release clean install container-builder container-build container-release

all: build

build:
	cargo build --release

container-builder:
	$(CONTAINER_ENGINE) build --target builder -t $(BUILD_IMAGE) .

container-builder-amd64:
	$(CONTAINER_ENGINE) build --platform linux/amd64 --target builder -t $(BUILD_IMAGE)-amd64 .

container-builder-arm64:
	$(CONTAINER_ENGINE) build --platform linux/arm64 --target builder -t $(BUILD_IMAGE)-arm64 .

container-build: container-builder
	$(CONTAINER_ENGINE) run --rm \
		-v $(CURDIR):/workspace \
		-w /workspace \
		$(BUILD_IMAGE) \
		cargo build --release

container-build-amd64: container-builder-amd64
	$(CONTAINER_ENGINE) run --rm --platform linux/amd64 \
		-v $(CURDIR):/workspace \
		-w /workspace \
		$(BUILD_IMAGE)-amd64 \
		cargo build --release --target-dir target-amd64

container-build-arm64: container-builder-arm64
	$(CONTAINER_ENGINE) run --rm --platform linux/arm64 \
		-v $(CURDIR):/workspace \
		-w /workspace \
		$(BUILD_IMAGE)-arm64 \
		cargo build --release --target-dir target-arm64

release: build
	$(eval RELEASE_NAME := onm-$(VERSION)-linux-$(shell uname -m | sed 's/x86_64/amd64/' | sed 's/aarch64/arm64/'))
	@mkdir -p $(RELEASE_DIR)/$(RELEASE_NAME)
	@for bin in $(BINARIES); do \
		cp target/release/$$bin $(RELEASE_DIR)/$(RELEASE_NAME)/; \
	done
	@cp README.md LICENSE $(RELEASE_DIR)/$(RELEASE_NAME)/
	@cp -R docs $(RELEASE_DIR)/$(RELEASE_NAME)/
	@cd $(RELEASE_DIR) && tar -czvf $(RELEASE_NAME).tar.gz $(RELEASE_NAME)
	@rm -rf $(RELEASE_DIR)/$(RELEASE_NAME)
	@echo "Release package: $(RELEASE_DIR)/$(RELEASE_NAME).tar.gz"

container-release: container-release-amd64 container-release-arm64
	@echo "Built cross-platform packages:"
	@ls -la $(RELEASE_DIR)/*.tar.gz

container-release-amd64: container-build-amd64
	$(eval RELEASE_NAME := onm-$(VERSION)-linux-amd64)
	@mkdir -p $(RELEASE_DIR)/$(RELEASE_NAME)
	@for bin in $(BINARIES); do \
		cp target-amd64/release/$$bin $(RELEASE_DIR)/$(RELEASE_NAME)/; \
	done
	@cp README.md LICENSE $(RELEASE_DIR)/$(RELEASE_NAME)/
	@cp -R docs $(RELEASE_DIR)/$(RELEASE_NAME)/
	@cd $(RELEASE_DIR) && tar -czvf $(RELEASE_NAME).tar.gz $(RELEASE_NAME)
	@rm -rf $(RELEASE_DIR)/$(RELEASE_NAME)
	@echo "Release package: $(RELEASE_DIR)/$(RELEASE_NAME).tar.gz"

container-release-arm64: container-build-arm64
	$(eval RELEASE_NAME := onm-$(VERSION)-linux-arm64)
	@mkdir -p $(RELEASE_DIR)/$(RELEASE_NAME)
	@for bin in $(BINARIES); do \
		cp target-arm64/release/$$bin $(RELEASE_DIR)/$(RELEASE_NAME)/; \
	done
	@cp README.md LICENSE $(RELEASE_DIR)/$(RELEASE_NAME)/
	@cp -R docs $(RELEASE_DIR)/$(RELEASE_NAME)/
	@cd $(RELEASE_DIR) && tar -czvf $(RELEASE_NAME).tar.gz $(RELEASE_NAME)
	@rm -rf $(RELEASE_DIR)/$(RELEASE_NAME)
	@echo "Release package: $(RELEASE_DIR)/$(RELEASE_NAME).tar.gz"

clean:
	cargo clean
	rm -rf $(RELEASE_DIR) target-amd64 target-arm64
	-$(CONTAINER_ENGINE) rmi $(BUILD_IMAGE) $(BUILD_IMAGE)-amd64 $(BUILD_IMAGE)-arm64 2>/dev/null || true

install: build
	@for bin in $(BINARIES); do \
		cp target/release/$$bin /usr/local/bin/; \
	done
	@echo "Installed: $(BINARIES)"
