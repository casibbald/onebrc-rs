PROJECT_ROOT_DIR := $(dir $(realpath $(lastword $(MAKEFILE_LIST))))
current_dir := $(notdir $(patsubst %/,%,$(dir $(mkfile_path))))

all: pydeps build release

.PHONY: pydeps

pydeps:
	cd "${PROJECT_ROOT_DIR}/hack"
	pwd
	pip3 install --upgrade pip
	pip3 install -r requirements.txt
	cd "${PROJECT_ROOT_DIR}"

.PHONY: build

build:
	cd "${PROJECT_ROOT_DIR}"
	cargo build

.PHONY: release

release:
	cd "${PROJECT_ROOT_DIR}"
	cargo build --release

.PHONY: clean
	cd "${PROJECT_ROOT_DIR}"
	rm -rf target
	find . -name __pycache__ -exec rm -rf {} \;
	rm weather_stations.json




