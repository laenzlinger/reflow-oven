.PHONY: help setup decrypt build flash ota monitor test clean
.DEFAULT_GOAL := help
SHELL := /bin/bash

USB_PORT ?= /dev/ttyACM0
ESP_ENV := . $(HOME)/export-esp.sh 2>/dev/null; export RUSTUP_TOOLCHAIN=esp;

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'

setup: ## Install tools via mise and configure hooks
	mise install
	mise exec -- espup install
	pre-commit install
	@echo "✅ Toolchain ready"

decrypt: ## Decrypt WiFi secrets into env vars (from home-assistant-config)
	@sops -d ../home-assistant-config/esphome/secrets.sops.yaml | python3 -c "\
	import sys, yaml; s=yaml.safe_load(sys.stdin); \
	print(f'WIFI_SSID={s[\"wifi_ssid\"]}'); \
	print(f'WIFI_PASS={s[\"wifi_password\"]}')" > firmware/.env

build: decrypt ## Build firmware
	@$(ESP_ENV) set -a && source firmware/.env && set +a && \
	cd firmware && cargo build --release

flash: decrypt ## Build and flash firmware
	@$(ESP_ENV) set -a && source firmware/.env && set +a && \
	cd firmware && cargo build --release && \
	espflash flash --port $(USB_PORT) --partition-table partitions.csv target/xtensa-esp32s3-espidf/release/reflow-oven

ota: decrypt ## Build and OTA flash over WiFi
	@$(ESP_ENV) set -a && source firmware/.env && set +a && \
	cd firmware && cargo build --release && \
	espflash save-image --chip esp32s3 target/xtensa-esp32s3-espidf/release/reflow-oven target/ota.bin && \
	curl --limit-rate 50k -X POST --data-binary @target/ota.bin http://reflow-oven.home/ota || true
	@echo "OTA sent — device is rebooting..."

monitor: ## Serial monitor
	@$(ESP_ENV) espflash monitor --port $(USB_PORT)

test: ## Run unit tests (host, no hardware needed)
	@rustc --test --edition 2021 firmware/src/pid.rs -o /tmp/pid_test && /tmp/pid_test
	@rustc --test --edition 2021 firmware/src/profile.rs -o /tmp/profile_test && /tmp/profile_test

clean: ## Clean build artifacts and decrypted secrets
	rm -f firmware/.env
	cd firmware && cargo clean
