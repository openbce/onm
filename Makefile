VERSION ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
RELEASE_DIR := release
ARCH ?= $(shell uname -m | sed 's/x86_64/amd64/' | sed 's/aarch64/arm64/')
RELEASE_NAME := onm-$(VERSION)-linux-$(ARCH)
BINARIES := ethctl smctl hcactl xpuctl
BUILD_IMAGE := onm-builder
CONTAINER_ENGINE ?= $(shell command -v podman 2>/dev/null || echo docker)

.PHONY: all build release clean install container-builder container-build container-release

all: build

build:
	cargo build --release

container-builder:
	$(CONTAINER_ENGINE) build --target builder -t $(BUILD_IMAGE) .

container-build: container-builder
	$(CONTAINER_ENGINE) run --rm \
		-v $(CURDIR):/workspace \
		-w /workspace \
		$(BUILD_IMAGE) \
		cargo build --release

release: build
	@mkdir -p $(RELEASE_DIR)/$(RELEASE_NAME)
	@for bin in $(BINARIES); do \
		cp target/release/$$bin $(RELEASE_DIR)/$(RELEASE_NAME)/; \
	done
	@cp README.md LICENSE $(RELEASE_DIR)/$(RELEASE_NAME)/
	@cd $(RELEASE_DIR) && tar -czvf $(RELEASE_NAME).tar.gz $(RELEASE_NAME)
	@rm -rf $(RELEASE_DIR)/$(RELEASE_NAME)
	@echo "Release package: $(RELEASE_DIR)/$(RELEASE_NAME).tar.gz"

container-release: container-build
	@mkdir -p $(RELEASE_DIR)/$(RELEASE_NAME)
	@for bin in $(BINARIES); do \
		cp target/release/$$bin $(RELEASE_DIR)/$(RELEASE_NAME)/; \
	done
	@cp README.md LICENSE $(RELEASE_DIR)/$(RELEASE_NAME)/
	@cd $(RELEASE_DIR) && tar -czvf $(RELEASE_NAME).tar.gz $(RELEASE_NAME)
	@rm -rf $(RELEASE_DIR)/$(RELEASE_NAME)
	@echo "Release package: $(RELEASE_DIR)/$(RELEASE_NAME).tar.gz"

clean:
	cargo clean
	rm -rf $(RELEASE_DIR)

install: build
	@for bin in $(BINARIES); do \
		cp target/release/$$bin /usr/local/bin/; \
	done
	@echo "Installed: $(BINARIES)"
