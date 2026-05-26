# metropipe - Universal Language Binder
# Requires: Brief compiler (../brief-compiler)

BRIEF_COMPILER := ../brief-compiler/target/release/brief-compiler
SRC := src/metropipe.bv
BUILD_DIR := build

.PHONY: all build clean test spec

all: build

build: $(BRIEF_COMPILER)
	@mkdir -p $(BUILD_DIR)
	cd ../brief-compiler && $(BRIEF_COMPILER) build ../metropipe/$(SRC)
	@mv ../brief-compiler/metropipe $(BUILD_DIR)/ 2>/dev/null || true
	@cp $(BUILD_DIR)/metropipe ./metropipe 2>/dev/null || true
	@echo "Built: ./metropipe"

$(BRIEF_COMPILER):
	@echo "Building Brief compiler..."
	cd ../brief-compiler && cargo build --release

test: build
	@echo "Running metropipe tests..."
	@python3 -c "import sys; sys.path.insert(0, 'clients/python'); from metropipe import MetroBroker; b = MetroBroker(); print('Python client: OK')"
	@node -e "const m = require('./clients/javascript/metropipe.js'); console.log('JS client: OK')"
	@echo "All client stubs load successfully"

spec:
	@cat docs/METROPOLITAN-SPEC.md

clean:
	rm -rf $(BUILD_DIR)
	rm -f ../brief-compiler/metropipe ../brief-compiler/metropipe.rs

install: build
	@cp ./metropipe ~/.local/bin/metropipe 2>/dev/null || sudo cp ./metropipe /usr/local/bin/metropipe
	@echo "Installed metropipe to PATH"

bindgen: $(BRIEF_COMPILER)
	@echo "Generating bindings from examples/services.dbv..."
	@cd ../brief-compiler && ./target/release/brief-compiler bind ../metropipe/examples/services.dbv --gen-stubs
	@echo "  -> clients/python/ (auto-generated)"
	@echo "  -> clients/javascript/ (auto-generated)"
	@echo "  -> clients/c/ (auto-generated)"
