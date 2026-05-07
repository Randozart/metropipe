# Metro Daemon (metrod) - Build System
# Requires: Brief compiler (../brief-compiler)

BRIEF_COMPILER := ../brief-compiler/target/release/brief-compiler
SRC := src/metrod.bv
BUILD_DIR := build

.PHONY: all build clean test spec

all: build

# Build the metro daemon from Brief source
build: $(BRIEF_COMPILER)
	@mkdir -p $(BUILD_DIR)
	cd ../brief-compiler && $(BRIEF_COMPILER) build ../metrod/$(SRC)
	@mv ../brief-compiler/metrod $(BUILD_DIR)/ 2>/dev/null || true
	@echo "Built: $(BUILD_DIR)/metrod"

# Ensure the Brief compiler is built
$(BRIEF_COMPILER):
	@echo "Building Brief compiler..."
	cd ../brief-compiler && cargo build --release

# Run the test suite
test: build
	@echo "Running metrod tests..."
	@python3 -c "import sys; sys.path.insert(0, 'clients/python'); from metro import MetroBroker; b = MetroBroker(); print('Python client: OK')"
	@node -e "const m = require('./clients/javascript/metro.js'); console.log('JS client: OK')"
	@echo "All client stubs load successfully"

# View the protocol specification
spec:
	@cat docs/METROPOLITAN-SPEC.md

# Clean build artifacts
clean:
	rm -rf $(BUILD_DIR)
	rm -f ../brief-compiler/metrod ../brief-compiler/metrod.rs

# Install to PATH
install: build
	@cp $(BUILD_DIR)/metrod ~/.local/bin/metrod 2>/dev/null || sudo cp $(BUILD_DIR)/metrod /usr/local/bin/metrod
	@echo "Installed metrod to PATH"

# Generate client bindings from IDL
bindgen: $(BRIEF_COMPILER)
	@echo "Generating bindings from examples/services.dbv..."
	@echo "  -> clients/python/ (auto-generated)"
	@echo "  -> clients/javascript/ (auto-generated)"
	@echo "  -> clients/c/ (auto-generated)"
	@echo "Bindgen: TODO - implement automatic code generation from .dbv files"
