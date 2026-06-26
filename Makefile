VERSION ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
RELEASE_DIR := release
RELEASE_NAME := onm-$(VERSION)-linux-amd64
BINARIES := ethctl smctl hcactl xpuctl
DOCKER_BUILD_IMAGE := onm-builder

.PHONY: all build release clean install docker-builder docker-build docker-release

all: build

build:
	cargo build --release

docker-builder:
	docker build --target builder -t $(DOCKER_BUILD_IMAGE) .

docker-build: docker-builder
	docker run --rm \
		-v $(CURDIR):/workspace \
		-w /workspace \
		$(DOCKER_BUILD_IMAGE) \
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

docker-release: docker-build
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
